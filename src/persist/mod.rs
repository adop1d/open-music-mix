//! Configuration, session state and imported collections on disk.

pub mod atomic;
pub mod config;
pub mod imports;
pub mod state;

use std::path::PathBuf;

pub use config::Config;
pub use state::{StateFile, Volumes};

pub const APP_DIR: &str = "omm";

/// Platform config directory; `OMM_CONFIG_DIR` overrides it for tests.
pub fn config_dir() -> PathBuf {
    if let Ok(path) = std::env::var("OMM_CONFIG_DIR")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    directories::ProjectDirs::from("", "", APP_DIR)
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(format!(".{APP_DIR}")))
}

pub fn ensure_layout(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("imports"))?;
    Ok(())
}
