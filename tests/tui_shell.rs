//! tui-shell spec: layout, keymap, overlays, i18n and responsiveness.

mod common;

use std::io;

use common::*;
use omm::app::Panel;
use omm::event::{AppEvent, Command as AppCommand, Reason, SourceStatus};
use omm::i18n::{I18n, Language};
use omm::persist::Config;
use omm::queue::{EntryState, Source};
use omm::ui::keymap::{Action, Key};

#[test]
fn default_terminal_size() {
    let mut app = app();
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("Explorer"), "{frame}");
    assert!(frame.contains("Queue"), "{frame}");
    assert!(frame.contains("Now Playing"), "{frame}");
}

#[test]
fn small_terminal() {
    let mut app = app();
    let frame = render(&mut app, 60, 18);
    assert!(frame.contains("Queue"), "{frame}");
    assert!(frame.contains("Now Playing"), "{frame}");
    assert!(!frame.contains("Explorer"), "{frame}");

    // The explorer is still reachable by cycling focus, as the help says.
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(app.focus);
        press_key(&mut app, Key::Tab);
    }
    assert!(seen.contains(&Panel::Explorer));
    assert!(!render(&mut app, 60, 18).contains("panicked"));
}

#[test]
fn switching_to_the_youtube_tab() {
    let mut app = app();
    press(&mut app, '2');
    assert_eq!(app.tab, Source::Youtube);
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("Search"), "{frame}");
    assert!(frame.contains("Queue"), "{frame}");
    assert!(frame.contains("Now Playing"), "{frame}");
}

#[test]
fn disabled_tab_is_still_reachable() {
    let mut app = app().with_status(
        Source::Local,
        SourceStatus::Unavailable(Reason::new("msg_mpd_unavailable")),
    );
    press(&mut app, '1');
    assert_eq!(app.tab, Source::Local);
    assert!(!app.is_available(Source::Local));
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("MPD is not available"), "{frame}");
    assert!(frame.contains("Queue"), "{frame}");
    assert!(frame.contains("Now Playing"), "{frame}");
}

#[test]
fn moving_in_a_list() {
    let mut app = app().with_library(library(&["The Cure", "Queen", "Blur"], vec![], vec![]));
    press(&mut app, '1');
    press(&mut app, 'j');
    press(&mut app, 'j');
    assert_eq!(app.local.selection, 2);
    let frame = render(&mut app, 80, 24);
    // The header stays in view, so the user knows which list they move in.
    assert!(frame.contains("Artists"), "{frame}");
    assert!(frame.contains("Blur"), "{frame}");
}

#[test]
fn overriding_a_binding() {
    let mut config = Config::default();
    config.keys.insert("next".to_string(), "]".to_string());
    let mut app = app_with(config);
    assert_eq!(app.keymap.key_for(Action::Next), Some(Key::Char(']')));
    assert_eq!(app.keymap.action_for(Key::Char('n')), None);
    assert_eq!(app.keymap.action_for(Key::Char(']')), Some(Action::Next));

    remote_queue(&mut app, 2);
    app.play_index(0);
    let _ = app.take_commands();
    press(&mut app, 'n');
    assert_eq!(app.queue.position, Some(0));
    press(&mut app, ']');
    assert_eq!(app.queue.position, Some(1));
}

#[test]
fn every_action_reachable_without_a_mouse() {
    let app = app();
    assert_eq!(Action::ALL.len(), 26);
    for action in Action::ALL {
        assert!(
            app.keymap.key_for(action).is_some(),
            "{} has no key",
            action.id()
        );
    }
    // One key per action, so help and dispatch never disagree.
    assert_eq!(app.keymap.bindings().len(), Action::ALL.len());
}

