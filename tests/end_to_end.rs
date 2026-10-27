//! Task 8.2: one offline session covering all three sources, playback, import
//! and persistence.

mod common;

use omm::app::App;
use omm::event::AppEvent;
use omm::persist::state;
use omm::process::{FakeImporter, FakeMpd, FakeMpv, FakeResolver};
use omm::queue::Source;
use omm::worker::Worker;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use common::*;

/// Runs every queued command through the worker and feeds the events back.
async fn settle(app: &mut App, worker: &mut Worker, rx: &mut UnboundedReceiver<AppEvent>) {
    for _ in 0..10 {
        let commands = app.take_commands();
        if commands.is_empty() {
            break;
        }
        for command in commands {
            worker.handle_command(command).await;
        }
        for event in events(rx) {
            app.handle(event);
        }
    }
}

#[tokio::test]
async fn end_to_end_queue_playback_import_and_persistence() {
    let dir = TempDir::new("end-to-end");
    let mpd = FakeMpd::with_library(
        vec!["The Cure".to_string()],
        vec![album("The Cure", "Disintegration")],
        vec![song(
            "Plainsong",
            "The Cure",
            "Disintegration",
            "the-cure/plainsong.flac",
        )],
    );
    let resolver = FakeResolver::new()
        .with_candidate_for("lullaby", "vid00000001", Some(duration_of(200)))
        .with_candidate_for("plainsong", "vid00000009", Some(duration_of(200)));
    let importer = FakeImporter::new(collection(
        "pl-1",
        "Disintegration",
        vec![track("Plainsong", "plainsong", Some("vid00000009"))],
    ));
    let backends = fakes(
        mpd,
        FakeMpv::default(),
        resolver,
        importer,
        workspace_in(&dir),
    );
    let (tx, mut rx) = unbounded_channel();
    let mut worker = Worker::new(backends, tx, dir.path().to_path_buf());
    let mut session = app().with_library(library(
        &["The Cure"],
        vec![album("The Cure", "Disintegration")],
        vec![song(
            "Plainsong",
            "The Cure",
            "Disintegration",
            "the-cure/plainsong.flac",
        )],
    ));

    // Local: open artist, open album, play the song.
    press(&mut session, '1');
    press(&mut session, 'l');
    press(&mut session, 'l');
    press_key(&mut session, omm::ui::keymap::Key::Enter);
    settle(&mut session, &mut worker, &mut rx).await;

    // YouTube: search and play.
    press(&mut session, '2');
    press(&mut session, '/');
    type_text(&mut session, "lullaby");
    press_key(&mut session, omm::ui::keymap::Key::Enter);
    settle(&mut session, &mut worker, &mut rx).await;

    // Import: modal, then play the imported track.
    press(&mut session, 'i');
    type_text(
        &mut session,
        "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
    );
    press_key(&mut session, omm::ui::keymap::Key::Enter);
    settle(&mut session, &mut worker, &mut rx).await;
    press(&mut session, '3');
    press(&mut session, 'l');
    press_key(&mut session, omm::ui::keymap::Key::Enter);
    settle(&mut session, &mut worker, &mut rx).await;

    assert_eq!(session.queue.len(), 3, "{:?}", session.ui.notice);
    let sources: Vec<_> = session
        .queue
        .entries
        .iter()
        .map(|entry| entry.source)
        .collect();
    assert_eq!(sources, [Source::Local, Source::Youtube, Source::Spotify]);
    assert_eq!(session.collections.len(), 1);
    assert_eq!(session.queue.history.len(), 3);
    assert_eq!(session.queue.history.front().unwrap().title, "Plainsong");

    let frame = render(&mut session, 120, 30);
    for needle in ["[local]", "[youtube]", "[spotify]", "Plainsong"] {
        assert!(frame.contains(needle), "missing {needle}:\n{frame}");
    }

    // Persistence round-trip keeps the queue and history.
    state::save(dir.path(), &session.state()).unwrap();
    let (restored, warnings) = state::load(dir.path());
    assert!(warnings.is_empty(), "{warnings:?}");
    let restarted = app().with_restored(restored);
    assert_eq!(restarted.queue.len(), 3);
    assert_eq!(restarted.queue.history.len(), 3);
    assert_eq!(restarted.tab, Source::Spotify);
}
