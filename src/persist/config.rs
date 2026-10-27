//! Config defaults, file merge and per-key validation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::atomic::write_atomic;

pub const CONFIG_FILE: &str = "config.toml";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub ui: UiConfig,
    pub playback: PlaybackConfig,
    pub mpd: MpdConfig,
    pub paths: PathsConfig,
    pub keys: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub language: Option<String>,
    pub theme: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    pub volume_local: i16,
    pub volume_remote: i16,
    pub volume_step: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MpdConfig {
    pub host: String,
    pub port: u16,
    pub socket: Option<PathBuf>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct PathsConfig {
    pub cache_dir: Option<PathBuf>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: None,
            theme: "default".to_string(),
        }
    }
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            volume_local: 70,
            volume_remote: 70,
            volume_step: 5,
        }
    }
}

impl Default for MpdConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6600,
            socket: None,
            password: None,
        }
    }
}

impl Config {
    /// Defaults -> file -> environment. Invalid values warn and fall back per key.
    pub fn load(dir: &Path) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let path = dir.join(CONFIG_FILE);
        let mut config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(text) => match toml::from_str::<Config>(&text) {
                    Ok(config) => config,
                    Err(error) => {
                        warnings.push(format!("{}: {error}", path.display()));
                        let _ = quarantine(&path);
                        Config::default()
                    }
                },
                Err(error) => {
                    warnings.push(format!("{}: {error}", path.display()));
                    Config::default()
                }
            }
        } else {
            Config::default()
        };

        config.validate(&mut warnings);
        config.apply_env();
        (config, warnings)
    }

    /// Write the default file when missing so the first run creates its directory.
    pub fn ensure_file(dir: &Path) -> std::io::Result<()> {
        let path = dir.join(CONFIG_FILE);
        if path.exists() {
            return Ok(());
        }
        let text = toml::to_string_pretty(&Config::default())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        write_atomic(&path, text.as_bytes())
    }

    fn validate(&mut self, warnings: &mut Vec<String>) {
        if !(0..=100).contains(&self.playback.volume_local) {
            warnings.push(format!(
                "playback.volume_local={} (using {})",
                self.playback.volume_local,
                PlaybackConfig::default().volume_local
            ));
            self.playback.volume_local = PlaybackConfig::default().volume_local;
        }
        if !(0..=100).contains(&self.playback.volume_remote) {
            warnings.push(format!(
                "playback.volume_remote={} (using {})",
                self.playback.volume_remote,
                PlaybackConfig::default().volume_remote
            ));
            self.playback.volume_remote = PlaybackConfig::default().volume_remote;
        }
        if !(1..=50).contains(&self.playback.volume_step) {
            warnings.push(format!(
                "playback.volume_step={} (using {})",
                self.playback.volume_step,
                PlaybackConfig::default().volume_step
            ));
            self.playback.volume_step = PlaybackConfig::default().volume_step;
        }
        if let Some(language) = &self.ui.language
            && !matches!(language.as_str(), "en" | "es")
        {
            warnings.push(format!("ui.language={language} (using en)"));
            self.ui.language = None;
        }
        if self.mpd.port == 0 {
            warnings.push("mpd.port=0 (using 6600)".to_string());
            self.mpd.port = MpdConfig::default().port;
        }
    }

    fn apply_env(&mut self) {
        if let Ok(host) = std::env::var("MPD_HOST")
            && !host.is_empty()
        {
            if host.starts_with('/') {
                self.mpd.socket = Some(PathBuf::from(host));
            } else {
                self.mpd.host = host;
            }
        }
        if let Ok(port) = std::env::var("MPD_PORT")
            && let Ok(port) = port.parse::<u16>()
            && port > 0
        {
            self.mpd.port = port;
        }
    }

    pub fn volume_for(&self, source: crate::queue::Source) -> u8 {
        let value = match source {
            crate::queue::Source::Local => self.playback.volume_local,
            _ => self.playback.volume_remote,
        };
        value.clamp(0, 100) as u8
    }

    pub fn set_volume_for(&mut self, source: crate::queue::Source, volume: u8) {
        match source {
            crate::queue::Source::Local => self.playback.volume_local = volume as i16,
            _ => self.playback.volume_remote = volume as i16,
        }
    }
}

fn quarantine(path: &Path) -> std::io::Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let target = path.with_extension(format!("corrupt-{stamp}"));
    std::fs::rename(path, &target)?;
    Ok(target)
}
