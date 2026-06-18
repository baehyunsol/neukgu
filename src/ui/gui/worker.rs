use super::browser::FilePreview;
use super::git::{GitInfo, GitOperation, get_git_info, git_operation};
use super::tab::TabId;
use base64::Engine;
use crate::{
    ChatId,
    ChatInput,
    Error,
    Hunk,
    LLMToken,
    MatchPreview,
    add_chat_turn,
    find_pattern_in_chats,
    get_global_index_dir,
    normalize_image,
    render_first_few_pages_of_pdf,
    synchronize_skills_config,
};
use crate::subprocess::{self, Output};
use crate::tool::parse_command;
use globset::{GlobBuilder, GlobMatcher};
use ragit_fs::{
    exists,
    file_size,
    is_dir,
    is_symlink,
    join,
    read_bytes,
    read_bytes_offset,
    read_dir,
    read_string,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct JobId(u64);

impl JobId {
    pub fn new() -> JobId {
        JobId(rand::random::<u64>())
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
}

#[derive(Clone, Debug)]
pub enum JobKind {
    Rg {
        path: String,
        regex: String,
    },
    Glob {
        path: String,
        pattern: String,
    },
    Run {
        path: String,
        command: String,
    },
    AddChatTurn {
        chat_id: ChatId,
        api_keys: HashMap<String, String>,
        input: ChatInput,
    },
    FindInChats {
        regex: String,
    },
    CalcDirectorySize {
        path: String,
    },
    GetFilePreview {
        path: String,
    },
    GetGitInfo {
        path: String,
    },
    GitOperation {
        operation: GitOperation,
        file_a: Option<String>,
        file_b: Option<String>,
        hunk: Hunk,
    },
    SynchronizeSkillsConfig,
}

#[derive(Clone, Debug)]
pub struct JobResult {
    pub id: Option<JobId>,
    pub kind: JobResultKind,
}

#[derive(Clone, Debug)]
pub enum JobResultKind {
    Rg { regex: String, matches: Vec<RgMatch>, count: usize },
    RgTimeout,
    RgError(String),
    Glob { pattern: String, matches: Vec<GlobMatch>, timeout: bool },
    GlobError(String),
    Run(Output),
    RunError(String),
    InvalidCommand { error: String },
    AddChatTurnSuccess,
    AddChatTurnError(String),
    CannotAttachFileToChat { file: String, error: String },
    FindInChats { regex: String, matches: Vec<(ChatId, Vec<MatchPreview>)> },
    FindInChatsError(String),
    CalcDirectorySize(u64),
    CalcDirectorySizeError(String),
    GetFilePreview { path: String, preview: FilePreview },
    GetGitInfo(GitInfo),
    GetGitInfoError(String),
    GitOperationSuccess(GitOperation),
    GitOperationFail { operation: GitOperation, error: String },
    WorkerError(String),
}

pub struct Worker {
    pub tx_from_main: mpsc::Sender<Job>,
    pub rx_to_main: mpsc::Receiver<JobResult>,
}

impl Worker {
    pub fn send(&self, msg: Job) -> Result<(), mpsc::SendError<Job>> {
        self.tx_from_main.send(msg)
    }

    pub fn try_recv(&self) -> Result<JobResult, mpsc::TryRecvError> {
        self.rx_to_main.try_recv()
    }
}

pub struct Workers {
    pub round_robin: usize,
    pub tab_id_by_job_id: HashMap<JobId, Option<TabId>>,
    pub workers: Vec<Worker>,
}

impl Workers {
    pub fn push(&mut self, tab_id: Option<TabId>, job: Job) -> Result<(), mpsc::SendError<Job>> {
        self.round_robin += 1;
        self.tab_id_by_job_id.insert(job.id, tab_id);
        self.workers[self.round_robin % self.workers.len()].send(job)
    }

    pub fn poll(&mut self) -> Vec<(JobResult, Option<TabId>)> {
        let mut result = vec![];
        let mut disconnected_workers = vec![];

        for (index, worker) in self.workers.iter().enumerate() {
            match worker.try_recv() {
                Ok(msg) => {
                    result.push(msg);
                },
                Err(mpsc::TryRecvError::Empty) => {},
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected_workers.push(index);
                },
            }
        }

        for index in disconnected_workers.into_iter() {
            self.workers[index] = init_worker();
        }

        let result = result.into_iter().map(
            |msg| {
                let tab_id = match msg.id {
                    Some(id) => self.tab_id_by_job_id.remove(&id).unwrap(),
                    None => None,
                };
                (msg, tab_id)
            }
        ).collect();
        result
    }
}

fn event_loop(tx_to_main: mpsc::Sender<JobResult>, rx_from_main: mpsc::Receiver<Job>) -> Result<(), Error> {
    for msg in rx_from_main {
        match msg {
            Job { id, kind: JobKind::Rg { path, regex } } => {
                let rg_result = subprocess::run(
                    String::from("rg"),
                    &[regex.to_string(), String::from("--json"), String::from("--context=5")],
                    false,
                    &[],
                    &path,
                    20,  // timeout
                    "",     // working_dir (it's None because it's not a neugku dir)
                    false,  // check_interruption
                )?;

                tx_to_main.send(JobResult { id: Some(id), kind: parse_rg_output(regex, rg_result) }).unwrap();
            },
            Job { id, kind: JobKind::Glob { path, pattern } } => {
                match GlobBuilder::new(&pattern).literal_separator(true).build() {
                    Ok(glob) => {
                        let matcher = glob.compile_matcher();
                        tx_to_main.send(JobResult { id: Some(id), kind: match_glob(&path, &matcher, 20 /* timeout */) }).unwrap();
                    },
                    Err(e) => {
                        tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::GlobError(e.to_string()) }).unwrap();
                    },
                }
            },
            Job { id, kind: JobKind::Run { path, command } } => match parse_command(&command) {
                Ok(command) => match subprocess::run(
                    command[0].to_string(),
                    &command[1..],
                    false,
                    &[],
                    &path,
                    600,    // timeout
                    "",     // working_dir (it's None because it's not a neugku dir)
                    false,  // check_interruption
                ) {
                    Ok(output) => {
                        tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::Run(output) }).unwrap();
                    },
                    Err(e) => {
                        tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::RunError(format!("{e:?}")) }).unwrap();
                    },
                },
                Err(e) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::InvalidCommand { error: format!("{e:?}") } }).unwrap();
                },
            },
            Job { id, kind: JobKind::AddChatTurn { chat_id, api_keys, input } } => match get_global_index_dir() {
                Ok(global_index_dir) => match input.into_query(chat_id, &global_index_dir) {
                    Ok(query) => match add_chat_turn_blocked(chat_id, api_keys, input, query, &global_index_dir) {
                        Ok(()) => {
                            tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::AddChatTurnSuccess }).unwrap();
                        },
                        Err(e) => {
                            tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::AddChatTurnError(format!("{e:?}")) }).unwrap();
                        },
                    },
                    Err((file, e)) => {
                        tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::CannotAttachFileToChat { file, error: format!("{e:?}") } }).unwrap();
                    },
                },
                Err(e) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::AddChatTurnError(format!("{e:?}")) }).unwrap();
                },
            },
            Job { id, kind: JobKind::FindInChats { regex } } => match find_pattern_in_chats(&regex) {
                Ok(matches) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::FindInChats { regex, matches } }).unwrap();
                },
                Err(e) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::FindInChatsError(format!("{e:?}")) }).unwrap();
                },
            },
            Job { id, kind: JobKind::CalcDirectorySize { path } } => match calc_directory_size(&path) {
                Ok(s) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::CalcDirectorySize(s) }).unwrap();
                },
                Err(e) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::CalcDirectorySizeError(format!("{e:?}")) }).unwrap();
                },
            },
            Job { id, kind: JobKind::GetFilePreview { path } } => {
                if let Ok(preview) = get_file_preview(&path) {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::GetFilePreview { path, preview } }).unwrap();
                }
            },
            Job { id, kind: JobKind::GetGitInfo { path } } => match get_git_info(&path) {
                Ok(i) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::GetGitInfo(i) }).unwrap();
                },
                Err(e) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::GetGitInfoError(format!("{e:?}")) }).unwrap();
                },
            },
            Job { id, kind: JobKind::GitOperation { operation, file_a, file_b, hunk } } => match git_operation(operation, file_a, file_b, hunk) {
                Ok(()) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::GitOperationSuccess(operation) }).unwrap();
                },
                Err(e) => {
                    tx_to_main.send(JobResult { id: Some(id), kind: JobResultKind::GitOperationFail { operation, error: format!("{e:?}") } }).unwrap();
                },
            },
            Job { id: _, kind: JobKind::SynchronizeSkillsConfig } => match get_global_index_dir() {
                Ok(global_index_dir) => {
                    if let Err(e) = synchronize_skills_config(&global_index_dir) {
                        eprintln!("Failed to update skills config: {e:?}");
                    }
                },
                Err(e) => {
                    eprintln!("Failed to get global index dir: {e:?}");
                },
            },
        }
    }

    Ok(())
}

