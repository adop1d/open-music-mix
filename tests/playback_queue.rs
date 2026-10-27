//! Scenarios for the unified queue: ordering, history and failure recovery.

mod common;

use std::time::Duration;

use common::*;
use omm::app::Panel;
use omm::event::{AppEvent, Command};
use omm::queue::{Locator, Source};
use omm::ui::keymap::Key;

#[test]
fn queue_mixes_entries_from_all_three_sources() {
    let mut app = app();
    app.queue.append(
        Source::Local,
        meta("Local Song", "Artist"),
        Locator::MpdUri("song.flac".to_string()),
    );
    app.queue.append(
        Source::Youtube,
        meta("Video Song", "Uploader"),
        Locator::YoutubeId("abc12345678".to_string()),
    );
    app.queue.append(
        Source::Spotify,
        meta("Imported Song", "Artist"),
        Locator::YoutubeId("xyz12345678".to_string()),
    );

    let sources: Vec<Source> = app.queue.entries.iter().map(|entry| entry.source).collect();
    assert_eq!(
        sources,
        vec![Source::Local, Source::Youtube, Source::Spotify]
    );
    assert_eq!(app.queue.len(), 3);
}

#[test]
fn source_remains_visible_while_playing() {
    let mut app = app();
    app.tab = Source::Spotify;
    app.queue.append(
        Source::Spotify,
        meta("Friday I'm in Love", "The Cure"),
        Locator::YoutubeId("abc12345678".to_string()),
    );
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });

    let text = render(&mut app, 100, 30);
    assert!(text.contains("[spotify]"), "source label missing: {text}");
    assert!(text.contains("Explorer"), "explorer missing: {text}");
    assert!(text.contains("Friday I'm in Love"));
}

#[test]
fn next_advances_within_the_queue() {
    let mut app = app();
    remote_queue(&mut app, 3);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    let _ = app.take_commands();

    press(&mut app, 'n');

    assert_eq!(app.queue.position, Some(1));
    assert_eq!(app.playback.title.as_deref(), Some("Track 2"));
    let commands = app.take_commands();
    assert!(matches!(
        commands.as_slice(),
        [Command::PlayRemote { entry_id, .. }] if *entry_id == app.queue.entries[1].id
    ));
}

#[test]
fn previous_at_the_first_entry() {
    let mut app = app();
    remote_queue(&mut app, 2);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    let _ = app.take_commands();

    press(&mut app, 'p');

    assert_eq!(app.queue.position, Some(0));
    assert_eq!(app.playback.title.as_deref(), Some("Track 1"));
    let commands = app.take_commands();
    assert!(matches!(
        commands.as_slice(),
        [Command::PlayRemote { entry_id, .. }] if *entry_id == app.queue.entries[0].id
    ));
}

#[test]
fn seek_on_a_remote_entry() {
    let mut app = app();
    remote_queue(&mut app, 1);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    app.handle(AppEvent::PlaybackProgress {
        state: omm::process::PlayerState::Playing,
        position: Duration::from_secs(85),
        duration: Some(Duration::from_secs(200)),
    });
    let _ = app.take_commands();

    press_key(&mut app, Key::Right);

    let commands = app.take_commands();
    assert!(
        matches!(
            commands.as_slice(),
            [Command::Seek { source: Source::Youtube, position }] if *position == Duration::from_secs(90)
        ),
        "unexpected commands: {commands:?}"
    );
    assert_eq!(app.playback.position, Duration::from_secs(90));
}

#[test]
fn volume_change_applies_to_the_active_source() {
    let mut app = app();
    app.playback.source = Some(Source::Local);
    let local_before = app.config.volume_for(Source::Local);
    let remote_before = app.config.volume_for(Source::Youtube);

    press(&mut app, '+');

    assert_eq!(app.config.volume_for(Source::Local), local_before + 5);
    assert_eq!(app.config.volume_for(Source::Youtube), remote_before);
    let commands = app.take_commands();
    assert!(matches!(
        commands.as_slice(),
        [Command::Volume {
            source: Source::Local,
            volume: 75
        }]
    ));
}

