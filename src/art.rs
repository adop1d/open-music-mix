//! Album art: protocol detection, decoding and a bounded decoded-image cache.

use std::collections::VecDeque;

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::queue::EntryId;

const CACHE_LIMIT: usize = 20;
const MIN_PROTOCOL_WIDTH: u16 = 20;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArtKey {
    pub source: crate::queue::Source,
    pub album: Option<String>,
    pub path: String,
}

pub struct ArtRenderer {
    picker: Picker,
}

impl Default for ArtRenderer {
    fn default() -> Self {
        Self::halfblocks()
    }
}

impl ArtRenderer {
    /// Environment hints only: `Picker::from_query_stdio` blocks forever on
    /// terminals that ignore its query (dumb ptys, some SSH sessions).
    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default().to_lowercase();
        let program = std::env::var("TERM_PROGRAM")
            .unwrap_or_default()
            .to_lowercase();
        let kitty = std::env::var_os("KITTY_WINDOW_ID").is_some()
            || ["kitty", "wezterm", "ghostty", "konsole"]
                .iter()
                .any(|name| term.contains(name) || program.contains(name));
        let mut picker = Picker::halfblocks();
        if kitty {
            picker.set_protocol_type(ProtocolType::Kitty);
        }
        Self { picker }
    }

    pub fn halfblocks() -> Self {
        Self {
            picker: Picker::halfblocks(),
        }
    }

    pub fn from_picker(picker: Picker) -> Self {
        Self { picker }
    }

    pub fn protocol_type(&self) -> ProtocolType {
        self.picker.protocol_type()
    }

    /// A protocol needs a minimum width; narrower panels downgrade to half-blocks.
    pub fn effective_protocol(&self, area: Rect) -> ProtocolType {
        if area.width < MIN_PROTOCOL_WIDTH {
            ProtocolType::Halfblocks
        } else {
            self.picker.protocol_type()
        }
    }

    pub fn protocol_for(&self, image: DynamicImage, area: Rect) -> StatefulProtocol {
        if self.effective_protocol(area) == ProtocolType::Halfblocks {
            Picker::halfblocks().new_resize_protocol(image)
        } else {
            self.picker.new_resize_protocol(image)
        }
    }
}

#[derive(Default)]
pub struct ArtState {
    pub entry_id: Option<EntryId>,
    pub image: Option<DynamicImage>,
    pub placeholder: Option<String>,
    pub protocol: Option<StatefulProtocol>,
    protocol_type: Option<ProtocolType>,
    cache: VecDeque<(ArtKey, DynamicImage)>,
}

impl ArtState {
    pub fn clear(&mut self) {
        self.entry_id = None;
        self.image = None;
        self.placeholder = None;
        self.protocol = None;
        self.protocol_type = None;
    }

    pub fn set_placeholder(&mut self, entry_id: EntryId, title: impl Into<String>) {
        self.entry_id = Some(entry_id);
        self.image = None;
        self.protocol = None;
        self.protocol_type = None;
        self.placeholder = Some(title.into());
    }

    pub fn set_image(&mut self, entry_id: EntryId, image: DynamicImage, key: ArtKey) {
        self.entry_id = Some(entry_id);
        self.placeholder = None;
        if let Some(index) = self.cache.iter().position(|(cached, _)| cached == &key) {
            let cached = self.cache.remove(index).unwrap().1;
            self.cache.push_front((key, cached.clone()));
            self.image = Some(cached);
        } else {
            self.cache.push_front((key, image.clone()));
            while self.cache.len() > CACHE_LIMIT {
                self.cache.pop_back();
            }
            self.image = Some(image);
        }
        self.protocol = None;
        self.protocol_type = None;
    }

    /// Build (or reuse) the protocol for the current panel area.
    pub fn prepare(&mut self, renderer: &ArtRenderer, area: Rect) {
        let wanted = renderer.effective_protocol(area);
        if self.protocol.is_some() && self.protocol_type == Some(wanted) {
            return;
        }
        if let Some(image) = &self.image {
            self.protocol = Some(renderer.protocol_for(image.clone(), area));
            self.protocol_type = Some(wanted);
        }
    }
}

pub fn decode(bytes: &[u8]) -> Option<DynamicImage> {
    image::load_from_memory(bytes).ok()
}

pub fn read(path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Folder cover search: cover.*, folder.*, front.* — no tag parser.
pub fn folder_cover(directory: &std::path::Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    let mut candidates: Vec<std::path::PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default()
                .to_lowercase();
            matches!(stem.as_str(), "cover" | "folder" | "front")
                && matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp")
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}