fn init_worker() -> Worker {
    let (tx_to_main, rx_to_main) = mpsc::channel();
    let (tx_from_main, rx_from_main) = mpsc::channel();

    thread::spawn(move || match event_loop(tx_to_main.clone(), rx_from_main) {
        Ok(()) => {},
        Err(e) => {
            tx_to_main.send(JobResult { id: None, kind: JobResultKind::WorkerError(format!("{e:?}")) }).unwrap();
        },
    });

    Worker { rx_to_main, tx_from_main }
}

pub fn init_workers(count: usize) -> Workers {
    Workers {
        round_robin: 0,
        tab_id_by_job_id: HashMap::new(),
        workers: (0..count).map(|_| init_worker()).collect(),
    }
}

#[derive(Clone, Debug)]
pub struct RgMatch {
    pub path: String,
    pub line: String,
    pub line_number: usize,
    pub submatches: Vec<(usize, usize)>,
}

fn parse_rg_output(regex: String, output: Output) -> JobResultKind {
    #[derive(Debug, Deserialize)]
    struct RgMatchLine {
        // r#type: String,
        data: RgMatchData,
    }

    #[derive(Debug, Deserialize)]
    struct RgMatchData {
        path: TextOrBytes,
        lines: TextOrBytes,
        line_number: usize,
        // absolute_offset: usize,
        submatches: Vec<Submatch>,
    }

    #[derive(Debug, Deserialize)]
    struct Submatch {
        // r#match: TextOrBytes,
        start: usize,
        end: usize,
    }

    #[derive(Debug, Deserialize)]
    struct TextOrBytes {
        text: Option<String>,
        bytes: Option<String>,
    }

    impl TextOrBytes {
        pub fn to_string(&self) -> String {
            if let Some(s) = &self.text {
                s.to_string()
            } else if let Some(s) = &self.bytes {
                let bytes = base64::prelude::BASE64_STANDARD.decode(s).unwrap();
                String::from_utf8_lossy(&bytes).to_string()
            } else {
                todo!()
            }
        }
    }

    if output.timeout {
        return JobResultKind::RgTimeout;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("error") {
        return JobResultKind::RgError(stderr.to_string());
    }

    let mut matches = vec![];
    let mut count = 0;

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        match serde_json::from_str::<RgMatchLine>(line) {
            Ok(line) => {
                let rg_match = RgMatch {
                    path: line.data.path.to_string(),
                    line: line.data.lines.to_string(),
                    line_number: line.data.line_number,
                    submatches: line.data.submatches.iter().map(
                        |submatch| (submatch.start, submatch.end)
                    ).collect(),
                };
                count += rg_match.submatches.len();
                matches.push(rg_match);
            },
            Err(_) => {
                // eprintln!("{line}");
            },
        }
    }

    JobResultKind::Rg { regex, matches, count }
}

