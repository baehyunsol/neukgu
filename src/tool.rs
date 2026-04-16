use crate::{
    Config,
    Context,
    Error,
    ImageId,
    LogEntry,
    StringOrImage,
    export_to_sandbox,
    from_browser_error,
    import_from_sandbox,
    normalize_and_get_id,
    subprocess,
};
use headless_chrome::Browser;
use headless_chrome::browser::LaunchOptions as BrowserLaunchOptions;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use ragit_fs::{
    basename,
    create_dir_all,
    exists,
    parent,
    read_bytes,
    read_dir,
    read_string,
    remove_dir_all,
    write_bytes,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

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
pub use run::load_available_binaries;
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

                // If the AI tries to read `../../Documents/`, that's a permission error whether or not the path exists.
                if !check_read_permission(path) {
                    return Ok(Err(ToolCallError::NoPermissionToRead { path: joined_path }));
                }

                if !exists(&joined_path) {
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
                            Ok(Err(ToolCallError::TooManyLinesToRead {
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
                    TypedFile::Image(id) => Ok(Ok(ToolCallSuccess::ReadImage { path: joined_path, id })),
                    TypedFile::Dir(entries) => {
                        if entries.len() as u64 > config.dir_max_entries && start.is_none() && end.is_none() {
                            Ok(Err(ToolCallError::TooManyEntriesToRead {
                                path: joined_path,
                                entries: entries.len() as u64,
                                given_range: (*start, *end),
                            }))
                        }

                        else if let Some(end) = end && start_i + config.dir_max_entries < *end {
                            Ok(Err(ToolCallError::TooManyEntriesToRead {
                                path: joined_path,
                                entries: entries.len() as u64,
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
                let joined_path = path.join("/");

                if content.len() as u64 > config.text_file_max_len {
                    return Ok(Err(ToolCallError::TextTooLongToWrite {
                        path: joined_path,
                        length: content.len() as u64,
                        limit: config.text_file_max_len,
                    }));
                }

                match (*mode, exists(&joined_path)) {
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

                let parent_path = parent(&joined_path)?;

                if *mode == WriteMode::Create && !exists(&parent_path) {
                    create_dir_all(&parent_path)?;
                }

                if !check_write_permission(path) {
                    return Ok(Err(ToolCallError::NoPermissionToWrite { path: joined_path }));
                }

                let byte_count = content.len() as u64;
                let char_count = content.chars().count() as u64;
                let line_count = content.lines().count() as u64;
                write_string(
                    &joined_path,
                    content,
                    (*mode).into(),
                )?;

                Ok(Ok(ToolCallSuccess::Write {
                    path: joined_path,
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

                for bin in read_dir("bins", false)?.iter() {
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

                let sandbox_at = export_to_sandbox(&config.sandbox_root)?;
                let bin_path = context.get_bin_path(&sandbox_at, &binary)?;
                let started_at = Instant::now();
                let result = subprocess::run(
                    bin_path,
                    if command.len() > 1 { &command[1..] } else { &command[0..0] },
                    &sandbox_at,
                    timeout,
                )?;
                let elapsed_ms = Instant::now().duration_since(started_at).as_millis() as u64;
                import_from_sandbox(&sandbox_at, false /* copy_index_dir */)?;
                remove_dir_all(&sandbox_at)?;

                let timeout_value = timeout;
                let stdout_dst = stdout;
                let stderr_dst = stderr;
                let subprocess::Output { status, stdout, stderr, timeout } = result;

                let stdout = match stdout_dst {
                    Some(path) => {
                        write_bytes(
                            &path.join("/"),
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
                            &path.join("/"),
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
                if let Some(request) = &context.user_request {
                    todo!()
                }

                else {
                    todo!()
                }
            },
            ToolCall::Ask { id: _, to: AskTo::Web, question } => {
                let answer = ask_question_to_web(question, &mut context.logger).await?;
                Ok(Ok(ToolCallSuccess::Ask { to: AskTo::Web, answer }))
            },
            ToolCall::Render { input, output } => {
                let joined_output = output.join("/");
                let joined_input = input.join("/");

                if !check_write_permission(output) {
                    return Ok(Err(ToolCallError::NoPermissionToWrite { path: joined_output }));
                }

                let url = {
                    let path = WebOrFile::from(input);

                    if let WebOrFile::File(path) = &path {
                        if !check_read_permission(path) {
                            return Ok(Err(ToolCallError::NoPermissionToRead { path: joined_input }));
                        }

                        if !exists(&joined_input) {
                            return Ok(Err(ToolCallError::NoSuchFile { path: joined_input }));
                        }

                        if joined_input.to_ascii_lowercase().ends_with(".svg") {
                            let svg_data = read_string(&joined_input)?;

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
                            pixmap.save_png(&joined_output)?;
                            // the VIBE ends here

                            let png_data = read_bytes(&joined_output)?;
                            let image_id = normalize_and_get_id(&png_data)?;
                            return Ok(Ok(ToolCallSuccess::Render { input: joined_input, output_path: joined_output, output_image: image_id }));
                        }
                    }

                    path.to_url()?
                };

                let browser = Browser::new(BrowserLaunchOptions {
                    window_size: Some((1920, 1080)),
                    ..BrowserLaunchOptions::default()
                }).map_err(from_browser_error)?;
                let tab = browser.new_tab().map_err(from_browser_error)?;
                tab.navigate_to(&url).map_err(from_browser_error)?;

                // TODO: maybe we have to wait a few seconds until it loads?
                let png_data = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true).map_err(from_browser_error)?;
                let image_id = normalize_and_get_id(&png_data)?;
                write_bytes(&joined_output, &png_data, ragit_fs::WriteMode::CreateOrTruncate)?;
                Ok(Ok(ToolCallSuccess::Render { input: url, output_path: joined_output, output_image: image_id }))
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
            ToolCall::Render { input, output } => format!(
                "Render `{}` to `{}`",
                input.join("/"),
                output.join("/"),
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
    },
}

impl ToolCallSuccess {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<StringOrImage> {
        match self {
            ToolCallSuccess::ReadText { content, .. } => {
                if content.is_empty() {
                    // claude requires every turn to be non-empty
                    vec![StringOrImage::String(String::from("(This file is empty.)"))]
                }

                else {
                    vec![StringOrImage::String(content.to_string())]
                }
            },
            ToolCallSuccess::ReadImage { id, .. } => vec![StringOrImage::Image(*id)],
            ToolCallSuccess::ReadDir { path, entries, total_entries, range } => {
                let entries_count = match range {
                    (None, None) => format!("{total_entries} entries"),
                    (None | Some(0), Some(e)) => format!("{total_entries} entries (seeing the first {} entries)", e + 1),
                    (Some(s), None) => format!("{total_entries} entries (seeing the last {} entries)", total_entries - s),
                    _ => todo!(),
                };
                let entries = entries.iter().map(
                    |entry| match entry {
                        FileEntry::TextFile { name, bytes, lines, .. } => format!("{name}\ttext\t{}\t{lines} lines", prettify_bytes(*bytes)),
                        FileEntry::ImageFile { name, bytes } => format!("{name}\timage\t{}", prettify_bytes(*bytes)),
                        FileEntry::EtcFile { name, bytes } => format!("{name}\tetc\t{}", prettify_bytes(*bytes)),
                        FileEntry::Dir { name } => format!("{name}/\tdirectory"),
                    }
                ).collect::<Vec<_>>().join("\n");
                let s = format!(
                    "{path}{}\n{entries_count}\n\n{entries}",
                    if path.ends_with("/") { "" } else { "/" },
                );
                vec![StringOrImage::String(s)]
            },
            ToolCallSuccess::Write { path, bytes, lines, .. } => {
                let s = format!("{} ({lines} lines) of text was successfully written to `{path}`.", prettify_bytes(*bytes));
                vec![StringOrImage::String(s)]
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
                vec![StringOrImage::String(s)]
            },
            ToolCallSuccess::Ask { answer, .. } => vec![StringOrImage::String(answer.to_string())],
            ToolCallSuccess::Render { input, output_path, output_image } => vec![
                StringOrImage::String(format!("Successfully opened `{input}` with chrome, captured a screenshot, and saved it to `{output_path}`.")),
                StringOrImage::Image(*output_image),
            ],
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
    TooManyLinesToRead {
        path: String,
        length: u64,
        limit: u64,
    },
    TooManyEntriesToRead {
        path: String,
        entries: u64,
        given_range: (Option<u64>, Option<u64>),
    },
    TooManyReadWithoutWrite,

    // write errors
    NoPermissionToWrite {
        path: String,
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
}

impl ToolCallError {
    pub fn to_llm_tokens(&self) -> Vec<StringOrImage> {
        match self {
            ToolCallError::NoSuchFile { path } => vec![
                StringOrImage::String(format!("There's no such file: `{path}`.")),
            ],
            ToolCallError::TextTooLongToRead { path, length, limit } => vec![
                StringOrImage::String(format!(
                    "The file `{path}` is too long to read at once. The file is {}, and the environment won't allow you to open a file that is larger than {}. You can read the first 200 lines with <end>200</end>, or use search tools like ripgrep.",
                    prettify_bytes(*length),
                    prettify_bytes(*limit),
                )),
            ],
            ToolCallError::TooManyReadWithoutWrite => vec![
                StringOrImage::String(String::from("You're keep reading files without updating logs. Please update the log files with what you've learnt, then continue reading this.")),
            ],
            ToolCallError::WriteModeError { path, mode, exists } => {
                let s = match (mode, exists) {
                    (WriteMode::Create, true) => format!("You can't create `{path}` because it already exists. Try with \"truncate\" or \"append\"."),
                    (WriteMode::Truncate, false) => format!("You can't truncate `{path}` because it does not exist. Try with \"create\"."),
                    (WriteMode::Append, false) => format!("You can't append to `{path}` because it does not exist. Try with \"create\"."),
                    _ => unreachable!(),
                };
                vec![StringOrImage::String(s)]
            },
            ToolCallError::NoSuchBinary { binary, available_binaries } => vec![
                StringOrImage::String(format!(
                    "There's no such binary: `{binary}`.\nAvailable binaries are: {}.{}",
                    available_binaries.join(", "),
                    if binary.contains("/") {
                        "\nDon't call the binary with its path. Run it with just the name."
                    } else {
                        ""
                    },
                )),
            ],
            _ => panic!("TODO: {self:?}"),
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
    pub fn check_arg_name(&self, arg: &[u8]) -> bool {
        match (self, arg) {
            (ToolKind::Read, b"path" | b"start" | b"end") => true,
            (ToolKind::Write, b"path" | b"mode" | b"content") => true,
            (ToolKind::Run, b"timeout" | b"command" | b"stdout" | b"stderr") => true,
            (ToolKind::Ask, b"to" | b"question") => true,
            (ToolKind::Render, b"input" | b"output") => true,
            _ => false,
        }
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

    Some(result)
}

pub fn prettify_bytes(b: u64) -> String {
    match b {
        0..=19_999 => format!("{b} bytes"),
        20_000..=19_999_999 => format!("{} KiB", b >> 10),
        20_000_000..=19_999_999_999 => format!("{} MiB", b >> 20),
        _ => format!("{} GiB", b >> 30),
    }
}

pub fn prettify_time(ms: u64) -> String {
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    if seconds < 60 {
        format!("{:.2} seconds", ms as f64 / 1000.0)
    } else if hours < 2 {
        format!("{minutes} minutes {} seconds", seconds % 60)
    } else {
        format!("{hours} hours {} minutes", minutes % 60)
    }
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
