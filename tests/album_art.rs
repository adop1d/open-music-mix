mod common;

use std::sync::{Arc, Mutex};

use omm::app::App;
use omm::art::{self, ArtKey, ArtRenderer};
use omm::event::AppEvent;
use omm::process::{FakeMpd, FakeResolver, SongInfo};
use omm::queue::{Entry, Locator, Queue, Source};
use omm::sources::SharedMpd;
use omm::worker::load_art_bytes;
use ratatui_image::picker::{Picker, ProtocolType};

use common::*;

fn entry(source: Source, locator: Locator) -> Entry {
    let mut queue = Queue::new();
    let id = queue.append(source, meta("Song", "Artist"), locator);
    queue.entry(id).unwrap().clone()
}

fn shared(mpd: FakeMpd) -> (SharedMpd, Arc<Mutex<FakeMpd>>) {
    let fake = Arc::new(Mutex::new(mpd));
    let shared: SharedMpd = fake.clone();
    (shared, fake)
}

fn kitty() -> ArtRenderer {
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    ArtRenderer::from_picker(picker)
}

fn playing(app: &mut App) -> omm::queue::EntryId {
    let id = app.queue.append(
        Source::Youtube,
        meta("Friday I'm in Love", "The Cure"),
        Locator::YoutubeId("vid00000001".to_string()),
    );
    app.play_index(app.queue.index_of(id).unwrap());
    app.handle(AppEvent::PlaybackStarted { entry_id: id });
    id
}

#[test]
fn kitty_terminal() {
    let renderer = kitty();
    assert_eq!(renderer.protocol_type(), ProtocolType::Kitty);
    let mut app = app().with_art_renderer(kitty());
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(&png_bytes()),
    });
    render(&mut app, 100, 40);
    assert!(app.art.protocol.is_some());
    assert_eq!(
        app.art_renderer
            .effective_protocol(ratatui::layout::Rect::new(0, 0, 40, 20)),
        ProtocolType::Kitty
    );
}

#[test]
fn unsupported_terminal() {
    let mut app = app().with_art_renderer(ArtRenderer::halfblocks());
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(&png_bytes()),
    });
    let frame = render(&mut app, 100, 40);
    assert!(
        !frame.contains('\u{1b}'),
        "half-blocks must not emit escapes"
    );
    assert_eq!(app.art_renderer.protocol_type(), ProtocolType::Halfblocks);
    assert!(app.art.protocol.is_some());
}

#[test]
fn art_available() {
    let mut app = app();
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(&png_bytes()),
    });
    assert!(app.art.image.is_some());
    assert!(app.art.placeholder.is_none());
    render(&mut app, 100, 40);
    assert!(app.art.protocol.is_some());
}

#[test]
fn slow_art_fetch() {
    let mut app = app();
    let id = playing(&mut app);
    let waiting = render(&mut app, 100, 40);
    assert!(waiting.contains("Friday I'm in Love"), "{waiting}");
    assert!(app.art.image.is_none());
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(&png_bytes()),
    });
    assert!(app.art.image.is_some());
    assert!(app.art.placeholder.is_none());
}

#[test]
fn local_art_from_the_daemon() {
    let dir = TempDir::new("art-daemon");
    let bytes = png_bytes();
    let (mpd, _fake) =
        shared(FakeMpd::with_library(vec![], vec![], vec![]).with_art(bytes.clone()));
    let song = entry(Source::Local, Locator::MpdUri("cure/song.flac".to_string()));
    let found = load_art_bytes(&song, &mpd, &FakeResolver::new(), dir.path());
    assert_eq!(found, Some(bytes));
}

#[test]
fn local_art_from_the_folder() {
    let dir = TempDir::new("art-folder");
    let music = dir.join("music");
    let album_dir = music.join("The Cure").join("Disintegration");
    std::fs::create_dir_all(&album_dir).unwrap();
    let bytes = png_bytes();
    std::fs::write(album_dir.join("cover.png"), &bytes).unwrap();

    let mut mpd = FakeMpd::with_library(vec![], vec![], vec![]);
    mpd.music_directory = Some(music.clone());
    let (shared_mpd, _fake) = shared(mpd);
    let song = entry(
        Source::Local,
        Locator::MpdUri("The Cure/Disintegration/Plainsong.flac".to_string()),
    );
    let found = load_art_bytes(&song, &shared_mpd, &FakeResolver::new(), dir.path());
    assert_eq!(found, Some(bytes));
}

#[test]
fn remote_art_from_the_resolver() {
    let dir = TempDir::new("art-remote");
    let bytes = png_bytes();
    let resolver = FakeResolver::new().with_thumbnail(bytes.clone());
    let (mpd, _fake) = shared(FakeMpd::with_library(vec![], vec![], vec![]));
    let track = entry(
        Source::Youtube,
        Locator::YoutubeId("vid00000001".to_string()),
    );
    let found = load_art_bytes(&track, &mpd, &resolver, dir.path());
    assert_eq!(found, Some(bytes));
}

#[test]
fn track_without_embedded_art() {
    let dir = TempDir::new("art-none");
    let music = dir.join("music");
    std::fs::create_dir_all(music.join("album")).unwrap();
    let mut mpd = FakeMpd::with_library(vec![], vec![], vec![]);
    mpd.music_directory = Some(music);
    let (shared_mpd, _fake) = shared(mpd);
    let song = entry(
        Source::Local,
        Locator::MpdUri("album/song.flac".to_string()),
    );
    assert_eq!(
        load_art_bytes(&song, &shared_mpd, &FakeResolver::new(), dir.path()),
        None
    );

    let mut app = app();
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: None,
    });
    let frame = render(&mut app, 100, 40);
    assert!(frame.contains("Friday I'm in Love"), "{frame}");
    assert!(!frame.to_lowercase().contains("error"), "{frame}");
}

#[test]
fn corrupt_image() {
    assert!(art::decode(b"definitely not an image").is_none());
    let mut app = app();
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(b"truncated png"),
    });
    assert!(app.art.image.is_none());
    let frame = render(&mut app, 100, 40);
    assert!(frame.contains("Friday I'm in Love"), "{frame}");
}

#[test]
fn resizing_the_terminal() {
    let mut app = app().with_art_renderer(kitty());
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(&png_bytes()),
    });
    for (width, height) in [(120, 50), (60, 16), (100, 40)] {
        let frame = render(&mut app, width, height);
        assert_eq!(frame.lines().count(), height as usize);
        assert!(app.art.protocol.is_some());
    }
}

#[test]
fn narrow_panel() {
    let renderer = kitty();
    assert_eq!(
        renderer.effective_protocol(ratatui::layout::Rect::new(0, 0, 12, 6)),
        ProtocolType::Halfblocks
    );
    assert_eq!(
        renderer.effective_protocol(ratatui::layout::Rect::new(0, 0, 32, 10)),
        ProtocolType::Kitty
    );

    let mut app = app().with_art_renderer(kitty());
    let id = playing(&mut app);
    app.handle(AppEvent::ArtReady {
        entry_id: id,
        image: art::decode(&png_bytes()),
    });
    app.art.set_image(
        id,
        art::decode(&png_bytes()).unwrap(),
        ArtKey {
            source: Source::Youtube,
            album: None,
            path: id.0.to_string(),
        },
    );
    render(&mut app, 60, 16);
    assert!(app.art.protocol.is_some());
}

#[allow(dead_code)]
fn _song(title: &str) -> SongInfo {
    song(title, "The Cure", "Disintegration", "cure/song.flac")
}
