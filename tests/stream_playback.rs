//! Scenarios for remote playback through yt-dlp and mpv.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use omm::app::Panel;
use omm::backends::Backends;
use omm::event::{AppEvent, Command, SourceStatus};
use omm::process::{FakeImporter, FakeMpd, FakeMpv, FakeResolver, PlayerState, ResolveError};
use omm::queue::{EntryId, Locator, Source};
use omm::sources::{RemoteAdapter, SharedMpv};
use omm::ui::keymap::Key;
use omm::worker::Worker;
use tokio::sync::mpsc::UnboundedReceiver;

fn rig(
    directory: &TempDir,
    resolver: FakeResolver,
    fake_mpv: FakeMpv,
) -> (Worker, UnboundedReceiver<AppEvent>, Arc<Mutex<FakeMpv>>) {
    let mpv = Arc::new(Mutex::new(fake_mpv));
    let backends = Backends {
        mpd: shared_mpd(FakeMpd::with_library(Vec::new(), Vec::new(), Vec::new())),
        mpv: mpv.clone(),
        resolver: Arc::new(resolver),
        importer: Arc::new(FakeImporter::new(collection("x", "X", Vec::new()))),
        workspace: Arc::new(workspace_in(directory)),
    };
    let (tx, rx) = event_channel();
    (Worker::new(backends, tx, directory.to_path_buf()), rx, mpv)
}

fn play_remote_command(locator: Locator) -> Command {
    Command::PlayRemote {
        entry_id: EntryId(1),
        source: Source::Youtube,
        locator,
        volume: 70,
    }
}

#[tokio::test]
async fn resolving_a_search_query() {
    let directory = TempDir::new("search-query");
    let resolver = FakeResolver::new();
    let handle = resolver.clone();
    let (mut worker, mut rx, mpv) = rig(&directory, resolver, FakeMpv::default());

    worker
        .handle_command(play_remote_command(Locator::YoutubeQuery(
            "the cure disintegration".to_string(),
        )))
        .await;

    assert_eq!(
        handle.resolve_calls(),
        vec![Locator::YoutubeQuery("the cure disintegration".to_string())]
    );
    assert!(matches!(
        events(&mut rx).as_slice(),
        [
            AppEvent::PlaybackStarted { .. },
            AppEvent::FirstAudio { .. }
        ] | [
            AppEvent::PlaybackStarted { .. },
            AppEvent::FirstAudio { .. },
            ..
        ]
    ));
    assert_eq!(
        mpv.lock().unwrap().played,
        vec!["http://fake.invalid/stream"]
    );
    assert!(mpv.lock().unwrap().volumes.contains(&70));
}

#[tokio::test]
async fn resolving_a_youtube_url() {
    let directory = TempDir::new("youtube-url");
    let resolver = FakeResolver::new();
    let handle = resolver.clone();
    let (mut worker, _rx, mpv) = rig(&directory, resolver, FakeMpv::default());

    worker
        .handle_command(play_remote_command(Locator::YoutubeId(
            "dQw4w9WgXcQ".to_string(),
        )))
        .await;

    assert_eq!(
        handle.resolve_calls(),
        vec![Locator::YoutubeId("dQw4w9WgXcQ".to_string())]
    );
    assert_eq!(
        mpv.lock().unwrap().played,
        vec!["http://fake.invalid/stream"]
    );
}

#[test]
fn pausing_a_remote_entry() {
    let mut app = app();
    app.tab = Source::Youtube;
    remote_entry(&mut app, "Lullaby");
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    let _ = app.take_commands();

    press_key(&mut app, Key::Space);
    assert_eq!(app.playback.state, PlayerState::Paused);
    assert!(matches!(
        app.take_commands().as_slice(),
        [Command::Pause {
            source: Source::Youtube
        }]
    ));

    let fake = Arc::new(Mutex::new(FakeMpv::default()));
    let mpv: SharedMpv = fake.clone();
    RemoteAdapter::new(mpv).pause(true).unwrap();
    assert_eq!(fake.lock().unwrap().paused, Some(true));

    app.handle(AppEvent::PlaybackProgress {
        state: PlayerState::Paused,
        position: Duration::from_secs(10),
        duration: None,
    });
    let text = render(&mut app, 100, 30);
    assert!(text.contains("paused"), "{text}");

    press_key(&mut app, Key::Space);
    assert_eq!(app.playback.state, PlayerState::Playing);
    assert!(matches!(
        app.take_commands().as_slice(),
        [Command::Resume {
            source: Source::Youtube
        }]
    ));
}

#[test]
fn seeking_a_remote_entry() {
    let mut app = app();
    remote_entry(&mut app, "Lullaby");
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    app.handle(AppEvent::PlaybackProgress {
        state: PlayerState::Playing,
        position: Duration::from_secs(58),
        duration: Some(Duration::from_secs(200)),
    });
    let _ = app.take_commands();

    press_key(&mut app, Key::Right);

    let commands = app.take_commands();
    assert!(matches!(
        commands.as_slice(),
        [Command::Seek { position, .. }] if *position == Duration::from_secs(63)
    ));

    let fake = Arc::new(Mutex::new(FakeMpv::default()));
    let mpv: SharedMpv = fake.clone();
    RemoteAdapter::new(mpv)
        .seek(Duration::from_secs(63))
        .unwrap();
    assert_eq!(fake.lock().unwrap().seeks, vec![Duration::from_secs(63)]);

    app.handle(AppEvent::PlaybackProgress {
        state: PlayerState::Playing,
        position: Duration::from_secs(63),
        duration: None,
    });
    assert_eq!(app.playback.position, Duration::from_secs(63));
}

