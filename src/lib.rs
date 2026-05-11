use ragit_fs::{
    FileError,
    WriteMode as RagitFsWriteMode,
    create_dir,
    exists,
    join,
    join3,
    join4,
    read_string,
    write_string,
};
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::fs::TryLockError;
use std::thread::sleep;
use std::time::{Duration, Instant};

mod chat;
mod config;
mod context;
mod error;
mod global;
mod image;
mod interrupt;
mod log;
mod model;
mod parse;
mod pdf;
mod prettify;
mod request;
mod response;
mod sandbox;
mod snapshot;
mod subprocess;
mod tool;
mod turn;
mod ui;

pub use config::Config;
pub use context::{
    ChosenTurn,
    Context,
    ContextJson,
    NeukguId,
    SessionId,
    SessionSummary,
};
pub use error::{Error, from_browser_error};
pub use global::{
    Project,
    clean_global_index_dir,
    get_global_index_dir,
    init_global_index_dir,
    load_all_indexes,
    remove_global_index,
    update_global_index,
};
pub use image::{ImageId, normalize_and_get_id};
use interrupt::{check_interruption, interrupt_be};
use log::{Logger, LogEntry, LogId, TokenUsage, load_log, load_logs_tail};
pub use model::{Agents, ApiProvider, Model};
pub use parse::{ParseError, ParsedSegment, get_first_tool_call, validate_parse_result};
use pdf::{PdfId, render_and_get_id, render_first_10_pages};
use prettify::{
    prettify_bytes,
    prettify_time,
    prettify_timestamp,
    prettify_tokens,
};
pub use request::{LLMToken, Request, Thinking, count_bytes_of_llm_tokens, stringify_llm_tokens};
use request::{MockState, reset_mock_state, revert_mock_state};
pub use response::Response;
use sandbox::{clean_dangling_sandboxes, clean_sandbox, copy_recursive, export_to_sandbox, import_from_sandbox};
use snapshot::{Snapshot, check_snapshot, clean_dangling_snapshots};
pub use tool::{
    AskTo,
    ToolCall,
    ToolCallError,
    ToolCallSuccess,
    ToolKind,
    WriteMode,
    load_available_binaries,
};
pub use turn::{
    Turn,
    TurnId,
    TurnKind,
    TurnPreview,
    TurnResult,
    TurnResultSummary,
    TurnSummary,
    get_turn_id,
};
pub use ui::{Be2Fe, Fe2Be, UserResponse, gui, tui};

pub async fn step(context: &mut Context, config: &mut Config) -> Result<(), Error> {
    context.sync_with_fe()?;

    if let Some(new_config) = context.new_config.take() {
        *config = new_config;
        config.store(&context.working_dir)?;
    }

    if context.is_paused()? {
        sleep(Duration::from_millis(100));  // prevent busy-loop
        return Ok(());
    }

    let lock_file_at = join3(&context.working_dir, ".neukgu", ".lock")?;
    let lock_file = std::fs::File::create(&lock_file_at).map_err(|e| FileError::from_std(e, &lock_file_at))?;

    match lock_file.try_lock() {
        Ok(()) => {},
        Err(TryLockError::WouldBlock) => {
            return Err(Error::FailedToAcquireWriteLock);
        },
        Err(TryLockError::Error(e)) => {
            return Err(e.into());
        },
    }

    let has_new_turn = match step_inner(context, config).await {
        Ok(f) => f,
        Err(e) => {
            context.logger.log(LogEntry::BackendError(format!("{e:?}")))?;
            return Err(e);
        },
    };

    if has_new_turn {
        update_global_index(&context)?;
    }

    // It sleeps 500ms in order to add *stability* to the program.
    // This is the safest place in the loop. The frontend can read the index
    // because every state is complete, and it's safe to kill the backend here.
    sleep(Duration::from_millis(500));

    drop(lock_file);
    Ok(())
}

