//! Spotify import: metadata only, through `spotdl save`. No audio is stored.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::process::command::{RunError, run_with_timeout};

const IMPORT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionKind {
    Playlist,
    Album,
    Track,
    Unknown,
}

impl CollectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CollectionKind::Playlist => "playlist",
            CollectionKind::Album => "album",
            CollectionKind::Track => "track",
            CollectionKind::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotifyRef {
    pub kind: CollectionKind,
    pub id: String,
}

impl SpotifyRef {
    pub fn url(&self, kind: CollectionKind) -> String {
        format!("https://open.spotify.com/{}/{}", kind.as_str(), self.id)
    }
}

/// Parse a public Spotify URL, a `spotify:` URI or a bare 22-character ID.
pub fn parse_reference(input: &str) -> Result<SpotifyRef, ImportError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(ImportError::InvalidInput(input.to_string()));
    }

    if let Some(rest) = text.strip_prefix("spotify:") {
        let mut parts = rest.split(':');
        let kind = parts.next().unwrap_or_default();
        let id = parts.next().unwrap_or_default();
        if let Some(kind) = kind_from_str(kind)
            && is_spotify_id(id)
        {
            return Ok(SpotifyRef {
                kind,
                id: id.to_string(),
            });
        }
        return Err(ImportError::InvalidInput(input.to_string()));
    }

    if let Some(path) = text
        .strip_prefix("https://open.spotify.com/")
        .or_else(|| text.strip_prefix("http://open.spotify.com/"))
    {
        let path = path.split(['?', '#']).next().unwrap_or(path);
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        // Optional locale segment: /intl-de/playlist/<id>
        let offset = if parts.first().is_some_and(|part| part.starts_with("intl-")) {
            1
        } else {
            0
        };
        if let (Some(kind), Some(id)) = (
            parts.get(offset).and_then(|value| kind_from_str(value)),
            parts.get(offset + 1),
        ) && is_spotify_id(id)
        {
            return Ok(SpotifyRef {
                kind,
                id: (*id).to_string(),
            });
        }
        return Err(ImportError::InvalidInput(input.to_string()));
    }

    if is_spotify_id(text) {
        return Ok(SpotifyRef {
            kind: CollectionKind::Unknown,
            id: text.to_string(),
        });
    }

    Err(ImportError::InvalidInput(input.to_string()))
}

fn kind_from_str(value: &str) -> Option<CollectionKind> {
    match value {
        "playlist" => Some(CollectionKind::Playlist),
        "album" => Some(CollectionKind::Album),
        "track" => Some(CollectionKind::Track),
        _ => None,
    }
}

