//! Source adapters used by the worker and by tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::process::{AlbumInfo, MpdBackend, MpvBackend, PlaybackStatus, SongInfo, SourceError};

pub type SharedMpd = Arc<Mutex<dyn MpdBackend>>;
pub type SharedMpv = Arc<Mutex<dyn MpvBackend>>;

/// Snapshot of the local library loaded in one MPD round trip.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub artists: Vec<String>,
    pub albums: Vec<AlbumInfo>,
    pub folders: Vec<String>,
    pub playlists: Vec<String>,
    pub songs: Vec<SongInfo>,
}

impl Library {
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn artist_albums(&self, artist: &str) -> Vec<AlbumInfo> {
        self.albums
            .iter()
            .filter(|album| album.artist.eq_ignore_ascii_case(artist))
            .cloned()
            .collect()
    }

    pub fn album_songs(&self, artist: &str, album: &str) -> Vec<SongInfo> {
        self.songs
            .iter()
            .filter(|song| {
                (artist.is_empty() || song.artist.eq_ignore_ascii_case(artist))
                    && song.album.as_deref().unwrap_or_default() == album
            })
            .cloned()
            .collect()
    }

    pub fn folder_songs(&self, folder: &str) -> Vec<SongInfo> {
        self.songs
            .iter()
            .filter(|song| song.uri.starts_with(folder))
            .cloned()
            .collect()
    }

    pub fn playlist_songs(&self, _name: &str) -> Vec<SongInfo> {
        self.songs.clone()
    }
}

/// Local playback stays behind MPD: append, start, pause, stop. Never clears.
pub struct LocalAdapter {
    mpd: SharedMpd,
}

impl LocalAdapter {
    pub fn new(mpd: SharedMpd) -> Self {
        Self { mpd }
    }

    pub fn play(&self, uris: &[String]) -> Result<(), SourceError> {
        self.mpd.lock().unwrap().enqueue_and_play(uris)
    }

    pub fn pause(&self, paused: bool) -> Result<(), SourceError> {
        self.mpd.lock().unwrap().pause(paused)
    }

    pub fn stop(&self) -> Result<(), SourceError> {
        self.mpd.lock().unwrap().stop()
    }

    pub fn seek(&self, position: Duration) -> Result<(), SourceError> {
        self.mpd.lock().unwrap().seek(position)
    }

    pub fn set_volume(&self, volume: u8) -> Result<(), SourceError> {
        self.mpd.lock().unwrap().set_volume(volume)
    }

    pub fn status(&self) -> Result<PlaybackStatus, SourceError> {
        self.mpd.lock().unwrap().status()
    }

    pub fn library(&self) -> Result<Library, SourceError> {
        let mut mpd = self.mpd.lock().unwrap();
        let artists = mpd.artists()?;
        let albums = mpd.albums()?;
        let folders = mpd.folders()?;
        let playlists = mpd.playlists()?;
        let mut songs = Vec::new();
        // One pass per album would be N round trips; RealMpd caches the listing,
        // fakes ignore the arguments. Keep the snapshot behind one method.
        for album in &albums {
            songs.extend(mpd.album_songs(&album.artist, &album.name)?);
        }
        Ok(Library {
            artists,
            albums,
            folders,
            playlists,
            songs,
        })
    }
}

/// Remote playback through one long-lived mpv process.
pub struct RemoteAdapter {
    mpv: SharedMpv,
}

impl RemoteAdapter {
    pub fn new(mpv: SharedMpv) -> Self {
        Self { mpv }
    }

    pub fn play(&self, url: &str) -> Result<(), SourceError> {
        self.mpv.lock().unwrap().play(url)
    }

    pub fn pause(&self, paused: bool) -> Result<(), SourceError> {
        self.mpv.lock().unwrap().pause(paused)
    }

    pub fn seek(&self, position: Duration) -> Result<(), SourceError> {
        self.mpv.lock().unwrap().seek(position)
    }

    pub fn set_volume(&self, volume: u8) -> Result<(), SourceError> {
        self.mpv.lock().unwrap().set_volume(volume)
    }

    pub fn stop(&self) -> Result<(), SourceError> {
        self.mpv.lock().unwrap().stop()
    }

    pub fn next_event(&self, timeout: Duration) -> Option<crate::process::PlayerEvent> {
        self.mpv.lock().unwrap().next_event(timeout)
    }
}
