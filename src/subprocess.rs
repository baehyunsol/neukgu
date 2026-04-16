// copy-pasted from Sodigy (commit e2ec298ff146)

use crate::Error;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Output {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timeout: bool,
}

pub fn run(
    binary: String,
    args: &[String],
    cwd: &str,
    timeout: u64,  // seconds
) -> Result<Output, Error> {
    let timeout = (timeout * 1000) as u128;
    let mut child_process = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut child_stdout = child_process.stdout.take().unwrap();
    let mut child_stderr = child_process.stderr.take().unwrap();
    let stdout_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = child_stdout.read_to_end(&mut buf);
        buf
    });
    let stderr_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = child_stderr.read_to_end(&mut buf);
        buf
    });

    let started_at = Instant::now();
    let mut sleep_for = 5;
    let mut timeout_flag = false;

    let status: i32 = loop {
        if Instant::now().duration_since(started_at.clone()).as_millis() > timeout {
            child_process.kill()?;
            child_process.wait()?;
            timeout_flag = true;
            break -200;
        }

        match child_process.try_wait()? {
            Some(status) => {
                break status.code().unwrap_or(-300);
            },
            None => {
                thread::sleep(Duration::from_millis(sleep_for));
                sleep_for = (sleep_for * 2).min(128);
            },
        }
    };

    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();

    Ok(Output {
        status,
        stdout,
        stderr,
        timeout: timeout_flag,
    })
}
