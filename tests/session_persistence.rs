mod common;

use std::path::Path;
use std::process::Command;

use omm::app::App;
use omm::i18n::I18n;
use omm::persist::config::CONFIG_FILE;
use omm::persist::state::{STATE_FILE, StateFile, Volumes};
use omm::persist::{self, Config, atomic, imports, state};
use omm::process::PlayerState;
use omm::queue::{EntryState, Locator, Source};
use omm::ui::keymap::Key;

use common::*;

fn seeded_app() -> App {
    let mut app = app();
    app.queue.append(
        Source::Local,
        meta("Plainsong", "The Cure"),
        Locator::MpdUri("cure/plainsong.flac".to_string()),
    );
    app.queue.append(
        Source::Youtube,
        meta("Lullaby", "The Cure"),
        Locator::YoutubeId("vid00000001".to_string()),
    );
    app.queue.append(
        Source::Spotify,
        meta("Pictures of You", "The Cure"),
        Locator::SpotifyTrack("the cure pictures of you".to_string()),
    );
    app.queue.set_position(1);
    app
}

#[test]
fn first_run_creates_the_state_directory() {
    let dir = TempDir::new("first-run");
    let root = dir.join("config");
    assert!(!root.exists());

    persist::ensure_layout(&root).unwrap();
    Config::ensure_file(&root).unwrap();

    assert!(root.is_dir());
    assert!(root.join("imports").is_dir());
    assert!(root.join(CONFIG_FILE).is_file());

    let (config, warnings) = Config::load(&root);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.playback.volume_local, 70);

    let (restored, warnings) = state::load(&root);
    assert!(warnings.is_empty());
    assert!(restored.queue.is_empty());
    state::save(&root, &restored).unwrap();
    assert!(root.join(STATE_FILE).is_file());
}

#[test]
fn no_writes_in_the_working_directory() {
    let dir = TempDir::new("cwd-writes");
    let cwd = dir.join("cwd");
    let config_dir = dir.join("config");
    std::fs::create_dir_all(&cwd).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_omm"))
        .args([
            "--import",
            "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
        ])
        .env("OMM_CONFIG_DIR", &config_dir)
        .env("PATH", "")
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert!(!status.status.success());

    let leftovers: Vec<_> = std::fs::read_dir(&cwd)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert!(config_dir.join(CONFIG_FILE).is_file());
    assert!(config_dir.join("imports").is_dir());
}

#[test]
fn queue_restored_after_restart() {
    let dir = TempDir::new("restore-queue");
    let seeded = seeded_app();
    state::save(dir.path(), &seeded.state()).unwrap();

    let (restored, warnings) = state::load(dir.path());
    assert!(warnings.is_empty(), "{warnings:?}");
    let restarted = app().with_restored(restored);
    assert_eq!(restarted.queue.len(), 3);
    assert_eq!(restarted.queue.entries[0].meta.title, "Plainsong");
    assert_eq!(restarted.queue.entries[1].source, Source::Youtube);
    assert_eq!(restarted.queue.entries[2].source, Source::Spotify);
    assert_eq!(restarted.queue.position, Some(1));
}

#[test]
fn startup_does_not_resolve_remote_entries() {
    let dir = TempDir::new("no-resolve");
    let file = StateFile {
        queue: seeded_app().queue.entries,
        position: Some(1),
        history: Vec::new(),
        volumes: Volumes::default(),
        tab: Some(Source::Youtube),
        ..StateFile::default()
    };
    state::save(dir.path(), &file).unwrap();
    let (restored, _) = state::load(dir.path());

    let mut app = app().with_restored(restored);
    assert_eq!(app.take_commands().len(), 0);
    assert_eq!(app.playback.state, PlayerState::Paused);
    assert_eq!(app.playback.entry_id, Some(app.queue.entries[1].id));
    assert_eq!(app.queue.len(), 3);
}

