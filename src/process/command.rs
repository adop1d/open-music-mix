//! Small subprocess helper with a hard timeout. No external dependency needed.

use std::io;
use std::process::{Command, Output, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub enum RunError {
    NotFound,
    TimedOut,
    Io(io::Error),
}

/// Run a command, killing it when `timeout` elapses.
pub fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<Output, RunError> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => RunError::NotFound,
            _ => RunError::Io(error),
        })?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().map_err(RunError::Io);
            }
            Ok(None) => {}
            Err(error) => return Err(RunError::Io(error)),
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RunError::TimedOut);
        }
        sleep(Duration::from_millis(5));
    }
}
