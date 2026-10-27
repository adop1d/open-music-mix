//! Shared harness: temporary directories, app builders, fixtures and rendering.

#![allow(dead_code)]

use std::io::Cursor;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use omm::app::App;
use omm::backends::{Backends, TempWorkspace};
use omm::event::AppEvent;
use omm::i18n::I18n;
use omm::persist::Config;
use omm::process::{
    AlbumInfo, Collection, CollectionKind, FakeImporter, FakeMpd, FakeMpv, FakeResolver,
    ImportedTrack, SongInfo, SpotifyRef,
};
use omm::queue::{Source, TrackMeta};
use omm::sources::Library;
use omm::ui;
use omm::ui::keymap::Key;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Unique directory removed on drop, so tests never touch the working tree.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "omm-test-{label}-{}-{stamp}-{counter}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub fn app() -> App {
    App::new(Config::default(), I18n::english())
}

pub fn app_with(config: Config) -> App {
    App::new(config, I18n::english())
}

pub fn song(title: &str, artist: &str, album: &str, uri: &str) -> SongInfo {
    SongInfo {
        title: title.to_string(),
        artist: artist.to_string(),
        album: if album.is_empty() {
            None
        } else {
            Some(album.to_string())
        },
        uri: uri.to_string(),
        duration: Some(Duration::from_secs(200)),
    }
}

pub fn album(artist: &str, name: &str) -> AlbumInfo {
    AlbumInfo {
        artist: artist.to_string(),
        name: name.to_string(),
    }
}

pub fn library(artists: &[&str], albums: Vec<AlbumInfo>, songs: Vec<SongInfo>) -> Library {
    Library {
        artists: artists.iter().map(|artist| artist.to_string()).collect(),
        albums,
        folders: Vec::new(),
        playlists: Vec::new(),
        songs,
    }
}

/// Metadata plus an already-resolved video id, as stored after a successful match.
pub fn track(title: &str, query: &str, video_id: Option<&str>) -> ImportedTrack {
    ImportedTrack {
        title: title.to_string(),
        artists: vec!["The Cure".to_string()],
        album: Some("Disintegration".to_string()),
        duration: Some(Duration::from_secs(200)),
        isrc: Some("GBAYE8900001".to_string()),
        query: query.to_string(),
        youtube_id: video_id.map(|id| id.to_string()),
        unavailable: video_id.is_none(),
    }
}

pub fn collection(id: &str, name: &str, tracks: Vec<ImportedTrack>) -> Collection {
    Collection {
        id: id.to_string(),
        name: name.to_string(),
        kind: CollectionKind::Playlist,
        url: format!("https://open.spotify.com/playlist/{id}"),
        tracks,
    }
}

pub fn reference(url: &str) -> SpotifyRef {
    omm::process::parse_reference(url).expect("valid spotify reference")
}

pub fn meta(title: &str, artist: &str) -> TrackMeta {
    TrackMeta::new(title, artist)
}

pub fn press(app: &mut App, key: char) {
    app.handle_key(Key::Char(key));
}

pub fn press_key(app: &mut App, key: Key) {
    app.handle_key(key);
}

pub fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        if character == ' ' {
            app.handle_key(Key::Space);
        } else {
            app.handle_key(Key::Char(character));
        }
    }
}

/// Render one frame to plain text; each row ends with a newline.
pub fn render(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::render(app, frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            let symbol = buffer.cell((x, y)).map(|cell| cell.symbol()).unwrap_or(" ");
            text.push_str(symbol);
        }
        text.push('\n');
    }
    text
}

pub fn png_bytes() -> Vec<u8> {
    let image = image::RgbImage::from_fn(8, 8, |x, y| {
        image::Rgb([(x * 30) as u8, (y * 30) as u8, 128])
    });
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

pub fn shared_mpd(fake: FakeMpd) -> omm::sources::SharedMpd {
    Arc::new(Mutex::new(fake))
}

pub fn shared_mpv(fake: FakeMpv) -> omm::sources::SharedMpv {
    Arc::new(Mutex::new(fake))
}

pub fn fakes(
    mpd: FakeMpd,
    mpv: FakeMpv,
    resolver: FakeResolver,
    importer: FakeImporter,
    workspace: TempWorkspace,
) -> Backends {
    Backends::fakes(mpd, mpv, resolver, importer, workspace)
}

pub fn workspace_in(directory: &TempDir) -> TempWorkspace {
    TempWorkspace::at(directory.join("workspace"))
}

pub fn event_channel() -> (UnboundedSender<AppEvent>, UnboundedReceiver<AppEvent>) {
    unbounded_channel()
}

/// Drain everything queued without awaiting.
pub fn events(rx: &mut UnboundedReceiver<AppEvent>) -> Vec<AppEvent> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

/// Queue one local entry directly, for tests that only need a filled queue.
pub fn local_entry(app: &mut App, title: &str) {
    app.queue.append(
        Source::Local,
        meta(title, "The Cure"),
        omm::queue::Locator::MpdUri(format!("the-cure/{title}.flac")),
    );
}

pub fn remote_entry(app: &mut App, title: &str) {
    app.queue.append(
        Source::Youtube,
        meta(title, "The Cure"),
        omm::queue::Locator::YoutubeQuery(title.to_string()),
    );
}

/// A queue of n remote entries numbered from one.
pub fn remote_queue(app: &mut App, count: usize) {
    for index in 1..=count {
        remote_entry(app, &format!("Track {index}"));
    }
}

pub fn duration_of(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}
