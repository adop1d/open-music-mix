//! Adapter-level checks: local and remote adapters against their fakes.

use std::sync::{Arc, Mutex};

use omm::event::{AppEvent, Reason, SourceStatus};
use omm::process::{FakeMpd, FakeMpv, PlayerEvent};
use omm::queue::Source;
use omm::sources::{LocalAdapter, RemoteAdapter, SharedMpd, SharedMpv};

use omm::app::App;
use omm::i18n::I18n;
use omm::persist::Config;

mod common;

fn app() -> App {
    App::new(Config::default(), I18n::english())
}

fn shared(fake: FakeMpd) -> (Arc<Mutex<FakeMpd>>, SharedMpd) {
    let fake = Arc::new(Mutex::new(fake));
    let shared: SharedMpd = fake.clone();
    (fake, shared)
}

fn remote(fake: FakeMpv) -> (Arc<Mutex<FakeMpv>>, SharedMpv) {
    let fake = Arc::new(Mutex::new(fake));
    let shared: SharedMpv = fake.clone();
    (fake, shared)
}

#[test]
fn fake_status_reaches_the_app() {
    let (_fake, mpd) = shared(FakeMpd::default());
    let adapter = LocalAdapter::new(mpd);
    let error = adapter.library().err().unwrap();
    assert!(matches!(
        error,
        omm::process::SourceError::Disconnected
    ));

    let mut app = app();
    app.handle(AppEvent::Status {
        source: Source::Local,
        status: SourceStatus::Disconnected(Reason::new("msg_mpd_disconnected")),
    });
    assert!(!app.is_available(Source::Local));
    assert!(app.is_available(Source::Youtube));
}

#[test]
fn fake_library_round_trips_through_the_adapter() {
    let fake = FakeMpd::with_library(
        vec!["The Cure".to_string()],
        vec![common::album("The Cure", "Disintegration")],
        vec![
            common::song("Plainsong", "The Cure", "Disintegration", "cure/1.flac"),
            common::song(
                "Pictures of You",
                "The Cure",
                "Disintegration",
                "cure/2.flac",
            ),
        ],
    );
    let (_handle, mpd) = shared(fake);
    let library = LocalAdapter::new(mpd).library().unwrap();
    assert_eq!(library.artists, vec!["The Cure".to_string()]);
    assert_eq!(library.albums.len(), 1);
    assert_eq!(library.album_songs("The Cure", "Disintegration").len(), 2);
}

#[test]
fn fake_play_keeps_the_daemon_queue() {
    let fake = FakeMpd::with_library(
        vec![],
        vec![],
        vec![common::song("A", "Artist", "Album", "a.flac")],
    );
    let (handle, mpd) = shared(fake);
    {
        let mut guard = handle.lock().unwrap();
        guard.daemon_queue = (0..3).map(|index| format!("old/{index}.flac")).collect();
    }
    LocalAdapter::new(mpd)
        .play(&["b.flac".to_string(), "c.flac".to_string()])
        .unwrap();
    let guard = handle.lock().unwrap();
    assert_eq!(guard.daemon_queue.len(), 5);
    assert_eq!(guard.play_calls.last().unwrap().len(), 2);
}

#[test]
fn fake_mpv_maps_every_command() {
    let (handle, mpv) = remote(FakeMpv::default());
    let adapter = RemoteAdapter::new(mpv);
    adapter.play("https://media.invalid/one").unwrap();
    adapter.pause(true).unwrap();
    adapter.seek(std::time::Duration::from_secs(42)).unwrap();
    adapter.set_volume(33).unwrap();
    adapter.stop().unwrap();

    let guard = handle.lock().unwrap();
    assert_eq!(guard.played, vec!["https://media.invalid/one"]);
    assert_eq!(guard.paused, Some(true));
    assert_eq!(guard.seeks, vec![std::time::Duration::from_secs(42)]);
    assert_eq!(guard.volumes, vec![33]);
    assert!(guard.stopped);
}

#[test]
fn fake_mpv_events_reach_the_adapter() {
    let (handle, mpv) = remote(FakeMpv::default());
    handle
        .lock()
        .unwrap()
        .events
        .push_back(PlayerEvent::FirstAudio);
    let event = RemoteAdapter::new(mpv).next_event(std::time::Duration::ZERO);
    assert_eq!(event, Some(PlayerEvent::FirstAudio));
}

#[test]
fn crash_is_reported_and_the_next_play_respawns_once() {
    use omm::backends::Backends;
    use omm::event::{AppEvent, Command};
    use omm::process::{FakeImporter, FakeResolver};
    use omm::queue::{EntryId, Locator};
    use omm::worker::Worker;

    let directory = common::TempDir::new("respawn");
    let mpv = Arc::new(Mutex::new(FakeMpv {
        crash_on_play: true,
        ..FakeMpv::default()
    }));
    let backends = Backends {
        mpd: Arc::new(Mutex::new(FakeMpd::with_library(vec![], vec![], vec![]))),
        mpv: mpv.clone(),
        resolver: Arc::new(FakeResolver::new()),
        importer: Arc::new(FakeImporter::new(common::collection(
            "playlist-x",
            "X",
            vec![],
        ))),
        workspace: Arc::new(common::workspace_in(&directory)),
    };
    let (tx, mut rx) = common::event_channel();
    let mut worker = Worker::new(backends, tx, directory.path().to_path_buf());
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(worker.handle_command(Command::PlayRemote {
        entry_id: EntryId(0),
        source: Source::Youtube,
        locator: Locator::YoutubeId("dQw4w9WgXcQ".to_string()),
        volume: 70,
    }));
    assert!(
        common::events(&mut rx)
            .iter()
            .any(|event| matches!(event, AppEvent::EntryFailed { .. }))
    );
    assert!(!mpv.lock().unwrap().alive);

    mpv.lock().unwrap().crash_on_play = false;
    runtime.block_on(worker.handle_command(Command::PlayRemote {
        entry_id: EntryId(0),
        source: Source::Youtube,
        locator: Locator::YoutubeId("dQw4w9WgXcQ".to_string()),
        volume: 70,
    }));
    let guard = mpv.lock().unwrap();
    assert!(guard.alive);
    assert_eq!(guard.spawns, 1);
    assert_eq!(guard.played, vec!["http://fake.invalid/stream"]);
}

#[test]
fn player_events_are_typed() {
    assert_ne!(PlayerEvent::FirstAudio, PlayerEvent::EndFile);
    assert_eq!(PlayerEvent::Paused(true), PlayerEvent::Paused(true));
    assert_eq!(Source::ALL.len(), 3);
}
