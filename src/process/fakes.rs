//! In-memory doubles for every process boundary. Shared by unit and integration tests.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use crate::queue::Locator;

use super::{
    AlbumInfo, Candidate, Collection, ImportError, MpdBackend, MpvBackend, PlaybackStatus,
    PlayerEvent, PlayerState, ResolveError, Resolved, Resolver, SongInfo, SourceError,
    SpotifyImporter, SpotifyRef,
};

#[derive(Debug, Default)]
pub struct FakeMpd {
    pub connected: bool,
    pub connect_error: Option<SourceError>,
    pub artists: Vec<String>,
    pub albums: Vec<AlbumInfo>,
    pub folders: Vec<String>,
    pub playlists: Vec<String>,
    pub songs: Vec<SongInfo>,
    pub daemon_queue: Vec<String>,
    pub play_calls: Vec<Vec<String>>,
    pub paused: Option<bool>,
    pub stopped: bool,
    pub seeks: Vec<Duration>,
    pub volume: u8,
    pub art: Option<Vec<u8>>,
    pub music_directory: Option<std::path::PathBuf>,
    pub disconnect_during_playback: bool,
}

impl FakeMpd {
    pub fn with_library(
        artists: Vec<String>,
        albums: Vec<AlbumInfo>,
        songs: Vec<SongInfo>,
    ) -> Self {
        Self {
            connected: true,
            artists,
            albums,
            songs,
            ..Self::default()
        }
    }

    pub fn with_art(mut self, art: Vec<u8>) -> Self {
        self.art = Some(art);
        self
    }

    fn connected(&self) -> Result<(), SourceError> {
        if self.connected {
            Ok(())
        } else {
            Err(SourceError::Disconnected)
        }
    }
}

impl MpdBackend for FakeMpd {
    fn connect(&mut self) -> Result<(), SourceError> {
        if let Some(error) = &self.connect_error {
            return Err(error.clone());
        }
        self.connected = true;
        Ok(())
    }

    fn artists(&mut self) -> Result<Vec<String>, SourceError> {
        self.connected()?;
        Ok(self.artists.clone())
    }

    fn albums(&mut self) -> Result<Vec<AlbumInfo>, SourceError> {
        self.connected()?;
        Ok(self.albums.clone())
    }

    fn folders(&mut self) -> Result<Vec<String>, SourceError> {
        self.connected()?;
        Ok(self.folders.clone())
    }

    fn playlists(&mut self) -> Result<Vec<String>, SourceError> {
        self.connected()?;
        Ok(self.playlists.clone())
    }

    fn artist_albums(&mut self, artist: &str) -> Result<Vec<AlbumInfo>, SourceError> {
        self.connected()?;
        Ok(self
            .albums
            .iter()
            .filter(|album| album.artist.eq_ignore_ascii_case(artist))
            .cloned()
            .collect())
    }

    fn album_songs(&mut self, artist: &str, album: &str) -> Result<Vec<SongInfo>, SourceError> {
        self.connected()?;
        Ok(self
            .songs
            .iter()
            .filter(|song| {
                (artist.is_empty() || song.artist.eq_ignore_ascii_case(artist))
                    && song.album.as_deref().unwrap_or_default() == album
            })
            .cloned()
            .collect())
    }

    fn playlist_songs(&mut self, _name: &str) -> Result<Vec<SongInfo>, SourceError> {
        self.connected()?;
        Ok(self.songs.clone())
    }

    fn folder_songs(&mut self, _folder: &str) -> Result<Vec<SongInfo>, SourceError> {
        self.connected()?;
        Ok(self.songs.clone())
    }

    fn enqueue_and_play(&mut self, uris: &[String]) -> Result<(), SourceError> {
        self.connected()?;
        self.play_calls.push(uris.to_vec());
        self.daemon_queue.extend(uris.iter().cloned());
        if self.disconnect_during_playback {
            self.connected = false;
            return Err(SourceError::Disconnected);
        }
        Ok(())
    }

    fn pause(&mut self, paused: bool) -> Result<(), SourceError> {
        self.connected()?;
        self.paused = Some(paused);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), SourceError> {
        self.connected()?;
        self.stopped = true;
        Ok(())
    }

    fn seek(&mut self, position: Duration) -> Result<(), SourceError> {
        self.connected()?;
        self.seeks.push(position);
        Ok(())
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), SourceError> {
        self.connected()?;
        self.volume = volume;
        Ok(())
    }

    fn status(&mut self) -> Result<PlaybackStatus, SourceError> {
        self.connected()?;
        Ok(PlaybackStatus {
            state: if self.paused == Some(true) {
                PlayerState::Paused
            } else if self.play_calls.is_empty() {
                PlayerState::Stopped
            } else {
                PlayerState::Playing
            },
            position: Duration::ZERO,
            duration: None,
            volume: Some(self.volume),
        })
    }

    fn album_art(&mut self, _uri: &str) -> Result<Option<Vec<u8>>, SourceError> {
        self.connected()?;
        Ok(self.art.clone())
    }

    fn music_directory(&mut self) -> Option<std::path::PathBuf> {
        self.music_directory.clone()
    }
}

#[derive(Debug)]
pub struct FakeMpv {
    pub alive: bool,
    pub spawn_error: Option<SourceError>,
    pub played: Vec<String>,
    pub paused: Option<bool>,
    pub seeks: Vec<Duration>,
    pub volumes: Vec<u8>,
    pub events: VecDeque<PlayerEvent>,
    pub crash_on_play: bool,
    pub spawns: usize,
    pub stopped: bool,
    pub thumbnail: Option<Vec<u8>>,
}

