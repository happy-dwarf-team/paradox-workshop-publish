//! Running steamcmd: stdin closed, output read line by line as it comes,
//! and a timeout after which the process is killed.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// How a run ended.
#[derive(Debug)]
pub struct Run {
    /// `None` when the process was killed or ended by a signal.
    pub code: Option<i32>,
    /// stdout and stderr in arrival order, one line per entry.
    pub lines: Vec<String>,
    pub timed_out: bool,
}

/// Runs `program` with `args` in `cwd`, with stdin closed. Each output line
/// goes to `on_line` as soon as it arrives. After `timeout` the process is
/// killed and [`Run::timed_out`] is set.
pub fn run<I, S>(
    program: &Path,
    args: I,
    cwd: &Path,
    env: &[(&str, &OsStr)],
    timeout: Duration,
    on_line: &mut dyn FnMut(&str),
) -> io::Result<Run>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn()?;

    let (sender, receiver) = mpsc::channel();
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|pipe| {
        let sender = sender.clone();
        thread::spawn(move || read_lines(pipe, &sender))
    })
    .collect();
    drop(sender);

    let deadline = Instant::now() + timeout;
    let mut lines = Vec::new();
    let mut take = |line: String, lines: &mut Vec<String>| {
        on_line(&line);
        lines.push(line);
    };
    let (code, timed_out) = loop {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => take(line, &mut lines),
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = child.try_wait()? {
            break (status.code(), false);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
    };

    // Collect what is still buffered. A grandchild that keeps a pipe open
    // must not block us, so this waits a few seconds at most.
    let drain_until = Instant::now() + Duration::from_secs(5);
    while let Some(left) = drain_until.checked_duration_since(Instant::now()) {
        match receiver.recv_timeout(left) {
            Ok(line) => take(line, &mut lines),
            Err(_) => break,
        }
    }
    for reader in readers {
        if reader.is_finished() {
            let _ = reader.join();
        }
    }
    Ok(Run {
        code,
        lines,
        timed_out,
    })
}

/// Splits a pipe into lines on `\n` or `\r` (steamcmd redraws progress
/// lines with `\r`), decoding invalid UTF-8 lossily.
fn read_lines(mut pipe: Box<dyn Read + Send>, sender: &mpsc::Sender<String>) {
    let mut pending = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = match pipe.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        for &byte in &buffer[..read] {
            if byte == b'\n' || byte == b'\r' {
                if !pending.is_empty() {
                    let line = String::from_utf8_lossy(&pending).into_owned();
                    pending.clear();
                    if sender.send(line).is_err() {
                        return;
                    }
                }
            } else {
                pending.push(byte);
            }
        }
    }
    if !pending.is_empty() {
        let _ = sender.send(String::from_utf8_lossy(&pending).into_owned());
    }
}
