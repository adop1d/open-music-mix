//! Worker: executes commands against backends and reports events back.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::backends::{Backends, TempWorkspace, find_binary};
use crate::event::{AppEvent, Command, Reason, SourceStatus};
use crate::process::{
    Candidate, Collection, ImportError, PlayerEvent, PlayerState, ResolveError, Resolver,
    SourceError, SpotifyImporter, SpotifyRef,
};
use crate::queue::{Entry, Source};
use crate::sources::{LocalAdapter, RemoteAdapter, SharedMpd};

const FIRST_AUDIO_BUDGET: Duration = Duration::from_secs(2);
const IMPORT_CONCURRENCY: usize = 4;

pub struct Worker {
    pub backends: Backends,
    pub tx: UnboundedSender<AppEvent>,
    pub config_dir: std::path::PathBuf,
    monitor: Option<tokio::task::JoinHandle<()>>,
}

impl Worker {
    pub fn new(
        backends: Backends,
        tx: UnboundedSender<AppEvent>,
        config_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            backends,
            tx,
            config_dir,
            monitor: None,
        }
    }

    pub async fn run(mut self, mut rx: UnboundedReceiver<Command>) {
        while let Some(command) = rx.recv().await {
            self.handle_command(command).await;
        }
        self.cleanup();
    }

    pub fn cleanup(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            monitor.abort();
        }
        self.backends.mpv.lock().unwrap().shutdown();
        self.backends.workspace.clear_all();
    }

    /// Source availability without touching the network beyond one connect attempt.
    pub fn probe_events(&self, find: &dyn Fn(&str) -> bool) -> Vec<AppEvent> {
        let mut events = Vec::new();
        if find("mpd") {
            let result = self.backends.mpd.lock().unwrap().connect();
            if let Err(error) = result {
                events.push(AppEvent::Status {
                    source: Source::Local,
                    status: SourceStatus::Unavailable(Reason::with_detail(
                        "msg_mpd_unavailable",
                        error.message(),
                    )),
                });
            }
        } else {
            events.push(AppEvent::Status {
                source: Source::Local,
                status: SourceStatus::Unavailable(Reason::new("msg_mpd_unavailable")),
            });
        }

        if !find("yt-dlp") {
            for source in [Source::Youtube, Source::Spotify] {
                events.push(AppEvent::Status {
                    source,
                    status: SourceStatus::Unavailable(Reason::new("msg_ytdlp_missing")),
                });
            }
        } else if !find("mpv") {
            for source in [Source::Youtube, Source::Spotify] {
                events.push(AppEvent::Status {
                    source,
                    status: SourceStatus::Unavailable(Reason::new("msg_mpv_missing")),
                });
            }
        } else if let Err(error) = self.backends.spawn_player() {
            for source in [Source::Youtube, Source::Spotify] {
                events.push(AppEvent::Status {
                    source,
                    status: SourceStatus::Unavailable(Reason::with_detail(
                        "msg_player_failed",
                        error.message(),
                    )),
                });
            }
        }
        events
    }

    pub async fn handle_command(&mut self, command: Command) {
        match command {
            Command::PlayLocal {
                entry_id,
                uris,
                volume,
            } => self.play_local(entry_id, uris, volume).await,
            Command::PlayRemote {
                entry_id,
                source,
                locator,
                volume,
            } => self.play_remote(entry_id, source, locator, volume).await,
            Command::Pause { source } => self.pause(source).await,
            Command::Resume { source } => self.resume(source).await,
            Command::Stop { source } => self.stop(source).await,
            Command::Seek { source, position } => self.seek(source, position).await,
            Command::Volume { source, volume } => self.volume(source, volume).await,
            Command::FetchArt { entry } => self.fetch_art(entry).await,
            Command::LoadLibrary => self.load_library().await,
            Command::ProbeSources => {
                let events = self.probe_events(&|name| find_binary(name).is_some());
                for event in events {
                    let _ = self.tx.send(event);
                }
            }
            Command::Import { reference } => self.import(reference).await,
            Command::Shutdown => {
                self.cleanup();
            }
        }
    }

    async fn play_local(&mut self, entry_id: crate::queue::EntryId, uris: Vec<String>, volume: u8) {
        let mpd = self.backends.mpd.clone();
        let result = tokio::task::spawn_blocking(move || {
            let adapter = LocalAdapter::new(mpd);
            // MPD outputs without a mixer reject volume; playback must still start.
            let _ = adapter.set_volume(volume);
            adapter.play(&uris)
        })
        .await
        .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())));

        self.backends.workspace.clear_audio();
        match result {
            Ok(()) => {
                let _ = self.tx.send(AppEvent::PlaybackStarted { entry_id });
            }
            Err(error) => {
                self.status_local(&error);
                let _ = self.tx.send(AppEvent::EntryFailed {
                    entry_id,
                    reason: error.message(),
                });
            }
        }
    }

    async fn play_remote(
        &mut self,
        entry_id: crate::queue::EntryId,
        _source: Source,
        locator: crate::queue::Locator,
        volume: u8,
    ) {
        let resolver = self.backends.resolver.clone();
        let target = locator.clone();
        let latency = resolver.latency();
        if !latency.is_zero() {
            tokio::time::sleep(latency).await;
        }
        let started = tokio::time::Instant::now();
        let resolved = tokio::task::spawn_blocking(move || resolver.resolve(&target))
            .await
            .unwrap_or_else(|error| Err(ResolveError::Unreachable(error.to_string())));

        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                self.status_resolve(&error);
                let _ = self.tx.send(AppEvent::EntryFailed {
                    entry_id,
                    reason: format!("{}: {}", error.class(), error.message()),
                });
                return;
            }
        };

        let mpv = self.backends.mpv.clone();
        let url = resolved.url.clone();
        let played = tokio::task::spawn_blocking(move || {
            let mut player = mpv.lock().unwrap();
            if !player.is_alive() {
                player.spawn()?;
            }
            player.set_volume(volume)?;
            player.play(&url)
        })
        .await
        .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())));

        if let Err(error) = played {
            self.status_player(&error);
            let _ = self.tx.send(AppEvent::EntryFailed {
                entry_id,
                reason: error.message(),
            });
            return;
        }

        self.backends.workspace.clear_audio();
        let _ = self.tx.send(AppEvent::PlaybackStarted { entry_id });

        let mpv = self.backends.mpv.clone();
        while started.elapsed() < FIRST_AUDIO_BUDGET {
            let event = tokio::task::spawn_blocking({
                let mpv = mpv.clone();
                move || mpv.lock().unwrap().next_event(Duration::from_millis(50))
            })
            .await
            .unwrap_or(None);
            match event {
                Some(PlayerEvent::FirstAudio) => break,
                Some(PlayerEvent::Failed(reason)) => {
                    let _ = self.tx.send(AppEvent::EntryFailed { entry_id, reason });
                    return;
                }
                Some(PlayerEvent::EndFile) => break,
                Some(_) => {}
                None => tokio::time::sleep(Duration::from_millis(5)).await,
            }
        }
        let _ = self.tx.send(AppEvent::FirstAudio {
            entry_id,
            elapsed: started.elapsed(),
        });
        self.spawn_monitor(entry_id);
    }

    /// Translates mpv events into app events after the first audio frame.
    fn spawn_monitor(&mut self, entry_id: crate::queue::EntryId) {
        if let Some(monitor) = self.monitor.take() {
            monitor.abort();
        }
        let mpv = self.backends.mpv.clone();
        let tx = self.tx.clone();
        self.monitor = Some(tokio::spawn(async move {
            let mut duration = None;
            let mut state = PlayerState::Playing;
            loop {
                if tx.is_closed() {
                    break;
                }
                let poll = {
                    let mpv = mpv.clone();
                    move || mpv.lock().unwrap().next_event(Duration::from_millis(100))
                };
                let event = tokio::task::spawn_blocking(poll).await.unwrap_or(None);
                match event {
                    Some(PlayerEvent::EndFile) => {
                        let _ = tx.send(AppEvent::TrackEnded);
                        break;
                    }
                    Some(PlayerEvent::Failed(reason)) => {
                        let _ = tx.send(AppEvent::EntryFailed { entry_id, reason });
                        break;
                    }
                    Some(PlayerEvent::Paused(paused)) => {
                        state = if paused {
                            PlayerState::Paused
                        } else {
                            PlayerState::Playing
                        };
                    }
                    Some(PlayerEvent::Duration(value)) => duration = Some(value),
                    Some(PlayerEvent::Position(position)) => {
                        let _ = tx.send(AppEvent::PlaybackProgress {
                            state,
                            position,
                            duration,
                        });
                    }
                    Some(_) => {}
                    None => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            }
        }));
    }

    async fn pause(&mut self, source: Source) {
        let result = match source {
            Source::Local => {
                let mpd = self.backends.mpd.clone();
                tokio::task::spawn_blocking(move || LocalAdapter::new(mpd).pause(true))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
            _ => {
                let mpv = self.backends.mpv.clone();
                tokio::task::spawn_blocking(move || RemoteAdapter::new(mpv).pause(true))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
        };
        if let Err(error) = result {
            self.status_for(source, &error);
        }
    }

    async fn resume(&mut self, source: Source) {
        let result = match source {
            Source::Local => {
                let mpd = self.backends.mpd.clone();
                tokio::task::spawn_blocking(move || LocalAdapter::new(mpd).pause(false))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
            _ => {
                let mpv = self.backends.mpv.clone();
                tokio::task::spawn_blocking(move || RemoteAdapter::new(mpv).pause(false))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
        };
        if let Err(error) = result {
            self.status_for(source, &error);
        }
    }

    async fn stop(&mut self, source: Source) {
        let result = match source {
            Source::Local => {
                let mpd = self.backends.mpd.clone();
                tokio::task::spawn_blocking(move || LocalAdapter::new(mpd).stop())
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
            _ => {
                let mpv = self.backends.mpv.clone();
                tokio::task::spawn_blocking(move || RemoteAdapter::new(mpv).stop())
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
        };
        if let Err(error) = result {
            self.status_for(source, &error);
        }
    }

    async fn seek(&mut self, source: Source, position: Duration) {
        let result = match source {
            Source::Local => {
                let mpd = self.backends.mpd.clone();
                tokio::task::spawn_blocking(move || LocalAdapter::new(mpd).seek(position))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
            _ => {
                let mpv = self.backends.mpv.clone();
                tokio::task::spawn_blocking(move || RemoteAdapter::new(mpv).seek(position))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
        };
        if let Err(error) = result {
            self.status_for(source, &error);
        }
    }

    async fn volume(&mut self, source: Source, volume: u8) {
        let result = match source {
            Source::Local => {
                let mpd = self.backends.mpd.clone();
                tokio::task::spawn_blocking(move || LocalAdapter::new(mpd).set_volume(volume))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
            _ => {
                let mpv = self.backends.mpv.clone();
                tokio::task::spawn_blocking(move || RemoteAdapter::new(mpv).set_volume(volume))
                    .await
                    .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())))
            }
        };
        if let Err(error) = result {
            self.status_for(source, &error);
        }
    }

    async fn load_library(&mut self) {
        let mpd = self.backends.mpd.clone();
        let result = tokio::task::spawn_blocking(move || LocalAdapter::new(mpd).library())
            .await
            .unwrap_or_else(|error| Err(SourceError::Failed(error.to_string())));
        match result {
            Ok(library) => {
                let _ = self.tx.send(AppEvent::Status {
                    source: Source::Local,
                    status: SourceStatus::Ready,
                });
                let _ = self.tx.send(AppEvent::LibraryLoaded(library));
            }
            Err(error) => self.status_local(&error),
        }
    }

    async fn fetch_art(&mut self, entry: Entry) {
        let mpd = self.backends.mpd.clone();
        let resolver = self.backends.resolver.clone();
        let directory = self.backends.workspace.art_dir();
        let entry_id = entry.id;
        let bytes = tokio::task::spawn_blocking(move || {
            load_art_bytes(&entry, &mpd, resolver.as_ref(), &directory)
        })
        .await
        .unwrap_or(None);
        let image = bytes.and_then(|bytes| crate::art::decode(&bytes));
        let _ = self.tx.send(AppEvent::ArtReady { entry_id, image });
    }

    async fn import(&mut self, reference: SpotifyRef) {
        let importer = self.backends.importer.clone();
        let lookup = reference.clone();
        let fetched = tokio::task::spawn_blocking(move || importer.fetch(&lookup))
            .await
            .unwrap_or_else(|error| Err(ImportError::Failed(error.to_string())));
        let mut collection = match fetched {
            Ok(collection) => prepare_collection(collection, &reference),
            Err(error) => {
                let _ = self.tx.send(AppEvent::ImportFailed { error });
                return;
            }
        };

        // Save metadata before matching so an aborted import stays browsable.
        let _ = crate::persist::imports::save(&self.config_dir, &collection);
        let _ = self.tx.send(AppEvent::ImportProgress {
            name: collection.name.clone(),
            matched: 0,
            total: collection.tracks.len(),
        });

        let resolver = self.backends.resolver.clone();
        let mut matched = collection.clone();
        tokio::task::spawn_blocking(move || {
            match_tracks(&mut matched, resolver.as_ref());
            matched
        })
        .await
        .map(|updated| collection = updated)
        .ok();

        let _ = crate::persist::imports::save(&self.config_dir, &collection);
        let _ = self.tx.send(AppEvent::ImportProgress {
            name: collection.name.clone(),
            matched: collection.matched(),
            total: collection.tracks.len(),
        });
        let _ = self.tx.send(AppEvent::ImportFinished { collection });
    }

    fn status_local(&mut self, error: &SourceError) {
        let status = match error {
            SourceError::Disconnected => {
                SourceStatus::Disconnected(Reason::new("msg_mpd_disconnected"))
            }
            other => SourceStatus::Unavailable(Reason::with_detail(
                "msg_mpd_unavailable",
                other.message(),
            )),
        };
        let _ = self.tx.send(AppEvent::Status {
            source: Source::Local,
            status,
        });
    }

    fn status_player(&mut self, error: &SourceError) {
        let reason = match error {
            SourceError::MissingBinary(_) => Reason::new("msg_mpv_missing"),
            other => Reason::with_detail("msg_player_failed", other.message()),
        };
        for source in [Source::Youtube, Source::Spotify] {
            let _ = self.tx.send(AppEvent::Status {
                source,
                status: SourceStatus::Unavailable(reason.clone()),
            });
        }
    }

    fn status_resolve(&mut self, error: &ResolveError) {
        let reason = match error {
            ResolveError::MissingBinary(_) => Reason::new("msg_ytdlp_missing"),
            other => Reason::with_detail("msg_remote_disabled", other.message()),
        };
        for source in [Source::Youtube, Source::Spotify] {
            let _ = self.tx.send(AppEvent::Status {
                source,
                status: SourceStatus::Unavailable(reason.clone()),
            });
        }
    }

    fn status_for(&mut self, source: Source, error: &SourceError) {
        match source {
            Source::Local => self.status_local(error),
            _ => self.status_player(error),
        }
    }
}

/// Local art: daemon picture first, then a folder image; remote: resolver thumbnail.
pub fn load_art_bytes(
    entry: &Entry,
    mpd: &SharedMpd,
    resolver: &dyn Resolver,
    art_dir: &Path,
) -> Option<Vec<u8>> {
    match entry.source {
        Source::Local => {
            let uri = match &entry.locator {
                crate::queue::Locator::MpdUri(uri) => uri.clone(),
                crate::queue::Locator::LocalPath(path) => path.display().to_string(),
                _ => return None,
            };
            let (music_directory, daemon_art) = {
                let mut backend = mpd.lock().unwrap();
                let directory = backend.music_directory();
                let art = backend.album_art(&uri).ok().flatten();
                (directory, art)
            };
            if daemon_art.is_some() {
                return daemon_art;
            }
            let path = Path::new(&uri);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else if let Some(directory) = music_directory {
                directory.join(path)
            } else {
                path.to_path_buf()
            };
            let folder = path.parent()?;
            crate::art::folder_cover(folder).and_then(|cover| crate::art::read(&cover))
        }
        _ => {
            let video_id = match &entry.locator {
                crate::queue::Locator::YoutubeId(id) => id.clone(),
                _ => return None,
            };
            resolver.thumbnail(&video_id, art_dir)
        }
    }
}

/// Stable id and canonical URL, so a re-import updates the same file.
pub fn prepare_collection(mut collection: Collection, reference: &SpotifyRef) -> Collection {
    collection.id = format!("{}-{}", reference.kind.as_str(), reference.id);
    collection.kind = reference.kind;
    collection.url = reference.url(reference.kind);
    collection
}

/// Match every imported track; unmatched tracks stay in the collection.
pub fn match_tracks(collection: &mut Collection, resolver: &dyn Resolver) {
    let total = collection.tracks.len();
    let results: Mutex<Vec<Option<Candidate>>> = Mutex::new(vec![None; total]);
    let next = AtomicUsize::new(0);
    let workers = IMPORT_CONCURRENCY.min(total).max(1);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= total {
                        break;
                    }
                    let candidate = resolver
                        .search_candidate(&collection.tracks[index].query)
                        .ok();
                    results.lock().unwrap()[index] = candidate;
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    for (track, candidate) in collection.tracks.iter_mut().zip(results) {
        match candidate {
            Some(candidate) if duration_matches(track.duration, candidate.duration) => {
                track.youtube_id = Some(candidate.video_id);
                track.unavailable = false;
            }
            _ => {
                track.youtube_id = None;
                track.unavailable = true;
            }
        }
    }
}

fn duration_matches(spotify: Option<Duration>, youtube: Option<Duration>) -> bool {
    match (spotify, youtube) {
        (Some(spotify), Some(youtube)) => youtube.as_secs().abs_diff(spotify.as_secs()) <= 5,
        _ => true,
    }
}

/// Synchronous import used by `--import` before the TUI starts.
pub fn import_blocking(
    dir: &Path,
    input: &str,
    importer: &dyn SpotifyImporter,
    resolver: &dyn Resolver,
) -> Result<Collection, ImportError> {
    let reference = crate::process::parse_reference(input)?;
    let mut collection = prepare_collection(importer.fetch(&reference)?, &reference);
    // Metadata is saved before matching; an aborted run still leaves it browsable.
    let _ = crate::persist::imports::save(dir, &collection);
    match_tracks(&mut collection, resolver);
    let _ = crate::persist::imports::save(dir, &collection);
    Ok(collection)
}

/// Build the temporary workspace root, honouring an explicit cache directory.
pub fn workspace_for(config: &crate::persist::Config) -> TempWorkspace {
    match &config.paths.cache_dir {
        Some(path) => TempWorkspace::at(path.clone()),
        None => TempWorkspace::new(),
    }
}
