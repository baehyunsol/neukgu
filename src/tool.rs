use async_std::task::sleep;
use crate::{
    Config,
    Context,
    Error,
    ImageId,
    LLMToken,
    LogEntry,
    Model,
    ParsedSegment,
    Turn,
    TurnResult,
    TurnSummary,
    UserResponse,
    clean_sandbox,
    export_to_sandbox,
    from_browser_error,
    image,
    import_from_sandbox,
    normalize_and_get_id,
    prettify_bytes,
    prettify_time,
    subprocess,
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
mod read;
mod run;
mod write;

pub use ask::{AskTo, ask_question_to_web};
pub use chrome::WebOrFile;
pub use image_edit::ImageRequest;
pub use patch::{DiffKind, LineDiff, PatchError, parse_line_diff, patch_diff, patch_file};
pub use read::{
    FileEntry,
    RangeType,
    TypedFile,
    check_read_path,
    read_file,
};
pub use run::{ParseCommandError, load_available_binaries, parse_command};
use read::check_read_permission;
pub use write::{DumpOrRedirect, WriteMode, check_write_path};

type Path = Vec<String>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ToolCall {
    // start and end are both inclusive.
    // They're 1-based index.
    Read {
        path: Path,
        start: Option<u64>,
        end: Option<u64>,
    },
    Write {
        path: Path,
        mode: WriteMode,
        content: String,
    },
    Patch {
        path: Path,
        diff: Vec<LineDiff>,
    },
    Run {
        timeout: Option<u64>,
        command: Vec<String>,
        path: Option<Path>,
        env: Vec<(String, String)>,
        stdout: Option<Path>,
        stderr: Option<Path>,
    },
    Ask {
        // A random-generated integer.
        // It prevents the frontend from answering the same question multiple times.
        id: u64,

        to: AskTo,
        question: String,
    },
    Chrome {
        input: Path,
        output: Path,
        script: Option<String>,
    },
    ImageEdit {
        input: Path,
        prompt: String,
        size: Option<(u64, u64)>,
        output: Path,
    },
}

impl ToolCall {
    pub async fn run(&self, context: &mut Context, config: &Config) -> Result<Result<ToolCallSuccess, ToolCallError>, Error> {
        context.logger.log(LogEntry::ToolCallStart(self.clone()))?;

        match self {
            ToolCall::Read { path, start, end } => {
                if context.is_reading_too_much(config)? {
                    return Ok(Err(ToolCallError::TooManyReadWithoutSummary));
                }

                let joined_path = match check_read_path(path, &context.working_dir)? {
                    Ok((joined_path, _)) => joined_path,
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                let start_i = (*start).unwrap_or(0);
                let end_i = (*end).unwrap_or(u64::MAX - 1);

                match read_file(&joined_path, context)? {
                    TypedFile::Text(s) => {
                        if s.len() as u64 > config.text_file_max_len && start.is_none() && end.is_none() {
                            Ok(Err(ToolCallError::TextTooLongToRead {
                                path: joined_path,
                                length: s.len() as u64,
                                limit: config.text_file_max_len,
                            }))
                        }

                        else if let Some(end) = end && start_i + config.text_file_max_lines < *end {
                            Ok(Err(ToolCallError::TooManyTextLinesToRead {
                                path: joined_path,
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
                                    path: joined_path,
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
                                path: joined_path,
                                pages: pages.len() as u64,
                                limit: config.pdf_max_pages,
                                given_range: (*start, *end),
                            }))
                        }

                        else if let Some(end) = end && start_i + config.pdf_max_pages < *end {
                            Ok(Err(ToolCallError::TooManyPdfPagesToRead {
                                path: joined_path,
                                pages: *end - start_i + 1,
                                limit: config.pdf_max_pages,
                                given_range: (*start, Some(*end)),
                            }))
                        }

                        else {
                            match pages.get((start_i.max(1) as usize - 1)..(end_i as usize).min(pages.len())) {
                                Some(sliced_pages) if sliced_pages.len() > 0 || pages.is_empty() => Ok(Ok(ToolCallSuccess::ReadPdf {
                                    path: joined_path,
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
                    TypedFile::BrokenPdf { error } => Ok(Err(ToolCallError::BrokenFile { path: joined_path, kind: String::from("pdf"), error })),
                    TypedFile::Image(id, size) => Ok(Ok(ToolCallSuccess::ReadImage { path: joined_path, id, size })),
                    TypedFile::BrokenImage { error } => Ok(Err(ToolCallError::BrokenFile { path: joined_path, kind: String::from("image"), error })),
                    TypedFile::Dir(entries) => {
                        if entries.len() as u64 > config.dir_max_entries && (start.is_none() || start_i < 2) && end.is_none() {
                            Ok(Err(ToolCallError::TooManyDirEntriesToRead {
                                path: joined_path,
                                entries: entries.len() as u64,
                                limit: config.dir_max_entries,
                                given_range: (*start, *end),
                            }))
                        }

                        else if let Some(end) = end && start_i + config.dir_max_entries < *end {
                            Ok(Err(ToolCallError::TooManyDirEntriesToRead {
                                path: joined_path,
                                entries: *end - start_i + 1,
                                limit: config.dir_max_entries,
                                given_range: (*start, Some(*end)),
                            }))
                        }

                        else {
                            match entries.get((start_i.max(1) as usize - 1)..(end_i as usize).min(entries.len())) {
                                Some(sliced_entries) if sliced_entries.len() > 0 || entries.is_empty() => Ok(Ok(ToolCallSuccess::ReadDir {
                                    path: joined_path,
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
                            Ok(Ok(ToolCallSuccess::ReadSymlink {
                                path: joined_path,
                                pointee,
                            }))
                        }

                        else {
                            Ok(Err(ToolCallError::SymlinkWithRange {
                                path: joined_path,
                                pointee,
                                range: (*start, *end),
                            }))
                        }
                    },
                    TypedFile::Etc => Ok(Err(ToolCallError::InvalidFileType { path: joined_path })),
                }
            },
            ToolCall::Write { path, mode, content } => {
                let (joined_path, real_path) = match check_write_path(path, &context.working_dir, Some(*mode))? {
                    Ok((joined_path, real_path)) => (joined_path, real_path),
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                if content.len() as u64 > config.text_file_max_len {
                    return Ok(Err(ToolCallError::TextTooLongToWrite {
                        path: joined_path,
                        length: content.len() as u64,
                        limit: config.text_file_max_len,
                    }));
                }

                let prev_content = if *mode == WriteMode::Create {
                    String::new()
                } else {
                    String::from_utf8_lossy(&read_bytes(&real_path)?).to_string()
                };

                // It applies some heuristics to trailing/leading newlines.
                if *mode == WriteMode::Append {
                    write_string(
                        &real_path,
                        "\n",
                        (*mode).into(),
                    )?;
                }

                write_string(
                    &real_path,
                    content.trim(),
                    (*mode).into(),
                )?;

                if *mode != WriteMode::Append {
                    write_string(
                        &real_path,
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
                        &String::from_utf8_lossy(&read_bytes(&real_path)?),
                        5,
                        None,
                    ));
                }

                Ok(Ok(ToolCallSuccess::Write {
                    path: joined_path,
                    content: content.to_string(),
                    is_summary: is_summary_path(path),
                    diff,
                    mode: *mode,
                    bytes: byte_count,
                    chars: char_count,
                    lines: line_count,
                }))
            },
            ToolCall::Patch { path, diff } => {
                let (joined_path, real_path) = match check_write_path(path, &context.working_dir, None)? {
                    Ok((joined_path, real_path)) => (joined_path, real_path),
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                match (exists(&real_path), is_symlink(&real_path), is_dir(&real_path), read_string(&real_path)) {
                    (_, true, _, _) => {
                        return Ok(Err(ToolCallError::CannotPatchSymlink { path: joined_path }));
                    },
                    (false, _, _, _) => {
                        return Ok(Err(ToolCallError::CannotPatchNonExistFile { path: joined_path }));
                    },
                    (_, _, true, _) => {
                        return Ok(Err(ToolCallError::CannotPatchDir { path: joined_path }));
                    },
                    (_, _, _, Err(e)) => {
                        return Ok(Err(ToolCallError::CanOnlyPatchText { path: joined_path, error: format!("{e:?}") }));
                    },
                    _ => {},
                }

                let mut result = patch_file(&real_path, diff);

                if let Ok(ToolCallSuccess::Patch { new_content, path, .. }) = &mut result {
                    write_string(&real_path, new_content, WriteMode::Truncate.into())?;
                    *path = joined_path;
                }

                Ok(result)
            },
            ToolCall::Run { timeout, command, path, env, stdout, stderr } => {
                let mut env = env.to_vec();
                let stdout_path = stdout.clone();
                let stderr_path = stderr.clone();
                let mut env_joined_path = None;
                let (mut stdout_real_path, mut stderr_real_path) = (None, None);

                if let Some(path) = path {
                    match check_read_path(path, &context.working_dir)? {
                        Ok((joined_path, _)) => {
                            env_joined_path = Some(joined_path);
                        },
                        Err(e) => {
                            return Ok(Err(e));
                        },
                    }
                }

                if let Some(stdout) = stdout {
                    match check_write_path(stdout, &context.working_dir, None)? {
                        Ok((_, real_path)) => {
                            stdout_real_path = Some(real_path);
                        },
                        Err(e) => {
                            return Ok(Err(e));
                        },
                    }
                }

                if let Some(stderr) = stderr {
                    match check_write_path(stderr, &context.working_dir, None)? {
                        Ok((_, real_path)) => {
                            stderr_real_path = Some(real_path);
                        },
                        Err(e) => {
                            return Ok(Err(e));
                        },
                    }
                }

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
                let mut env_path = sandbox_at.clone();

                if let Some(path) = &env_joined_path {
                    env_path = join(&env_path, path)?;
                }

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
                    &env_path,
                    timeout,
                    &context.working_dir,
                    true,
                )?;
                let elapsed_ms = Instant::now().duration_since(started_at).as_millis() as u64;
                import_from_sandbox(&sandbox_at, &context.working_dir, false /* copy_index_dir */)?;
                clean_sandbox(&config.sandbox_root, &sandbox_at, &context.working_dir)?;

                let timeout_value = timeout;
                let subprocess::Output { status, stdout, stderr, elapsed_ms: _, timeout } = result;

                let stdout = match &stdout_real_path {
                    Some(path) => {
                        write_bytes(
                            path,
                            &stdout,
                            ragit_fs::WriteMode::CreateOrTruncate,
                        )?;
                        DumpOrRedirect::Redirect(stdout_path.unwrap())
                    },
                    None => DumpOrRedirect::Dump(String::from_utf8_lossy(&stdout).to_string()),
                };
                let stderr = match &stderr_real_path {
                    Some(path) => {
                        write_bytes(
                            path,
                            &stderr,
                            if stdout_real_path == stderr_real_path {
                                ragit_fs::WriteMode::AlwaysAppend
                            } else {
                                ragit_fs::WriteMode::CreateOrTruncate
                            },
                        )?;
                        DumpOrRedirect::Redirect(stderr_path.unwrap())
                    },
                    None => DumpOrRedirect::Dump(String::from_utf8_lossy(&stderr).to_string()),
                };

                if timeout {
                    Ok(Err(ToolCallError::Timeout {
                        path: env_joined_path,
                        command: command.to_vec(),
                        timeout: timeout_value,
                        stdout,
                        stderr,
                    }))
                }

                else {
                    Ok(Ok(ToolCallSuccess::Run {
                        path: env_joined_path,
                        command: command.to_vec(),
                        elapsed_ms,
                        exit_code: status,
                        stdout,
                        stderr,
                    }))
                }
            },
            ToolCall::Ask { id, to: AskTo::User, question } => {
                let response;
                let tool_call_result = 'block: {
                    if config.user_response_timeout == 0 {
                        response = UserResponse::Reject;
                        break 'block Err(ToolCallError::UserRejectedToRespond);
                    }

                    context.ask_to_user(*id, question.to_string())?;

                    if let Err(Error::FrontendNotAvailable) = context.wait_for_fe() {
                        response = UserResponse::Timeout;
                        break 'block Err(ToolCallError::UserNotResponding);
                    }

                    // It waits 3 more seconds than the set timeout because fe is a few seconds slower than be
                    for _ in 0..(config.user_response_timeout + 3) {
                        if let Some(response_) = context.check_user_response(*id)? {
                            response = response_.clone();

                            match response_ {
                                UserResponse::Answer(answer) => {
                                    break 'block Ok(ToolCallSuccess::Ask { to: AskTo::User, answer });
                                },
                                UserResponse::Timeout => {
                                    break 'block Err(ToolCallError::UserNotResponding);
                                },
                                UserResponse::Reject => {
                                    break 'block Err(ToolCallError::UserRejectedToRespond);
                                },
                            }
                        }

                        sleep(Duration::from_millis(1_000)).await;
                        context.sync_with_fe()?;
                    }

                    response = UserResponse::Timeout;
                    Err(ToolCallError::UserNotResponding)
                };

                context.answer_to_llm(*id, question.to_string(), response)?;
                Ok(tool_call_result)
            },
            ToolCall::Ask { id: _, to: AskTo::Web, question } => match config.agents.search {
                Model::Disabled | Model::Mock => Ok(Err(ToolCallError::WebSearchDisabled)),
                _ => {
                    let answer = ask_question_to_web(question, config, &context.working_dir, &mut context.logger).await?;
                    Ok(Ok(ToolCallSuccess::Ask { to: AskTo::Web, answer }))
                },
            },
            // TODO: error if `script` is set and `input` is not an html
            ToolCall::Chrome { script, input, output } => {
                let mut joined_input = input.join("/");
                let (joined_output, real_output_path) = match check_write_path(output, &context.working_dir, None)? {
                    Ok((joined_path, real_path)) => (joined_path, real_path),
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                let url = {
                    let path = WebOrFile::from(input);

                    if let WebOrFile::File(path) = &path {
                        let (joined_input_, real_input_path) = match check_read_path(path, &context.working_dir)? {
                            Ok((joined_path, real_path)) => (joined_path, real_path),
                            Err(e) => {
                                return Ok(Err(e));
                            },
                        };
                        joined_input = joined_input_;

                        if joined_input.to_ascii_lowercase().ends_with(".svg") {
                            let svg_data = read_string(&real_input_path)?;

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
                            pixmap.save_png(&real_output_path)?;
                            // the VIBE ends here

                            let png_data = read_bytes(&real_output_path)?;
                            let image_id = image::normalize_and_get_id(&png_data, &context.working_dir)?;
                            return Ok(Ok(ToolCallSuccess::Chrome { input: joined_input, output_path: joined_output, output_image: image_id, script_output: None }));
                        }
                    }

                    path.to_url(&context.working_dir)?
                };

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
                let image_id = image::normalize_and_get_id(&png_data, &context.working_dir)?;
                write_bytes(&real_output_path, &png_data, ragit_fs::WriteMode::CreateOrTruncate)?;
                Ok(Ok(ToolCallSuccess::Chrome { input: joined_input, output_path: joined_output, output_image: image_id, script_output }))
            },
            ToolCall::ImageEdit { input, prompt, size, output } => {
                if let Model::Disabled | Model::Mock = config.agents.image_edit {
                    return Ok(Err(ToolCallError::ImageEditDisabled));
                }

                let (joined_input, real_input_path) = match check_read_path(input, &context.working_dir)? {
                    Ok((joined_path, real_path)) => (joined_path, real_path),
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };
                let (joined_output, real_output_path) = match check_write_path(output, &context.working_dir, None)? {
                    Ok((joined_path, real_path)) => (joined_path, real_path),
                    Err(e) => {
                        return Ok(Err(e));
                    },
                };

                let input_image_id = match read_bytes(&real_input_path) {
                    Ok(bytes) => match normalize_and_get_id(&bytes, &context.working_dir) {
                        Ok(id) => id,
                        Err(_) => {
                            return Ok(Err(ToolCallError::NotAnImage { path: joined_input }));
                        },
                    },
                    Err(_) => {
                        return Ok(Err(ToolCallError::NotAnImage { path: joined_input }));
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
                let generated_image = ::image::load_from_memory(&generated_image_bytes)?;

                write_bytes(
                    &real_output_path,
                    &generated_image_bytes,
                    ragit_fs::WriteMode::CreateOrTruncate,
                )?;

                let generated_image_id = normalize_and_get_id(&generated_image_bytes, &context.working_dir)?;
                Ok(Ok(ToolCallSuccess::ImageEdit {
                    input: joined_input,
                    prompt: prompt.to_string(),
                    output_path: joined_output,
                    output_image: generated_image_id,
                    requested_size: *size,
                    generated_size: (generated_image.width() as u64, generated_image.height() as u64),
                }))
            },
        }
    }

    pub fn preview(&self) -> String {
        match self {
            ToolCall::Read { path, start, end } => format!(
                "Read `{}`{}",
                path.join("/"),
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
                "{} {} bytes to `{}`",
                match mode {
                    WriteMode::Create => "Create and write",
                    WriteMode::Truncate => "Truncate and write",
                    WriteMode::Append => "Append",
                },
                content.len(),
                path.join("/"),
            ),
            ToolCall::Patch { path, diff, .. } => {
                let add = diff.iter().filter(
                    |LineDiff { kind, .. }| *kind == DiffKind::Add
                ).count();
                let remove = diff.iter().filter(
                    |LineDiff { kind, .. }| *kind == DiffKind::Remove
                ).count();
                format!(
                    "Patch `{}` (add {add} line{}, remove {remove} line{})",
                    path.join("/"),
                    if add == 1 { "" } else { "s" },
                    if remove == 1 { "" } else { "s" },
                )
            },
            ToolCall::Run { command, .. } => format!(
                "Run `{}`",
                join_command_args(command),
            ),
            ToolCall::Ask { to, question, .. } => format!(
                "Ask to {} {:?}",
                format!("{to:?}").to_ascii_lowercase(),
                truncate_chars(question, 42),
            ),
            ToolCall::Chrome { script, input, output } => format!(
                "Open chrome and render `{}` to `{}`{}",
                input.join("/"),
                output.join("/"),
                if let Some(script) = script {
                    format!(" (script: {})", truncate_chars(script, 42))
                } else {
                    String::new()
                },
            ),
            ToolCall::ImageEdit { input, prompt, .. } => format!(
                "Editing `{}`{}",
                input.join("/"),
                truncate_chars(prompt, 42),
            ),
        }
    }
}

fn truncate_chars(s: &str, count: usize) -> String {
    assert!(count > 3);

    if s.chars().count() < count {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(count - 3).collect::<String>())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ToolCallSuccess {
    ReadText {
        path: String,
        content: String,
        total_lines: u64,
        range: (Option<u64>, Option<u64>),
    },
    ReadPdf {
        path: String,
        pages: Vec<ImageId>,
        total_pages: u64,
        range: (Option<u64>, Option<u64>),
    },
    ReadImage {
        path: String,
        id: ImageId,
        size: (u64, u64),
    },
    ReadDir {
        path: String,
        entries: Vec<FileEntry>,
        total_entries: u64,
        range: (Option<u64>, Option<u64>),
    },
    ReadSymlink {
        path: String,
        pointee: String,
    },
    Write {
        path: String,
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
        path: String,
        diff: Vec<LineDiff>,
        new_content: String,
    },
    Run {
        path: Option<String>,
        command: Vec<String>,
        elapsed_ms: u64,
        exit_code: i32,
        stdout: DumpOrRedirect,
        stderr: DumpOrRedirect,
    },
    Ask {
        to: AskTo,
        answer: String,
    },
    Chrome {
        input: String,
        output_path: String,
        output_image: ImageId,
        script_output: Option<String>,
    },
    ImageEdit {
        input: String,
        prompt: String,
        output_path: String,
        output_image: ImageId,
        requested_size: Option<(u64, u64)>,
        generated_size: (u64, u64),
    },
}

impl ToolCallSuccess {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<LLMToken> {
        match self {
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
                    if path.ends_with("/") { "" } else { "/" },
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
            ToolCallSuccess::Run { path, command, elapsed_ms, exit_code, stdout, stderr } => {
                let path = if let Some(path) = path {
                    format!("\n<path>{path}</path>")
                } else {
                    String::new()
                };
                let (stdout, stdout_truncated) = match stdout {
                    DumpOrRedirect::Dump(stdout) => truncate_middle(stdout, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stdout) => (format!("Redirected to {}", stdout.join("/")), None),
                };
                let (stderr, stderr_truncated) = match stderr {
                    DumpOrRedirect::Dump(stderr) => truncate_middle(stderr, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stderr) => (format!("Redirected to {}", stderr.join("/")), None),
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
            ToolCallSuccess::Chrome { input, output_path, output_image, script_output } => vec![
                LLMToken::String(format!(
                    "Successfully opened `{input}`{}{}, captured a screenshot, and saved it to `{output_path}`.{}",
                    if input.ends_with(".svg") { "" } else { " with chrome" },
                    if script_output.is_some() { ", ran javascript" } else { "" },
                    if let Some(script_output) = script_output { format!("\nscript output: {script_output}") } else { String::new() },
                )),
                LLMToken::Image(*output_image),
            ],
            ToolCallSuccess::ImageEdit { input, output_path, output_image, .. } => vec![
                LLMToken::String(format!("Successfully edited `{input}` and saved the result at `{output_path}`.")),
                LLMToken::Image(*output_image),
            ],
        }
    }

    pub fn get_result_path(&self) -> Result<Option<(String, Option<String>)>, Error> {
        match self {
            ToolCallSuccess::ReadText { path, .. } |
            ToolCallSuccess::ReadPdf { path, .. } |
            ToolCallSuccess::ReadImage { path, .. } |
            ToolCallSuccess::Write { path, .. } |
            ToolCallSuccess::Chrome { input: path, .. } => Ok(Some((parent(path)?, Some(basename(path)?)))),
            ToolCallSuccess::ReadDir { path, .. } => Ok(Some((path.to_string(), None))),
            _ => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ToolCallError {
    // read errors
    NoSuchFile {
        path: String,
    },
    NoPermissionToRead {
        path: String,
    },
    InvalidRange {
        r#type: RangeType,
        length: u64,
        given: (Option<u64>, Option<u64>),
    },
    InvalidFileType {
        path: String,
    },
    TextTooLongToRead {
        path: String,
        length: u64,
        limit: u64,
    },
    TooManyTextLinesToRead {
        path: String,
        length: u64,
        limit: u64,
    },
    TooManyPdfPagesToRead {
        path: String,
        pages: u64,
        limit: u64,
        given_range: (Option<u64>, Option<u64>),
    },
    TooManyDirEntriesToRead {
        path: String,
        entries: u64,
        limit: u64,
        given_range: (Option<u64>, Option<u64>),
    },
    TooManyReadWithoutSummary,
    BrokenFile {
        path: String,
        kind: String,
        error: String,
    },
    ReadingExactSameFile { path: String },
    SymlinkWithRange {
        path: String,
        pointee: String,
        range: (Option<u64>, Option<u64>),
    },

    // write errors
    NoPermissionToWrite {
        path: String,
    },
    // If the given path is `docs/`, that's a directory whether or not that already exists.
    CannotWriteToDirectory {
        path: String,
        exists: bool,
    },
    CannotCreateParentDirectory {
        parent: String,
        file: String,
    },
    WriteModeError {
        path: String,
        mode: WriteMode,
        exists: bool,
    },
    TextTooLongToWrite {
        path: String,
        length: u64,
        limit: u64,
    },
    NoSummaryInDoneFile,

    // patch errors
    CannotPatchSymlink { path: String },
    CannotPatchNonExistFile { path: String },
    CannotPatchDir { path: String },
    CanOnlyPatchText {
        path: String,

        // This is `format!("{:?}", read_string(path).unwrap_err)`
        error: String,
    },
    CannotApplyPatch(PatchError),

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
        path: Option<String>,
        command: Vec<String>,
        timeout: u64,
        stdout: DumpOrRedirect,
        stderr: DumpOrRedirect,
    },

    // ask errors
    UserNotResponding,
    UserRejectedToRespond,
    WebSearchDisabled,

    // image-edit errors
    NotAnImage {
        path: String,
    },
    ImageRequestError {
        status_code: u16,
        message: String,
    },
    ImageEditDisabled,

    // etc
    SupposedToWriteSummary { write_path: Option<String> },
    UserInterrupt,
}

impl ToolCallError {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<LLMToken> {
        let s = match self {
            ToolCallError::NoSuchFile { path } => format!("There's no such file: `{path}`."),
            ToolCallError::NoPermissionToRead { path } => format!("You don't have a permission to read: `{path}`."),
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

            ToolCallError::NoPermissionToWrite { path } => format!("You don't have a permission to write to: `{path}`."),
            ToolCallError::CannotWriteToDirectory { path, exists } => if *exists {
                format!("You can't write to `{path}` because it already exists and is a directory.")
            } else {
                let mut path = path.to_string();

                if !path.ends_with("/") {
                    path = format!("{path}/");
                }

                format!("You can't create a directory with that tool. If you want to create a directory `{path}`, just create a file inside the directory. Then all the intermediate directories will be created.")
            },
            ToolCallError::CannotCreateParentDirectory { parent, file } => format!(
                "Tried to create parent directory of `{file}`, but it failed. `{parent}` already exists and is not a directory",
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
            ToolCallError::CannotApplyPatch(PatchError::NoMatch) => String::from("I can't apply the patch because no matches are found."),
            ToolCallError::CannotApplyPatch(PatchError::MultipleMatch) => String::from("I found multiple matches in the file that can apply your patch. Please give me more contexts so that I can decide where to patch."),
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
                    DumpOrRedirect::Redirect(stdout) => (format!("Redirected to {}", stdout.join("/")), None),
                };
                let (stderr, stderr_truncated) = match stderr {
                    DumpOrRedirect::Dump(stderr) => truncate_middle(stderr, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stderr) => (format!("Redirected to {}", stderr.join("/")), None),
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
            ToolCallError::WriteModeError { path, .. } => Ok(Some((parent(path)?, Some(basename(path)?)))),
            ToolCallError::TooManyDirEntriesToRead { path, .. } => Ok(Some((path.to_string(), None))),
            _ => Ok(None),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ToolKind {
    Read,
    Write,
    Patch,
    Run,
    Ask,
    Chrome,
    ImageEdit,
}

impl ToolKind {
    pub fn all() -> Vec<ToolKind> {
        vec![
            ToolKind::Read,
            ToolKind::Write,
            ToolKind::Patch,
            ToolKind::Run,
            ToolKind::Ask,
            ToolKind::Chrome,
            ToolKind::ImageEdit,
        ]
    }

    pub fn from_name(name: &[u8]) -> Option<ToolKind> {
        match name {
            b"read" => Some(ToolKind::Read),
            b"write" => Some(ToolKind::Write),
            b"patch" => Some(ToolKind::Patch),
            b"run" => Some(ToolKind::Run),
            b"ask" => Some(ToolKind::Ask),
            b"chrome" => Some(ToolKind::Chrome),
            b"image-edit" => Some(ToolKind::ImageEdit),
            _ => None,
        }
    }

    pub fn tag_name(&self) -> &'static str {
        match self {
            ToolKind::Read => "read",
            ToolKind::Write => "write",
            ToolKind::Patch => "patch",
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
            ToolKind::Read => vec!["path", "start", "end"],
            ToolKind::Write => vec!["path", "mode", "content"],
            ToolKind::Patch => vec!["path", "diff"],
            ToolKind::Run => vec!["timeout", "command", "path", "env", "stdout", "stderr"],
            ToolKind::Ask => vec!["to", "question"],
            ToolKind::Chrome => vec!["input", "output", "script"],
            ToolKind::ImageEdit => vec!["input", "prompt", "size", "output"],
        }.iter().map(|arg| arg.to_string()).collect()
    }

    pub fn optional(&self) -> bool {
        match self {
            ToolKind::Read => false,
            ToolKind::Write => false,
            ToolKind::Patch => true,
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
                            if is_summary_path(path) {
                                Ok(Ok(()))
                            } else {
                                Ok(Err(ToolCallError::SupposedToWriteSummary { write_path: Some(path.join("/")) }))
                            }
                        },
                        ToolCall::Ask { to: AskTo::User, .. } => Ok(Ok(())),
                        _ => Ok(Err(ToolCallError::SupposedToWriteSummary { write_path: None })),
                    }
                }

                // 2. It wrote `logs/done` but there's no summary in the file or it's too short.
                else if let ToolCall::Write { path, content, .. } = tool {
                    if is_done_file(path) && content.len() < 10 {
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
                        if let ToolCall::Read { path: last_path, start: None, end: None } = tool && normalize_path(path) == normalize_path(last_path) && normalize_path(path).is_some() {
                            return Ok(Err(ToolCallError::ReadingExactSameFile { path: normalize_path(path).unwrap().join("/") }));
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

fn is_summary_path(path: &Path) -> bool {
    match normalize_path(path) {
        Some(path) => match (path.get(0), path.get(1), path.get(2)) {
            (Some(logs), Some(summary), None) if logs == "logs" && (summary.starts_with("summary") && summary.ends_with(".md") || summary == "done") => true,
            _ => false,
        },
        None => false,
    }
}

fn is_done_file(path: &Path) -> bool {
    match normalize_path(path) {
        Some(path) if path.join("/") == "logs/done" => true,
        _ => false,
    }
}

// Normalization fails if it tries escape the working directory.
fn normalize_path(path: &Path) -> Option<Path> {
    let mut result = vec![];

    for segment in path.iter() {
        match segment.as_str() {
            "." => {},
            ".." => match result.pop() {
                Some(_) => {},
                None => { return None; },
            },
            s => {
                result.push(s.to_string());
            },
        }
    }

    // `a/b/` and `a/b` are the same.
    if let Some("") = result.last().map(|s| s.as_str()) {
        result.pop().unwrap();
    }

    // `/a/b` (abs_path) is not allowed
    if result.iter().any(|s| s == "") {
        return None;
    }

    Some(result)
}

fn join_command_args(args: &[String]) -> String {
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
