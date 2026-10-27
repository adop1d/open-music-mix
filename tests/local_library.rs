//! Scenarios for browsing and playing the local MPD library.

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use omm::app::Panel;
use omm::event::{AppEvent, Command, SourceStatus};
use omm::process::{FakeImporter, FakeMpd, FakeMpv, FakeResolver, PlayerState};
use omm::queue::Source;
use omm::sources::{LocalAdapter, SharedMpd};
use omm::ui::keymap::Key;
use omm::worker::Worker;

fn cure_library() -> omm::sources::Library {
    library(
        &["The Cure", "Queen"],
        vec![
            album("The Cure", "Disintegration"),
            album("The Cure", "Wish"),
        ],
        vec![
            song(
                "Plainsong",
                "The Cure",
                "Disintegration",
                "the-cure/disintegration/01-plainsong.flac",
            ),
            song(
                "Pictures of You",
                "The Cure",
                "Disintegration",
                "the-cure/disintegration/02-pictures-of-you.flac",
            ),
            song(
                "Friday I'm in Love",
                "The Cure",
                "Wish",
                "the-cure/wish/01-friday.flac",
            ),
        ],
    )
}

#[test]
fn artist_view_lists_albums() {
    let mut app = app().with_library(cure_library());
    app.tab = Source::Local;
    app.focus = Panel::Explorer;

    press(&mut app, 'l');

    let names: Vec<String> = app.local_rows().iter().map(|row| row.label()).collect();
    assert_eq!(names, vec!["Disintegration", "Wish"]);
    assert_eq!(app.local.open_artist.as_deref(), Some("The Cure"));
}

#[test]
fn empty_local_library() {
    let mut app = app().with_library(Default::default());
    app.tab = Source::Local;
    let text = render(&mut app, 100, 30);
    assert!(text.contains("No local tracks"), "{text}");

    for key in ['j', 'k', 'l', 'h', 'n', 'p'] {
        press(&mut app, key);
    }
    assert!(app.local_rows().is_empty());
    assert!(!app.quit);
}

#[test]
fn playing_a_local_track() {
    let library = cure_library();
    let mut app = app().with_library(library.clone());
    app.tab = Source::Local;
    app.focus = Panel::Explorer;

    press(&mut app, 'l');
    press(&mut app, 'l');
    app.local.selection = 0;
    press_key(&mut app, Key::Enter);

    let commands = app.take_commands();
    let uris = match commands.as_slice() {
        [Command::PlayLocal { uris, volume, .. }] => {
            assert_eq!(*volume, 70);
            uris.clone()
        }
        other => panic!("expected PlayLocal, got {other:?}"),
    };
    assert_eq!(uris, vec!["the-cure/disintegration/01-plainsong.flac"]);

    let fake = Arc::new(Mutex::new(FakeMpd::with_library(
        library.artists.clone(),
        library.albums.clone(),
        library.songs.clone(),
    )));
    let mpd: SharedMpd = fake.clone();
    LocalAdapter::new(mpd).play(&uris).unwrap();
    assert_eq!(
        fake.lock().unwrap().daemon_queue,
        vec!["the-cure/disintegration/01-plainsong.flac"]
    );

    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    let text = render(&mut app, 100, 30);
    assert!(text.contains("Plainsong"));
}

#[test]
fn existing_daemon_queue_is_preserved() {
    let mut fake = FakeMpd::with_library(Vec::new(), Vec::new(), Vec::new());
    fake.daemon_queue = vec![
        "already/queued.flac".to_string(),
        "also/queued.flac".to_string(),
    ];
    let fake = Arc::new(Mutex::new(fake));
    let mpd: SharedMpd = fake.clone();

    LocalAdapter::new(mpd)
        .play(&["the-cure/wish/01-friday.flac".to_string()])
        .unwrap();

    assert_eq!(
        fake.lock().unwrap().daemon_queue,
        vec![
            "already/queued.flac",
            "also/queued.flac",
            "the-cure/wish/01-friday.flac"
        ]
    );
}