#[test]
fn queue_editing_from_the_keyboard() {
    let songs = vec![
        song("Plainsong", "The Cure", "Disintegration", "the-cure/1.flac"),
        song("Lovesong", "The Cure", "Disintegration", "the-cure/2.flac"),
    ];
    let mut app = app().with_library(library(
        &["The Cure"],
        vec![album("The Cure", "Disintegration")],
        songs,
    ));
    press(&mut app, '1');
    press(&mut app, 'l');
    press(&mut app, 'a');
    assert_eq!(app.queue.len(), 2);
    assert!(app.playback.entry_id.is_none());
    assert!(!app.take_commands().iter().any(|command| matches!(
        command,
        AppCommand::PlayLocal { .. } | AppCommand::PlayRemote { .. }
    )));
}

#[test]
fn removing_an_entry() {
    let mut app = app();
    remote_queue(&mut app, 3);
    press_key(&mut app, Key::Tab);
    press(&mut app, 'j');
    press(&mut app, 'd');
    assert_eq!(app.queue.len(), 2);
    assert!(
        app.queue
            .entries
            .iter()
            .all(|entry| entry.meta.title != "Track 2")
    );
}

#[test]
fn changing_volume() {
    let mut app = app();
    press(&mut app, '1');
    let before_local = app.config.volume_for(Source::Local);
    let before_remote = app.config.volume_for(Source::Youtube);
    press(&mut app, '+');
    assert_eq!(
        app.config.volume_for(Source::Local),
        (before_local + 5).min(100)
    );
    assert_eq!(app.config.volume_for(Source::Youtube), before_remote);
    let commands = app.take_commands();
    assert!(matches!(
        commands.as_slice(),
        [AppCommand::Volume {
            source: Source::Local,
            ..
        }]
    ));
}

#[test]
fn retrying_a_failed_entry() {
    let mut app = app();
    remote_queue(&mut app, 2);
    let id = app.queue.entries[0].id;
    app.queue.entries[0].state = EntryState::Failed("no-results".to_string());
    press_key(&mut app, Key::Tab);
    press(&mut app, 'r');
    assert_eq!(app.queue.entries[0].state, EntryState::Pending);
    let commands = app.take_commands();
    assert!(matches!(
        commands.as_slice(),
        [AppCommand::PlayRemote { .. }]
    ));

    app.handle(AppEvent::PlaybackStarted { entry_id: id });
    assert_eq!(app.queue.entries[0].state, EntryState::Playing);
}

#[test]
fn opening_help_during_playback() {
    let mut app = app();
    remote_queue(&mut app, 2);
    let id = app.queue.entries[0].id;
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted { entry_id: id });
    press(&mut app, '?');
    assert!(app.ui.help_open);
    let frame = render(&mut app, 100, 30);
    assert!(frame.contains("Help"), "{frame}");
    assert!(frame.contains("Move down"), "{frame}");
    assert_eq!(app.playback.state, omm::process::PlayerState::Playing);
}

#[test]
fn closing_help() {
    let mut app = app();
    remote_queue(&mut app, 3);
    press_key(&mut app, Key::Tab);
    press(&mut app, 'j');
    press(&mut app, 'j');
    press(&mut app, '?');
    assert!(app.ui.help_open);
    press_key(&mut app, Key::Esc);
    assert!(!app.ui.help_open);
    assert_eq!(app.focus, Panel::Queue);
    assert_eq!(app.queue.selection, 2);
}

#[test]
fn filtering_a_queue() {
    let mut app = app();
    app.queue.append(
        Source::Youtube,
        meta("Friday I'm in Love", "The Cure"),
        omm::queue::Locator::YoutubeQuery("friday".to_string()),
    );
    app.queue.append(
        Source::Youtube,
        meta("Bohemian Rhapsody", "Queen"),
        omm::queue::Locator::YoutubeQuery("bohemian".to_string()),
    );
    press_key(&mut app, Key::Tab);
    press(&mut app, '/');
    type_text(&mut app, "cure");
    assert_eq!(app.ui.search.as_deref(), Some("cure"));
    assert_eq!(app.visible_queue_indices(), vec![0]);
    let frame = render(&mut app, 120, 24);
    assert!(frame.contains("Friday I'm in Love"), "{frame}");
    assert!(!frame.contains("Bohemian Rhapsody"), "{frame}");
}

