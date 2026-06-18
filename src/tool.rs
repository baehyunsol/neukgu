use async_std::task::sleep;
use crate::{
    Config,
    Context,
    Error,
    ImageId,
    InterruptId,
    LLMToken,
    LogEntry,
    Model,
    ParsedSegment,
    SessionId,
    Turn,
    TurnResult,
    TurnSummary,
    clean_sandbox,
    export_to_sandbox,
    from_browser_error,
    import_from_sandbox,
    normalize_image,
    prettify_bytes,
    prettify_time,
    reset_working_dir,
    subprocess,
    truncate_chars,
    try_get_session_result,
};
use headless_chrome::Browser;
use headless_chrome::browser::LaunchOptions as BrowserLaunchOptions;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use ragit_fs::{
    basename,
    exists,
    into_abs_path,
    is_dir,
    is_symlink,
    join,
    join3,
    parent,
    read_bytes,
    read_dir,
    read_string,
    remove_dir_all,
    remove_file,
    write_bytes,
    write_string,
};
use serde::{Deserialize, Serialize};
use similar::Algorithm as DiffAlgorithm;
use similar::udiff::unified_diff;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod ask;
mod chrome;
mod image_edit;
mod patch;
mod path;
mod permission;
mod read;
mod run;
mod write;

pub use ask::{
    AskTo,
    QuestionKind,
    QuestionToUser,
    UserAnswer,
    UserResponse,
    ask_question_to_user,
    ask_question_to_web,
};
pub use chrome::WebOrFile;
pub use image_edit::ImageRequest;
pub use patch::{
    DiffKind,
    Hunk,
    LineDiff,
    PatchError,
    parse_line_diff,
    patch_diff,
    patch_file,
    revert_hunks,
};
pub use path::{Path, normalize_path};
pub use permission::{
    Permission,
    PermissionConfig,
    PermissionPreview,
    ToolPermissionKind,
    ask_permission_to_user,
    default_tool_permissions,
};
pub use read::{
    FileEntry,
    RangeType,
    TypedFile,
    check_read_path,
    read_file,
};
pub use run::{
    ParseCommandError,
    init_and_load_available_binaries,
    list_binaries,
    parse_command,
    to_bash,
};
pub use write::{DumpOrRedirect, WriteMode, check_write_path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ToolCall {
    Agent {
        name: String,
        prompt: String,
    },
    // start and end are both inclusive.
    // They're 1-based index.
    Read {
        path: String,
        start: Option<u64>,
        end: Option<u64>,
    },
    Write {
        path: String,
        mode: WriteMode,
        content: String,
    },
    Patch {
        path: String,
        diff: Vec<LineDiff>,
    },
    Remove {
        path: String,
    },
    Run {
        timeout: Option<u64>,
        command: Vec<String>,
        path: Option<String>,
        env: Vec<(String, String)>,
        stdout: Option<String>,
        stderr: Option<String>,
    },
    Ask {
        // It prevents the frontend from answering the same question multiple times.
        id: InterruptId,

        to: AskTo,
        question: QuestionToUser,
    },
    Chrome {
        input: WebOrFile,
        output: String,
        script: Option<String>,
    },
    ImageEdit {
        input: String,
        prompt: String,
        size: Option<(u64, u64)>,
        output: String,
    },
}

impl ToolCall {
    pub async fn run(&self, context: &mut Context, config: &Config) -> Result<Result<ToolCallSuccess, ToolCallError>, Error> {
        context.logger.log(LogEntry::ToolCallStart(self.clone()))?;

        if let Err(e) = ask_permission_to_user(self, context, config).await? {
            return Ok(Err(e));
        }

        match self {
            ToolCall::Agent { name, prompt } => {
                let session_id = SessionId::from_string_hash(&format!("name: {name}\nprompt: {prompt}"));

                if let Some(result) = try_get_session_result(session_id, &context.working_dir)? {
                    Ok(Ok(ToolCallSuccess::Agent { result }))
                }

                else {
                    let new_context = reset_working_dir(
                        prompt.to_string(),
                        Some(session_id),
                        &context.working_dir,
                        false,
                        true,
                    )?;
                    let new_session_id = new_context.session_id;
                    *context = new_context;
                    Err(Error::SwitchContext(new_session_id))
                }
            },
            ToolCall::Read { path, start, end } => {
                if context.is_reading_too_much(config)? {
                    return Ok(Err(ToolCallError::TooManyReadWithoutSummary));
                }

                let path = match check_read_path(path, &context.working_dir) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                let start_i = (*start).unwrap_or(0);
                let end_i = (*end).unwrap_or(u64::MAX - 1);

                match read_file(&path.absolute, context)? {
                    TypedFile::Text(s) => {
                        if s.len() as u64 > config.text_file_max_len && start.is_none() && end.is_none() {
                            Ok(Err(ToolCallError::TextTooLongToRead {
                                path,
                                length: s.len() as u64,
                                limit: config.text_file_max_len,
                            }))
                        }

                        else if let Some(end) = end && start_i + config.text_file_max_lines < *end {
                            Ok(Err(ToolCallError::TooManyTextLinesToRead {
                                path,
                                length: *end - start_i + 1,
                                limit: config.text_file_max_lines,
                            }))
                        }

                        else {
                            let mut lines = vec![];
                            let mut total_lines = 0;

                            for (i, line) in s.lines().enumerate() {
                                total_lines = i;

                                // both inclusive, and 1-based index
                                if start_i <= (i + 1) as u64 && (i + 1) as u64 <= end_i {
                                    lines.push(line);
                                }
                            }

                            if lines.is_empty() && total_lines > 0 {
                                Ok(Err(ToolCallError::InvalidRange {
                                    r#type: RangeType::Line,
                                    length: total_lines as u64,
                                    given: (*start, *end),
                                }))
                            }

                            else {
                                Ok(Ok(ToolCallSuccess::ReadText {
                                    path,
                                    content: lines.join("\n"),
                                    total_lines: total_lines as u64,
                                    range: (*start, *end),
                                }))
                            }
                        }
                    },
                    TypedFile::Pdf(pdf) => {
                        let pages = pdf.get_pages(&context.working_dir)?;

                        if pages.len() as u64 > config.pdf_max_pages && (start.is_none() || start_i < 2) && end.is_none() {
                            Ok(Err(ToolCallError::TooManyPdfPagesToRead {
                                path,
                                pages: pages.len() as u64,
                                limit: config.pdf_max_pages,
                                given_range: (*start, *end),
                            }))
                        }

                        else if let Some(end) = end && start_i + config.pdf_max_pages < *end {
                            Ok(Err(ToolCallError::TooManyPdfPagesToRead {
                                path,
                                pages: *end - start_i + 1,
                                limit: config.pdf_max_pages,
                                given_range: (*start, Some(*end)),
                            }))
                        }

                        else {
                            match pages.get((start_i.max(1) as usize - 1)..(end_i as usize).min(pages.len())) {
                                Some(sliced_pages) if sliced_pages.len() > 0 || pages.is_empty() => Ok(Ok(ToolCallSuccess::ReadPdf {
                                    path,
                                    pages: sliced_pages.to_vec(),
                                    total_pages: pages.len() as u64,
                                    range: (*start, *end),
                                })),
                                _ => Ok(Err(ToolCallError::InvalidRange {
                                    r#type: RangeType::PdfPage,
                                    length: pages.len() as u64,
                                    given: (*start, *end),
                                })),
                            }
                        }
                    },
                    TypedFile::BrokenPdf { error } => Ok(Err(ToolCallError::BrokenFile { path, kind: String::from("pdf"), error })),
                    TypedFile::Image(id, size) => Ok(Ok(ToolCallSuccess::ReadImage { path, id, size })),
                    TypedFile::BrokenImage { error } => Ok(Err(ToolCallError::BrokenFile { path, kind: String::from("image"), error })),
                    TypedFile::Dir(entries) => {
                        if entries.len() as u64 > config.dir_max_entries && (start.is_none() || start_i < 2) && end.is_none() {
                            Ok(Err(ToolCallError::TooManyDirEntriesToRead {
                                path,
                                entries: entries.len() as u64,
                                limit: config.dir_max_entries,
                                given_range: (*start, *end),
                            }))
                        }

                        else if let Some(end) = end && start_i + config.dir_max_entries < *end {
                            Ok(Err(ToolCallError::TooManyDirEntriesToRead {
                                path,
                                entries: *end - start_i + 1,
                                limit: config.dir_max_entries,
                                given_range: (*start, Some(*end)),
                            }))
                        }

                        else {
                            match entries.get((start_i.max(1) as usize - 1)..(end_i as usize).min(entries.len())) {
                                Some(sliced_entries) if sliced_entries.len() > 0 || entries.is_empty() => Ok(Ok(ToolCallSuccess::ReadDir {
                                    path,
                                    entries: sliced_entries.to_vec(),
                                    total_entries: entries.len() as u64,
                                    range: (*start, *end),
                                })),
                                _ => Ok(Err(ToolCallError::InvalidRange {
                                    r#type: RangeType::FileEntry,
                                    length: entries.len() as u64,
                                    given: (*start, *end),
                                })),
                            }
                        }
                    },
                    TypedFile::Symlink { pointee } => {
                        if end.is_none() && start.is_none() {
                            Ok(Ok(ToolCallSuccess::ReadSymlink { path, pointee }))
                        }

                        else {
                            Ok(Err(ToolCallError::SymlinkWithRange { path, pointee, range: (*start, *end) }))
                        }
                    },
                    TypedFile::Etc => Ok(Err(ToolCallError::InvalidFileType { path })),
                }
            },
            ToolCall::Write { path, mode, content } => {
                let path = match check_write_path(path, &context.working_dir, Some(*mode)) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                if content.len() as u64 > config.text_file_max_len {
                    return Ok(Err(ToolCallError::TextTooLongToWrite {
                        path,
                        length: content.len() as u64,
                        limit: config.text_file_max_len,
                    }));
                }

                let prev_content = if *mode == WriteMode::Create {
                    String::new()
                } else {
                    String::from_utf8_lossy(&read_bytes(&path.absolute)?).to_string()
                };

                // It applies some heuristics to trailing/leading newlines.
                if *mode == WriteMode::Append {
                    write_string(
                        &path.absolute,
                        "\n",
                        (*mode).into(),
                    )?;
                }

                write_string(
                    &path.absolute,
                    content.trim(),
                    (*mode).into(),
                )?;

                if *mode != WriteMode::Append {
                    write_string(
                        &path.absolute,
                        "\n",
                        WriteMode::Append.into(),
                    )?;
                }

                let byte_count = content.len() as u64;
                let char_count = content.chars().count() as u64;
                let line_count = content.lines().count() as u64;
                let mut diff = None;

                // `diff` is later used to calculate the original content of the file.
                // So we need `diff` even if the mode is `Append`.
                if let WriteMode::Truncate | WriteMode::Append = mode {
                    diff = Some(unified_diff(
                        DiffAlgorithm::Patience,
                        &prev_content,
                        &String::from_utf8_lossy(&read_bytes(&path.absolute)?),
                        5,
                        None,
                    ));
                }

                Ok(Ok(ToolCallSuccess::Write {
                    path: path.clone(),
                    content: content.to_string(),
                    is_summary: path.is_summary_file(),
                    diff,
                    mode: *mode,
                    bytes: byte_count,
                    chars: char_count,
                    lines: line_count,
                }))
            },
            ToolCall::Patch { path, diff } => {
                let path = match check_write_path(path, &context.working_dir, None) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                match (exists(&path.absolute), is_symlink(&path.absolute), is_dir(&path.absolute), read_string(&path.absolute)) {
                    (_, true, _, _) => {
                        return Ok(Err(ToolCallError::CannotPatchSymlink { path }));
                    },
                    (false, _, _, _) => {
                        return Ok(Err(ToolCallError::CannotPatchNonExistFile { path }));
                    },
                    (_, _, true, _) => {
                        return Ok(Err(ToolCallError::CannotPatchDir { path }));
                    },
                    (_, _, _, Err(e)) => {
                        return Ok(Err(ToolCallError::CanOnlyPatchText { path, error: format!("{e:?}") }));
                    },
                    _ => {},
                }

                let result = patch_file(&path, diff);

                if let Ok(ToolCallSuccess::Patch { new_content, path, .. }) = &result {
                    write_string(&path.absolute, new_content, WriteMode::Truncate.into())?;
                }

                Ok(result)
            },
            ToolCall::Remove { path } => {
                let path = match normalize_path(path, &context.working_dir) {
                    Some(path) => path,
                    None => {
                        return Ok(Err(ToolCallError::InvalidPath(path.to_string())));
                    },
                };

                if path.is_index_dir() {
                    return Ok(Err(ToolCallError::CannotWriteToIndexDir));
                }

                // TODO: what if it tries to remove `..`? Then it'll kill itself...
                match (exists(&path.absolute), is_symlink(&path.absolute), is_dir(&path.absolute)) {
                    (_, true, _) => todo!(),
                    (false, _, _) => Ok(Err(ToolCallError::CannotRemoveNonExistFile { path })),
                    (_, _, true) => {
                        remove_dir_all(&path.absolute)?;
                        Ok(Ok(ToolCallSuccess::RemoveDir { path }))
                    },
                    (_, _, false) => {
                        remove_file(&path.absolute)?;
                        Ok(Ok(ToolCallSuccess::RemoveFile { path }))
                    },
                }
            },
            ToolCall::Run { timeout, command, path, env, stdout, stderr } => {
                let mut env = env.to_vec();
                let (run_at, stdout_path, stderr_path) = match run::calc_run_paths(
                    path,
                    stdout,
                    stderr,
                    &context.working_dir,
                    true,  // check permissions
                ) {
                    Ok((p1, p2, p3)) => (p1, p2, p3),
                    Err(e) => return Ok(Err(e)),
                };

                // If `command` is empty, that's a parse error.
                let mut command = command.to_vec();
                let mut binary = command[0].to_string();
                let mut available_binaries = vec![];

                for bin in read_dir(&join(&context.working_dir, "bins")?, false)?.iter() {
                    available_binaries.push(basename(bin)?);
                }

                for bin in context.available_binaries.iter() {
                    if !available_binaries.contains(bin) {
                        available_binaries.push(bin.to_string());
                    }
                }

                available_binaries.sort();

                if !available_binaries.contains(&binary) {
                    return Ok(Err(ToolCallError::NoSuchBinary {
                        binary,
                        available_binaries,
                    }));
                }

                let timeout = timeout.unwrap_or(config.default_command_timeout);

                if timeout > config.command_max_timeout {
                    return Ok(Err(ToolCallError::CommandTimeoutTooLong {
                        max: config.command_max_timeout,
                        given: timeout,
                    }));
                }

                let sandbox_at = export_to_sandbox(&config.sandbox_root, &context.working_dir, false /* copy index dir */)?;

                let run_at_real_path = match &run_at {
                    Some(Path { relative: Some(path), .. }) => join(&sandbox_at, path)?,
                    Some(Path { relative: None, absolute }) => absolute.to_string(),
                    None => sandbox_at.to_string(),
                };

                if let Ok(home) = std::env::var("HOME") {
                    env.push((String::from("HOME"), home));
                }

                env.push((String::from("PATH"), into_abs_path(&join(&context.working_dir, "bins")?)?));

                if binary == "python3" || binary == "pip" {
                    if binary == "pip" {
                        command.insert(1, String::from("pip"));
                        command.insert(1, String::from("-m"));
                    }

                    binary = into_abs_path(&join(
                        &join(&context.working_dir, ".neukgu")?,
                        &join3("py-venv", "bin", "python3")?,
                    )?)?;
                }

                if binary == "cargo" {
                    env.push((
                        String::from("CARGO_TARGET_DIR"),

                        // If there're multiple cargo projects in the working directory,
                        // incremental compilation is not gonna work, but it's gonna compile anyway hahaha
                        join3(
                            &context.global_index_dir,
                            "cargo-targets",
                            &format!("{:016x}", context.neukgu_id.0),
                        )?,
                    ));
                }

                let started_at = Instant::now();
                let result = subprocess::run(
                    binary,
                    if command.len() > 1 { &command[1..] } else { &command[0..0] },
                    true,
                    &env,
                    &run_at_real_path,
                    timeout,
                    &context.working_dir,
                    true,
                )?;
                let elapsed_ms = Instant::now().duration_since(started_at).as_millis() as u64;
                import_from_sandbox(&sandbox_at, &context.working_dir, false /* copy_index_dir */)?;
                clean_sandbox(&config.sandbox_root, &sandbox_at, &context.working_dir)?;

                let timeout_value = timeout;
                let subprocess::Output { status, stdout, stderr, elapsed_ms: _, timeout } = result;

                let stdout = match &stdout_path {
                    Some(path) => {
                        write_bytes(
                            &path.absolute,
                            &stdout,
                            ragit_fs::WriteMode::CreateOrTruncate,
                        )?;
                        DumpOrRedirect::Redirect(stdout_path.clone().unwrap())
                    },
                    None => DumpOrRedirect::Dump(String::from_utf8_lossy(&stdout).to_string()),
                };
                let stderr = match &stderr_path {
                    Some(path) => {
                        write_bytes(
                            &path.absolute,
                            &stderr,
                            if &stdout_path == &stderr_path {
                                ragit_fs::WriteMode::AlwaysAppend
                            } else {
                                ragit_fs::WriteMode::CreateOrTruncate
                            },
                        )?;
                        DumpOrRedirect::Redirect(stderr_path.clone().unwrap())
                    },
                    None => DumpOrRedirect::Dump(String::from_utf8_lossy(&stderr).to_string()),
                };

                if timeout {
                    Ok(Err(ToolCallError::Timeout {
                        path: run_at,
                        command: command.to_vec(),
                        timeout: timeout_value,
                        stdout,
                        stderr,
                    }))
                }

                else {
                    Ok(Ok(ToolCallSuccess::Run {
                        path: run_at,
                        command: command.to_vec(),
                        elapsed_ms,
                        exit_code: status,
                        stdout,
                        stderr,
                    }))
                }
            },
            ToolCall::Ask { id, to: AskTo::User, question } => ask_question_to_user(*id, question, context, config).await,
            ToolCall::Ask { id: _, to: AskTo::Web, question } => match config.agents.search {
                Model::Disabled | Model::Mock => Ok(Err(ToolCallError::WebSearchDisabled)),
                _ => {
                    let QuestionToUser { question, .. } = question;
                    let answer = ask_question_to_web(question, config, &context.working_dir, &mut context.logger).await?;
                    Ok(Ok(ToolCallSuccess::Ask { to: AskTo::Web, answer: UserAnswer::FreeText(answer) }))
                },
            },
            // TODO: error if `script` is set and `input` is not an html
            ToolCall::Chrome { script, input, output } => {
                let output = match check_write_path(output, &context.working_dir, None) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                match input {
                    WebOrFile::Web(url) => {
                        // TODO: It occasionally panics on MacOS, when it launches the browser multiple times in a session.
                        let browser = Browser::new(BrowserLaunchOptions {
                            window_size: Some((1920, 1080)),
                            ..BrowserLaunchOptions::default()
                        }).map_err(from_browser_error)?;
                        let tab = browser.new_tab().map_err(from_browser_error)?;
                        let mut script_output = None;
                        tab.navigate_to(url).map_err(from_browser_error)?;

                        // TODO: timeout?
                        // TODO: test with larger objects (maybe truncate the result?)
                        if let Some(script) = script {
                            script_output = Some(format!("{:?}", tab.evaluate(script, false).map_err(from_browser_error)?));
                        }

                        // Some pages take time to load.
                        sleep(Duration::from_millis(2_000)).await;

                        let png_data = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true).map_err(from_browser_error)?;
                        let image_id = normalize_image(&png_data, &context.working_dir, 1200)?;
                        write_bytes(&output.absolute, &png_data, ragit_fs::WriteMode::CreateOrTruncate)?;
                        Ok(Ok(ToolCallSuccess::ChromeWeb { input: url.to_string(), output_path: output, output_image: image_id, script_output }))
                    },
                    WebOrFile::File(input) => {
                        let input = match check_read_path(input, &context.working_dir) {
                            Ok(path) => path,
                            Err(e) => {
                                return Ok(Err(e));
                            },
                        };

                        if input.absolute.ends_with(".svg") {
                            if script.is_some() {
                                return Ok(Err(ToolCallError::SvgWithScript));
                            }

                            let svg_data = read_string(&input.absolute)?;

                            // VIBE NOTE: gemini 3.1 wrote this code (svg to png)
                            let mut opt = resvg::usvg::Options::default();

                            if let Some(db) = Arc::get_mut(&mut opt.fontdb) {
                                db.load_system_fonts();
                                db.load_font_data(include_bytes!("../resources/SpaceMono-Regular.ttf").to_vec());
                            }

                            let rtree = resvg::usvg::Tree::from_str(&svg_data, &opt)?;
                            let pixmap_size = rtree.size().to_int_size();
                            let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
                            resvg::render(
                                &rtree,
                                resvg::usvg::Transform::identity(),
                                &mut pixmap.as_mut(),
                            );
                            pixmap.save_png(&output.absolute)?;
                            // the VIBE ends here

                            let png_data = read_bytes(&output.absolute)?;
                            let image_id = normalize_image(&png_data, &context.working_dir, 1200)?;
                            return Ok(Ok(ToolCallSuccess::Svg { input, output_path: output, output_image: image_id }));
                        }

                        // FIXME: redundant code
                        else {
                            let url = format!("file://{}", input.absolute);

                            // TODO: It occasionally panics on MacOS, when it launches the browser multiple times in a session.
                            let browser = Browser::new(BrowserLaunchOptions {
                                window_size: Some((1920, 1080)),
                                ..BrowserLaunchOptions::default()
                            }).map_err(from_browser_error)?;
                            let tab = browser.new_tab().map_err(from_browser_error)?;
                            let mut script_output = None;
                            tab.navigate_to(&url).map_err(from_browser_error)?;

                            // TODO: timeout?
                            // TODO: test with larger objects (maybe truncate the result?)
                            if let Some(script) = script {
                                script_output = Some(format!("{:?}", tab.evaluate(script, false).map_err(from_browser_error)?));
                            }

                            // Some pages take time to load.
                            sleep(Duration::from_millis(2_000)).await;

                            let png_data = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true).map_err(from_browser_error)?;
                            let image_id = normalize_image(&png_data, &context.working_dir, 1200)?;
                            write_bytes(&output.absolute, &png_data, ragit_fs::WriteMode::CreateOrTruncate)?;
                            Ok(Ok(ToolCallSuccess::ChromeFile { input, output_path: output, output_image: image_id, script_output }))
                        }
                    },
                }
            },
            ToolCall::ImageEdit { input, prompt, size, output } => {
                if let Model::Disabled | Model::Mock = config.agents.image_edit {
                    return Ok(Err(ToolCallError::ImageEditDisabled));
                }

                let input = match check_read_path(input, &context.working_dir) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };
                let output = match check_write_path(output, &context.working_dir, None) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                let input_image_id = match read_bytes(&input.absolute) {
                    Ok(bytes) => match normalize_image(&bytes, &context.working_dir, 1200) {
                        Ok(id) => id,
                        Err(_) => {
                            return Ok(Err(ToolCallError::NotAnImage { path: input }));
                        },
                    },
                    Err(_) => {
                        return Ok(Err(ToolCallError::NotAnImage { path: input }));
                    },
                };

                let request = ImageRequest {
                    model: config.agents.image_edit,
                    prompt: prompt.to_string(),
                    images: vec![input_image_id],
                    size: *size,
                };

                let response = match request.request(&context.working_dir, &context.logger).await {
                    Ok(response) => response,
                    Err(Error::ImageRequestError { status_code, message }) => {
                        return Ok(Err(ToolCallError::ImageRequestError { status_code, message }));
                    },
                    Err(e) => {
                        return Err(e);
                    },
                };

                let generated_image_bytes = response.data[0].decode_base64()?;
                let generated_image = image::load_from_memory(&generated_image_bytes)?;

                write_bytes(
                    &output.absolute,
                    &generated_image_bytes,
                    ragit_fs::WriteMode::CreateOrTruncate,
                )?;

                let generated_image_id = normalize_image(&generated_image_bytes, &context.working_dir, 1200)?;
                Ok(Ok(ToolCallSuccess::ImageEdit {
                    input,
                    prompt: prompt.to_string(),
                    output_path: output,
                    output_image: generated_image_id,
                    requested_size: *size,
                    generated_size: (generated_image.width() as u64, generated_image.height() as u64),
                }))
            },
        }
    }

    pub fn preview(&self) -> String {
        match self {
            ToolCall::Agent { name, .. } => format!("sub-agent `{name}`"),
            ToolCall::Read { path, start, end } => format!(
                "read `{path}` {}",
                if let (None, None) = (start, end) {
                    String::new()
                } else {
                    format!(
                        " ({}..={})",
                        if let Some(start) = start { format!("{start}") } else { String::new() },
                        if let Some(end) = end { format!("{end}") } else { String::new() },
                    )
                },
            ),
            ToolCall::Write { path, mode, content } => format!(
                "{} {} bytes to `{path}`",
                match mode {
                    WriteMode::Create => "create and write",
                    WriteMode::Truncate => "truncate and write",
                    WriteMode::Append => "append",
                },
                content.len(),
            ),
            ToolCall::Patch { path, diff, .. } => {
                let add = diff.iter().filter(
                    |LineDiff { kind, .. }| *kind == DiffKind::Add
                ).count();
                let remove = diff.iter().filter(
                    |LineDiff { kind, .. }| *kind == DiffKind::Remove
                ).count();
                format!(
                    "patch `{path}` (add {add} line{}, remove {remove} line{})",
                    if add == 1 { "" } else { "s" },
                    if remove == 1 { "" } else { "s" },
                )
            },
            ToolCall::Remove { path } => format!("remove `{path}`"),
            ToolCall::Run { command, .. } => format!("run `{}`", join_command_args(command)),
            ToolCall::Ask { to, question, .. } => format!(
                "ask to {} {:?}",
                format!("{to:?}").to_ascii_lowercase(),
                truncate_chars(&question.question, 42),
            ),
            ToolCall::Chrome { script, input, output } => {
                let (WebOrFile::Web(input) | WebOrFile::File(input)) = input;
                format!(
                    "open chrome and render `{input}` to `{output}`{}",
                    if let Some(script) = script {
                        format!(" (script: {})", truncate_chars(script, 42))
                    } else {
                        String::new()
                    },
                )
            },
            ToolCall::ImageEdit { input, prompt, .. } => format!(
                "editing `{input}` {:?}",
                truncate_chars(prompt, 42),
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ToolCallSuccess {
    Agent {
        result: String,
    },
    ReadText {
        path: Path,
        content: String,
        total_lines: u64,
        range: (Option<u64>, Option<u64>),
    },
    ReadPdf {
        path: Path,
        pages: Vec<ImageId>,
        total_pages: u64,
        range: (Option<u64>, Option<u64>),
    },
    ReadImage {
        path: Path,
        id: ImageId,
        size: (u64, u64),
    },
    ReadDir {
        path: Path,
        entries: Vec<FileEntry>,
        total_entries: u64,
        range: (Option<u64>, Option<u64>),
    },
    ReadSymlink {
        path: Path,
        pointee: String,
    },
    Write {
        path: Path,
        content: String,

        // If the LLM writes file at `logs/summary-XXX.md`, this flag is set.
        is_summary: bool,

        // If the LLM truncates an existing file, the harness calculates diff.
        diff: Option<String>,

        mode: WriteMode,
        bytes: u64,
        chars: u64,
        lines: u64,
    },
    Patch {
        path: Path,
        diff: Vec<LineDiff>,

        // Usually, AIs generate diffs with little or no contexts. That's more efficient for AIs.
        // But for human, it's better to have more contexts. So, the harness adds context lines.
        diff_with_context: Vec<LineDiff>,

        new_content: String,
    },
    RemoveFile { path: Path },
    RemoveDir { path: Path },
    Run {
        path: Option<Path>,
        command: Vec<String>,
        elapsed_ms: u64,
        exit_code: i32,
        stdout: DumpOrRedirect,
        stderr: DumpOrRedirect,
    },
    Ask {
        to: AskTo,
        answer: UserAnswer,
    },
    ChromeWeb {
        input: String,
        output_path: Path,
        output_image: ImageId,
        script_output: Option<String>,
    },
    ChromeFile {
        input: Path,
        output_path: Path,
        output_image: ImageId,
        script_output: Option<String>,
    },
    Svg {
        input: Path,
        output_path: Path,
        output_image: ImageId,
    },
    ImageEdit {
        input: Path,
        prompt: String,
        output_path: Path,
        output_image: ImageId,
        requested_size: Option<(u64, u64)>,
        generated_size: (u64, u64),
    },

    // User instructions are converted to the below turns.
    QuestionFromUser {
        q: String,
        a: String,
    },
    InstructionFromUser(String),
}

impl ToolCallSuccess {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<LLMToken> {
        match self {
            ToolCallSuccess::Agent { result } => vec![LLMToken::String(result.to_string())],
            ToolCallSuccess::ReadText { content, .. } => {
                if content.is_empty() {
                    // claude requires every turn to be non-empty
                    vec![LLMToken::String(String::from("(This file is empty.)"))]
                }

                else {
                    vec![LLMToken::String(content.to_string())]
                }
            },
            ToolCallSuccess::ReadPdf { pages, .. } => pages.iter().map(
                |id| LLMToken::Image(*id)
            ).collect(),
            ToolCallSuccess::ReadImage { id, size: (w, h), path } => vec![
                LLMToken::String(format!("path: {path}\nsize: {w}x{h}")),
                LLMToken::Image(*id),
            ],
            ToolCallSuccess::ReadDir { path, entries, total_entries, range } => {
                let entries_count = match range {
                    (None, None) => format!("{total_entries} entries"),
                    (None | Some(0 | 1), Some(e)) => format!("{total_entries} entries (seeing the first {} entries)", e + 1),
                    (Some(s), None) => format!("{total_entries} entries (seeing the last {} entries)", total_entries - s + 1),
                    (Some(s), Some(e)) => format!("{total_entries} entries (seeing {s}..={e})"),
                };
                let entries = entries.iter().map(
                    |entry| match entry {
                        FileEntry::TextFile { name, bytes, lines, .. } => format!("{name}\ttext\t{}\t{lines} lines", prettify_bytes(*bytes)),
                        FileEntry::PdfFile { name, pages } => format!("{name}\tpdf\t{pages} pages"),
                        FileEntry::BrokenPdf { name, bytes } => format!("{name}\t{}\t(Failed to parse pdf file)", prettify_bytes(*bytes)),
                        FileEntry::ImageFile { name, size: (w, h) } => format!("{name}\timage\t{w}x{h}"),
                        FileEntry::BrokenImage { name, bytes } => format!("{name}\t{}\t(Failed to parse image file)", prettify_bytes(*bytes)),
                        FileEntry::EtcFile { name, bytes } => format!("{name}\t{}", prettify_bytes(*bytes)),
                        FileEntry::Dir { name } => format!("{name}/\tdirectory"),
                        FileEntry::Symlink { name } => format!("{name}\tsymlink"),
                    }
                ).collect::<Vec<_>>().join("\n");
                let s = format!(
                    "{path}{}\n{entries_count}\n\n{entries}",
                    if path.to_string().ends_with("/") { "" } else { "/" },
                );
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::ReadSymlink { path, pointee } => {
                let s = format!("`{path}` is a symlink that points to `{pointee}`.");
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::Write { path, bytes, lines, .. } => {
                let s = format!("{} ({lines} lines) of text was successfully written to `{path}`.", prettify_bytes(*bytes));
                vec![LLMToken::String(s)]
            },
            // The diffs are already in the context, so it doesn't have to show the diffs again.
            ToolCallSuccess::Patch { path, .. } => {
                let s = format!("Successfully updated `{path}`.");
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::RemoveFile { path } => {
                let s = format!("Successfully removed file `{path}`.");
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::RemoveDir { path } => {
                let s = format!("Successfully removed directory `{path}`.");
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::Run { path, command, elapsed_ms, exit_code, stdout, stderr } => {
                let path = if let Some(path) = path {
                    format!("\n<path>{path}</path>")
                } else {
                    String::new()
                };
                let (stdout, stdout_truncated) = match stdout {
                    DumpOrRedirect::Dump(stdout) => truncate_middle(stdout, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stdout) => (format!("Redirected to `{stdout}`"), None),
                };
                let (stderr, stderr_truncated) = match stderr {
                    DumpOrRedirect::Dump(stderr) => truncate_middle(stderr, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stderr) => (format!("Redirected to `{stderr}`"), None),
                };
                let stdout_truncated = if let Some(stdout_truncated) = stdout_truncated {
                    format!("\nstdout is very long, so {stdout_truncated} bytes were truncated")
                } else {
                    String::new()
                };
                let stderr_truncated = if let Some(stderr_truncated) = stderr_truncated {
                    format!("\nstderr is very long, so {stderr_truncated} bytes were truncated")
                } else {
                    String::new()
                };

                let s = format!(
"{stdout_truncated}{stderr_truncated}
<run_result>{path}
<command>{}</command>
<elapsed>{}</elapsed>
<exit_code>{exit_code}</exit_code>
<stdout>
{stdout}
</stdout>
<stderr>
{stderr}
</stderr>
</run_result>
",
                    join_command_args(command),
                    prettify_time(*elapsed_ms),
                );
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::Ask { answer, .. } => vec![LLMToken::String(answer.to_string())],
            ToolCallSuccess::ChromeWeb { output_path, output_image, script_output, .. } |
            ToolCallSuccess::ChromeFile { output_path, output_image, script_output, .. } => {
                let input = match self {
                    ToolCallSuccess::ChromeWeb { input, .. } => input.to_string(),
                    ToolCallSuccess::ChromeFile { input, .. } => input.to_string(),
                    _ => unreachable!(),
                };

                vec![
                    LLMToken::String(format!(
                        "Successfully opened `{input}`{}, captured a screenshot, and saved it to `{output_path}`.{}",
                        if script_output.is_some() { ", ran javascript" } else { "" },
                        if let Some(script_output) = script_output { format!("\nscript output: {script_output}") } else { String::new() },
                    )),
                    LLMToken::Image(*output_image),
                ]
            },
            ToolCallSuccess::Svg { input, output_path, output_image } => vec![
                    LLMToken::String(format!(
                        "Successfully opened `{input}`, captured a screenshot, and saved it to `{output_path}`.\nNOTE: For svg files, the harness uses rust resvg library to render the files instead of using chrome. If you really want to use chrome, convert the file to html (or anything other than svg) and try again.",
                    )),
                    LLMToken::Image(*output_image),
            ],
            ToolCallSuccess::ImageEdit { input, output_path, output_image, .. } => vec![
                LLMToken::String(format!("Successfully edited `{input}` and saved the result at `{output_path}`.")),
                LLMToken::Image(*output_image),
            ],

            // LLM shouldn't be able to see this.
            ToolCallSuccess::QuestionFromUser { q, a } => vec![LLMToken::String(format!("
Answered a user question. NOTE: this QA has nothing to do with the current work and you should ignore this.

<question>
{q}
</question>

<answer>
{a}
</answer>
"))],

            ToolCallSuccess::InstructionFromUser(i) => vec![LLMToken::String(i.to_string())],
        }
    }

    pub fn get_result_path(&self) -> Result<Option<(String, Option<String>)>, Error> {
        match self {
            ToolCallSuccess::ReadText { path, .. } |
            ToolCallSuccess::ReadPdf { path, .. } |
            ToolCallSuccess::ReadImage { path, .. } |
            ToolCallSuccess::ReadSymlink { path, .. } |
            ToolCallSuccess::Write { path, .. } |
            ToolCallSuccess::Patch { path, .. } |
            ToolCallSuccess::ImageEdit { output_path: path, .. } => Ok(Some((parent(&path.absolute)?, Some(basename(&path.absolute)?)))),
            ToolCallSuccess::ReadDir { path, .. } => Ok(Some((path.absolute.to_string(), None))),

            // TODO: what if AI reads a directory with chrome?
            ToolCallSuccess::ChromeFile { input, .. } => Ok(Some((parent(&input.absolute)?, Some(basename(&input.absolute)?)))),
            _ => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ToolCallError {
    InvalidPath(String),

    // permission errors (by user)
    ToolPermissionDeniedByUser {
        kind: ToolPermissionKind,
        path: Option<Path>,
        not_responding: bool,
    },
    RunPermissionDeniedByUser {
        command: Vec<String>,
        not_responding: bool,
    },

    // read errors
    CannotReadIndexDir,
    NoSuchFile {
        path: Path,
    },
    InvalidRange {
        r#type: RangeType,
        length: u64,
        given: (Option<u64>, Option<u64>),
    },
    InvalidFileType {
        path: Path,
    },
    TextTooLongToRead {
        path: Path,
        length: u64,
        limit: u64,
    },
    TooManyTextLinesToRead {
        path: Path,
        length: u64,
        limit: u64,
    },
    TooManyPdfPagesToRead {
        path: Path,
        pages: u64,
        limit: u64,
        given_range: (Option<u64>, Option<u64>),
    },
    TooManyDirEntriesToRead {
        path: Path,
        entries: u64,
        limit: u64,
        given_range: (Option<u64>, Option<u64>),
    },
    TooManyReadWithoutSummary,
    BrokenFile {
        path: Path,
        kind: String,
        error: String,
    },
    ReadingExactSameFile { path: Path },
    SymlinkWithRange {
        path: Path,
        pointee: String,
        range: (Option<u64>, Option<u64>),
    },

    // write errors
    // If the given path is `docs/`, that's a directory whether or not that already exists.
    CannotWriteToIndexDir,
    CannotWriteToDirectory {
        path: Path,
        exists: bool,
    },
    CannotCreateParentDirectory {
        parent: String,
        path: Path,
        error: Option<String>,
    },
    WriteModeError {
        path: Path,
        mode: WriteMode,
        exists: bool,
    },
    TextTooLongToWrite {
        path: Path,
        length: u64,
        limit: u64,
    },
    NoSummaryInDoneFile,

    // patch errors
    CannotPatchSymlink { path: Path },
    CannotPatchNonExistFile { path: Path },
    CannotPatchDir { path: Path },
    CanOnlyPatchText {
        path: Path,

        // This is `format!("{:?}", read_string(path).unwrap_err())`
        error: String,
    },
    CannotApplyPatch(PatchError),

    // remove errors
    CannotRemoveNonExistFile { path: Path },

    // run errors
    CommandTimeoutTooLong {
        max: u64,
        given: u64,
    },
    NoSuchBinary {
        binary: String,
        available_binaries: Vec<String>,
    },
    Timeout {
        path: Option<Path>,
        command: Vec<String>,
        timeout: u64,
        stdout: DumpOrRedirect,
        stderr: DumpOrRedirect,
    },

    // ask errors
    UserNotResponding,
    UserRejectedToRespond,
    WebSearchDisabled,

    // chrome errors
    SvgWithScript,

    // image-edit errors
    NotAnImage {
        path: Path,
    },
    ImageRequestError {
        status_code: u16,
        message: String,
    },
    ImageEditDisabled,

    // etc
    SupposedToWriteSummary { write_path: Option<Path> },
    UserInterrupt,
}

impl ToolCallError {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<LLMToken> {
        let s = match self {
            ToolCallError::NoSuchFile { path } => format!("There's no such file: `{path}`."),
            ToolCallError::CannotReadIndexDir => String::from("You're not allowed to read inside `.neukgu/`."),
            ToolCallError::InvalidRange { r#type: range_type, length, given: (start, end) } => format!(
                "{}..{} is an invalid range. The {} only has {length} {}.",
                if let Some(start) = start { format!("{start}") } else { String::new() },
                if let Some(end) = end { format!("{end}") } else { String::new() },
                match range_type {
                    RangeType::Line | RangeType::PdfPage => "file",
                    RangeType::FileEntry => "directory",
                },
                match (range_type, *length == 1) {
                    (RangeType::Line, true) => "line",
                    (RangeType::Line, false) => "lines",
                    (RangeType::PdfPage, true) => "page",
                    (RangeType::PdfPage, false) => "pages",
                    (RangeType::FileEntry, true) => "entry",
                    (RangeType::FileEntry, false) => "entries",
                },
            ),
            ToolCallError::TextTooLongToRead { path, length, limit } => format!(
                "The file `{path}` is too long to read at once. The file is {}, and the environment won't allow you to open a file that is larger than {}. You can read the first 100 lines with <read><end>100</end><path>{path}</path></read>, or use search tools like ripgrep.",
                prettify_bytes(*length),
                prettify_bytes(*limit),
            ),
            ToolCallError::TooManyTextLinesToRead { length, limit, .. } => format!(
                "You're trying to read too many lines at once. You're trying to read {length} lines, and the environment won't allow you to read more than {limit} lines at once.",
            ),
            ToolCallError::TooManyDirEntriesToRead { path, entries, limit: _, given_range: (None, None) } => format!(
                "`{path}/` is gigantic, it has {entries} entries. Please specify range with <start> and <end>. For example, you can read the first 100 entries with <end>100</end>.",
            ),
            ToolCallError::TooManyDirEntriesToRead { limit, .. } => format!("You can read at most `{limit}` entries at once. Please give me a smaller range."),
            ToolCallError::TooManyReadWithoutSummary => String::from("You're keep reading files without writing summaries. Please write a summary of what you're doing and what you've discovered so far at logs/summary-XXX.md."),
            ToolCallError::ReadingExactSameFile { path } => format!("You already read `{path}` and I just gave you the content of `{path}`. Try do something else."),
            ToolCallError::BrokenFile { path, kind, error } => format!("Tried to read {path}, but failed to parse the {kind} file.\nerror: {error}"),

            ToolCallError::ToolPermissionDeniedByUser { kind, path, not_responding } => format!(
                "The user denied to give you a permission to {}{}. The harness asked {:?} to the user and the user {}.",
                kind.describe(),
                if let Some(path) = path { format!(" `{path}`") } else { String::new() },
                kind.question(),
                if *not_responding {
                    "didn't respond"
                } else {
                    "said no"
                },
            ),
            ToolCallError::CannotWriteToDirectory { path, exists } => if *exists {
                format!("You can't write to `{path}` because it already exists and is a directory.")
            } else {
                let mut path = path.to_string();

                if !path.ends_with("/") {
                    path = format!("{path}/");
                }

                format!("You can't create a directory with that tool. If you want to create a directory `{path}`, just create a file inside the directory. Then all the intermediate directories will be created.")
            },
            ToolCallError::CannotCreateParentDirectory { parent, path, error } => format!(
                "Tried to create parent directory of `{path}`, but it failed.\nParent directory: `{parent}`{}",
                if let Some(e) = error { format!("\nError: {e}") } else { String::new() },
            ),
            ToolCallError::WriteModeError { path, mode, exists } => match (mode, exists) {
                (WriteMode::Create, true) => format!("You can't create `{path}` because it already exists. Try with \"truncate\" or \"append\"."),
                (WriteMode::Truncate, false) => format!("You can't truncate `{path}` because it does not exist. Try with \"create\"."),
                (WriteMode::Append, false) => format!("You can't append to `{path}` because it does not exist. Try with \"create\"."),
                _ => unreachable!(),
            },
            ToolCallError::TextTooLongToWrite { path, length, limit } => format!(
                "Failed to write to the file.\nYou attempted to write too long contents to `{path}`. The environment allows up to {} write at once, but your content is {}.\nYou can try to make it shorter, or split it into multiple files.",
                prettify_bytes(*limit),
                prettify_bytes(*length),
            ),
            ToolCallError::NoSummaryInDoneFile => String::from("You're supposed to write summary of what you've done at `logs/done` file. Try write the file again with the summary."),
            ToolCallError::CanOnlyPatchText { path, error } => format!("`read_string({path:?})` failed with `{error}`."),
            ToolCallError::CannotApplyPatch(e) => match e {
                PatchError::NoMatch { missing_context_marker: None } => String::from("I can't apply the patch because no matches are found."),
                PatchError::NoMatch { missing_context_marker: Some(line) } => format!(
                    "It seems like you're confused with the context marker. A context line in unified diff format consists of a context marker (' ' in this case) and the content of the line.\nSo, you have to insert ' ' before {line:?} to make it a context line, like {:?}.",
                    format!(" {line}"),
                ),
                PatchError::MultipleMatch => String::from("I found multiple matches in the file that can apply your patch. Please give me more contexts so that I can decide where to patch."),
                PatchError::NoUpdate => String::from("I can't apply the patch because the patch only has context lines, and there're no lines to remove or update. Please specify what lines to remove or delete."),
            },
            ToolCallError::CannotRemoveNonExistFile { path } => format!("You can't remove `{path}` because the file doesn't exist."),
            ToolCallError::RunPermissionDeniedByUser { command, not_responding } => format!(
                "The user denied to give you a permission to run `{}`.{}",
                join_command_args(command),
                if *not_responding { "\nNOTE: I asked for a permission, but the user is not responding, so I failed to get a permission." } else { "" },
            ),
            ToolCallError::NoSuchBinary { binary, available_binaries } => format!(
                "There's no such binary: `{binary}`.\nAvailable binaries are: {}.{}{}",
                available_binaries.join(", "),
                if binary.contains("/") {
                    "\nDon't call the binary with its path. Run it with just the name."
                } else {
                    ""
                },
                if binary == "cd" {
                    "

If you want to run the binary in another directory, use `<path>` parameter, like this:

<run>
<path>path-to-run-binary/</path>
<command>your-command</command>
</run>"
                } else if binary == "ls" {
                    "

If you want to list the directory, use `<read>` tool with the directory's path, like this:

<read>
<path>path-to-the-directory/</path>
</read>
"
                } else {
                    ""
                },
            ),
            ToolCallError::Timeout { path, command, timeout, stdout, stderr } => {
                let path = if let Some(path) = path {
                    format!("\n<path>{path}</path>")
                } else {
                    String::new()
                };
                let (stdout, stdout_truncated) = match stdout {
                    DumpOrRedirect::Dump(stdout) => truncate_middle(stdout, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stdout) => (format!("Redirected to {}", stdout.to_string()), None),
                };
                let (stderr, stderr_truncated) = match stderr {
                    DumpOrRedirect::Dump(stderr) => truncate_middle(stderr, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stderr) => (format!("Redirected to {}", stderr.to_string()), None),
                };
                let stdout_truncated = if let Some(stdout_truncated) = stdout_truncated {
                    format!("\nstdout is very long, so {stdout_truncated} bytes were truncated")
                } else {
                    String::new()
                };
                let stderr_truncated = if let Some(stderr_truncated) = stderr_truncated {
                    format!("\nstderr is very long, so {stderr_truncated} bytes were truncated")
                } else {
                    String::new()
                };

                format!(
"
Command timeout! The process didn't terminate for {timeout} seconds.
{stdout_truncated}{stderr_truncated}
<run_result>{path}
<command>{}</command>
<stdout>
{stdout}
</stdout>
<stderr>
{stderr}
</stderr>
</run_result>
",
                    join_command_args(command),
                )
            },
            ToolCallError::UserNotResponding => String::from("User is not responding."),
            ToolCallError::UserRejectedToRespond => String::from("User doesn't want to answer your question."),
            ToolCallError::WebSearchDisabled => String::from("The web search agent is not available now."),
            ToolCallError::ImageRequestError { status_code, message } => format!(
                "The API request returned status code {status_code}:\n\n{message}",
            ),
            ToolCallError::ImageEditDisabled => String::from("The image-edit agent is not available now."),
            ToolCallError::SupposedToWriteSummary { write_path } => match write_path {
                Some(path) => format!("`{path}` is not a correct path for a summary file. You have to write summary at `logs/`. The summary file name must start with \"summary\", and has extension \".md\". For example, `logs/summary-refactor.md` or `logs/summary-test.md`"),
                None => String::from("You're supposed to summarize what you're doing and what you've discovered so far and write the summary at `logs/summary-XXX.md`."),
            },
            ToolCallError::UserInterrupt => format!(
                "(This turn is supposed to be removed by `context.discard_previous_turn()`. If you, the human user, see this message in GUI or if you, an AI agent, see this message in the context, there's a bug in the harness.)",
            ),
            _ => panic!("TODO: {self:?}"),
        };

        vec![LLMToken::String(s)]
    }

    pub fn get_result_path(&self) -> Result<Option<(String, Option<String>)>, Error> {
        match self {
            ToolCallError::InvalidFileType { path } |
            ToolCallError::TextTooLongToRead { path, .. } |
            ToolCallError::TooManyTextLinesToRead { path, .. } |
            ToolCallError::TooManyPdfPagesToRead { path, .. } |
            ToolCallError::BrokenFile { path, .. } |
            ToolCallError::WriteModeError { path, .. } => Ok(Some((parent(&path.absolute)?, Some(basename(&path.absolute)?)))),
            ToolCallError::TooManyDirEntriesToRead { path, .. } => Ok(Some((path.absolute.to_string(), None))),
            _ => Ok(None),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ToolKind {
    Agent,
    Read,
    Write,
    Patch,
    Remove,
    Run,
    Ask,
    Chrome,
    ImageEdit,
}

impl ToolKind {
    pub fn all() -> Vec<ToolKind> {
        vec![
            ToolKind::Agent,
            ToolKind::Read,
            ToolKind::Write,
            ToolKind::Patch,
            ToolKind::Remove,
            ToolKind::Run,
            ToolKind::Ask,
            ToolKind::Chrome,
            ToolKind::ImageEdit,
        ]
    }

    pub fn from_name(name: &[u8]) -> Option<ToolKind> {
        match name {
            b"agent" => Some(ToolKind::Agent),
            b"read" => Some(ToolKind::Read),
            b"write" => Some(ToolKind::Write),
            b"patch" => Some(ToolKind::Patch),
            b"remove" => Some(ToolKind::Remove),
            b"run" => Some(ToolKind::Run),
            b"ask" => Some(ToolKind::Ask),
            b"chrome" => Some(ToolKind::Chrome),
            b"image-edit" => Some(ToolKind::ImageEdit),
            _ => None,
        }
    }

    pub fn tag_name(&self) -> &'static str {
        match self {
            ToolKind::Agent => "agent",
            ToolKind::Read => "read",
            ToolKind::Write => "write",
            ToolKind::Patch => "patch",
            ToolKind::Remove => "remove",
            ToolKind::Run => "run",
            ToolKind::Ask => "ask",
            ToolKind::Chrome => "chrome",
            ToolKind::ImageEdit => "image-edit",
        }
    }

    pub fn check_arg_name(&self, arg: &[u8]) -> bool {
        self.valid_args().contains(&String::from_utf8_lossy(arg).to_string())
    }

    pub fn valid_args(&self) -> Vec<String> {
        match self {
            ToolKind::Agent => vec!["name", "prompt"],
            ToolKind::Read => vec!["path", "start", "end"],
            ToolKind::Write => vec!["path", "mode", "content"],
            ToolKind::Patch => vec!["path", "diff"],
            ToolKind::Remove => vec!["path"],
            ToolKind::Run => vec!["timeout", "command", "path", "env", "stdout", "stderr"],
            ToolKind::Ask => vec!["to", "question"],
            ToolKind::Chrome => vec!["input", "output", "script"],
            ToolKind::ImageEdit => vec!["input", "prompt", "size", "output"],
        }.iter().map(|arg| arg.to_string()).collect()
    }

    pub fn optional(&self) -> bool {
        match self {
            ToolKind::Agent => true,
            ToolKind::Read => false,
            ToolKind::Write => false,
            ToolKind::Patch => true,
            ToolKind::Remove => false,
            ToolKind::Run => false,
            ToolKind::Ask => false,
            ToolKind::Chrome => true,
            ToolKind::ImageEdit => true,
        }
    }
}

impl Context {
    // Sometimes the harness want the AI to do specific things.
    pub fn validate_tool_call(&mut self, tool: &ToolCall) -> Result<Result<(), ToolCallError>, Error> {
        match self.history.last() {
            Some(TurnSummary { id, .. }) => {
                let turn_id = id.clone();
                let last_turn = self.load_turn(&turn_id)?;

                // 1. If the last turn was `TooManyReadWithoutSuammry` and the current tool is not writing summary, it's an error.
                if let Turn { turn_result: TurnResult::ToolCallError(ToolCallError::TooManyReadWithoutSummary), .. } = &last_turn {
                    match tool {
                        ToolCall::Write { path, .. } => {
                            let path = normalize_path(path, &self.working_dir);

                            if path.as_ref().map(|p| p.is_summary_file()).unwrap_or(false) {
                                Ok(Ok(()))
                            } else {
                                Ok(Err(ToolCallError::SupposedToWriteSummary { write_path: path }))
                            }
                        },
                        ToolCall::Ask { to: AskTo::User, .. } => Ok(Ok(())),
                        _ => Ok(Err(ToolCallError::SupposedToWriteSummary { write_path: None })),
                    }
                }

                // 2. It wrote `logs/done` but there's no summary in the file or it's too short.
                else if let ToolCall::Write { path, content, .. } = tool {
                    let path = normalize_path(path, &self.working_dir);

                    if path.as_ref().map(|p| p.is_done_file()).unwrap_or(false) && content.len() < 10 {
                        Ok(Err(ToolCallError::NoSummaryInDoneFile))
                    }

                    else {
                        Ok(Ok(()))
                    }
                }

                // 3. It's reading the same file with the same range, over and over.
                //    -> When I ask something to GPT, it just keeps reading `neukgu-instruction.md` over and over, and
                //       I have no idea why. Maybe something's wrong with the model. I have to manually interrupt and
                //       say "stop reading the instruction and start working". Instead of manual interruptions, the harness
                //       interrupts the model and nudges it.
                else if let ToolCall::Read { path, start: None, end: None } = tool {
                    if let Some(ParsedSegment { tool: Some(tool), .. }) = &last_turn.parse_result {
                        if let ToolCall::Read { path: last_path, start: None, end: None } = tool && normalize_path(path, &self.working_dir) == normalize_path(last_path, &self.working_dir) && normalize_path(path, &self.working_dir).is_some() {
                            return Ok(Err(ToolCallError::ReadingExactSameFile { path: normalize_path(path, &self.working_dir).unwrap() }));
                        }
                    }

                    Ok(Ok(()))
                }

                else {
                    Ok(Ok(()))
                }
            },
            None => Ok(Ok(())),
        }
    }
}

pub fn join_command_args(args: &[String]) -> String {
    args.iter().map(
        |arg| if arg.contains(" ") { format!("{arg:?}") } else { arg.to_string() }
    ).collect::<Vec<_>>().join(" ")
}

fn truncate_middle(s: &str, limit: u64) -> (String, Option<usize>) {
    if (s.len() as u64) < limit {
        (s.to_string(), None)
    } else {
        let d = limit * 2 / 5;
        let pre = String::from_utf8_lossy(&s.as_bytes()[..(d as usize)]).to_string();
        let post = String::from_utf8_lossy(&s.as_bytes()[(s.len() - d as usize)..]).to_string();
        let truncated = s.len() as u64 - d * 2;
        (format!("{pre}...({truncated} bytes truncated)...{post}"), Some(truncated as usize))
    }
}