#[test]
fn pausing_local_playback() {
    let mut app = app();
    app.tab = Source::Local;
    local_entry(&mut app, "Plainsong");
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    app.playback.position = std::time::Duration::from_secs(42);
    let _ = app.take_commands();

    press_key(&mut app, Key::Space);

    assert_eq!(app.playback.state, PlayerState::Paused);
    assert_eq!(app.playback.position, std::time::Duration::from_secs(42));
    assert!(matches!(
        app.take_commands().as_slice(),
        [Command::Pause {
            source: Source::Local
        }]
    ));

    let fake = Arc::new(Mutex::new(FakeMpd::with_library(
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )));
    let mpd: SharedMpd = fake.clone();
    LocalAdapter::new(mpd).pause(true).unwrap();
    assert_eq!(fake.lock().unwrap().paused, Some(true));

    app.handle(AppEvent::PlaybackProgress {
        state: PlayerState::Paused,
        position: std::time::Duration::from_secs(42),
        duration: None,
    });
    let text = render(&mut app, 100, 30);
    assert!(text.contains("paused"), "{text}");
}

#[test]
fn daemon_not_installed() {
    let directory = TempDir::new("no-mpd");
    let backends = fakes(
        FakeMpd::default(),
        FakeMpv::default(),
        FakeResolver::new(),
        FakeImporter::new(collection("empty", "Empty", Vec::new())),
        workspace_in(&directory),
    );
    let (tx, _rx) = event_channel();
    let worker = Worker::new(backends, tx, directory.to_path_buf());

    let events = worker.probe_events(&|name| name != "mpd");

    let mut app = app();
    for event in events {
        app.handle(event);
    }
    assert!(!app.is_available(Source::Local));
    assert_eq!(
        app.statuses.get(&Source::Local),
        Some(&SourceStatus::Unavailable(omm::event::Reason::new(
            "msg_mpd_unavailable"
        )))
    );
    assert!(app.is_available(Source::Youtube));
    assert!(app.is_available(Source::Spotify));

    press(&mut app, '1');
    assert_eq!(app.tab, Source::Local);
    let text = render(&mut app, 100, 30);
    assert!(text.contains("MPD is not available"), "{text}");
}

#[tokio::test]
async fn daemon_disconnects_during_playback() {
    let directory = TempDir::new("disconnect");
    let fake = FakeMpd {
        disconnect_during_playback: true,
        ..FakeMpd::default()
    };
    let backends = fakes(
        fake,
        FakeMpv::default(),
        FakeResolver::new(),
        FakeImporter::new(collection("empty", "Empty", Vec::new())),
        workspace_in(&directory),
    );
    let (tx, mut rx) = event_channel();
    let mut worker = Worker::new(backends, tx, directory.to_path_buf());

    worker
        .handle_command(Command::PlayLocal {
            entry_id: omm::queue::EntryId(1),
            uris: vec!["the-cure/wish/01-friday.flac".to_string()],
            volume: 70,
        })
        .await;

    let events = events(&mut rx);
    let status = events
        .iter()
        .find_map(|event| match event {
            AppEvent::Status { source, status } => Some((*source, status.clone())),
            _ => None,
        })
        .expect("status event");
    assert_eq!(status.0, Source::Local);
    assert!(matches!(status.1, SourceStatus::Disconnected(_)));

    let mut app = app();
    for event in events {
        app.handle(event);
    }
    assert!(!app.is_available(Source::Local));

    press(&mut app, 'j');
    press(&mut app, '2');
    assert_eq!(app.tab, Source::Youtube);
    assert!(!app.quit);
}

#[test]
fn search_filters_the_artist_list() {
    let mut library = cure_library();
    library.artists = vec![
        "The Cure".to_string(),
        "Queen".to_string(),
        "Curebound".to_string(),
    ];
    let mut app = app().with_library(library);
    app.tab = Source::Local;
    app.focus = Panel::Explorer;

    press(&mut app, '/');
    type_text(&mut app, "cure");

    let names: Vec<String> = app
        .filtered_local_rows()
        .iter()
        .map(|row| row.label())
        .collect();
    assert_eq!(names, vec!["The Cure", "Curebound"]);
}
