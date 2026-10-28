//! Boundaries around every external process. Real implementations are thin
//! wrappers; fakes implement the same traits so the suite runs offline.

pub mod command;
pub mod fakes;
pub mod mpd;
pub mod mpv;
pub mod resolver;
pub mod spotdl;

use std::time::Duration;

use crate::queue::{Locator, TrackMeta};

pub use fakes::{FakeImporter, FakeMpd, FakeMpv, FakeResolver};
pub use mpd::RealMpd;
pub use mpv::RealMpv;
pub use resolver::RealResolver;
pub use spotdl::RealSpotifyImporter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    MissingBinary(&'static str),
    Unreachable(String),
    Disconnected,
    Failed(String),
}

impl SourceError {
    pub fn message(&self) -> String {
        match self {
            SourceError::MissingBinary(name) => format!("{name} not found"),
            SourceError::Unreachable(reason) => reason.clone(),
            SourceError::Disconnected => "connection lost".to_string(),
            SourceError::Failed(reason) => reason.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub uri: String,
    pub duration: Option<Duration>,
}

impl SongInfo {
    pub fn meta(&self) -> TrackMeta {
        TrackMeta {
            title: self.title.clone(),
            artists: if self.artist.is_empty() {
                Vec::new()
            } else {
                vec![self.artist.clone()]
            },
            album: self.album.clone(),
            duration: self.duration,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumInfo {
    pub artist: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackStatus {
    pub state: PlayerState,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: Option<u8>,
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        Self {
            state: PlayerState::Stopped,
            position: Duration::ZERO,
            duration: None,
            volume: None,
        }
    }
}

/// Local source boundary. Implementations must never clear the daemon queue.
pub trait MpdBackend: Send {
    fn connect(&mut self) -> Result<(), SourceError>;
    fn artists(&mut self) -> Result<Vec<String>, SourceError>;
    fn albums(&mut self) -> Result<Vec<AlbumInfo>, SourceError>;
    fn folders(&mut self) -> Result<Vec<String>, SourceError>;
    fn playlists(&mut self) -> Result<Vec<String>, SourceError>;
    fn artist_albums(&mut self, artist: &str) -> Result<Vec<AlbumInfo>, SourceError>;
    fn album_songs(&mut self, artist: &str, album: &str) -> Result<Vec<SongInfo>, SourceError>;
    fn playlist_songs(&mut self, name: &str) -> Result<Vec<SongInfo>, SourceError>;
    fn folder_songs(&mut self, folder: &str) -> Result<Vec<SongInfo>, SourceError>;
    /// `addid` each URI then `playid` the first added one. Existing queue survives.
    fn enqueue_and_play(&mut self, uris: &[String]) -> Result<(), SourceError>;
    fn pause(&mut self, paused: bool) -> Result<(), SourceError>;
    fn stop(&mut self) -> Result<(), SourceError>;
    fn seek(&mut self, position: Duration) -> Result<(), SourceError>;
    fn set_volume(&mut self, volume: u8) -> Result<(), SourceError>;
    fn status(&mut self) -> Result<PlaybackStatus, SourceError>;
    fn album_art(&mut self, uri: &str) -> Result<Option<Vec<u8>>, SourceError>;

    /// Root used to resolve MPD URIs to a folder image, when the daemon reports it.
    fn music_directory(&mut self) -> Option<std::path::PathBuf> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerEvent {
    FirstAudio,
    EndFile,
    Paused(bool),
    Position(Duration),
    Duration(Duration),
    Failed(String),
}

/// Remote player boundary (mpv over its JSON IPC socket).
pub trait MpvBackend: Send {
    fn spawn(&mut self) -> Result<(), SourceError>;
    fn play(&mut self, url: &str) -> Result<(), SourceError>;
    fn pause(&mut self, paused: bool) -> Result<(), SourceError>;
    fn seek(&mut self, position: Duration) -> Result<(), SourceError>;
    fn set_volume(&mut self, volume: u8) -> Result<(), SourceError>;
    fn stop(&mut self) -> Result<(), SourceError>;
    fn next_event(&mut self, timeout: Duration) -> Option<PlayerEvent>;
    fn is_alive(&mut self) -> bool;
    fn shutdown(&mut self);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub url: String,
    pub video_id: Option<String>,
    pub duration: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub video_id: String,
    pub duration: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    MissingBinary(&'static str),
    NoResults(String),
    Unreachable(String),
    Timeout,
}

impl ResolveError {
    pub fn class(&self) -> &'static str {
        match self {
            ResolveError::MissingBinary(_) => "missing-binary",
            ResolveError::NoResults(_) => "no-results",
            ResolveError::Unreachable(_) => "unreachable",
            ResolveError::Timeout => "timeout",
        }
    }

    pub fn message(&self) -> String {
        match self {
            ResolveError::MissingBinary(name) => format!("{name} not found"),
            ResolveError::NoResults(query) => format!("no results for {query}"),
            ResolveError::Unreachable(reason) => reason.clone(),
            ResolveError::Timeout => "resolver timed out".to_string(),
        }
    }
}

/// Remote stream resolution boundary (yt-dlp).
pub trait Resolver: Send + Sync {
    fn resolve(&self, locator: &Locator) -> Result<Resolved, ResolveError>;
    /// URL that lets mpv resolve the stream itself (fresh URL and headers).
    fn playback_url(&self, _locator: &Locator) -> Option<String> {
        None
    }
    /// Search used during import matching; returns the best candidate's id and duration.
    fn search_candidate(&self, query: &str) -> Result<Candidate, ResolveError>;
    /// Artificial delay for deterministic timing tests; real resolvers return zero.
    fn latency(&self) -> Duration {
        Duration::ZERO
    }
    /// Fetch a video thumbnail into `directory`; default means no art available.
    fn thumbnail(&self, _video_id: &str, _directory: &std::path::Path) -> Option<Vec<u8>> {
        None
    }
}

pub use spotdl::{
    Collection, CollectionKind, ImportError, ImportedTrack, SpotifyImporter, SpotifyRef,
    parse_reference,
};