impl Default for FakeMpv {
    fn default() -> Self {
        Self {
            alive: true,
            spawn_error: None,
            played: Vec::new(),
            paused: None,
            seeks: Vec::new(),
            volumes: Vec::new(),
            events: VecDeque::new(),
            crash_on_play: false,
            spawns: 0,
            stopped: false,
            thumbnail: None,
        }
    }
}

impl MpvBackend for FakeMpv {
    fn spawn(&mut self) -> Result<(), SourceError> {
        if let Some(error) = &self.spawn_error {
            return Err(error.clone());
        }
        self.spawns += 1;
        self.alive = true;
        Ok(())
    }

    fn play(&mut self, url: &str) -> Result<(), SourceError> {
        if self.crash_on_play {
            self.alive = false;
            return Err(SourceError::Disconnected);
        }
        self.played.push(url.to_string());
        self.events.push_back(PlayerEvent::FirstAudio);
        Ok(())
    }

    fn pause(&mut self, paused: bool) -> Result<(), SourceError> {
        self.paused = Some(paused);
        Ok(())
    }

    fn seek(&mut self, position: Duration) -> Result<(), SourceError> {
        self.seeks.push(position);
        Ok(())
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), SourceError> {
        self.volumes.push(volume);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), SourceError> {
        self.stopped = true;
        Ok(())
    }

    fn next_event(&mut self, _timeout: Duration) -> Option<PlayerEvent> {
        self.events.pop_front()
    }

    fn is_alive(&mut self) -> bool {
        self.alive
    }

    fn shutdown(&mut self) {
        self.alive = false;
    }
}

#[derive(Clone)]
pub struct FakeResolver {
    url: String,
    candidate: Result<Candidate, ResolveError>,
    failure: Option<ResolveError>,
    latency: Duration,
    thumbnail: Option<Vec<u8>>,
    candidates: std::collections::HashMap<String, Candidate>,
    resolve_calls: std::sync::Arc<Mutex<Vec<Locator>>>,
    search_calls: std::sync::Arc<Mutex<Vec<String>>>,
}

impl Default for FakeResolver {
    fn default() -> Self {
        Self {
            url: "http://fake.invalid/stream".to_string(),
            candidate: Ok(Candidate {
                video_id: "fakevideo01".to_string(),
                duration: None,
            }),
            failure: None,
            latency: Duration::ZERO,
            thumbnail: None,
            candidates: std::collections::HashMap::new(),
            resolve_calls: std::sync::Arc::new(Mutex::new(Vec::new())),
            search_calls: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl FakeResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    pub fn failing(mut self, error: ResolveError) -> Self {
        self.failure = Some(error);
        self
    }

    pub fn with_candidate(
        mut self,
        video_id: impl Into<String>,
        duration: Option<Duration>,
    ) -> Self {
        self.candidate = Ok(Candidate {
            video_id: video_id.into(),
            duration,
        });
        self
    }

    pub fn with_candidate_error(mut self, error: ResolveError) -> Self {
        self.candidate = Err(error);
        self
    }

    pub fn with_thumbnail(mut self, bytes: Vec<u8>) -> Self {
        self.thumbnail = Some(bytes);
        self
    }

    pub fn with_candidate_for(
        mut self,
        query: impl Into<String>,
        video_id: impl Into<String>,
        duration: Option<Duration>,
    ) -> Self {
        self.candidates.insert(
            query.into(),
            Candidate {
                video_id: video_id.into(),
                duration,
            },
        );
        self
    }

    pub fn resolve_calls(&self) -> Vec<Locator> {
        self.resolve_calls.lock().unwrap().clone()
    }

    pub fn resolve_count(&self) -> usize {
        self.resolve_calls.lock().unwrap().len()
    }

    pub fn search_calls(&self) -> Vec<String> {
        self.search_calls.lock().unwrap().clone()
    }

    pub fn search_count(&self) -> usize {
        self.search_calls.lock().unwrap().len()
    }
}

impl Resolver for FakeResolver {
    fn resolve(&self, locator: &Locator) -> Result<Resolved, ResolveError> {
        self.resolve_calls.lock().unwrap().push(locator.clone());
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        Ok(Resolved {
            url: self.url.clone(),
            video_id: None,
            duration: None,
        })
    }

    fn search_candidate(&self, query: &str) -> Result<Candidate, ResolveError> {
        self.search_calls.lock().unwrap().push(query.to_string());
        match self.candidates.get(query) {
            Some(candidate) => Ok(candidate.clone()),
            None => self.candidate.clone(),
        }
    }

    fn latency(&self) -> Duration {
        self.latency
    }

    fn thumbnail(&self, _video_id: &str, _directory: &std::path::Path) -> Option<Vec<u8>> {
        self.thumbnail.clone()
    }
}

#[derive(Clone)]
pub struct FakeImporter {
    pub response: Result<Collection, ImportError>,
    pub delay: Duration,
    calls: std::sync::Arc<Mutex<Vec<SpotifyRef>>>,
}

impl FakeImporter {
    pub fn new(collection: Collection) -> Self {
        Self {
            response: Ok(collection),
            delay: Duration::ZERO,
            calls: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(error: ImportError) -> Self {
        Self {
            response: Err(error),
            delay: Duration::ZERO,
            calls: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn calls(&self) -> Vec<SpotifyRef> {
        self.calls.lock().unwrap().clone()
    }
}

impl SpotifyImporter for FakeImporter {
    fn fetch(&self, reference: &SpotifyRef) -> Result<Collection, ImportError> {
        self.calls.lock().unwrap().push(reference.clone());
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        self.response.clone()
    }
}