#[tokio::test(start_paused = true)]
async fn normal_playback_start() {
    let directory = TempDir::new("normal-start");
    let resolver = FakeResolver::new().with_latency(Duration::from_millis(300));
    let (mut worker, mut rx, _mpv) = rig(&directory, resolver, FakeMpv::default());

    worker
        .handle_command(play_remote_command(Locator::YoutubeQuery(
            "lullaby".to_string(),
        )))
        .await;

    let events = events(&mut rx);
    let started = events
        .iter()
        .position(|event| matches!(event, AppEvent::PlaybackStarted { .. }))
        .expect("playback started");
    let first_audio = events
        .iter()
        .position(|event| matches!(event, AppEvent::FirstAudio { .. }))
        .expect("first audio");
    assert!(started < first_audio);

    match &events[first_audio] {
        AppEvent::FirstAudio { elapsed, .. } => {
            assert!(*elapsed < Duration::from_secs(2), "{elapsed:?}");
        }
        other => panic!("expected FirstAudio, got {other:?}"),
    }
}

#[tokio::test]
async fn track_change_removes_the_previous_buffer() {
    let directory = TempDir::new("buffer-change");
    let (mut worker, _rx, _mpv) = rig(&directory, FakeResolver::new(), FakeMpv::default());
    let audio = directory.join("workspace/audio");

    worker
        .handle_command(play_remote_command(Locator::YoutubeId(
            "vid00000001".to_string(),
        )))
        .await;
    std::fs::write(audio.join("buffered.webm"), b"old audio").unwrap();

    worker
        .handle_command(play_remote_command(Locator::YoutubeId(
            "vid00000002".to_string(),
        )))
        .await;

    let entries: Vec<_> = std::fs::read_dir(&audio).unwrap().collect();
    assert!(entries.is_empty(), "old buffer still present");
}

#[tokio::test]
async fn exit_removes_buffered_audio() {
    let directory = TempDir::new("buffer-exit");
    let (mut worker, _rx, _mpv) = rig(&directory, FakeResolver::new(), FakeMpv::default());
    let audio = directory.join("workspace/audio");
    std::fs::write(audio.join("buffered.webm"), b"old audio").unwrap();

    worker.cleanup();

    assert!(!audio.exists(), "buffer directory survived exit");
}

#[test]
fn resolver_not_installed() {
    let directory = TempDir::new("no-resolver");
    let backends = fakes(
        FakeMpd::with_library(Vec::new(), Vec::new(), Vec::new()),
        FakeMpv::default(),
        FakeResolver::new(),
        FakeImporter::new(collection("x", "X", Vec::new())),
        workspace_in(&directory),
    );
    let (tx, _rx) = event_channel();
    let worker = Worker::new(backends, tx, directory.to_path_buf());

    let events = worker.probe_events(&|name| name == "mpd" || name == "mpv");
    let mut app = app();
    for event in events {
        app.handle(event);
    }

    assert!(app.is_available(Source::Local));
    for source in [Source::Youtube, Source::Spotify] {
        assert!(!app.is_available(source));
        assert_eq!(
            app.statuses.get(&source),
            Some(&SourceStatus::Unavailable(omm::event::Reason::new(
                "msg_ytdlp_missing"
            )))
        );
    }
    press(&mut app, '2');
    let text = render(&mut app, 100, 30);
    assert!(text.contains("yt-dlp"), "{text}");
}

#[tokio::test]
async fn player_process_crashes() {
    let directory = TempDir::new("player-crash");
    let fake = FakeMpv {
        crash_on_play: true,
        ..FakeMpv::default()
    };
    let (mut worker, mut rx, _mpv) = rig(&directory, FakeResolver::new(), fake);
    let mut app = app();
    app.queue.append(
        Source::Youtube,
        meta("Lullaby", "The Cure"),
        Locator::YoutubeId("vid00000001".to_string()),
    );
    let entry_id = app.queue.entries[0].id;

    worker
        .handle_command(Command::PlayRemote {
            entry_id,
            source: Source::Youtube,
            locator: Locator::YoutubeId("vid00000001".to_string()),
            volume: 70,
        })
        .await;

    let events = events(&mut rx);
    for event in events {
        app.handle(event);
    }
    assert!(app.queue.entries[0].state.is_failed());
    assert!(
        app.ui
            .notice
            .as_deref()
            .unwrap_or_default()
            .contains("failed")
    );

    press_key(&mut app, Key::Tab);
    assert_eq!(app.focus, Panel::Queue);
}

#[tokio::test]
async fn no_results_for_a_query() {
    let directory = TempDir::new("no-results");
    let resolver =
        FakeResolver::new().failing(ResolveError::NoResults("obscure bootleg".to_string()));
    let (mut worker, mut rx, _mpv) = rig(&directory, resolver, FakeMpv::default());

    worker
        .handle_command(play_remote_command(Locator::YoutubeQuery(
            "obscure bootleg".to_string(),
        )))
        .await;

    let mut app = app();
    remote_entry(&mut app, "Obscure Bootleg");
    let before = app.queue.len();
    let events = events(&mut rx);
    for event in events {
        app.handle(event);
    }

    let notice = app.ui.notice.clone().unwrap_or_default();
    assert!(notice.contains("no-results"), "{notice}");
    assert!(
        notice.contains("no results for obscure bootleg"),
        "{notice}"
    );
    assert_eq!(app.queue.len(), before);
}
