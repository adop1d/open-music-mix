//! Argument parsing without a parser crate.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cli {
    Tui,
    Import(String),
    Help,
}

pub const USAGE: &str = "\
omm — local MPD, YouTube and Spotify in one terminal queue

Usage:
  omm                 start the player
  omm --import <url>  import a Spotify playlist, album or track, then start
  omm --help          show this help

External binaries: mpd, mpv, yt-dlp, spotdl
Config and state live under the platform config directory.
";

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Cli, String> {
    let mut import: Option<String> = None;
    let mut help = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => help = true,
            "--import" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("--import needs a URL\n\n{USAGE}"))?;
                import = Some(value);
            }
            other if other.starts_with("--import=") => {
                import = Some(other["--import=".len()..].to_string());
            }
            other => {
                return Err(format!("unknown argument: {other}\n\n{USAGE}"));
            }
        }
    }
    if help {
        return Ok(Cli::Help);
    }
    match import {
        Some(url) if url.trim().is_empty() => Err(format!("--import needs a URL\n\n{USAGE}")),
        Some(url) => Ok(Cli::Import(url)),
        None => Ok(Cli::Tui),
    }
}