fn is_spotify_id(value: &str) -> bool {
    value.len() == 22 && value.chars().all(|c| c.is_ascii_alphanumeric())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedTrack {
    pub title: String,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<Duration>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub youtube_id: Option<String>,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    pub url: String,
    #[serde(default)]
    pub tracks: Vec<ImportedTrack>,
}

impl Collection {
    pub fn matched(&self) -> usize {
        self.tracks
            .iter()
            .filter(|track| !track.unavailable)
            .count()
    }

    pub fn unmatched(&self) -> usize {
        self.tracks.iter().filter(|track| track.unavailable).count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    InvalidInput(String),
    Private,
    MissingBinary(&'static str),
    Timeout,
    Failed(String),
}

impl ImportError {
    pub fn message(&self) -> String {
        match self {
            ImportError::InvalidInput(input) => format!(
                "invalid Spotify reference: {input}. Use a playlist, album or track URL or a 22-character ID"
            ),
            ImportError::Private => "only public collections are supported".to_string(),
            ImportError::MissingBinary(name) => format!("{name} not found"),
            ImportError::Timeout => "import timed out".to_string(),
            ImportError::Failed(reason) => reason.clone(),
        }
    }
}

pub trait SpotifyImporter: Send + Sync {
    fn fetch(&self, reference: &SpotifyRef) -> Result<Collection, ImportError>;
}

pub struct RealSpotifyImporter {
    program: String,
}

impl Default for RealSpotifyImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl RealSpotifyImporter {
    pub fn new() -> Self {
        Self {
            program: "spotdl".to_string(),
        }
    }

    pub fn with_program(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
        }
    }

    fn save(&self, url: &str) -> Result<Collection, ImportError> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // spotdl rejects any save file that does not end in `.spotdl`.
        let path: PathBuf = std::env::temp_dir().join(format!(
            "omm-import-{}-{stamp}-{sequence}.spotdl",
            std::process::id()
        ));
        let mut command = Command::new(&self.program);
        command.args(["save", url, "--save-file", &path.display().to_string()]);
        let output = match run_with_timeout(&mut command, IMPORT_TIMEOUT) {
            Ok(output) => output,
            Err(RunError::NotFound) => return Err(ImportError::MissingBinary("spotdl")),
            Err(RunError::TimedOut) => return Err(ImportError::Timeout),
            Err(RunError::Io(error)) => return Err(ImportError::Failed(error.to_string())),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            let private = [
                "private",
                "authentication",
                "credentials",
                "login",
                "403",
                "401",
            ]
            .iter()
            .any(|needle| stderr.contains(needle));
            let _ = std::fs::remove_file(&path);
            return Err(if private {
                ImportError::Private
            } else {
                let first = stderr.lines().next().unwrap_or("spotdl failed").to_string();
                ImportError::Failed(first)
            });
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| ImportError::Failed(error.to_string()))?;
        let _ = std::fs::remove_file(&path);
        parse_spotdl_json(&text)
    }
}

impl SpotifyImporter for RealSpotifyImporter {
    fn fetch(&self, reference: &SpotifyRef) -> Result<Collection, ImportError> {
        if reference.kind != CollectionKind::Unknown {
            return self.save(&reference.url(reference.kind));
        }
        let mut last = ImportError::Failed("no collection found".to_string());
        for kind in [
            CollectionKind::Playlist,
            CollectionKind::Album,
            CollectionKind::Track,
        ] {
            match self.save(&reference.url(kind)) {
                Ok(collection) => return Ok(collection),
                Err(ImportError::Private) => last = ImportError::Private,
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}

pub fn parse_spotdl_json(text: &str) -> Result<Collection, ImportError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| ImportError::Failed(error.to_string()))?;
    let (name, raw_tracks) = if let Some(tracks) = value.as_array() {
        (String::new(), tracks.clone())
    } else {
        let tracks = value
            .get("tracks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let name = ["name", "title", "playlist_name", "album_name"]
            .iter()
            .find_map(|key| value.get(key).and_then(Value::as_str))
            .unwrap_or_default()
            .to_string();
        (name, tracks)
    };

    let tracks: Vec<ImportedTrack> = raw_tracks.iter().filter_map(track_from_json).collect();
    if tracks.is_empty() {
        return Err(ImportError::Failed("no tracks in import".to_string()));
    }
    let name = if name.is_empty() {
        tracks[0].title.clone()
    } else {
        name
    };
    let id = if let Some(from_json) = value.get("id").and_then(Value::as_str).map(str::to_string) {
        from_json
    } else {
        format!("{}-{}", slug(&name), tracks.len())
    };
    Ok(Collection {
        id,
        name,
        kind: CollectionKind::Unknown,
        url: String::new(),
        tracks,
    })
}

fn track_from_json(value: &Value) -> Option<ImportedTrack> {
    let title = value
        .get("name")
        .or_else(|| value.get("title"))?
        .as_str()?
        .to_string();
    let artists = artists_from_json(value);
    let album = ["album_name", "album"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string);
    let duration = value
        .get("duration")
        .and_then(Value::as_f64)
        .map(Duration::from_secs_f64)
        .or_else(|| {
            value
                .get("duration_ms")
                .and_then(Value::as_f64)
                .map(|ms| Duration::from_secs_f64(ms / 1000.0))
        });
    let isrc = value
        .get("isrc")
        .and_then(Value::as_str)
        .map(str::to_string);
    let query = if artists.is_empty() {
        title.clone()
    } else {
        format!("{} {title}", artists.join(" "))
    };
    Some(ImportedTrack {
        title,
        artists,
        album,
        duration,
        isrc,
        query,
        youtube_id: None,
        unavailable: false,
    })
}

fn artists_from_json(value: &Value) -> Vec<String> {
    if let Some(artists) = value.get("artists").and_then(Value::as_array) {
        return artists
            .iter()
            .map(|artist| {
                artist
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| {
                        artist
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_default()
            })
            .filter(|name| !name.is_empty())
            .collect();
    }
    value
        .get("artist")
        .and_then(Value::as_str)
        .map(|artist| vec![artist.to_string()])
        .unwrap_or_default()
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
