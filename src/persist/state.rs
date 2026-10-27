//! `state.json` format v1: queue, position, history, volumes and tab.

use std::collections::VecDeque;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::atomic::write_atomic;
use crate::queue::{Entry, EntryState, HistoryItem, Source};

pub const STATE_FILE: &str = "state.json";
pub const STATE_FORMAT: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Volumes {
    pub local: u8,
    pub remote: u8,
}

impl Default for Volumes {
    fn default() -> Self {
        Self {
            local: 70,
            remote: 70,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StateFile {
    pub format: u32,
    pub queue: Vec<Entry>,
    pub position: Option<usize>,
    pub history: Vec<HistoryItem>,
    pub volumes: Volumes,
    pub tab: Option<Source>,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            format: STATE_FORMAT,
            queue: Vec::new(),
            position: None,
            history: Vec::new(),
            volumes: Volumes::default(),
            tab: None,
        }
    }
}

impl StateFile {
    pub fn restore_states(&mut self) {
        // A restored session starts paused; no audio starts until the user plays.
        for entry in &mut self.queue {
            match entry.state {
                EntryState::Playing => entry.state = EntryState::Paused,
                EntryState::Pending | EntryState::Ready => entry.state = EntryState::Pending,
                _ => {}
            }
        }
    }

    pub fn history_deque(&self) -> VecDeque<HistoryItem> {
        self.history.iter().cloned().collect()
    }
}

/// Load state; corrupt files are quarantined and newer formats left untouched.
pub fn load(dir: &Path) -> (StateFile, Vec<String>) {
    let path = dir.join(STATE_FILE);
    let mut warnings = Vec::new();
    if !path.exists() {
        return (StateFile::default(), warnings);
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            return (StateFile::default(), warnings);
        }
    };
    match serde_json::from_str::<StateFile>(&text) {
        Ok(state) if state.format > STATE_FORMAT => {
            warnings.push(format!("{} (format {})", path.display(), state.format));
            (StateFile::default(), warnings)
        }
        Ok(mut state) => {
            if state.format < STATE_FORMAT {
                warnings.push(format!("{} (format {})", path.display(), state.format));
                state = StateFile::default();
            }
            state.restore_states();
            (state, warnings)
        }
        Err(error) => {
            let target = path.with_extension(format!("corrupt-{}", unix_now()));
            let _ = std::fs::rename(&path, &target);
            warnings.push(format!("{}: {error}", target.display()));
            (StateFile::default(), warnings)
        }
    }
}

pub fn save(dir: &Path, state: &StateFile) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
    write_atomic(&dir.join(STATE_FILE), text.as_bytes())
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
