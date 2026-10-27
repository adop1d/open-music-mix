//! MPD protocol client. The TUI never decodes local audio and never clears the
//! daemon queue; it only appends and starts.

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use mpd::{Client, Id, Song};

use super::{AlbumInfo, MpdBackend, PlaybackStatus, PlayerState, SongInfo, SourceError};

/// The crate only accepts `Song` or its private object-safe path trait, so a
/// bare URI is wrapped in a minimal song record.
fn uri_song(uri: &str) -> Song {
    Song {
        file: uri.to_string(),
        ..Default::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct MpdSettings {
    pub host: String,
    pub port: u16,
    pub socket: Option<PathBuf>,
    pub password: Option<String>,
}

enum Conn {
    Tcp(Client<TcpStream>),
    #[cfg(unix)]
    Unix(Client<UnixStream>),
}

pub struct RealMpd {
    settings: MpdSettings,
    conn: Option<Conn>,
    backoff: Duration,
    last_attempt: Option<Instant>,
}

impl RealMpd {
    pub fn new(settings: MpdSettings) -> Self {
        Self {
            settings,
            conn: None,
            backoff: Duration::ZERO,
            last_attempt: None,
        }
    }

    fn delay_elapsed(&self) -> bool {
        self.last_attempt
            .map(|last| last.elapsed() >= self.backoff)
            .unwrap_or(true)
    }

    fn open(&self) -> Result<Conn, SourceError> {
        if let Some(socket) = &self.settings.socket {
            #[cfg(unix)]
            {
                let stream = UnixStream::connect(socket).map_err(|error| {
                    SourceError::Unreachable(format!("{}: {error}", socket.display()))
                })?;
                let mut client =
                    Client::new(stream).map_err(|error| SourceError::Failed(error.to_string()))?;
                if let Some(password) = &self.settings.password {
                    client
                        .login(password)
                        .map_err(|error| SourceError::Failed(error.to_string()))?;
                }
                return Ok(Conn::Unix(client));
            }
            #[cfg(not(unix))]
            return Err(SourceError::Unreachable(
                "unix sockets are not supported on this platform".to_string(),
            ));
        }

        let host = if self.settings.host.is_empty() {
            "127.0.0.1"
        } else {
            &self.settings.host
        };
        let port = if self.settings.port == 0 {
            6600
        } else {
            self.settings.port
        };
        let address = (host, port)
            .to_socket_addrs()
            .map_err(|error| SourceError::Unreachable(error.to_string()))?
            .next()
            .ok_or_else(|| SourceError::Unreachable("no address".to_string()))?;
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
            .map_err(|error| SourceError::Unreachable(error.to_string()))?;
        let mut client =
            Client::new(stream).map_err(|error| SourceError::Failed(error.to_string()))?;
        if let Some(password) = &self.settings.password {
            client
                .login(password)
                .map_err(|error| SourceError::Failed(error.to_string()))?;
        }
        Ok(Conn::Tcp(client))
    }

    fn ensure(&mut self) -> Result<&mut Conn, SourceError> {
        if self.conn.is_none() && self.delay_elapsed() {
            self.last_attempt = Some(Instant::now());
            match self.open() {
                Ok(conn) => {
                    self.conn = Some(conn);
                    self.backoff = Duration::ZERO;
                }
                Err(error) => {
                    self.backoff = if self.backoff.is_zero() {
                        Duration::from_millis(500)
                    } else {
                        (self.backoff * 2).min(Duration::from_secs(30))
                    };
                    return Err(error);
                }
            }
        }
        self.conn.as_mut().ok_or(SourceError::Disconnected)
    }

    fn failed<T>(&mut self, error: mpd::error::Error) -> Result<T, SourceError> {
        let disconnected = matches!(error, mpd::error::Error::Io(_));
        if disconnected {
            self.conn = None;
        }
        Err(if disconnected {
            SourceError::Disconnected
        } else {
            SourceError::Failed(error.to_string())
        })
    }

    fn songs(&mut self) -> Result<Vec<Song>, SourceError> {
        // MPD's `listall` returns bare paths without tags, and this crate has no
        // `listallinfo`, so walk directories with `lsinfo` to keep metadata.
        let mut songs = Vec::new();
        let mut pending = vec![String::new()];
        while let Some(dir) = pending.pop() {
            let conn = self.ensure()?;
            let result = match conn {
                Conn::Tcp(client) => client.lsinfo(&uri_song(&dir)),
                #[cfg(unix)]
                Conn::Unix(client) => client.lsinfo(&uri_song(&dir)),
            };
            let entries = match result {
                Ok(entries) => entries,
                Err(error) => return self.failed(error),
            };
            for entry in entries {
                let subdirs: Vec<&String> = entry
                    .tags
                    .iter()
                    .filter(|(key, _)| key == "directory")
                    .map(|(_, value)| value)
                    .collect();
                if subdirs.is_empty() {
                    if !entry.file.is_empty() {
                        songs.push(entry);
                    }
                } else {
                    for name in subdirs {
                        pending.push(if dir.is_empty() {
                            name.clone()
                        } else {
                            format!("{dir}/{name}")
                        });
                    }
                }
            }
        }
        Ok(songs)
    }
}

fn tag(song: &Song, name: &str) -> Option<String> {
    song.tags
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn song_info(song: Song) -> SongInfo {
    let album = tag(&song, "Album");
    let artist = song
        .artist
        .clone()
        .or_else(|| tag(&song, "AlbumArtist"))
        .unwrap_or_default();
    let title = song
        .title
        .clone()
        .or_else(|| song.name.clone())
        .unwrap_or_else(|| song.file.clone());
    SongInfo {
        title,
        artist,
        album,
        uri: song.file,
        duration: song.duration,
    }
}

impl MpdBackend for RealMpd {
    fn connect(&mut self) -> Result<(), SourceError> {
        self.ensure().map(|_| ())
    }

    fn artists(&mut self) -> Result<Vec<String>, SourceError> {
        let mut names: Vec<String> = self
            .songs()?
            .iter()
            .map(|song| {
                song.artist
                    .clone()
                    .or_else(|| tag(song, "AlbumArtist"))
                    .unwrap_or_default()
            })
            .filter(|artist| !artist.is_empty())
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        names.dedup();
        Ok(names)
    }

    fn albums(&mut self) -> Result<Vec<AlbumInfo>, SourceError> {
        let mut albums: Vec<AlbumInfo> = self
            .songs()?
            .iter()
            .map(|song| AlbumInfo {
                artist: song
                    .artist
                    .clone()
                    .or_else(|| tag(song, "AlbumArtist"))
                    .unwrap_or_default(),
                name: tag(song, "Album").unwrap_or_default(),
            })
            .filter(|album| !album.name.is_empty())
            .collect();
        albums.sort_by_key(|album| (album.artist.to_lowercase(), album.name.to_lowercase()));
        albums.dedup();
        Ok(albums)
    }

    fn folders(&mut self) -> Result<Vec<String>, SourceError> {
        let mut folders: Vec<String> = self
            .songs()?
            .iter()
            .filter_map(|song| song.file.split('/').next().map(str::to_string))
            .filter(|folder| !folder.is_empty())
            .collect();
        folders.sort_by_key(|folder| folder.to_lowercase());
        folders.dedup();
        Ok(folders)
    }

    fn playlists(&mut self) -> Result<Vec<String>, SourceError> {
        let conn = self.ensure()?;
        let result = match conn {
            Conn::Tcp(client) => client.playlists(),
            #[cfg(unix)]
            Conn::Unix(client) => client.playlists(),
        };
        match result {
            Ok(playlists) => Ok(playlists
                .into_iter()
                .map(|playlist| playlist.name)
                .collect()),
            Err(error) => self.failed(error),
        }
    }

    fn artist_albums(&mut self, artist: &str) -> Result<Vec<AlbumInfo>, SourceError> {
        Ok(self
            .albums()?
            .into_iter()
            .filter(|album| album.artist.eq_ignore_ascii_case(artist))
            .collect())
    }

    fn album_songs(&mut self, artist: &str, album: &str) -> Result<Vec<SongInfo>, SourceError> {
        Ok(self
            .songs()?
            .into_iter()
            .map(song_info)
            .filter(|song| {
                song.artist.eq_ignore_ascii_case(artist)
                    && song.album.as_deref().unwrap_or_default() == album
            })
            .collect())
    }

    fn playlist_songs(&mut self, name: &str) -> Result<Vec<SongInfo>, SourceError> {
        let conn = self.ensure()?;
        let result = match conn {
            Conn::Tcp(client) => client.playlist(name),
            #[cfg(unix)]
            Conn::Unix(client) => client.playlist(name),
        };
        match result {
            Ok(songs) => Ok(songs.into_iter().map(song_info).collect()),
            Err(error) => self.failed(error),
        }
    }

    fn folder_songs(&mut self, folder: &str) -> Result<Vec<SongInfo>, SourceError> {
        Ok(self
            .songs()?
            .into_iter()
            .filter(|song| song.file.starts_with(folder))
            .map(song_info)
            .collect())
    }

    fn enqueue_and_play(&mut self, uris: &[String]) -> Result<(), SourceError> {
        if uris.is_empty() {
            return Ok(());
        }
        let conn = self.ensure()?;
        let result = (|| -> Result<(), mpd::error::Error> {
            let mut ids: Vec<Id> = Vec::with_capacity(uris.len());
            for uri in uris {
                let song = uri_song(uri);
                ids.push(match conn {
                    Conn::Tcp(client) => client.push(&song)?,
                    #[cfg(unix)]
                    Conn::Unix(client) => client.push(&song)?,
                });
            }
            let first = ids[0];
            match conn {
                Conn::Tcp(client) => client.switch(first),
                #[cfg(unix)]
                Conn::Unix(client) => client.switch(first),
            }
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        }
    }

    fn pause(&mut self, paused: bool) -> Result<(), SourceError> {
        let conn = self.ensure()?;
        let result = match conn {
            Conn::Tcp(client) => client.pause(paused),
            #[cfg(unix)]
            Conn::Unix(client) => client.pause(paused),
        };
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        }
    }

    fn stop(&mut self) -> Result<(), SourceError> {
        let conn = self.ensure()?;
        let result = match conn {
            Conn::Tcp(client) => client.stop(),
            #[cfg(unix)]
            Conn::Unix(client) => client.stop(),
        };
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        }
    }

    fn seek(&mut self, position: Duration) -> Result<(), SourceError> {
        let conn = self.ensure()?;
        let seconds = position;
        let result = match conn {
            Conn::Tcp(client) => client.rewind(seconds),
            #[cfg(unix)]
            Conn::Unix(client) => client.rewind(seconds),
        };
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        }
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), SourceError> {
        let conn = self.ensure()?;
        let result = match conn {
            Conn::Tcp(client) => client.volume(volume.min(100) as i8),
            #[cfg(unix)]
            Conn::Unix(client) => client.volume(volume.min(100) as i8),
        };
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        }
    }

    fn status(&mut self) -> Result<PlaybackStatus, SourceError> {
        let conn = self.ensure()?;
        let result = match conn {
            Conn::Tcp(client) => client.status(),
            #[cfg(unix)]
            Conn::Unix(client) => client.status(),
        };
        match result {
            Ok(status) => Ok(PlaybackStatus {
                state: match status.state {
                    mpd::State::Play => PlayerState::Playing,
                    mpd::State::Pause => PlayerState::Paused,
                    mpd::State::Stop => PlayerState::Stopped,
                },
                position: status.elapsed.unwrap_or_default(),
                duration: status.duration,
                volume: u8::try_from(status.volume).ok(),
            }),
            Err(error) => self.failed(error),
        }
    }

    fn music_directory(&mut self) -> Option<PathBuf> {
        let conn = self.ensure().ok()?;
        match conn {
            Conn::Tcp(client) => client.music_directory().ok().map(PathBuf::from),
            #[cfg(unix)]
            Conn::Unix(client) => client.music_directory().ok().map(PathBuf::from),
        }
    }

    fn album_art(&mut self, uri: &str) -> Result<Option<Vec<u8>>, SourceError> {
        let conn = self.ensure()?;
        let song = uri_song(uri);
        let result = match conn {
            Conn::Tcp(client) => client.albumart(&song),
            #[cfg(unix)]
            Conn::Unix(client) => client.albumart(&song),
        };
        match result {
            Ok(bytes) if bytes.is_empty() => Ok(None),
            Ok(bytes) => Ok(Some(bytes)),
            Err(mpd::error::Error::Io(ref error))
                if error.kind() == io::ErrorKind::UnexpectedEof =>
            {
                Ok(None)
            }
            Err(mpd::error::Error::Proto(_)) => Ok(None),
            Err(error) => self.failed(error),
        }
    }
}