#[test]
fn removing_the_current_entry() {
    let mut app = app();
    remote_queue(&mut app, 3);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    app.focus = Panel::Queue;
    app.queue.selection = 0;
    let _ = app.take_commands();

    press(&mut app, 'd');

    assert_eq!(app.queue.len(), 2);
    assert_eq!(app.queue.position, Some(0));
    assert_eq!(app.playback.title.as_deref(), Some("Track 2"));
    let commands = app.take_commands();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::PlayRemote { .. })),
        "next entry was not started: {commands:?}"
    );
}

#[test]
fn clearing_the_queue_stops_playback() {
    let mut app = app();
    remote_queue(&mut app, 2);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    let _ = app.take_commands();

    press(&mut app, 'c');

    assert!(app.queue.is_empty());
    assert_eq!(app.playback.entry_id, None);
    assert_eq!(app.playback.state, omm::process::PlayerState::Stopped);
    let commands = app.take_commands();
    assert!(commands.iter().any(|command| matches!(
        command,
        Command::Stop {
            source: Source::Youtube
        }
    )));
}

#[test]
fn playing_tracks_records_history() {
    let mut app = app();
    remote_queue(&mut app, 3);
    for index in 0..3 {
        let id = app.queue.entries[index].id;
        app.play_index(index);
        app.handle(AppEvent::PlaybackStarted { entry_id: id });
    }

    let titles: Vec<&str> = app
        .queue
        .history()
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Track 3", "Track 2", "Track 1"]);
}

#[test]
fn history_survives_a_restart() {
    let directory = TempDir::new("history");
    let mut player = app();
    remote_queue(&mut player, 2);
    for index in 0..2 {
        let id = player.queue.entries[index].id;
        player.play_index(index);
        player.handle(AppEvent::PlaybackStarted { entry_id: id });
    }
    let state = player.state();
    omm::persist::state::save(&directory, &state).unwrap();

    let (loaded, warnings) = omm::persist::state::load(&directory);
    assert!(warnings.is_empty(), "{warnings:?}");
    let restored = app().with_restored(loaded);

    let titles: Vec<&str> = restored
        .queue
        .history()
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Track 2", "Track 1"]);
}

#[test]
fn history_is_bounded() {
    let mut app = app();
    remote_queue(&mut app, 1);
    for index in 0..250 {
        let mut entry = app.queue.entries[0].clone();
        entry.meta.title = format!("Song {index}");
        app.queue.record_history(&entry, 1);
    }

    assert_eq!(app.queue.history().len(), omm::queue::HISTORY_LIMIT);
    assert_eq!(app.queue.history().front().unwrap().title, "Song 249");
    assert_eq!(app.queue.history().back().unwrap().title, "Song 50");
}

#[test]
fn replaying_a_track_adds_a_new_entry() {
    let mut app = app();
    remote_queue(&mut app, 1);
    let id = app.queue.entries[0].id;
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted { entry_id: id });
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted { entry_id: id });

    assert_eq!(app.queue.history().len(), 2);
    assert_eq!(app.queue.entries.len(), 1);
}

#[test]
fn failed_resolution_skips_to_the_next_entry() {
    let mut app = app();
    remote_queue(&mut app, 2);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });
    let _ = app.take_commands();

    app.handle(AppEvent::EntryFailed {
        entry_id: app.queue.entries[0].id,
        reason: "no-results".to_string(),
    });

    assert!(app.queue.entries[0].state.is_failed());
    assert_eq!(app.queue.position, Some(1));
    let commands = app.take_commands();
    assert!(commands.iter().any(
        |command| matches!(command, Command::PlayRemote { entry_id, .. } if *entry_id == app.queue.entries[1].id)
    ));
}

#[test]
fn last_entry_fails() {
    let mut app = app();
    remote_queue(&mut app, 1);
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted {
        entry_id: app.queue.entries[0].id,
    });

    app.handle(AppEvent::EntryFailed {
        entry_id: app.queue.entries[0].id,
        reason: "unreachable".to_string(),
    });

    assert!(app.queue.entries[0].state.is_failed());
    assert_eq!(app.playback.state, omm::process::PlayerState::Paused);
    assert!(!app.quit);
}
