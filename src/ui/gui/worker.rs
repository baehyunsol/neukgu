use super::tab::TabId;
use base64::Engine;
use crate::{ChatId, Error, LLMToken, subprocess, subprocess::Output};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

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
    AddChatTurn {
        chat_id: ChatId,
        query: Vec<LLMToken>,
    },
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
    WorkerError(String),
}

pub struct Worker {
    pub id: usize,
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
            self.workers[index] = init_worker(index);
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
                    &[],
                    &path,
                    20,  // timeout
                    "",     // working_dir (it's None because it's not a neugku dir)
                    false,  // check_interruption
                )?;

                tx_to_main.send(JobResult { id: Some(id), kind: parse_rg_output(regex, rg_result) }).unwrap();
            },
            Job { id, kind: JobKind::AddChatTurn { chat_id, query } } => {
                let mut chat = Chat::load(chat_id, _)?;
                // How can I call an async function..??
                // do I need a subprocess??
                // does `block_on` work here?
                // chat.add_turn(query).await?;
                todo!()
            },
        }
    }

    Ok(())
}

fn init_worker(id: usize) -> Worker {
    let (tx_to_main, rx_to_main) = mpsc::channel();
    let (tx_from_main, rx_from_main) = mpsc::channel();

    thread::spawn(move || match event_loop(tx_to_main.clone(), rx_from_main) {
        Ok(()) => {},
        Err(e) => {
            tx_to_main.send(JobResult { id: None, kind: JobResultKind::WorkerError(format!("{e:?}")) }).unwrap();
        },
    });

    Worker { id, rx_to_main, tx_from_main }
}

pub fn init_workers(count: usize) -> Workers {
    Workers {
        round_robin: 0,
        tab_id_by_job_id: HashMap::new(),
        workers: (0..count).map(|id| init_worker(id)).collect(),
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
        r#type: String,
        data: RgMatchData,
    }

    #[derive(Debug, Deserialize)]
    struct RgMatchData {
        path: TextOrBytes,
        lines: TextOrBytes,
        line_number: usize,
        absolute_offset: usize,
        submatches: Vec<Submatch>,
    }

    #[derive(Debug, Deserialize)]
    struct Submatch {
        r#match: TextOrBytes,
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
