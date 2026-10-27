//! Application state machine. Never awaits: slow work goes through `Command`s.

mod input;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::art::ArtState;
use crate::event::{AppEvent, Command, Reason, SourceStatus};
use crate::i18n::I18n;
use crate::persist::{Config, StateFile, Volumes};
use crate::process::{AlbumInfo, Collection, PlayerState, SongInfo};
use crate::queue::{EntryId, EntryState, Queue, Source};
use crate::sources::Library;
use crate::ui::keymap::Keymap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    Explorer,
    Queue,
    NowPlaying,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::Explorer => Panel::Queue,
            Panel::Queue => Panel::NowPlaying,
            Panel::NowPlaying => Panel::Explorer,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Panel::Explorer => Panel::NowPlaying,
            Panel::Queue => Panel::Explorer,
            Panel::NowPlaying => Panel::Queue,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalView {
    Artists,
    Albums,
    Folders,
    Playlists,
}

impl LocalView {
    pub fn next(self) -> Self {
        match self {
            LocalView::Artists => LocalView::Albums,
            LocalView::Albums => LocalView::Folders,
            LocalView::Folders => LocalView::Playlists,
            LocalView::Playlists => LocalView::Artists,
        }
    }

    pub fn i18n_key(self) -> &'static str {
        match self {
            LocalView::Artists => "view_artists",
            LocalView::Albums => "view_albums",
            LocalView::Folders => "view_folders",
            LocalView::Playlists => "view_playlists",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalSongs {
    Album { artist: String, name: String },
    Folder(String),
    Playlist(String),
    Artist(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBrowser {
    pub view: LocalView,
    pub open_artist: Option<String>,
    pub songs: Option<LocalSongs>,
    pub selection: usize,
}

impl Default for LocalBrowser {
    fn default() -> Self {
        Self {
            view: LocalView::Artists,
            open_artist: None,
            songs: None,
            selection: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalRow {
    Artist(String),
    Album(AlbumInfo),
    Folder(String),
    Playlist(String),
    Song(SongInfo),
}

impl LocalRow {
    pub fn label(&self) -> String {
        match self {
            LocalRow::Artist(name) => name.clone(),
            LocalRow::Album(album) => album.name.clone(),
            LocalRow::Folder(folder) => folder.clone(),
            LocalRow::Playlist(playlist) => playlist.clone(),
            LocalRow::Song(song) => song.title.clone(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            LocalRow::Artist(_) => String::new(),
            LocalRow::Album(album) => album.artist.clone(),
            LocalRow::Folder(_) => String::new(),
            LocalRow::Playlist(_) => String::new(),
            LocalRow::Song(song) => song.artist.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SpotifyBrowser {
    pub open_collection: Option<usize>,
    pub selection: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ImportStatus {
    pub name: String,
    pub matched: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Default)]
pub struct UiState {
    pub search: Option<String>,
    pub search_active: bool,
    pub help_open: bool,
    pub import_open: bool,
    pub import_input: String,
    pub import_status: Option<ImportStatus>,
    pub notice: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Playback {
    pub state: PlayerState,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub entry_id: Option<EntryId>,
    pub source: Option<Source>,
    pub title: Option<String>,
    pub first_audio: Option<Duration>,
    pub loaded: bool,
    pub volumes: Volumes,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            state: PlayerState::Stopped,
            position: Duration::ZERO,
            duration: None,
            entry_id: None,
            source: None,
            title: None,
            first_audio: None,
            loaded: false,
            volumes: Volumes::default(),
        }
    }
}

pub struct App {
    pub config: Config,
    pub i18n: I18n,
    pub keymap: Keymap,
    pub queue: Queue,
    pub library: Library,
    pub local: LocalBrowser,
    pub spotify: SpotifyBrowser,
    pub collections: Vec<Collection>,
    pub statuses: HashMap<Source, SourceStatus>,
    pub focus: Panel,
    pub tab: Source,
    pub playback: Playback,
    pub ui: UiState,
    pub art: ArtState,
    pub art_renderer: crate::art::ArtRenderer,
    pub commands: Vec<Command>,
    pub quit: bool,
    pub pending_play: Option<EntryId>,
    pub config_dir: Option<PathBuf>,
    pub dirty: bool,
}

impl App {
    pub fn new(config: Config, i18n: I18n) -> Self {
        let keymap = Keymap::with_overrides(&config.keys);
        let volumes = Volumes {
            local: config.volume_for(Source::Local),
            remote: config.volume_for(Source::Youtube),
        };
        let mut statuses = HashMap::new();
        for source in Source::ALL {
            statuses.insert(source, SourceStatus::Ready);
        }
        let mut app = Self {
            config,
            i18n,
            keymap,
            queue: Queue::new(),
            library: Library::default(),
            local: LocalBrowser::default(),
            spotify: SpotifyBrowser::default(),
            collections: Vec::new(),
            statuses,
            focus: Panel::Explorer,
            tab: Source::Local,
            playback: Playback::default(),
            ui: UiState::default(),
            art: ArtState::default(),
            art_renderer: crate::art::ArtRenderer::halfblocks(),
            commands: Vec::new(),
            quit: false,
            pending_play: None,
            config_dir: None,
            dirty: false,
        };
        app.playback.volumes = volumes;
        app
    }

    pub fn with_library(mut self, library: Library) -> Self {
        self.library = library;
        self
    }

    pub fn with_collections(mut self, collections: Vec<Collection>) -> Self {
        self.collections = collections;
        self
    }

    pub fn with_status(mut self, source: Source, status: SourceStatus) -> Self {
        self.statuses.insert(source, status);
        self
    }

    pub fn with_art_renderer(mut self, renderer: crate::art::ArtRenderer) -> Self {
        self.art_renderer = renderer;
        self
    }

    pub fn with_restored(mut self, state: StateFile) -> Self {
        self.queue.restore(state.queue.clone(), state.position);
        self.queue.history = state.history_deque();
        while self.queue.history.len() > crate::queue::HISTORY_LIMIT {
            self.queue.history.pop_back();
        }
        self.playback.volumes = state.volumes.clone();
        self.config
            .set_volume_for(Source::Local, state.volumes.local);
        self.config
            .set_volume_for(Source::Youtube, state.volumes.remote);
        if let Some(position) = state.position
            && let Some(entry) = self.queue.entries.get(position)
        {
            self.playback.entry_id = Some(entry.id);
            self.playback.source = Some(entry.source);
            self.playback.title = Some(entry.meta.title.clone());
            self.playback.duration = entry.meta.duration;
            self.playback.state = PlayerState::Paused;
            self.playback.loaded = false;
        }
        if let Some(tab) = state.tab {
            self.tab = tab;
        }
        self
    }

    pub fn state(&self) -> StateFile {
        StateFile {
            format: crate::persist::state::STATE_FORMAT,
            queue: self.queue.entries.clone(),
            position: self.queue.position,
            history: self.queue.history.iter().cloned().collect(),
            volumes: Volumes {
                local: self.config.volume_for(Source::Local),
                remote: self.config.volume_for(Source::Youtube),
            },
            tab: Some(self.tab),
        }
    }

    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_available(&self, source: Source) -> bool {
        self.statuses
            .get(&source)
            .map(SourceStatus::is_ready)
            .unwrap_or(false)
    }

    pub fn reason_for(&self, source: Source) -> Option<&Reason> {
        self.statuses.get(&source).and_then(SourceStatus::reason)
    }

    pub fn push_notice(&mut self, notice: impl Into<String>) {
        self.ui.notice = Some(notice.into());
    }

    pub fn warning(&mut self, label_key: &str, detail: &str) {
        self.ui.notice = Some(format!("{}: {detail}", self.i18n.t(label_key)));
    }

    pub fn handle(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Tick => {}
            AppEvent::Status { source, status } => self.on_status(source, status),
            AppEvent::LibraryLoaded(library) => {
                self.library = library;
                self.clamp_selections();
            }
            AppEvent::PlaybackStarted { entry_id } => self.on_playback_started(entry_id),
            AppEvent::PlaybackProgress {
                state,
                position,
                duration,
            } => {
                self.playback.state = state;
                self.playback.position = position;
                if duration.is_some() {
                    self.playback.duration = duration;
                }
            }
            AppEvent::PlaybackStopped => {
                self.playback.state = PlayerState::Paused;
            }
            AppEvent::TrackEnded => self.on_track_ended(),
            AppEvent::EntryFailed { entry_id, reason } => self.on_entry_failed(entry_id, reason),
            AppEvent::FirstAudio { entry_id, elapsed } => {
                if self.playback.entry_id == Some(entry_id) {
                    self.playback.first_audio = Some(elapsed);
                }
            }
            AppEvent::ImportProgress {
                name,
                matched,
                total,
            } => {
                self.ui.import_status = Some(ImportStatus {
                    name: name.clone(),
                    matched,
                    total,
                });
                self.push_notice(format!(
                    "{} {name} — {matched} / {total}",
                    self.i18n.t("msg_import_progress")
                ));
            }
            AppEvent::ImportFinished { collection } => {
                self.merge_collection(collection);
                self.ui.import_status = None;
                self.dirty = true;
            }
            AppEvent::ImportFailed { error } => {
                self.ui.import_status = None;
                let message = error.message();
                self.push_notice(format!("{}: {message}", self.i18n.t("msg_import_failed")));
            }
            AppEvent::ArtReady { entry_id, image } => {
                if self.playback.entry_id == Some(entry_id) {
                    match image {
                        Some(image) => self.art.set_image(
                            entry_id,
                            image,
                            crate::art::ArtKey {
                                source: self.playback.source.unwrap_or(Source::Local),
                                album: self
                                    .queue
                                    .entry(entry_id)
                                    .and_then(|entry| entry.meta.album.clone()),
                                path: entry_id.0.to_string(),
                            },
                        ),
                        None => {
                            let title = self
                                .queue
                                .entry(entry_id)
                                .map(|entry| entry.meta.title.clone())
                                .unwrap_or_default();
                            self.art.set_placeholder(entry_id, title);
                        }
                    }
                }
            }
            AppEvent::Message(message) => self.push_notice(message),
            AppEvent::Quit => self.quit = true,
        }
    }

    fn on_status(&mut self, source: Source, status: SourceStatus) {
        if let Some(reason) = status.reason() {
            let message = self.reason_message(reason);
            self.push_notice(message);
        }
        self.statuses.insert(source, status);
    }

    pub fn reason_message(&self, reason: &Reason) -> String {
        let base = self.i18n.t(reason.key);
        match &reason.detail {
            Some(detail) => format!("{base}: {detail}"),
            None => base,
        }
    }

    fn on_playback_started(&mut self, entry_id: EntryId) {
        self.pending_play = None;
        for entry in &mut self.queue.entries {
            if entry.state == EntryState::Playing && entry.id != entry_id {
                entry.state = EntryState::Ready;
            }
        }
        let Some(index) = self.queue.index_of(entry_id) else {
            return;
        };
        self.queue.set_position(index);
        let entry = self.queue.entries[index].clone();
        {
            let current = self.queue.entry_mut(entry_id).unwrap();
            current.state = EntryState::Playing;
        }
        self.queue
            .record_history(&entry, crate::persist::state::unix_now());
        self.playback.state = PlayerState::Playing;
        self.playback.entry_id = Some(entry_id);
        self.playback.source = Some(entry.source);
        self.playback.title = Some(entry.meta.title.clone());
        self.playback.duration = entry.meta.duration;
        self.playback.position = Duration::ZERO;
        self.playback.first_audio = None;
        self.playback.loaded = true;
        self.art.set_placeholder(entry_id, entry.meta.title.clone());
        self.commands.push(Command::FetchArt { entry });
        self.dirty = true;
    }

    fn on_track_ended(&mut self) {
        if let Some(entry) = self.queue.current_mut()
            && entry.state == EntryState::Playing
        {
            entry.state = EntryState::Ready;
        }
        self.playback.position = self.playback.duration.unwrap_or_default();
        if let Some(next) = self.queue.next_position() {
            self.play_index(next);
        } else {
            self.playback.state = PlayerState::Stopped;
            self.playback.entry_id = None;
            self.playback.title = None;
            self.art.clear();
        }
    }

    fn on_entry_failed(&mut self, entry_id: EntryId, reason: String) {
        let title = self
            .queue
            .entry(entry_id)
            .map(|entry| entry.meta.title.clone())
            .unwrap_or_default();
        self.queue
            .mark_state(entry_id, EntryState::Failed(reason.clone()));
        self.push_notice(format!(
            "{}: {title} ({reason})",
            self.i18n.t("msg_track_failed")
        ));
        if self.queue.current().map(|entry| entry.id) == Some(entry_id) {
            if let Some(next) = self.queue.next_position() {
                self.play_index(next);
            } else {
                self.playback.state = PlayerState::Paused;
            }
        }
        self.dirty = true;
    }

    fn merge_collection(&mut self, collection: Collection) {
        let matched = collection.matched();
        let unmatched = collection.unmatched();
        let name = collection.name.clone();
        if let Some(existing) = self
            .collections
            .iter_mut()
            .find(|existing| existing.id == collection.id)
        {
            *existing = collection;
        } else {
            self.collections.push(collection);
        }
        self.collections
            .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        self.push_notice(format!(
            "{}: {name} — {matched} {} / {unmatched} {}",
            self.i18n.t("msg_import_done"),
            self.i18n.t("msg_import_matched"),
            self.i18n.t("msg_import_unmatched")
        ));
    }

    pub fn local_rows(&self) -> Vec<LocalRow> {
        if let Some(songs) = &self.local.songs {
            let songs = match songs {
                LocalSongs::Album { artist, name } => self.library.album_songs(artist, name),
                LocalSongs::Folder(folder) => self.library.folder_songs(folder),
                LocalSongs::Playlist(name) => self.library.playlist_songs(name),
                LocalSongs::Artist(artist) => self
                    .library
                    .artist_albums(artist)
                    .into_iter()
                    .flat_map(|album| self.library.album_songs(&album.artist, &album.name))
                    .collect(),
            };
            return songs.into_iter().map(LocalRow::Song).collect();
        }
        if let Some(artist) = &self.local.open_artist {
            return self
                .library
                .artist_albums(artist)
                .into_iter()
                .map(LocalRow::Album)
                .collect();
        }
        match self.local.view {
            LocalView::Artists => self
                .library
                .artists
                .iter()
                .cloned()
                .map(LocalRow::Artist)
                .collect(),
            LocalView::Albums => self
                .library
                .albums
                .iter()
                .cloned()
                .map(LocalRow::Album)
                .collect(),
            LocalView::Folders => self
                .library
                .folders
                .iter()
                .cloned()
                .map(LocalRow::Folder)
                .collect(),
            LocalView::Playlists => self
                .library
                .playlists
                .iter()
                .cloned()
                .map(LocalRow::Playlist)
                .collect(),
        }
    }

    pub fn filtered_local_rows(&self) -> Vec<LocalRow> {
        let rows = self.local_rows();
        match self.query() {
            Some(query) if !query.is_empty() => {
                let needle = query.to_lowercase();
                rows.into_iter()
                    .filter(|row| {
                        row.label().to_lowercase().contains(&needle)
                            || row.detail().to_lowercase().contains(&needle)
                    })
                    .collect()
            }
            _ => rows,
        }
    }

    pub fn visible_queue_indices(&self) -> Vec<usize> {
        match self.query() {
            Some(query) => self.queue.filtered_indices(query),
            None => (0..self.queue.len()).collect(),
        }
    }

    pub fn spotify_rows(&self) -> Vec<SpotifyRow> {
        if let Some(index) = self.spotify.open_collection
            && let Some(collection) = self.collections.get(index)
        {
            return collection
                .tracks
                .iter()
                .cloned()
                .map(SpotifyRow::Track)
                .collect();
        }
        self.collections
            .iter()
            .cloned()
            .map(SpotifyRow::Collection)
            .collect()
    }

    pub fn query(&self) -> Option<&String> {
        self.ui.search.as_ref().filter(|query| !query.is_empty())
    }

    pub fn clamp_selections(&mut self) {
        let local_len = self.filtered_local_rows().len();
        if local_len == 0 {
            self.local.selection = 0;
        } else {
            self.local.selection = self.local.selection.min(local_len - 1);
        }
        let visible = self.visible_queue_indices();
        if visible.is_empty() {
            self.queue.selection = 0;
        } else if !visible.contains(&self.queue.selection) {
            self.queue.selection = visible[0];
        }
        let spotify_len = self.spotify_rows().len();
        if spotify_len == 0 {
            self.spotify.selection = 0;
        } else {
            self.spotify.selection = self.spotify.selection.min(spotify_len - 1);
        }
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    pub fn selected_local_row(&self) -> Option<LocalRow> {
        self.filtered_local_rows()
            .get(self.local.selection)
            .cloned()
    }

    pub fn selected_spotify_row(&self) -> Option<SpotifyRow> {
        self.spotify_rows().get(self.spotify.selection).cloned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpotifyRow {
    Collection(Collection),
    Track(crate::process::ImportedTrack),
}
