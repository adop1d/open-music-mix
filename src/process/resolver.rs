//! yt-dlp resolver. Every network call goes through this process.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::process::command::{RunError, run_with_timeout};

use super::{Candidate, ResolveError, Resolved, Resolver};
use crate::queue::Locator;

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);
const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
const THUMBNAIL_TIMEOUT: Duration = Duration::from_secs(15);

pub struct RealResolver {
    program: String,
}

impl Default for RealResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl RealResolver {
    pub fn new() -> Self {
        Self {
            program: "yt-dlp".to_string(),
        }
    }

    pub fn with_program(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
        }
    }

    fn target(locator: &Locator) -> Result<String, ResolveError> {
        match locator {
            Locator::YoutubeId(id) => Ok(format!("https://www.youtube.com/watch?v={id}")),
            Locator::YoutubeQuery(query) => Ok(format!("ytsearch1:{query}")),
            Locator::SpotifyTrack(query) => Ok(query.clone()),
            Locator::LocalPath(path) => Ok(path.display().to_string()),
            Locator::MpdUri(uri) => Ok(uri.clone()),
        }
    }

    fn classify(output: Result<std::process::Output, RunError>) -> Result<String, ResolveError> {
        match output {
            Ok(output) if output.status.success() => {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
                if stderr.contains("no results")
                    || stderr.contains("does not exist")
                    || stderr.contains("unsupported url")
                    || stderr.contains("not found")
                {
                    Err(ResolveError::NoResults(String::new()))
                } else {
                    let first = stderr
                        .lines()
                        .next()
                        .unwrap_or("resolver failed")
                        .to_string();
                    Err(ResolveError::Unreachable(first))
                }
            }
            Err(RunError::NotFound) => Err(ResolveError::MissingBinary("yt-dlp")),
            Err(RunError::TimedOut) => Err(ResolveError::Timeout),
            Err(RunError::Io(error)) => Err(ResolveError::Unreachable(error.to_string())),
        }
    }
}

impl Resolver for RealResolver {
    fn resolve(&self, locator: &Locator) -> Result<Resolved, ResolveError> {
        let target = Self::target(locator)?;
        let mut command = Command::new(&self.program);
        command.args([
            "--no-playlist",
            "--no-warnings",
            "--get-url",
            "-f",
            "bestaudio/best",
            &target,
        ]);
        let stdout = Self::classify(run_with_timeout(&mut command, RESOLVE_TIMEOUT))?;
        let url = stdout
            .lines()
            .next()
            .filter(|line| !line.is_empty())
            .ok_or_else(|| ResolveError::NoResults(target.clone()))?
            .to_string();
        Ok(Resolved {
            url,
            video_id: match locator {
                Locator::YoutubeId(id) => Some(id.clone()),
                _ => None,
            },
            duration: None,
        })
    }

    fn search_candidate(&self, query: &str) -> Result<Candidate, ResolveError> {
        let target = format!("ytsearch1:{query}");
        let mut command = Command::new(&self.program);
        command.args([
            "--no-playlist",
            "--no-warnings",
            "--skip-download",
            "--print",
            "%(id)s|%(duration)s",
            &target,
        ]);
        let stdout =
            Self::classify(run_with_timeout(&mut command, SEARCH_TIMEOUT)).map_err(|error| {
                match error {
                    ResolveError::NoResults(_) => ResolveError::NoResults(query.to_string()),
                    other => other,
                }
            })?;
        let mut parts = stdout.lines().next().unwrap_or_default().split('|');
        let video_id = parts.next().unwrap_or_default().trim().to_string();
        if video_id.is_empty() || video_id == "NA" {
            return Err(ResolveError::NoResults(query.to_string()));
        }
        let duration = parts
            .next()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map(Duration::from_secs_f64);
        Ok(Candidate { video_id, duration })
    }

    fn thumbnail(&self, video_id: &str, directory: &Path) -> Option<Vec<u8>> {
        let path = self.download_thumbnail(video_id, directory)?;
        std::fs::read(path).ok()
    }
}

impl RealResolver {
    /// Download the thumbnail for a video into `directory`; returns the file.
    pub fn download_thumbnail(&self, video_id: &str, directory: &Path) -> Option<PathBuf> {
        let _ = std::fs::create_dir_all(directory);
        let pattern = format!("{}/%(id)s", directory.display());
        let mut command = Command::new(&self.program);
        command.args([
            "--no-playlist",
            "--no-warnings",
            "--skip-download",
            "--write-thumbnail",
            "-o",
            &pattern,
            &format!("https://www.youtube.com/watch?v={video_id}"),
        ]);
        run_with_timeout(&mut command, THUMBNAIL_TIMEOUT).ok()?;
        let prefix = video_id.to_string();
        std::fs::read_dir(directory)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| stem == prefix)
                    .unwrap_or(false)
            })
    }
}