#[derive(Clone, Debug)]
pub struct GlobMatch {
    pub path: String,

    // These are necessary for gui tabs.
    pub is_symlink: bool,
    pub is_dir: bool,
}

fn match_glob(path: &str, matcher: &GlobMatcher, timeout: u64) -> JobResultKind {
    if !exists(path) || !is_dir(path) {
        return JobResultKind::GlobError(format!("{:?}", read_dir(path, false).unwrap_err()));
    }

    let mut buffer = vec![];
    let timeout = match_glob_worker(path, matcher, Instant::now(), timeout, &mut buffer);
    JobResultKind::Glob { pattern: matcher.glob().glob().to_string(), matches: buffer, timeout }
}

fn match_glob_worker(path: &str, matcher: &GlobMatcher, started_at: Instant, timeout: u64, buffer: &mut Vec<GlobMatch>) -> bool {
    if Instant::now().duration_since(started_at.clone()).as_secs() > timeout {
        return true;
    }

    for entry in read_dir(path, true).unwrap_or(vec![]) {
        let e_is_dir = is_dir(&entry);
        let e_is_symlink = is_symlink(&entry);

        // It doesn't follow symlinks otherwise it might loop infinitely.
        if !e_is_symlink && e_is_dir {
            let timeout = match_glob_worker(&entry, matcher, started_at, timeout, buffer);

            if timeout {
                return true;
            }
        }

        if matcher.is_match(&entry) {
            buffer.push(GlobMatch {
                path: entry,
                is_symlink: e_is_symlink,
                is_dir: e_is_dir,
            });
        }
    }

    false
}