// The returned boolean tells whether a new turn is added to the context or not.
async fn step_inner(context: &mut Context, config: &Config) -> Result<bool, Error> {
    if let Some((id, interrupt)) = context.check_question_from_user()? {
        context.process_question_from_user(id, interrupt, config)?;
        context.store()?;
        context.remove_done_mark()?;
    }

    if context.is_marked_done()? {
        sleep(Duration::from_millis(1_000));  // prevent busy-loop
        return Ok(false);
    }

    let mut user_interrupt = false;
    let mut turn_kind = TurnKind::Agent;
    let raw_response = match &context.curr_raw_response {
        Some((r, _)) => r.to_string(),
        None => match context.get_fake_turn() {
            Some((r, kind)) => {
                turn_kind = kind;
                context.start_turn(r.to_string(), 0);
                r
            },
            None => {
                let llm_call_started_at = Instant::now();
                let request = context.to_request(config)?;
                let response = match request.request(&context.working_dir, &context.logger).await {
                    Ok(response) => response.response.to_string(),
                    Err(Error::UserInterrupt) => {
                        context.logger.log(LogEntry::UserInterruptWhileLLMRequest)?;
                        user_interrupt = true;
                        String::new()
                    },
                    Err(e) => {
                        return Err(e);
                    },
                };
                let llm_elapsed_ms = Instant::now().duration_since(llm_call_started_at).as_millis() as u64;
                context.start_turn(response.to_string(), llm_elapsed_ms);
                response
            },
        },
    };

    if user_interrupt {
        context.discard_current_turn();
        sleep(Duration::from_millis(300));  // wait for fe to update its state
        context.sync_with_fe()?;
        return Ok(false);
    }

    context.store()?;
    context.sync_with_fe()?;

    if context.is_paused()? {
        return Ok(true);
    }

    let tool_call_started_at = Instant::now();
    let mut wrote_summary = false;
    let (parse_result, turn_result) = match parse::parse(raw_response.as_bytes()) {
        Ok(parse_result) => match validate_parse_result(&parse_result) {
            // A valid response has exactly 1 tool-call.
            Ok(tool_call) => {
                if let Err(e) = context.validate_tool_call(&tool_call)? {
                    (Some(parse_result), TurnResult::ToolCallError(e))
                }

                else {
                    match tool_call.run(context, config).await {
                        Ok(tool_call_result) => {
                            context.logger.log(LogEntry::ToolCallEnd(tool_call_result.clone()))?;

                            match tool_call_result {
                                Ok(s) => {
                                    if let ToolCallSuccess::Write { is_summary: true, .. } = &s {
                                        wrote_summary = true;
                                    }

                                    (Some(parse_result), TurnResult::ToolCallSuccess(s))
                                },
                                Err(e) => (Some(parse_result), TurnResult::ToolCallError(e)),
                            }
                        },
                        Err(Error::UserInterrupt) => {
                            context.logger.log(LogEntry::UserInterruptWhileToolCall)?;
                            user_interrupt = true;
                            (Some(parse_result), TurnResult::ToolCallError(ToolCallError::UserInterrupt))
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                }
            },
            Err(e) => (Some(parse_result), TurnResult::ParseError(e)),
        },
        Err(e) => (None, TurnResult::ParseError(e)),
    };

    let new_turn_id = context.finish_turn(
        parse_result,
        turn_result,
        Instant::now().duration_since(tool_call_started_at).as_millis() as u64,
        config,
        turn_kind,
    )?;

    if wrote_summary {
        context.summaries.push(new_turn_id);
    }

    if user_interrupt {
        context.discard_previous_turn();
        sleep(Duration::from_millis(300));  // wait for fe to update its state
    }

    context.sync_with_fe()?;
    context.store()?;

    if !user_interrupt && context.has_to_create_snapshot() {
        context.create_snapshot()?;
    }

    Ok(true)
}

pub fn validate_project_name(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::EmptyProjectName);
    }

    for ch in name.chars() {
        match ch {
            '0'..='9' |
            'a'..='z' |
            'A'..='Z' |
            '가'..='힣' |
            'ㄱ'..='ㅣ' |
            ' ' | '_' | '-' => {},
            _ => {
                return Err(Error::NotAllowedCharInProjectName { name: name.to_string(), ch });
            },
        }
    }

    Ok(())
}