#[test]
fn clearing_the_query() {
    let mut app = app();
    remote_queue(&mut app, 2);
    press_key(&mut app, Key::Tab);
    press(&mut app, '/');
    type_text(&mut app, "Track 1");
    assert_eq!(app.visible_queue_indices(), vec![0]);
    press_key(&mut app, Key::Esc);
    assert!(app.ui.search.is_none());
    assert_eq!(app.visible_queue_indices(), vec![0, 1]);
}

#[test]
fn spanish_interface() {
    let (i18n, warnings) = I18n::load(Some("es"), None);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut app = omm::app::App::new(Default::default(), i18n);
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("Explorador"), "{frame}");
    assert!(frame.contains("Cola"), "{frame}");
    assert!(frame.contains("Reproduciendo"), "{frame}");
    press(&mut app, '?');
    let frame = render(&mut app, 100, 30);
    assert!(frame.contains("Bajar"), "{frame}");
    assert!(frame.contains("Ayuda"), "{frame}");
}

#[test]
fn missing_translation() {
    let i18n = I18n::from_tables(
        "panel_queue = \"Queue\"\nonly_english = \"English fallback\"\n",
        "panel_queue = \"Cola\"\n",
        Language::Spanish,
        None,
    );
    assert_eq!(i18n.t("panel_queue"), "Cola");
    assert_eq!(i18n.t("only_english"), "English fallback");
    assert_eq!(i18n.t("not_a_key"), "not_a_key");
}

#[test]
fn user_override_file() {
    let dir = TempDir::new("i18n-override");
    std::fs::create_dir_all(dir.join("i18n")).unwrap();
    std::fs::write(dir.join("i18n/es.toml"), "panel_queue = \"Fila\"\n").unwrap();
    let (i18n, warnings) = I18n::load(Some("es"), Some(&dir));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(i18n.t("panel_queue"), "Fila");
    assert_eq!(i18n.t("panel_explorer"), "Explorador");
}

#[test]
fn invalid_override_file() {
    let dir = TempDir::new("i18n-invalid");
    std::fs::create_dir_all(dir.join("i18n")).unwrap();
    std::fs::write(dir.join("i18n/es.toml"), "not = [valid").unwrap();
    let (i18n, warnings) = I18n::load(Some("es"), Some(&dir));
    assert_eq!(i18n.t("panel_queue"), "Cola");
    assert!(
        warnings.iter().any(|warning| warning.contains("es.toml")),
        "{warnings:?}"
    );
}

#[test]
fn typing_during_a_slow_import() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut app = app();
        remote_queue(&mut app, 3);
        press_key(&mut app, Key::Tab);
        let (event_tx, mut event_rx) = event_channel();
        let (command_tx, _command_rx) = tokio::sync::mpsc::unbounded_channel::<AppCommand>();
        event_tx
            .send(AppEvent::ImportProgress {
                name: "Slow playlist".to_string(),
                matched: 2,
                total: 40,
            })
            .unwrap();
        for character in "jj".chars() {
            event_tx.send(AppEvent::Key(Key::Char(character))).unwrap();
        }

        let mut frames = 0;
        let running = omm::event_loop::pump(
            &mut app,
            &mut event_rx,
            &command_tx,
            &mut |_app: &mut omm::app::App| -> io::Result<()> {
                frames += 1;
                Ok(())
            },
        )
        .await
        .unwrap();

        assert!(running);
        assert_eq!(app.queue.selection, 2);
        assert_eq!(app.ui.import_status.as_ref().unwrap().matched, 2);
        assert_eq!(frames, 1);
        assert!(app.ui.notice.as_deref().unwrap().contains("2 / 40"));
    });
}