fn add_chat_turn_blocked(
    chat_id: ChatId,
    api_keys: HashMap<String, String>,
    input: ChatInput,
    query: Vec<LLMToken>,
    global_index_dir: &str,
) -> Result<(), Error> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(add_chat_turn(chat_id, api_keys, input, query, global_index_dir))
}

fn calc_directory_size(path: &str) -> Result<u64, Error> {
    let mut result: u64 = 0;

    for entry in read_dir(path, false)? {
        if is_symlink(&entry) {
            continue;
        }

        else if is_dir(&entry) {
            result += calc_directory_size(&entry)?;
        }

        else {
            result += file_size(&entry)?;
        }
    }

    Ok(result)
}

fn get_file_preview(path: &str) -> Result<FilePreview, Error> {
    if is_dir(path) {
        return Ok(FilePreview::None);
    }

    let thumbnail_dir = join(
        &get_global_index_dir()?,
        "thumbnails",
    )?;
    let file_size = file_size(path)?;

    if file_size <= 512 {
        if let Ok(s) = read_string(path) {
            return Ok(FilePreview::Text { preview: s, truncated: false });
        }
    }

    // I don't want to waste time reading a very big file.
    else if file_size > 0x07ff_ffff {
        return Ok(FilePreview::None);
    }

    else {
        let prefix = read_bytes_offset(path, 0, 512)?;

        // If the file is valid utf-8, one of these must be valid utf-8.
        match (
            String::from_utf8(prefix[..509].to_vec()),
            String::from_utf8(prefix[..510].to_vec()),
            String::from_utf8(prefix[..511].to_vec()),
            String::from_utf8(prefix[..512].to_vec()),
        ) {
            (_, _, _, Ok(s)) |
            (_, _, Ok(s), _) |
            (_, Ok(s), _, _) |
            (Ok(s), _, _, _) => {
                return Ok(FilePreview::Text { preview: s, truncated: true });
            },
            _ => {},
        }
    }

    let bytes = read_bytes(path)?;

    match normalize_image(&bytes, &thumbnail_dir, 96) {
        Ok(image_id) => Ok(FilePreview::Image { thumbnail_path: image_id.path(&thumbnail_dir)?, size: (image_id.width, image_id.height) }),
        Err(_) => match render_first_few_pages_of_pdf(&bytes, 1, 96) {
            Ok(Some((pages, total_pages))) => match normalize_image(&pages[0], &thumbnail_dir, 80) {
                Ok(image_id) => Ok(FilePreview::Pdf { thumbnail_path: image_id.path(&thumbnail_dir)?, total_pages }),
                Err(_) => Ok(FilePreview::None),
            },
            _ => Ok(FilePreview::None),
        },
    }
}
