//! Events flowing into the app and commands flowing out to workers.

use std::time::Duration;

use image::DynamicImage;

use crate::process::{Collection, ImportError, PlayerState, SpotifyRef};
use crate::queue::{Entry, EntryId, Locator, Source};
use crate::sources::Library;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
    pub key: &'static str,
    pub detail: Option<String>,
}

impl Reason {
    pub fn new(key: &'static str) -> Self {
        Self { key, detail: None }
    }

    pub fn with_detail(key: &'static str, detail: impl Into<String>) -> Self {
        Self {
            key,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceStatus {
    Ready,
    Unavailable(Reason),
    Disconnected(Reason),
}

impl SourceStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, SourceStatus::Ready)
    }

    pub fn reason(&self) -> Option<&Reason> {
        match self {
            SourceStatus::Ready => None,
            SourceStatus::Unavailable(reason) | SourceStatus::Disconnected(reason) => Some(reason),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Key(crate::ui::keymap::Key),
    Tick,
    Status {
        source: Source,
        status: SourceStatus,
    },
    LibraryLoaded(Library),
    PlaybackStarted {
        entry_id: EntryId,
    },
    PlaybackProgress {
        state: PlayerState,
        position: Duration,
        duration: Option<Duration>,
    },
    PlaybackStopped,
    TrackEnded,
    EntryFailed {
        entry_id: EntryId,
        reason: String,
    },
    FirstAudio {
        entry_id: EntryId,
        elapsed: Duration,
    },
    ImportProgress {
        name: String,
        matched: usize,
        total: usize,
    },
    ImportFinished {
        collection: Collection,
    },
    ImportFailed {
        error: ImportError,
    },
    ArtReady {
        entry_id: EntryId,
        image: Option<DynamicImage>,
    },
    Message(String),
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    PlayLocal {
        entry_id: EntryId,
        uris: Vec<String>,
        volume: u8,
    },
    PlayRemote {
        entry_id: EntryId,
        source: Source,
        locator: Locator,
        volume: u8,
    },
    Pause {
        source: Source,
    },
    Resume {
        source: Source,
    },
    Stop {
        source: Source,
    },
    Seek {
        source: Source,
        position: Duration,
    },
    Volume {
        source: Source,
        volume: u8,
    },
    FetchArt {
        entry: Entry,
    },
    LoadLibrary,
    ProbeSources,
    Import {
        reference: SpotifyRef,
    },
    Shutdown,
}