pub fn init_working_dir(
    instruction: Option<String>,
    working_dir: &str,
    config: Config,
    is_in_global_index_dir: bool,
) -> Result<(), Error> {
    if exists(&join(working_dir, ".neukgu/")?) {
        return Err(Error::IndexDirAlreadyExists);
    }

    if !exists(&join(working_dir, "neukgu-instruction.md")?) {
        write_string(
            &join(working_dir, "neukgu-instruction.md")?,
            &instruction.unwrap_or(String::new()),
            RagitFsWriteMode::AlwaysCreate,
        )?;
    }

    else if instruction.is_some() {
        return Err(Error::InstructionAlreadyExists);
    }

    for d in ["logs", "bins"] {
        let dd = join(working_dir, d)?;

        if !exists(&dd) {
            create_dir(&dd)?;
        }
    }

    create_dir(&join(working_dir, ".neukgu")?)?;
    create_dir(&join3(working_dir, ".neukgu", "images")?)?;
    create_dir(&join3(working_dir, ".neukgu", "pdfs")?)?;
    create_dir(&join3(working_dir, ".neukgu", "turns")?)?;
    create_dir(&join3(working_dir, ".neukgu", "logs")?)?;
    create_dir(&join3(working_dir, ".neukgu", "interruptions")?)?;
    create_dir(&join3(working_dir, ".neukgu", "snapshots")?)?;

    write_string(
        &join4(working_dir, ".neukgu", "logs", "log")?,
        "",
        RagitFsWriteMode::AlwaysCreate,
    )?;
    write_string(
        &join4(working_dir, ".neukgu", "logs", "tokens.json")?,
        "{}",
        RagitFsWriteMode::AlwaysCreate,
    )?;
    write_string(
        &join3(working_dir, ".neukgu", "snapshots.json")?,
        "[]",
        RagitFsWriteMode::AlwaysCreate,
    )?;

    write_string(
        &join3(working_dir, ".neukgu", "be2fe.json")?,
        &serde_json::to_string(&Be2Fe::default())?,
        RagitFsWriteMode::AlwaysCreate,
    )?;

    write_string(
        &join3(working_dir, ".neukgu", "fe2be.json")?,
        &serde_json::to_string(&Fe2Be::default())?,
        RagitFsWriteMode::AlwaysCreate,
    )?;

    write_string(
        &join3(working_dir, ".neukgu", "wal")?,
        &serde_json::to_string(&HashSet::<String>::new())?,
        RagitFsWriteMode::AlwaysCreate,
    )?;

    let context = Context::new(&config, working_dir, is_in_global_index_dir)?;
    context.store()?;
    config.store(working_dir)?;

    init_global_index_dir(&context.global_index_dir)?;
    update_global_index(&context)?;

    Ok(())
}

pub fn reset_working_dir(instruction: String, working_dir: &str) -> Result<(), Error> {
    let lock_file_at = join3(working_dir, ".neukgu", ".lock")?;
    let lock_file = std::fs::File::create(&lock_file_at).map_err(|e| FileError::from_std(e, &lock_file_at))?;

    clean_dangling_sandboxes(working_dir)?;
    write_string(
        &join(working_dir, "neukgu-instruction.md")?,
        &instruction,
        RagitFsWriteMode::CreateOrTruncate,
    )?;
    write_string(
        &join4(working_dir, ".neukgu", "logs", "log")?,
        "",
        RagitFsWriteMode::CreateOrTruncate,
    )?;
    write_string(
        &join3(working_dir, ".neukgu", "be2fe.json")?,
        &serde_json::to_string(&Be2Fe::default())?,
        RagitFsWriteMode::CreateOrTruncate,
    )?;
    write_string(
        &join3(working_dir, ".neukgu", "fe2be.json")?,
        &serde_json::to_string(&Fe2Be::default())?,
        RagitFsWriteMode::CreateOrTruncate,
    )?;

    let config = Config::load(working_dir)?;
    let old_context = Context::load(&config, working_dir)?;
    let mut new_context = Context::new(&config, working_dir, old_context.is_in_global_index_dir)?;
    new_context.neukgu_id = old_context.neukgu_id;
    new_context.store()?;
    new_context.remove_done_mark()?;

    if config.agents.big == Model::Mock {
        reset_mock_state(working_dir)?;
    }

    drop(lock_file);
    Ok(())
}

