use async_std::task::sleep;
use crate::{
    Config,
    Context,
    Error,
    ImageId,
    LLMToken,
    LogEntry,
    UserResponse,
    clean_sandbox,
    export_to_sandbox,
    from_browser_error,
    image,
    import_from_sandbox,
    prettify_bytes,
    prettify_time,
    subprocess,
};
use headless_chrome::Browser;
use headless_chrome::browser::LaunchOptions as BrowserLaunchOptions;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use ragit_fs::{
    basename,
    create_dir_all,
    exists,
    into_abs_path,
    is_dir,
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
use std::time::{Duration, Instant};

mod ask;
mod read;
mod render;
mod run;
mod write;

pub use ask::{AskTo, ask_question_to_web};
pub use read::{
    FileEntry,
    RangeType,
    TypedFile,
    check_read_permission,
    read_file,
};
pub use render::WebOrFile;
pub use run::{check_python_venv, load_available_binaries};
pub use write::{DumpOrRedirect, WriteMode, check_write_permission};

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
    Run {
        timeout: Option<u64>,
        command: Vec<String>,
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
    Render {
        input: Path,
        output: Path,
        script: Option<String>,
    },
}

impl ToolCall {
    pub async fn run(&self, context: &mut Context, config: &Config) -> Result<Result<ToolCallSuccess, ToolCallError>, Error> {
        context.logger.log(LogEntry::ToolCallStart(self.clone()))?;

        match self {
            ToolCall::Read { path, start, end } => {
                if context.is_reading_too_much()? {
                    return Ok(Err(ToolCallError::TooManyReadWithoutWrite));
                }

                let joined_path = match normalize_path(path) {
                    Some(path) if path.is_empty() => String::from("."),
                    Some(path) => path.join("/"),
                    None => path.join("/"),
                };

                // If `join` fails, `check_read_permission` will catch this!
                let real_path = join(&context.working_dir, &joined_path).unwrap_or(joined_path.clone());

                // If the AI tries to read `../../Documents/`, that's a permission error whether or not the path exists.
                if !check_read_permission(path) {
                    return Ok(Err(ToolCallError::NoPermissionToRead { path: joined_path }));
                }

                if !exists(&real_path) {
                    return Ok(Err(ToolCallError::NoSuchFile { path: joined_path }));
                }

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
                    TypedFile::Image(id) => Ok(Ok(ToolCallSuccess::ReadImage { path: joined_path, id })),
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
                    TypedFile::Etc => Ok(Err(ToolCallError::InvalidFileType { path: joined_path })),
                }
            },
            ToolCall::Write { path, mode, content } => {
                let joined_path = match normalize_path(path) {
                    Some(path) if path.is_empty() => String::from("."),
                    Some(path) => path.join("/"),
                    None => path.join("/"),
                };

                // If `join` fails, `check_write_permission` will catch this!
                let real_path = join(&context.working_dir, &joined_path).unwrap_or(joined_path.clone());

                if content.len() as u64 > config.text_file_max_len {
                    return Ok(Err(ToolCallError::TextTooLongToWrite {
                        path: joined_path,
                        length: content.len() as u64,
                        limit: config.text_file_max_len,
                    }));
                }

                match (*mode, exists(&real_path)) {
                    (mode @ (WriteMode::Truncate | WriteMode::Append), _) if is_dir(&real_path) => {
                        return Ok(Err(ToolCallError::IsDir { path: joined_path, mode }));
                    },
                    (WriteMode::Create, false) |
                    (WriteMode::Truncate, true) |
                    (WriteMode::Append, true) => {},
                    (mode, exists) => {
                        return Ok(Err(ToolCallError::WriteModeError {
                            path: joined_path,
                            mode,
                            exists,
                        }));
                    },
                }

                let parent_path = parent(&real_path)?;

                if *mode == WriteMode::Create && !exists(&parent_path) {
                    create_dir_all(&parent_path)?;
                }

                if !check_write_permission(path) {
                    return Ok(Err(ToolCallError::NoPermissionToWrite { path: joined_path }));
                }

                let byte_count = content.len() as u64;
                let char_count = content.chars().count() as u64;
                let line_count = content.lines().count() as u64;
                let mut diff = None;

                if *mode == WriteMode::Truncate {
                    let prev_content = String::from_utf8_lossy(&read_bytes(&real_path)?).to_string();
                    diff = Some(unified_diff(
                        DiffAlgorithm::Patience,
                        &prev_content,
                        content,
                        5,
                        None,
                    ));
                }

                write_string(
                    &real_path,
                    content,
                    (*mode).into(),
                )?;

                Ok(Ok(ToolCallSuccess::Write {
                    path: joined_path,
                    content: content.to_string(),
                    diff,
                    mode: *mode,
                    bytes: byte_count,
                    chars: char_count,
                    lines: line_count,
                }))
            },
            ToolCall::Run { timeout, command, stdout, stderr } => {
                if let Some(stdout) = stdout && !check_write_permission(stdout) {
                    return Ok(Err(ToolCallError::NoPermissionToWrite {
                        path: stdout.join("/"),
                    }));
                }

                if let Some(stderr) = stderr && !check_write_permission(stderr) {
                    return Ok(Err(ToolCallError::NoPermissionToWrite {
                        path: stderr.join("/"),
                    }));
                }

                // If `command` is empty, that's a parse error.
                let binary = command[0].to_string();
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
                let bin_path = context.get_bin_path(&sandbox_at, &binary)?;
                let mut env: Vec<(&str, String)> = vec![];

                if bin_path == "python3" || bin_path == "pip" {
                    let venv_dir = join3(&context.working_dir, ".neukgu", "py-venv")?;
                    let venv_bin = join(&venv_dir, "bin")?;
                    env.push(("PATH", into_abs_path(&venv_bin)?));
                    env.push(("VIRTUAL_ENV", venv_dir));

                    check_python_venv(&env, &sandbox_at, &context.working_dir)?;
                }

                let started_at = Instant::now();
                let result = subprocess::run(
                    bin_path,
                    if command.len() > 1 { &command[1..] } else { &command[0..0] },
                    &env,
                    &sandbox_at,
                    timeout,
                    &context.working_dir,
                    true,
                )?;
                let elapsed_ms = Instant::now().duration_since(started_at).as_millis() as u64;
                import_from_sandbox(&sandbox_at, &context.working_dir, false /* copy_index_dir */)?;
                clean_sandbox(&config.sandbox_root, &sandbox_at, &context.working_dir)?;

                let timeout_value = timeout;
                let stdout_dst = stdout;
                let stderr_dst = stderr;
                let subprocess::Output { status, stdout, stderr, timeout } = result;

                let stdout = match stdout_dst {
                    Some(path) => {
                        write_bytes(
                            &join(&context.working_dir, &path.join("/"))?,
                            &stdout,
                            ragit_fs::WriteMode::CreateOrTruncate,
                        )?;
                        DumpOrRedirect::Redirect(path.to_vec())
                    },
                    None => DumpOrRedirect::Dump(String::from_utf8_lossy(&stdout).to_string()),
                };
                let stderr = match stderr_dst {
                    Some(path) => {
                        write_bytes(
                            &join(&context.working_dir, &path.join("/"))?,
                            &stderr,
                            if stdout_dst == stderr_dst {
                                ragit_fs::WriteMode::AlwaysAppend
                            } else {
                                ragit_fs::WriteMode::CreateOrTruncate
                            },
                        )?;
                        DumpOrRedirect::Redirect(path.to_vec())
                    },
                    None => DumpOrRedirect::Dump(String::from_utf8_lossy(&stderr).to_string()),
                };

                if timeout {
                    Ok(Err(ToolCallError::Timeout {
                        command: command.to_vec(),
                        timeout: timeout_value,
                        stdout,
                        stderr,
                    }))
                }

                else {
                    Ok(Ok(ToolCallSuccess::Run {
                        command: command.to_vec(),
                        elapsed_ms,
                        exit_code: status,
                        stdout,
                        stderr,
                    }))
                }
            },
            ToolCall::Ask { id, to: AskTo::User, question } => {
                context.ask_to_user(*id, question.to_string())?;

                let response;
                let tool_call_result = 'block: {
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
            ToolCall::Ask { id: _, to: AskTo::Web, question } => {
                let answer = ask_question_to_web(question, &context.working_dir, &mut context.logger, config.model).await?;
                Ok(Ok(ToolCallSuccess::Ask { to: AskTo::Web, answer }))
            },
            // TODO: error if `script` is set and `input` is not an html
            ToolCall::Render { script, input, output } => {
                let joined_output = output.join("/");
                let joined_input = input.join("/");

                if !check_write_permission(output) {
                    return Ok(Err(ToolCallError::NoPermissionToWrite { path: joined_output }));
                }

                let real_output_path = join(&context.working_dir, &joined_output)?;
                let real_input_path = join(&context.working_dir, &joined_input)?;

                let url = {
                    let path = WebOrFile::from(input);

                    if let WebOrFile::File(path) = &path {
                        if !check_read_permission(path) {
                            return Ok(Err(ToolCallError::NoPermissionToRead { path: joined_input }));
                        }

                        if !exists(&real_input_path) {
                            return Ok(Err(ToolCallError::NoSuchFile { path: joined_input }));
                        }

                        if joined_input.to_ascii_lowercase().ends_with(".svg") {
                            let svg_data = read_string(&real_input_path)?;

                            // VIBE NOTE: gemini 3.1 wrote this code (svg to png)
                            let opt = resvg::usvg::Options::default();
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
                            return Ok(Ok(ToolCallSuccess::Render { input: joined_input, output_path: joined_output, output_image: image_id, script_output: None }));
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

                // TODO: maybe we have to wait a few seconds until it loads?
                let png_data = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true).map_err(from_browser_error)?;
                let image_id = image::normalize_and_get_id(&png_data, &context.working_dir)?;
                write_bytes(&real_output_path, &png_data, ragit_fs::WriteMode::CreateOrTruncate)?;
                Ok(Ok(ToolCallSuccess::Render { input: joined_input, output_path: joined_output, output_image: image_id, script_output }))
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
            ToolCall::Run { command, .. } => format!(
                "Run `{}`",
                join_command_args(command),
            ),
            ToolCall::Ask { to, .. } => format!(
                "Ask to {}",
                format!("{to:?}").to_ascii_lowercase(),
            ),
            ToolCall::Render { script, input, output } => format!(
                "Render `{}` to `{}`{}",
                input.join("/"),
                output.join("/"),
                if let Some(script) = script { format!(" (script: {script})") } else { String::new() },
            ),
        }
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
    },
    ReadDir {
        path: String,
        entries: Vec<FileEntry>,
        total_entries: u64,
        range: (Option<u64>, Option<u64>),
    },
    Write {
        path: String,
        content: String,
        diff: Option<String>,
        mode: WriteMode,
        bytes: u64,
        chars: u64,
        lines: u64,
    },
    Run {
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
    Render {
        input: String,
        output_path: String,
        output_image: ImageId,
        script_output: Option<String>,
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
            ToolCallSuccess::ReadImage { id, .. } => vec![LLMToken::Image(*id)],
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
                    }
                ).collect::<Vec<_>>().join("\n");
                let s = format!(
                    "{path}{}\n{entries_count}\n\n{entries}",
                    if path.ends_with("/") { "" } else { "/" },
                );
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::Write { path, bytes, lines, .. } => {
                let s = format!("{} ({lines} lines) of text was successfully written to `{path}`.", prettify_bytes(*bytes));
                vec![LLMToken::String(s)]
            },
            ToolCallSuccess::Run { command, elapsed_ms, exit_code, stdout, stderr } => {
                let stdout = match stdout {
                    DumpOrRedirect::Dump(stdout) => truncate_middle(stdout, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stdout) => format!("Redirected to {}", stdout.join("/")),
                };
                let stderr = match stderr {
                    DumpOrRedirect::Dump(stderr) => truncate_middle(stderr, config.stdout_max_len),
                    DumpOrRedirect::Redirect(stderr) => format!("Redirected to {}", stderr.join("/")),
                };

                let s = format!(
"
<run_result>
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
            ToolCallSuccess::Render { input, output_path, output_image, script_output } => vec![
                LLMToken::String(format!(
                    "Successfully opened `{input}`{}{}, captured a screenshot, and saved it to `{output_path}`.{}",
                    if input.ends_with(".svg") { "" } else { " with chrome" },
                    if script_output.is_some() { ", ran javascript" } else { "" },
                    if let Some(script_output) = script_output { format!("\nscript output: {script_output}") } else { String::new() },
                )),
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
            ToolCallSuccess::Render { input: path, .. } => Ok(Some((parent(path)?, Some(basename(path)?)))),
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
    TooManyReadWithoutWrite,
    BrokenFile {
        path: String,
        kind: String,
        error: String,
    },

    // write errors
    NoPermissionToWrite {
        path: String,
    },
    IsDir {
        path: String,
        mode: WriteMode,
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
        command: Vec<String>,
        timeout: u64,
        stdout: DumpOrRedirect,
        stderr: DumpOrRedirect,
    },

    // ask errors
    UserNotResponding,
    UserRejectedToRespond,

    // etc
    UserInterrupt,
}

impl ToolCallError {
    pub fn to_llm_tokens(&self) -> Vec<LLMToken> {
        match self {
            ToolCallError::NoSuchFile { path } => vec![
                LLMToken::String(format!("There's no such file: `{path}`.")),
            ],
            ToolCallError::TextTooLongToRead { path, length, limit } => vec![
                LLMToken::String(format!(
                    "The file `{path}` is too long to read at once. The file is {}, and the environment won't allow you to open a file that is larger than {}. You can read the first 200 lines with <end>200</end>, or use search tools like ripgrep.",
                    prettify_bytes(*length),
                    prettify_bytes(*limit),
                )),
            ],
            ToolCallError::TooManyTextLinesToRead { length, limit, .. } => vec![
                LLMToken::String(format!(
                    "You're trying to read too many lines at once. You're trying to read {length} lines, and the environment won't allow you to read more than {limit} lines at once.",
                )),
            ],
            ToolCallError::TooManyDirEntriesToRead { path, entries, limit: _, given_range: (None, None) } => vec![
                LLMToken::String(format!("`{path}/` is gigantic, it has {entries} entries. Please specify range with <start> and <end>. For example, you can read the first 100 entries with <end>100</end>.")),
            ],
            ToolCallError::TooManyDirEntriesToRead { limit, .. } => vec![
                LLMToken::String(format!("You can read at most `{limit}` entries at once. Please give me a smaller range.")),
            ],
            ToolCallError::TooManyReadWithoutWrite => vec![
                LLMToken::String(String::from("You're keep reading files without updating logs. Please update the log files with what you've learnt, then continue reading this.")),
            ],
            ToolCallError::BrokenFile { path, kind, error } => vec![
                LLMToken::String(format!("Tried to read {path}, but failed to parse the {kind} file.\nerror: {error}")),
            ],
            ToolCallError::WriteModeError { path, mode, exists } => {
                let s = match (mode, exists) {
                    (WriteMode::Create, true) => format!("You can't create `{path}` because it already exists. Try with \"truncate\" or \"append\"."),
                    (WriteMode::Truncate, false) => format!("You can't truncate `{path}` because it does not exist. Try with \"create\"."),
                    (WriteMode::Append, false) => format!("You can't append to `{path}` because it does not exist. Try with \"create\"."),
                    _ => unreachable!(),
                };
                vec![LLMToken::String(s)]
            },
            ToolCallError::TextTooLongToWrite { path, length, limit } => vec![
                LLMToken::String(format!(
                    "Failed to write to the file.\nYou attempted to write too long contents to `{path}`. The environment allows up to {} write at once, but your content is {}.\nYou can try to make it shorter, or split it into multiple files.",
                    prettify_bytes(*limit),
                    prettify_bytes(*length),
                )),
            ],
            ToolCallError::NoSuchBinary { binary, available_binaries } => vec![
                LLMToken::String(format!(
                    "There's no such binary: `{binary}`.\nAvailable binaries are: {}.{}",
                    available_binaries.join(", "),
                    if binary.contains("/") {
                        "\nDon't call the binary with its path. Run it with just the name."
                    } else {
                        ""
                    },
                )),
            ],
            ToolCallError::UserNotResponding => vec![
                LLMToken::String(format!(
                    "User is not responding.",
                )),
            ],
            ToolCallError::UserRejectedToRespond => vec![
                LLMToken::String(format!(
                    "User doesn't want to answer your question.",
                )),
            ],
            ToolCallError::UserInterrupt => vec![
                LLMToken::String(format!(
                    "(This turn is supposed to be removed by `context.discard_previous_turn()`. If you, the human user, see this message in GUI or if you, an AI agent, see this message in the context, there's a bug in the harness.)",
                )),
            ],
            _ => panic!("TODO: {self:?}"),
        }
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ToolKind {
    Read,
    Write,
    Run,
    Ask,
    Render,
}

impl ToolKind {
    pub fn all() -> Vec<ToolKind> {
        vec![
            ToolKind::Read,
            ToolKind::Write,
            ToolKind::Run,
            ToolKind::Ask,
            ToolKind::Render,
        ]
    }

    pub fn check_arg_name(&self, arg: &[u8]) -> bool {
        match (self, arg) {
            (ToolKind::Read, b"path" | b"start" | b"end") => true,
            (ToolKind::Read, _) => false,
            (ToolKind::Write, b"path" | b"mode" | b"content") => true,
            (ToolKind::Write, _) => false,
            (ToolKind::Run, b"timeout" | b"command" | b"stdout" | b"stderr") => true,
            (ToolKind::Run, _) => false,
            (ToolKind::Ask, b"to" | b"question") => true,
            (ToolKind::Ask, _) => false,
            (ToolKind::Render, b"input" | b"output" | b"script") => true,
            (ToolKind::Render, _) => false,
        }
    }

    pub fn valid_args(&self) -> Vec<String> {
        match self {
            ToolKind::Read => vec!["path", "start", "end"],
            ToolKind::Write => vec!["path", "mode", "content"],
            ToolKind::Run => vec!["timeout", "command", "stdout", "stderr"],
            ToolKind::Ask => vec!["to", "question"],
            ToolKind::Render => vec!["input", "output", "script"],
        }.iter().map(|arg| arg.to_string()).collect()
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

fn truncate_middle(s: &str, limit: u64) -> String {
    if (s.len() as u64) < limit {
        s.to_string()
    } else {
        let d = limit * 2 / 5;
        let pre = String::from_utf8_lossy(&s.as_bytes()[..(d as usize)]).to_string();
        let post = String::from_utf8_lossy(&s.as_bytes()[(s.len() - d as usize)..]).to_string();
        format!("{pre}...({} bytes truncated)...{post}", s.len() as u64 - d * 2)
    }
}
