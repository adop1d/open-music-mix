//! Every documented config key loads back unchanged (7.6).

use omm::persist::Config;
use omm::ui::keymap::{Action, Key, Keymap};

mod common;

fn write(dir: &common::TempDir, text: &str) {
    std::fs::write(dir.join("config.toml"), text).unwrap();
}

#[test]
fn every_documented_config_key_round_trips() {
    let dir = common::TempDir::new("config-keys");
    write(
        &dir,
        r#"
[ui]
language = "es"
theme = "default"

[playback]
volume_local = 40
volume_remote = 60
volume_step = 10

[mpd]
host = "10.0.0.5"
port = 6601
socket = "/tmp/omm-mpd.sock"
password = "secret"

[paths]
cache_dir = "/tmp/omm-cache"

[keys]
next = "]"
"#,
    );
    let (config, warnings) = Config::load(dir.path());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.ui.language.as_deref(), Some("es"));
    assert_eq!(config.ui.theme, "default");
    assert_eq!(config.playback.volume_local, 40);
    assert_eq!(config.playback.volume_remote, 60);
    assert_eq!(config.playback.volume_step, 10);
    assert_eq!(config.mpd.host, "10.0.0.5");
    assert_eq!(config.mpd.port, 6601);
    assert_eq!(
        config.mpd.socket.as_deref(),
        Some(std::path::Path::new("/tmp/omm-mpd.sock"))
    );
    assert_eq!(config.mpd.password.as_deref(), Some("secret"));
    assert_eq!(
        config.paths.cache_dir.as_deref(),
        Some(std::path::Path::new("/tmp/omm-cache"))
    );
    let keymap = Keymap::with_overrides(&config.keys);
    assert_eq!(keymap.key_for(Action::Next), Some(Key::Char(']')));
    assert_eq!(keymap.key_for(Action::MoveDown), Some(Key::Char('j')));
}

#[test]
fn every_action_key_override_round_trips() {
    let dir = common::TempDir::new("config-actions");
    for action in Action::ALL {
        let mut config = Config::default();
        config.keys.insert(action.id().to_string(), "z".to_string());
        write(&dir, &toml::to_string(&config).unwrap());
        let (loaded, warnings) = Config::load(dir.path());
        assert!(warnings.is_empty(), "{}: {warnings:?}", action.id());
        let keymap = Keymap::with_overrides(&loaded.keys);
        assert_eq!(
            keymap.key_for(action),
            Some(Key::Char('z')),
            "{} did not round-trip",
            action.id()
        );
        // Re-binding a key removes it from the action that had it before.
        assert_eq!(keymap.action_for(Key::Char('z')), Some(action));
        assert_eq!(keymap.bindings().len(), Action::ALL.len());
    }
}

#[test]
fn unknown_and_invalid_overrides_are_ignored() {
    let dir = common::TempDir::new("config-invalid-keys");
    write(
        &dir,
        r#"
[keys]
not_an_action = "z"
next = "not-a-key"
"#,
    );
    let (config, warnings) = Config::load(dir.path());
    assert!(warnings.is_empty(), "{warnings:?}");
    let keymap = Keymap::with_overrides(&config.keys);
    assert_eq!(keymap.key_for(Action::Next), Some(Key::Char('n')));
}

#[test]
fn defaults_apply_when_the_file_is_missing() {
    let dir = common::TempDir::new("config-defaults");
    let (config, warnings) = Config::load(dir.path());
    assert!(warnings.is_empty());
    assert_eq!(config, Config::default());
    assert_eq!(config.playback.volume_step, 5);
    assert_eq!(config.mpd.port, 6600);
}
