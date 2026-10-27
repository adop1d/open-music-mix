//! Replace-file atomic writes: temp file in the same directory, flush, rename.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let temporary = parent.join(format!(".{name}.tmp"));
    {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)
}