pub fn roll_back_working_dir(id: &TurnId, working_dir: &str) -> Result<(), Error> {
    let lock_file_at = join3(working_dir, ".neukgu", ".lock")?;
    let lock_file = std::fs::File::create(&lock_file_at).map_err(|e| FileError::from_std(e, &lock_file_at))?;
    let snapshot_at = join4(working_dir, ".neukgu", "snapshots", &id.0)?;
    let snapshots_at = join3(working_dir, ".neukgu", "snapshots.json")?;
    let py_venv_snapshot = join3(&snapshot_at, ".neukgu", "py-venv")?;
    let py_venv_working_dir = join3(working_dir, ".neukgu", "py-venv")?;
    let snapshots: Vec<Snapshot> = load_json(&snapshots_at)?;
    let snapshot = snapshots.into_iter().filter(|snapshot| &snapshot.turn == id).next();
    let Some(snapshot) = snapshot else { return Err(Error::CannotFindSnapshot(id.clone())); };

    clean_dangling_sandboxes(working_dir)?;
    clean_dangling_snapshots(snapshot.seq, working_dir)?;
    copy_recursive(&snapshot_at, working_dir, true, false /* copy index dir */)?;
    copy_recursive(&py_venv_snapshot, &py_venv_working_dir, true, false)?;

    write_string(
        &join3(working_dir, ".neukgu", "context.json")?,
        &serde_json::to_string_pretty(&snapshot.context)?,
        RagitFsWriteMode::CreateOrTruncate,
    )?;

    // It doesn't rollback configs. I'm not sure whether I should rollback it or not...
    // write_string(
    //     &join3(working_dir, ".neukgu", "config.json")?,
    //     &serde_json::to_string_pretty(&snapshot.config)?,
    //     RagitFsWriteMode::CreateOrTruncate,
    // )?;

    if let Some(mock_state) = &snapshot.mock_state {
        write_string(
            &join3(working_dir, ".neukgu", "mock.json")?,
            &serde_json::to_string_pretty(&mock_state)?,
            RagitFsWriteMode::CreateOrTruncate,
        )?;
    }

    write_string(
        &join3(working_dir, ".neukgu", "be2fe.json")?,
        &serde_json::to_string(&Be2Fe::default())?,
        RagitFsWriteMode::CreateOrTruncate,
    )?;
    write_string(
        &join3(working_dir, ".neukgu", "fe2be.json")?,
        &serde_json::to_string(&Fe2Be::default())?,
        RagitFsWriteMode::CreateOrTruncate,
    )?;

    let logger = Logger::new(join3(working_dir, ".neukgu", "logs")?, true, true);
    logger.log(LogEntry::RollBack(id.clone()))?;

    drop(lock_file);
    Ok(())
}

pub fn load_json<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let mut curr_error: Option<Error> = None;

    // Maybe another process is writing the file, so we try this 4 times.
    for i in 0..4 {
        if i > 0 {
            sleep(Duration::from_millis(i * i * 50));
        }

        let s = match read_string(path) {
            Ok(s) => s,
            Err(e) => {
                curr_error = Some(e.into());
                continue;
            },
        };

        let j = match serde_json::from_str::<T>(&s) {
            Ok(j) => j,
            Err(e) => {
                curr_error = Some(e.into());
                continue;
            },
        };

        return Ok(j);
    }

    Err(curr_error.unwrap())
}

pub fn get_neukgu_id(working_dir: &str) -> Result<NeukguId, Error> {
    let config = Config::load(working_dir)?;
    let context = Context::load(&config, working_dir)?;
    Ok(context.neukgu_id)
}

fn hash_bytes(s: &[u8]) -> u128 {
    let mut r = 0;

    for (i, b) in s.iter().enumerate() {
        let c = (((r >> 24) & 0x00ff_ffff) << 24) | ((i & 0xfff) << 12) as u128 | *b as u128;
        let cc = c * c + c + 1;
        r += cc;
        r &= 0xffff_ffff_ffff_ffff_ffff_ffff;
    }

    r
}
