//! Backend bundle and the temporary workspace used for art and audio buffers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::persist::Config;
use crate::process::{
    FakeImporter, FakeMpd, FakeMpv, FakeResolver, RealMpd, RealMpv, RealResolver,
    RealSpotifyImporter, Resolver, SpotifyImporter,
};
use crate::sources::{SharedMpd, SharedMpv};

/// Everything written below this directory is temporary and removed on exit.
pub struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("omm-{}-{stamp}", std::process::id()));
        let workspace = Self { root };
        let _ = std::fs::create_dir_all(workspace.audio_dir());
        let _ = std::fs::create_dir_all(workspace.art_dir());
        workspace
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let workspace = Self { root };
        let _ = std::fs::create_dir_all(workspace.audio_dir());
        let _ = std::fs::create_dir_all(workspace.art_dir());
        workspace
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn audio_dir(&self) -> PathBuf {
        self.root.join("audio")
    }

    pub fn art_dir(&self) -> PathBuf {
        self.root.join("art")
    }

    /// Called when the entry changes: the previous buffer must not survive.
    pub fn clear_audio(&self) {
        let _ = std::fs::remove_dir_all(self.audio_dir());
        let _ = std::fs::create_dir_all(self.audio_dir());
    }

    pub fn clear_all(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Default for TempWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub struct Backends {
    pub mpd: SharedMpd,
    pub mpv: SharedMpv,
    pub resolver: Arc<dyn Resolver>,
    pub importer: Arc<dyn SpotifyImporter>,
    pub workspace: Arc<TempWorkspace>,
}

impl Backends {
    pub fn real(config: &Config, workspace: TempWorkspace) -> Self {
        let mpd = RealMpd::new(crate::process::mpd::MpdSettings {
            host: config.mpd.host.clone(),
            port: config.mpd.port,
            socket: config.mpd.socket.clone(),
            password: config.mpd.password.clone(),
        });
        let mpv = RealMpv::new(workspace.root());
        if let Some(cache) = &config.paths.cache_dir {
            let _ = std::fs::create_dir_all(cache);
        }
        Self {
            mpd: Arc::new(std::sync::Mutex::new(mpd)),
            mpv: Arc::new(std::sync::Mutex::new(mpv)),
            resolver: Arc::new(RealResolver::new()),
            importer: Arc::new(RealSpotifyImporter::new()),
            workspace: Arc::new(workspace),
        }
    }

    /// All-fake bundle for the offline suite.
    pub fn fakes(
        mpd: FakeMpd,
        mpv: FakeMpv,
        resolver: FakeResolver,
        importer: FakeImporter,
        workspace: TempWorkspace,
    ) -> Self {
        Self {
            mpd: Arc::new(std::sync::Mutex::new(mpd)),
            mpv: Arc::new(std::sync::Mutex::new(mpv)),
            resolver: Arc::new(resolver),
            importer: Arc::new(importer),
            workspace: Arc::new(workspace),
        }
    }

    pub fn spawn_player(&self) -> Result<(), crate::process::SourceError> {
        self.mpv.lock().unwrap().spawn()
    }
}

/// Search PATH for an executable by name.
pub fn find_binary(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
