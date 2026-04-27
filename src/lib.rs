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

mod config;
mod context;
mod error;
mod image;
mod interrupt;
mod log;
mod parse;
mod pdf;
mod prettify;
mod request;
mod response;
mod sandbox;
mod subprocess;
mod tool;
mod turn;
mod ui;

pub use config::Config;
pub use context::{ChosenTurn, Context, ContextJson};
pub use error::{Error, from_browser_error};
pub use image::{ImageId, normalize_and_get_id};
use interrupt::{check_interruption, interrupt_be};
use log::{Logger, LogEntry, LogId, TokenUsage, load_log, load_logs_tail};
pub use parse::{ParseError, ParsedSegment, get_first_tool_call, validate_parse_result};
use pdf::{PdfId, render_and_get_id};
use prettify::{
    prettify_bytes,
    prettify_time,
    prettify_tokens,
};
pub use request::{LLMToken, Model, Request, Thinking, count_bytes_of_llm_tokens, stringify_llm_tokens};
use request::{reset_mock_state, revert_mock_state};
pub use response::Response;
pub use sandbox::{clean_dangling_sandboxes, clean_sandbox, export_to_sandbox, import_from_sandbox};
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
    TurnPreview,
    TurnResult,
    TurnResultSummary,
    TurnSummary,
    get_turn_id,
};
pub use ui::{Be2Fe, Fe2Be, UserResponse, gui, tui};

pub async fn step(context: &mut Context, config: &Config) -> Result<(), Error> {
    context.sync_with_fe()?;

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

    clean_dangling_sandboxes(&context.working_dir)?;
    let backup_dir = export_to_sandbox(&config.sandbox_root, &context.working_dir)?;

    match step_inner(context, config).await {
        Ok(()) => {
            clean_sandbox(&config.sandbox_root, &backup_dir, &context.working_dir)?;
        },
        Err(e) => {
            import_from_sandbox(&backup_dir, &context.working_dir, true /* copy index dir */)?;
            clean_sandbox(&config.sandbox_root, &backup_dir, &context.working_dir)?;
            context.logger.log(LogEntry::BackendError(format!("{e:?}")))?;
            return Err(e);
        },
    }

    drop(lock_file);
    Ok(())
}

async fn step_inner(context: &mut Context, config: &Config) -> Result<(), Error> {
    if let Some((id, interrupt)) = context.check_question_from_user()? {
        context.process_question_from_user(id, interrupt, config)?;
        context.store()?;
        context.remove_done_mark()?;
    }

    // TODO: When it's marked done, it still creates and removes sandbox-backup everytime,
    //       which is a gigantic overhead. I temporily alleviated it with longer sleep,
    //       but I need a better solution.
    if context.is_marked_done()? {
        sleep(Duration::from_millis(1_000));  // prevent busy-loop
        return Ok(());
    }

    let mut user_interrupt = false;
    let raw_response = match &context.curr_raw_response {
        Some((r, _)) => r.to_string(),
        None => {
            let llm_call_started_at = Instant::now();
            let mut request = context.to_request(config)?;
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
            context.start_turn(response.clone(), llm_elapsed_ms);
            response
        },
    };

    if user_interrupt {
        context.discard_current_turn();
        sleep(Duration::from_millis(300));  // wait for fe to update its state
        context.sync_with_fe()?;
        return Ok(());
    }

    context.store()?;
    context.sync_with_fe()?;

    if context.is_paused()? {
        return Ok(());
    }

    let tool_call_started_at = Instant::now();
    let (parse_result, turn_result) = match parse::parse(raw_response.as_bytes()) {
        Ok(parse_result) => match validate_parse_result(&parse_result) {
            // A valid response has exactly 1 tool-call.
            Ok(tool_call) => match tool_call.run(context, config).await {
                Ok(tool_call_result) => {
                    context.logger.log(LogEntry::ToolCallEnd(tool_call_result.clone()))?;

                    match tool_call_result {
                        Ok(s) => (Some(parse_result), TurnResult::ToolCallSuccess(s)),
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
            },
            Err(e) => (Some(parse_result), TurnResult::ParseError(e)),
        },
        Err(e) => (None, TurnResult::ParseError(e)),
    };

    context.finish_turn(
        parse_result,
        turn_result,
        Instant::now().duration_since(tool_call_started_at).as_millis() as u64,
        config,
        false,
    )?;

    if user_interrupt {
        context.discard_previous_turn();
        sleep(Duration::from_millis(300));  // wait for fe to update its state
    }

    context.sync_with_fe()?;
    context.store()?;
    Ok(())
}

pub fn validate_project_name(name: &str) -> Result<(), Error> {
    for ch in name.chars() {
        match ch {
            '0'..='9' |
            'a'..='z' |
            'A'..='Z' |
            '가'..='힣' |
            ' ' | '_' | '-' => {},
            _ => {
                return Err(Error::NotAllowedCharInProjectName { name: name.to_string(), ch });
            },
        }
    }

    Ok(())
}

pub fn init_working_dir(instruction: Option<String>, working_dir: &str, model: Model) -> Result<(), Error> {
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

    let mut config = Config::default();
    config.model = model;
    config.store(working_dir)?;

    let context = Context::new(&config, working_dir)?;
    context.store()?;

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
    let context = Context::new(&config, working_dir)?;
    context.store()?;
    context.remove_done_mark()?;

    if config.model == Model::Mock {
        reset_mock_state(working_dir)?;
    }

    drop(lock_file);
    Ok(())
}

pub fn load_json<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let mut curr_error: Option<Error> = None;

    // Maybe another process is writing the file, so we try this 3 times.
    for i in 0..3 {
        if i > 0 {
            sleep(Duration::from_millis(i * i));
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
