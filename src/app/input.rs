//! Key handling and selection actions.

use std::time::Duration;

use crate::event::Command;
use crate::process::{ImportedTrack, PlayerState, SongInfo};
use crate::queue::{EntryId, EntryState, Locator, Source, TrackMeta};
use crate::ui::keymap::{Action, Key};

use super::{App, LocalRow, LocalSongs, Panel, SpotifyRow};

impl App {
    pub fn handle_key(&mut self, key: Key) {
        if self.ui.help_open {
            let toggles = self.keymap.action_for(key) == Some(Action::Help);
            if toggles || key == Key::Esc {
                self.ui.help_open = false;
            }
            return;
        }
        if self.ui.import_open {
            self.handle_import_key(key);
            return;
        }
        if self.ui.search_active {
            self.handle_search_key(key);
            return;
        }
        let Some(action) = self.keymap.action_for(key) else {
            return;
        };
        self.dispatch(action);
    }

    fn handle_import_key(&mut self, key: Key) {
        match key {
            Key::Enter => {
                let text = self.ui.import_input.trim().to_string();
                self.ui.import_open = false;
                if !text.is_empty() {
                    self.submit_import(&text);
                }
            }
            Key::Esc => self.ui.import_open = false,
            Key::Backspace => {
                self.ui.import_input.pop();
            }
            Key::Char(c) => self.ui.import_input.push(c),
            Key::Space => self.ui.import_input.push(' '),
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: Key) {
        match key {
            Key::Esc => self.clear_search(),
            Key::Backspace => {
                if let Some(query) = &mut self.ui.search {
                    query.pop();
                }
                self.clamp_selections();
            }
            Key::Enter => {
                if self.focus == Panel::Explorer && self.tab == Source::Youtube {
                    let query = self.ui.search.clone().unwrap_or_default();
                    self.ui.search_active = false;
                    self.play_youtube_input(&query);
                } else {
                    self.ui.search_active = false;
                }
            }
            Key::Char(c) => {
                self.ui.search.get_or_insert_with(String::new).push(c);
                self.clamp_selections();
            }
            Key::Space => {
                self.ui.search.get_or_insert_with(String::new).push(' ');
                self.clamp_selections();
            }
            _ => {}
        }
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::MoveDown => self.move_selection(1),
            Action::MoveUp => self.move_selection(-1),
            Action::Collapse => self.collapse(),
            Action::Expand => self.expand(),
            Action::CyclePanel => self.focus = self.focus.next(),
            Action::TabLocal => self.select_tab(Source::Local),
            Action::TabYoutube => self.select_tab(Source::Youtube),
            Action::TabSpotify => self.select_tab(Source::Spotify),
            Action::PlayPause => self.toggle_playback(),
            Action::PlayNow => self.play_selection(),
            Action::Append => self.append_selection(),
            Action::Remove => self.remove_selected(),
            Action::Clear => self.clear_queue(),
            Action::Next => self.next_entry(),
            Action::Previous => self.previous_entry(),
            Action::SeekBack => self.seek_by(-5),
            Action::SeekForward => self.seek_by(5),
            Action::VolumeUp => self.change_volume(1),
            Action::VolumeDown => self.change_volume(-1),
            Action::Retry => self.retry_selected(),
            Action::Import => self.open_import(),
            Action::Search => self.open_search(),
            Action::Help => self.ui.help_open = true,
            Action::Escape => {
                if self.ui.search.is_some() {
                    self.clear_search();
                } else {
                    self.ui.notice = None;
                }
            }
            Action::CycleView => self.cycle_view(),
            Action::Quit => {
                self.quit = true;
                self.commands.push(Command::Shutdown);
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        match self.focus {
            Panel::Queue => {
                let visible = self.visible_queue_indices();
                if visible.is_empty() {
                    return;
                }
                let current = visible
                    .iter()
                    .position(|index| *index == self.queue.selection)
                    .unwrap_or(0);
                let next = (current as isize + delta).clamp(0, visible.len() as isize - 1) as usize;
                self.queue.selection = visible[next];
            }
            Panel::Explorer => match self.tab {
                Source::Local => {
                    let len = self.filtered_local_rows().len();
                    self.local.selection = step(self.local.selection, delta, len);
                }
                Source::Spotify => {
                    let len = self.spotify_rows().len();
                    self.spotify.selection = step(self.spotify.selection, delta, len);
                }
                Source::Youtube => {}
            },
            Panel::NowPlaying => {}
        }
    }

    fn select_tab(&mut self, source: Source) {
        self.tab = source;
        self.focus = Panel::Explorer;
        self.clamp_selections();
    }

    fn open_search(&mut self) {
        self.ui.search_active = true;
        self.ui.search.get_or_insert_with(String::new);
    }

    pub fn clear_search(&mut self) {
        self.ui.search = None;
        self.ui.search_active = false;
        self.clamp_selections();
    }

    fn expand(&mut self) {
        match self.focus {
            Panel::Explorer => match self.tab {
                Source::Local => self.open_local_selection(),
                Source::Spotify => self.open_spotify_selection(),
                Source::Youtube => {}
            },
            _ => self.focus = self.focus.next(),
        }
    }

    fn collapse(&mut self) {
        match self.focus {
            Panel::Explorer => {
                if self.local.songs.is_some() {
                    self.local.songs = None;
                    self.local.selection = 0;
                } else if self.local.open_artist.is_some() {
                    self.local.open_artist = None;
                    self.local.selection = 0;
                } else if self.spotify.open_collection.is_some() {
                    self.spotify.open_collection = None;
                    self.spotify.selection = 0;
                } else {
                    self.focus = self.focus.previous();
                }
            }
            _ => self.focus = self.focus.previous(),
        }
    }

    fn open_local_selection(&mut self) {
        let Some(row) = self.selected_local_row() else {
            return;
        };
        self.local.open_artist = None;
        self.local.songs = Some(match row {
            LocalRow::Artist(artist) => {
                self.local.open_artist = Some(artist.clone());
                self.local.selection = 0;
                return;
            }
            LocalRow::Album(album) => LocalSongs::Album {
                artist: album.artist,
                name: album.name,
            },
            LocalRow::Folder(folder) => LocalSongs::Folder(folder),
            LocalRow::Playlist(playlist) => LocalSongs::Playlist(playlist),
            LocalRow::Song(_) => return,
        });
        self.local.selection = 0;
    }

    fn open_spotify_selection(&mut self) {
        if self.spotify.open_collection.is_some() {
            return;
        }
        if let Some(SpotifyRow::Collection(collection)) = self.selected_spotify_row()
            && let Some(index) = self
                .collections
                .iter()
                .position(|candidate| candidate.id == collection.id)
        {
            self.spotify.open_collection = Some(index);
            self.spotify.selection = 0;
        }
    }

    fn cycle_view(&mut self) {
        self.local.view = self.local.view.next();
        self.local.open_artist = None;
        self.local.songs = None;
        self.local.selection = 0;
    }

    fn toggle_playback(&mut self) {
        match self.playback.state {
            PlayerState::Playing => {
                let source = self.playback.source.unwrap_or(self.tab);
                self.commands.push(Command::Pause { source });
                self.playback.state = PlayerState::Paused;
            }
            PlayerState::Paused => {
                if self.playback.loaded {
                    if let Some(source) = self.playback.source {
                        self.commands.push(Command::Resume { source });
                        self.playback.state = PlayerState::Playing;
                    }
                } else if let Some(index) = self.queue.position {
                    self.play_index(index);
                } else {
                    self.play_selection();
                }
            }
            PlayerState::Stopped => self.play_selection(),
        }
    }

    fn play_selection(&mut self) {
        if self.focus == Panel::Queue {
            if self.queue.entries.get(self.queue.selection).is_some() {
                self.play_index(self.queue.selection);
            }
            return;
        }
        match self.tab {
            Source::Local => {
                if !self.is_available(Source::Local) {
                    self.notice_for(Source::Local);
                    return;
                }
                self.play_local_selection();
            }
            Source::Youtube => {
                if !self.is_available(Source::Youtube) {
                    self.notice_for(Source::Youtube);
                    return;
                }
                let query = self.ui.search.clone().unwrap_or_default();
                self.play_youtube_input(&query);
            }
            Source::Spotify => {
                if !self.is_available(Source::Spotify) {
                    self.notice_for(Source::Spotify);
                    return;
                }
                self.play_spotify_selection();
            }
        }
    }

    fn notice_for(&mut self, source: Source) {
        if let Some(reason) = self.reason_for(source).cloned() {
            let message = self.reason_message(&reason);
            self.push_notice(message);
        }
    }

    fn play_local_selection(&mut self) {
        let Some(row) = self.selected_local_row() else {
            return;
        };
        let songs: Vec<SongInfo> = match row {
            LocalRow::Song(song) => vec![song],
            LocalRow::Album(album) => self.library.album_songs(&album.artist, &album.name),
            LocalRow::Folder(folder) => self.library.folder_songs(&folder),
            LocalRow::Playlist(playlist) => self.library.playlist_songs(&playlist),
            LocalRow::Artist(artist) => self
                .library
                .artist_albums(&artist)
                .into_iter()
                .flat_map(|album| self.library.album_songs(&album.artist, &album.name))
                .collect(),
        };
        self.play_local_songs(songs);
    }

    fn play_local_songs(&mut self, songs: Vec<SongInfo>) {
        if songs.is_empty() {
            self.push_notice(self.i18n.t("empty_library"));
            return;
        }
        let mut uris = Vec::with_capacity(songs.len());
        let mut first = None;
        for song in songs {
            let id = self.queue.append(
                Source::Local,
                song.meta(),
                Locator::MpdUri(song.uri.clone()),
            );
            if first.is_none() {
                first = Some(id);
            }
            uris.push(song.uri);
        }
        if let Some(id) = first {
            let index = self.queue.index_of(id).unwrap_or(0);
            self.queue.set_position(index);
            let entry = self.queue.entries[index].clone();
            self.begin_playback(&entry);
            self.commands.push(Command::PlayLocal {
                entry_id: id,
                uris,
                volume: self.config.volume_for(Source::Local),
            });
        }
    }

    fn append_selection(&mut self) {
        match self.focus {
            Panel::Queue => self.append_selected_queue_entry(),
            _ => match self.tab {
                Source::Local => {
                    let Some(row) = self.selected_local_row() else {
                        return;
                    };
                    let songs: Vec<SongInfo> = match row {
                        LocalRow::Song(song) => vec![song],
                        LocalRow::Album(album) => {
                            self.library.album_songs(&album.artist, &album.name)
                        }
                        LocalRow::Folder(folder) => self.library.folder_songs(&folder),
                        LocalRow::Playlist(playlist) => self.library.playlist_songs(&playlist),
                        LocalRow::Artist(artist) => self
                            .library
                            .artist_albums(&artist)
                            .into_iter()
                            .flat_map(|album| self.library.album_songs(&album.artist, &album.name))
                            .collect(),
                    };
                    for song in songs {
                        self.queue
                            .append(Source::Local, song.meta(), Locator::MpdUri(song.uri));
                    }
                    self.dirty = true;
                }
                Source::Spotify => {
                    if let Some(SpotifyRow::Track(track)) = self.selected_spotify_row() {
                        let _ = self.append_imported_track(track);
                    }
                }
                Source::Youtube => {
                    let query = self.ui.search.clone().unwrap_or_default();
                    if !query.trim().is_empty() {
                        let locator = youtube_locator(&query);
                        let meta = TrackMeta::new(query.clone(), "");
                        self.queue.append(Source::Youtube, meta, locator);
                        self.dirty = true;
                    }
                }
            },
        }
    }

    fn append_selected_queue_entry(&mut self) {
        let Some(entry) = self.queue.entries.get(self.queue.selection).cloned() else {
            return;
        };
        self.queue
            .append(entry.source, entry.meta.clone(), entry.locator.clone());
        self.dirty = true;
    }

    fn append_imported_track(&mut self, track: ImportedTrack) -> Option<EntryId> {
        if track.unavailable {
            self.push_notice(format!(
                "{}: {}",
                self.i18n.t("msg_no_results"),
                track.title
            ));
            return None;
        }
        let id = self.queue.append(
            Source::Spotify,
            imported_meta(&track),
            imported_locator(&track),
        );
        self.dirty = true;
        Some(id)
    }

    fn play_spotify_selection(&mut self) {
        match self.selected_spotify_row() {
            Some(SpotifyRow::Track(track)) => self.play_imported_track(track),
            Some(SpotifyRow::Collection(collection)) => {
                if let Some(index) = self
                    .collections
                    .iter()
                    .position(|candidate| candidate.id == collection.id)
                    && self.spotify.open_collection.is_none()
                {
                    self.spotify.open_collection = Some(index);
                    self.spotify.selection = 0;
                    return;
                }
                let mut first = None;
                for track in collection.tracks {
                    if let Some(id) = self.append_imported_track(track)
                        && first.is_none()
                    {
                        first = Some(id);
                    }
                }
                if let Some(index) = first.and_then(|id| self.queue.index_of(id)) {
                    self.play_index(index);
                }
            }
            None => {}
        }
    }

    fn play_imported_track(&mut self, track: ImportedTrack) {
        if track.unavailable {
            self.push_notice(format!(
                "{}: {}",
                self.i18n.t("msg_no_results"),
                track.title
            ));
            return;
        }
        let id = self.queue.append(
            Source::Spotify,
            imported_meta(&track),
            imported_locator(&track),
        );
        if let Some(index) = self.queue.index_of(id) {
            self.queue.set_position(index);
            self.play_index(index);
        }
    }

    fn play_youtube_input(&mut self, input: &str) {
        let input = input.trim();
        if input.is_empty() {
            return;
        }
        let locator = youtube_locator(input);
        let meta = TrackMeta::new(input.to_string(), "");
        let id = self.queue.append(Source::Youtube, meta, locator);
        let index = self.queue.index_of(id).unwrap_or(0);
        self.queue.set_position(index);
        self.play_index(index);
        self.clear_search();
    }

    pub fn play_index(&mut self, index: usize) {
        let Some(entry) = self.queue.entries.get(index).cloned() else {
            return;
        };
        self.queue.set_position(index);
        self.queue.selection = index;
        if let Some(current) = self.queue.entry_mut(entry.id)
            && current.state.is_failed()
        {
            current.state = EntryState::Pending;
        }
        self.begin_playback(&entry);
        match entry.source {
            Source::Local => {
                let uri = match &entry.locator {
                    Locator::MpdUri(uri) => uri.clone(),
                    Locator::LocalPath(path) => path.display().to_string(),
                    _ => return,
                };
                self.commands.push(Command::PlayLocal {
                    entry_id: entry.id,
                    uris: vec![uri],
                    volume: self.config.volume_for(Source::Local),
                });
            }
            Source::Youtube | Source::Spotify => {
                self.commands.push(Command::PlayRemote {
                    entry_id: entry.id,
                    source: entry.source,
                    locator: entry.locator.clone(),
                    volume: self.config.volume_for(entry.source),
                });
            }
        }
    }

    fn begin_playback(&mut self, entry: &crate::queue::Entry) {
        self.pending_play = Some(entry.id);
        self.playback.entry_id = Some(entry.id);
        self.playback.source = Some(entry.source);
        self.playback.title = Some(entry.meta.title.clone());
        self.playback.duration = entry.meta.duration;
        self.playback.position = Duration::ZERO;
        self.playback.state = PlayerState::Playing;
        self.playback.loaded = false;
        self.playback.first_audio = None;
        self.art.set_placeholder(entry.id, entry.meta.title.clone());
    }

    fn remove_selected(&mut self) {
        if self.queue.entries.is_empty() {
            return;
        }
        let index = self.queue.selection.min(self.queue.len() - 1);
        let outcome = self.queue.remove(index);
        if outcome.was_current {
            if let Some(next) = outcome.next_position {
                self.play_index(next);
            } else {
                self.stop_playback();
            }
        }
        self.clamp_selections();
        self.dirty = true;
    }

    fn clear_queue(&mut self) {
        let source = self
            .queue
            .current()
            .map(|entry| entry.source)
            .or(self.playback.source);
        self.queue.clear();
        if let Some(source) = source {
            self.commands.push(Command::Stop { source });
        }
        self.stop_playback();
        self.dirty = true;
    }

    fn stop_playback(&mut self) {
        self.playback.state = PlayerState::Stopped;
        self.playback.entry_id = None;
        self.playback.title = None;
        self.playback.source = None;
        self.playback.position = Duration::ZERO;
        self.playback.loaded = false;
        self.art.clear();
    }

    fn next_entry(&mut self) {
        if let Some(next) = self.queue.next_position() {
            self.play_index(next);
        }
    }

    fn previous_entry(&mut self) {
        if let Some(position) = self.queue.position {
            let target = position.saturating_sub(1);
            self.play_index(target);
        }
    }

    fn seek_by(&mut self, seconds: i64) {
        let Some(source) = self.playback.source else {
            return;
        };
        let current = self.playback.position.as_secs() as i64;
        let target = (current + seconds).max(0) as u64;
        if let Some(duration) = self.playback.duration {
            let limit = duration.as_secs();
            if target > limit {
                return;
            }
        }
        let position = Duration::from_secs(target);
        self.playback.position = position;
        self.commands.push(Command::Seek { source, position });
    }

    fn change_volume(&mut self, direction: i16) {
        let source = self.playback.source.unwrap_or(self.tab);
        let step = self.config.playback.volume_step;
        let current = self.config.volume_for(source) as i16;
        let volume = (current + direction * step).clamp(0, 100) as u8;
        self.config.set_volume_for(source, volume);
        match source {
            Source::Local => self.playback.volumes.local = volume,
            _ => self.playback.volumes.remote = volume,
        }
        self.commands.push(Command::Volume { source, volume });
        self.dirty = true;
    }

    fn retry_selected(&mut self) {
        let index = if self.focus == Panel::Queue {
            self.queue.selection
        } else {
            self.queue.position.unwrap_or(self.queue.selection)
        };
        let Some(entry) = self.queue.entries.get(index).cloned() else {
            return;
        };
        if !entry.state.is_failed() {
            return;
        }
        if let Some(current) = self.queue.entry_mut(entry.id) {
            current.state = EntryState::Pending;
        }
        self.play_index(index);
    }

    fn open_import(&mut self) {
        self.ui.import_open = true;
        self.ui.import_input.clear();
    }

    /// Returns false when the reference is invalid; the notice names accepted formats.
    pub fn submit_import(&mut self, input: &str) -> bool {
        match crate::process::parse_reference(input) {
            Ok(reference) => {
                self.ui.import_status = Some(super::ImportStatus {
                    name: input.to_string(),
                    matched: 0,
                    total: 0,
                });
                self.commands.push(Command::Import { reference });
                true
            }
            Err(error) => {
                self.push_notice(format!(
                    "{}: {}",
                    self.i18n.t("msg_import_invalid"),
                    error.message()
                ));
                false
            }
        }
    }
}

fn step(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current as isize + delta).clamp(0, len as isize - 1) as usize
    }
}

