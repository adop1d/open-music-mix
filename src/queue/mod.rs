//! Queue entries and the single source-agnostic queue.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

pub const HISTORY_LIMIT: usize = 200;

/// The three audio sources the player unifies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Source {
    Local,
    Youtube,
    Spotify,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Local, Source::Youtube, Source::Spotify];

    pub fn label(self) -> &'static str {
        match self {
            Source::Local => "local",
            Source::Youtube => "youtube",
            Source::Spotify => "spotify",
        }
    }

    pub fn i18n_key(self) -> &'static str {
        match self {
            Source::Local => "tab_local",
            Source::Youtube => "tab_youtube",
            Source::Spotify => "tab_spotify",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntryId(pub u64);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMeta {
    pub title: String,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<Duration>,
}

impl TrackMeta {
    pub fn new(title: impl Into<String>, artist: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            artists: vec![artist.into()],
            album: None,
            duration: None,
        }
    }

    pub fn artist_line(&self) -> String {
        self.artists.join(", ")
    }
}

/// Where an entry comes from. Kept separate from metadata so a restored session
/// never has to resolve anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locator {
    LocalPath(PathBuf),
    MpdUri(String),
    YoutubeId(String),
    YoutubeQuery(String),
    SpotifyTrack(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryState {
    Pending,
    Ready,
    Playing,
    Paused,
    Failed(String),
    Unavailable,
}

impl EntryState {
    pub fn is_failed(&self) -> bool {
        matches!(self, EntryState::Failed(_) | EntryState::Unavailable)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub source: Source,
    pub meta: TrackMeta,
    pub locator: Locator,
    pub state: EntryState,
}

impl Entry {
    pub fn new(id: EntryId, source: Source, meta: TrackMeta, locator: Locator) -> Self {
        Self {
            id,
            source,
            meta,
            locator,
            state: EntryState::Pending,
        }
    }

    pub fn title(&self) -> &str {
        &self.meta.title
    }

    pub fn subtitle(&self) -> String {
        self.meta.artist_line()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub source: Source,
    pub title: String,
    /// Unix seconds.
    pub timestamp: u64,
}

#[derive(Debug, Default)]
pub struct Queue {
    pub entries: Vec<Entry>,
    pub position: Option<usize>,
    pub history: VecDeque<HistoryItem>,
    /// Row selected in the queue panel; independent of the playing position.
    pub selection: usize,
    next_id: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RemoveOutcome {
    pub removed: Option<Entry>,
    pub was_current: bool,
    pub next_position: Option<usize>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_history(history: Vec<HistoryItem>) -> Self {
        let mut queue = Self::new();
        queue.history = history.into_iter().collect();
        queue
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn append(&mut self, source: Source, meta: TrackMeta, locator: Locator) -> EntryId {
        let id = EntryId(self.next_id);
        self.next_id += 1;
        self.entries.push(Entry::new(id, source, meta, locator));
        id
    }

    pub fn insert(
        &mut self,
        index: usize,
        source: Source,
        meta: TrackMeta,
        locator: Locator,
    ) -> EntryId {
        let id = EntryId(self.next_id);
        self.next_id += 1;
        let index = index.min(self.entries.len());
        self.entries
            .insert(index, Entry::new(id, source, meta, locator));
        if let Some(position) = self.position
            && index <= position
        {
            self.position = Some(position + 1);
        }
        id
    }

    /// Remove one entry. The caller starts `next_position` when `was_current`.
    pub fn remove(&mut self, index: usize) -> RemoveOutcome {
        if index >= self.entries.len() {
            return RemoveOutcome {
                removed: None,
                was_current: false,
                next_position: self.position,
            };
        }
        let removed = self.entries.remove(index);
        let was_current = self.position == Some(index);
        let next_position = match self.position {
            None => None,
            Some(position) if position == index => {
                if self.entries.is_empty() {
                    None
                } else {
                    Some(index.min(self.entries.len() - 1))
                }
            }
            Some(position) if position > index => Some(position - 1),
            Some(position) => Some(position),
        };
        self.position = next_position;
        if self.selection >= self.entries.len() && !self.entries.is_empty() {
            self.selection = self.entries.len() - 1;
        }
        RemoveOutcome {
            removed: Some(removed),
            was_current,
            next_position,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.position = None;
        self.selection = 0;
    }

    pub fn move_entry(&mut self, from: usize, to: usize) -> bool {
        if from >= self.entries.len() || to >= self.entries.len() || from == to {
            return false;
        }
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        if let Some(position) = self.position {
            self.position = Some(match position {
                p if p == from => to,
                p if from < p && p <= to => p - 1,
                p if to <= p && p < from => p + 1,
                p => p,
            });
        }
        true
    }

    pub fn current(&self) -> Option<&Entry> {
        self.position
            .and_then(|position| self.entries.get(position))
    }

    pub fn current_mut(&mut self) -> Option<&mut Entry> {
        self.position
            .and_then(|position| self.entries.get_mut(position))
    }

    pub fn set_position(&mut self, position: usize) {
        if position < self.entries.len() {
            self.position = Some(position);
            self.selection = position;
        }
    }

    pub fn next_position(&self) -> Option<usize> {
        self.position
            .map(|position| position + 1)
            .filter(|next| *next < self.entries.len())
    }

    pub fn previous_position(&self) -> Option<usize> {
        self.position.and_then(|position| position.checked_sub(1))
    }

    pub fn entry_mut(&mut self, id: EntryId) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn index_of(&self, id: EntryId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    pub fn mark_state(&mut self, id: EntryId, state: EntryState) {
        if let Some(entry) = self.entry_mut(id) {
            entry.state = state;
        }
    }

    pub fn record_history(&mut self, entry: &Entry, timestamp: u64) {
        self.history.push_front(HistoryItem {
            source: entry.source,
            title: entry.meta.title.clone(),
            timestamp,
        });
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_back();
        }
    }

    pub fn history(&self) -> &VecDeque<HistoryItem> {
        &self.history
    }

    /// Case-insensitive title/artist match, preserving the original order.
    pub fn filtered_indices(&self, query: &str) -> Vec<usize> {
        let needle = query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                needle.is_empty()
                    || entry.meta.title.to_lowercase().contains(&needle)
                    || entry.meta.artist_line().to_lowercase().contains(&needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn restore(&mut self, entries: Vec<Entry>, position: Option<usize>) {
        self.next_id = entries
            .iter()
            .map(|entry| entry.id.0 + 1)
            .max()
            .unwrap_or(0);
        self.entries = entries;
        self.position = position.filter(|position| *position < self.entries.len());
        self.selection = self.position.unwrap_or(0);
        if let Some(entry) = self.current_mut() {
            entry.state = EntryState::Paused;
        }
    }
}