#[test]
fn restored_session_starts_paused() {
    let dir = TempDir::new("restored-paused");
    let mut file = seeded_app().state();
    file.queue[0].state = EntryState::Playing;
    file.position = Some(0);
    state::save(dir.path(), &file).unwrap();

    let (restored, _) = state::load(dir.path());
    assert_eq!(restored.queue[0].state, EntryState::Paused);
    let mut app = app().with_restored(restored);
    assert_eq!(app.playback.state, PlayerState::Paused);
    assert!(!app.playback.loaded);
    assert!(app.take_commands().is_empty());

    press_key(&mut app, Key::Space);
    let commands = app.take_commands();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, omm::event::Command::PlayLocal { .. })),
        "{commands:?}"
    );
}

#[test]
fn interrupted_write() {
    let dir = TempDir::new("interrupted");
    let seeded = seeded_app();
    state::save(dir.path(), &seeded.state()).unwrap();
    let before = std::fs::read(dir.join(STATE_FILE)).unwrap();

    // A leftover temp file from a killed process is ignored by load.
    std::fs::write(dir.join(".state.json.tmp"), b"{\"half\":").unwrap();
    let (restored, warnings) = state::load(dir.path());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(restored.queue.len(), 3);

    // A failed replace leaves the previous file byte-for-byte intact.
    std::fs::write(dir.join("blocker"), b"not a directory").unwrap();
    let unwritable = dir.join("blocker").join(STATE_FILE);
    assert!(atomic::write_atomic(&unwritable, b"{\"format\":1}").is_err());
    assert_eq!(std::fs::read(dir.join(STATE_FILE)).unwrap(), before);
}

#[test]
fn corrupt_state() {
    let dir = TempDir::new("corrupt");
    std::fs::write(dir.join(STATE_FILE), b"{ this is not json").unwrap();
    let (restored, warnings) = state::load(dir.path());
    assert!(restored.queue.is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(!dir.join(STATE_FILE).exists());
    let quarantined: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.contains("corrupt"))
        .collect();
    assert_eq!(quarantined.len(), 1, "{quarantined:?}");
}

#[test]
fn invalid_configuration_value() {
    let dir = TempDir::new("invalid-config");
    persist::ensure_layout(dir.path()).unwrap();
    std::fs::write(
        dir.join(CONFIG_FILE),
        "[playback]\nvolume_local = 999\nvolume_remote = -4\nvolume_step = 0\n\
         [ui]\nlanguage = \"fr\"\n[mpd]\nport = 0\n",
    )
    .unwrap();

    let (config, warnings) = Config::load(dir.path());
    assert_eq!(config.playback.volume_local, 70);
    assert_eq!(config.playback.volume_remote, 70);
    assert_eq!(config.playback.volume_step, 5);
    assert_eq!(config.mpd.port, 6600);
    for key in [
        "playback.volume_local",
        "playback.volume_remote",
        "playback.volume_step",
        "ui.language",
        "mpd.port",
    ] {
        assert!(
            warnings.iter().any(|warning| warning.contains(key)),
            "missing {key}: {warnings:?}"
        );
    }

    let mut running = app_with(config);
    let frame = render(&mut running, 100, 30);
    assert!(frame.contains("Queue"), "{frame}");
}

#[test]
fn newer_state_version() {
    let dir = TempDir::new("newer-format");
    let text = "{\n  \"format\": 2,\n  \"queue\": [],\n  \"position\": null\n}\n";
    std::fs::write(dir.join(STATE_FILE), text).unwrap();

    let (restored, warnings) = state::load(dir.path());
    assert!(restored.queue.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("format 2"), "{warnings:?}");
    assert_eq!(
        std::fs::read_to_string(dir.join(STATE_FILE)).unwrap(),
        text,
        "a newer file must never be rewritten"
    );
    assert!(
        !std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().contains("corrupt"))
    );
}

#[allow(dead_code)]
fn _keep(imports_dir: &Path, i18n: &I18n) {
    let _ = imports::load_all(imports_dir);
    let _ = i18n;
}