fn imported_meta(track: &ImportedTrack) -> TrackMeta {
    TrackMeta {
        title: track.title.clone(),
        artists: track.artists.clone(),
        album: track.album.clone(),
        duration: track.duration,
    }
}

fn imported_locator(track: &ImportedTrack) -> Locator {
    match &track.youtube_id {
        Some(id) => Locator::YoutubeId(id.clone()),
        None => Locator::SpotifyTrack(track.query.clone()),
    }
}

/// A watch URL or bare video ID resolves directly; anything else is a search.
pub fn youtube_locator(input: &str) -> Locator {
    let input = input.trim();
    if let Some(rest) = input.strip_prefix("https://youtu.be/") {
        let id = rest.split(['?', '&', '#']).next().unwrap_or(rest);
        if !id.is_empty() {
            return Locator::YoutubeId(id.to_string());
        }
    }
    for marker in ["v=", "/shorts/", "/embed/"] {
        if let Some(position) = input.find(marker) {
            let rest = &input[position + marker.len()..];
            let id = rest.split(['&', '?', '#', '/']).next().unwrap_or(rest);
            if !id.is_empty() {
                return Locator::YoutubeId(id.to_string());
            }
        }
    }
    if input.len() == 11
        && input
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Locator::YoutubeId(input.to_string());
    }
    Locator::YoutubeQuery(input.to_string())
}
