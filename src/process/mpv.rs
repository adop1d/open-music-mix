//! Long-lived mpv process controlled over its JSON IPC socket.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use serde_json::{Value, json};

use super::{MpvBackend, PlayerEvent, SourceError};

pub struct RealMpv {
    program: String,
    socket_path: PathBuf,
    cache_dir: PathBuf,
    child: Option<Child>,
    #[cfg(unix)]
    stream: Option<UnixStream>,
    #[cfg(unix)]
    reader: Option<BufReader<UnixStream>>,
}

impl RealMpv {
    pub fn new(workspace: &Path) -> Self {
        Self {
            program: "mpv".to_string(),
            socket_path: workspace.join("mpv.sock"),
            cache_dir: workspace.join("audio"),
            child: None,
            #[cfg(unix)]
            stream: None,
            #[cfg(unix)]
            reader: None,
        }
    }

    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    #[cfg(unix)]
    fn send(&mut self, value: Value) -> Result<(), SourceError> {
        let stream = self.stream.as_mut().ok_or(SourceError::Disconnected)?;
        let line = format!("{value}\n");
        stream
            .write_all(line.as_bytes())
            .map_err(|error| SourceError::Failed(error.to_string()))
    }

    #[cfg(not(unix))]
    fn send(&mut self, _value: Value) -> Result<(), SourceError> {
        Err(SourceError::Failed(
            "mpv IPC requires a unix socket".to_string(),
        ))
    }
}

impl MpvBackend for RealMpv {
    fn spawn(&mut self) -> Result<(), SourceError> {
        if self.child.is_some() {
            return Ok(());
        }
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::create_dir_all(&self.cache_dir);

        let child = Command::new(&self.program)
            .args([
                "--idle=yes",
                "--no-video",
                "--audio-display=no",
                // ytdl:// URLs need mpv's yt-dlp hook for a fresh signed URL and headers.
                "--ytdl-format=bestaudio/best",
                // YouTube's default web clients can return 403 on signed URLs.
                "--ytdl-raw-options=extractor-args=youtube:player_client=android",
                "--really-quiet",
                "--cache=yes",
                "--cache-secs=20",
                "--demuxer-max-bytes=64MiB",
            ])
            .arg(format!("--input-ipc-server={}", self.socket_path.display()))
            // mpv 0.37 renamed --cache-dir; the old name aborts startup.
            .arg(format!("--demuxer-cache-dir={}", self.cache_dir.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => SourceError::MissingBinary("mpv"),
                _ => SourceError::Failed(error.to_string()),
            })?;
        self.child = Some(child);

        #[cfg(unix)]
        {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Ok(stream) = UnixStream::connect(&self.socket_path) {
                    stream
                        .set_read_timeout(Some(Duration::from_millis(50)))
                        .ok();
                    let reader = BufReader::new(
                        stream
                            .try_clone()
                            .map_err(|error| SourceError::Failed(error.to_string()))?,
                    );
                    self.stream = Some(stream);
                    self.reader = Some(reader);
                    let _ = self.send(json!({"command": ["observe_property", 1, "time-pos"]}));
                    let _ = self.send(json!({"command": ["observe_property", 2, "duration"]}));
                    let _ = self.send(json!({"command": ["observe_property", 3, "pause"]}));
                    return Ok(());
                }
                if !self.is_alive() {
                    return Err(SourceError::Failed("mpv exited during startup".to_string()));
                }
                if Instant::now() >= deadline {
                    return Err(SourceError::Unreachable(
                        "mpv IPC socket did not appear".to_string(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        #[cfg(not(unix))]
        Err(SourceError::Failed(
            "mpv IPC requires a unix socket".to_string(),
        ))
    }

    fn play(&mut self, url: &str) -> Result<(), SourceError> {
        self.send(json!({"command": ["loadfile", url, "replace"]}))
    }

    fn pause(&mut self, paused: bool) -> Result<(), SourceError> {
        self.send(json!({"command": ["set_property", "pause", paused]}))
    }

    fn seek(&mut self, position: Duration) -> Result<(), SourceError> {
        let seconds = position.as_secs_f64();
        self.send(json!({"command": ["seek", seconds, "absolute"]}))
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), SourceError> {
        self.send(json!({"command": ["set_property", "volume", volume.min(100)]}))
    }

    fn stop(&mut self) -> Result<(), SourceError> {
        self.send(json!({"command": ["stop"]}))
    }

    fn next_event(&mut self, timeout: Duration) -> Option<PlayerEvent> {
        #[cfg(unix)]
        {
            let reader = self.reader.as_mut()?;
            let deadline = Instant::now() + timeout;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => return None,
                    Ok(_) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&line)
                            && let Some(event) = map_event(&value)
                        {
                            return Some(event);
                        }
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => return None,
                }
                if Instant::now() >= deadline {
                    return None;
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = timeout;
            None
        }
    }

    fn is_alive(&mut self) -> bool {
        self.child
            .as_mut()
            .map(|child| matches!(child.try_wait(), Ok(None)))
            .unwrap_or(false)
    }

    fn shutdown(&mut self) {
        let _ = self.send(json!({"command": ["quit"]}));
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        #[cfg(unix)]
        {
            self.stream = None;
            self.reader = None;
        }
        let _ = fs::remove_file(&self.socket_path);
    }
}

fn map_event(value: &Value) -> Option<PlayerEvent> {
    match value.get("event")?.as_str()? {
        "playback-restart" => Some(PlayerEvent::FirstAudio),
        "end-file" => {
            // ytdl:// emits a redirect end-file before the real file starts.
            match value.get("reason").and_then(|reason| reason.as_str()) {
                Some("redirect") => None,
                Some("error") => Some(PlayerEvent::Failed(
                    value
                        .get("file_error")
                        .and_then(|error| error.as_str())
                        .unwrap_or("playback failed")
                        .to_string(),
                )),
                _ => Some(PlayerEvent::EndFile),
            }
        }
        "property-change" => {
            let name = value.get("name")?.as_str()?;
            let data = value.get("data")?;
            match name {
                "time-pos" => data.as_f64().map(|seconds| {
                    PlayerEvent::Position(Duration::from_secs_f64(seconds.max(0.0)))
                }),
                "duration" => data.as_f64().map(|seconds| {
                    PlayerEvent::Duration(Duration::from_secs_f64(seconds.max(0.0)))
                }),
                "pause" => data.as_bool().map(PlayerEvent::Paused),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redirect_end_file_is_not_a_track_end() {
        assert_eq!(
            map_event(&json!({"event": "end-file", "reason": "redirect"})),
            None
        );
        assert_eq!(
            map_event(&json!({"event": "end-file", "reason": "eof"})),
            Some(PlayerEvent::EndFile)
        );
    }

    #[test]
    fn error_end_file_is_a_failure() {
        assert_eq!(
            map_event(&json!({
                "event": "end-file",
                "reason": "error",
                "file_error": "HTTP error 403 Forbidden"
            })),
            Some(PlayerEvent::Failed("HTTP error 403 Forbidden".to_string()))
        );
    }
}
