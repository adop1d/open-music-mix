//! Imported collections live next to state, one file per collection.

use std::path::Path;

use crate::process::Collection;

use super::atomic::write_atomic;

pub fn save(dir: &Path, collection: &Collection) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(collection).map_err(std::io::Error::other)?;
    let path = dir
        .join("imports")
        .join(format!("{}.json", slug(&collection.id)));
    write_atomic(&path, text.as_bytes())
}

pub fn load_all(dir: &Path) -> Vec<Collection> {
    let directory = dir.join("imports");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut collections: Vec<Collection> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension == "json")
                .unwrap_or(false)
        })
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str::<Collection>(&text).ok())
        .collect();
    collections.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    collections
}

pub fn find(dir: &Path, id: &str) -> Option<Collection> {
    load_all(dir)
        .into_iter()
        .find(|collection| collection.id == id)
}

fn slug(value: &str) -> String {
    let mut slug: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').to_string()
}
