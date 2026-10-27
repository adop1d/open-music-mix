//! Vim-style keymap with TOML overrides. The help overlay is generated from it.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    MoveDown,
    MoveUp,
    Collapse,
    Expand,
    CyclePanel,
    TabLocal,
    TabYoutube,
    TabSpotify,
    PlayPause,
    PlayNow,
    Append,
    Remove,
    Clear,
    Next,
    Previous,
    SeekBack,
    SeekForward,
    VolumeUp,
    VolumeDown,
    Retry,
    Import,
    Search,
    Help,
    Escape,
    CycleView,
    Quit,
}

impl Action {
    pub const ALL: [Action; 26] = [
        Action::MoveDown,
        Action::MoveUp,
        Action::Collapse,
        Action::Expand,
        Action::CyclePanel,
        Action::TabLocal,
        Action::TabYoutube,
        Action::TabSpotify,
        Action::PlayPause,
        Action::PlayNow,
        Action::Append,
        Action::Remove,
        Action::Clear,
        Action::Next,
        Action::Previous,
        Action::SeekBack,
        Action::SeekForward,
        Action::VolumeUp,
        Action::VolumeDown,
        Action::Retry,
        Action::Import,
        Action::Search,
        Action::Help,
        Action::Escape,
        Action::CycleView,
        Action::Quit,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Action::MoveDown => "move_down",
            Action::MoveUp => "move_up",
            Action::Collapse => "collapse",
            Action::Expand => "expand",
            Action::CyclePanel => "cycle_panel",
            Action::TabLocal => "tab_local",
            Action::TabYoutube => "tab_youtube",
            Action::TabSpotify => "tab_spotify",
            Action::PlayPause => "play_pause",
            Action::PlayNow => "play_now",
            Action::Append => "append",
            Action::Remove => "remove",
            Action::Clear => "clear",
            Action::Next => "next",
            Action::Previous => "previous",
            Action::SeekBack => "seek_back",
            Action::SeekForward => "seek_forward",
            Action::VolumeUp => "volume_up",
            Action::VolumeDown => "volume_down",
            Action::Retry => "retry",
            Action::Import => "import",
            Action::Search => "search",
            Action::Help => "help",
            Action::Escape => "escape",
            Action::CycleView => "cycle_view",
            Action::Quit => "quit",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    pub fn i18n_key(self) -> &'static str {
        match self {
            Action::MoveDown => "action_move_down",
            Action::MoveUp => "action_move_up",
            Action::Collapse => "action_collapse",
            Action::Expand => "action_expand",
            Action::CyclePanel => "action_cycle_panel",
            Action::TabLocal => "action_tab_local",
            Action::TabYoutube => "action_tab_youtube",
            Action::TabSpotify => "action_tab_spotify",
            Action::PlayPause => "action_play_pause",
            Action::PlayNow => "action_play_now",
            Action::Append => "action_append",
            Action::Remove => "action_remove",
            Action::Clear => "action_clear",
            Action::Next => "action_next",
            Action::Previous => "action_previous",
            Action::SeekBack => "action_seek_back",
            Action::SeekForward => "action_seek_forward",
            Action::VolumeUp => "action_volume_up",
            Action::VolumeDown => "action_volume_down",
            Action::Retry => "action_retry",
            Action::Import => "action_import",
            Action::Search => "action_search",
            Action::Help => "action_help",
            Action::Escape => "action_escape",
            Action::CycleView => "action_cycle_view",
            Action::Quit => "action_quit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Space,
    Enter,
    Esc,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Backspace,
}

impl Key {
    pub fn from_event(event: KeyEvent) -> Option<Self> {
        if event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return None;
        }
        Some(match event.code {
            KeyCode::Char(' ') => Key::Space,
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab => Key::Tab,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Backspace => Key::Backspace,
            _ => return None,
        })
    }

    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        match value {
            "space" => Some(Key::Space),
            "enter" => Some(Key::Enter),
            "esc" | "escape" => Some(Key::Esc),
            "tab" => Some(Key::Tab),
            "up" => Some(Key::Up),
            "down" => Some(Key::Down),
            "left" => Some(Key::Left),
            "right" => Some(Key::Right),
            "backspace" => Some(Key::Backspace),
            _ => {
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(Key::Char(c)),
                    _ => None,
                }
            }
        }
    }

    pub fn display(self) -> String {
        match self {
            Key::Char(' ') => "space".to_string(),
            Key::Char(c) => c.to_string(),
            Key::Space => "space".to_string(),
            Key::Enter => "enter".to_string(),
            Key::Esc => "esc".to_string(),
            Key::Tab => "tab".to_string(),
            Key::Up => "up".to_string(),
            Key::Down => "down".to_string(),
            Key::Left => "left".to_string(),
            Key::Right => "right".to_string(),
            Key::Backspace => "backspace".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: HashMap<Key, Action>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::defaults()
    }
}

impl Keymap {
    pub fn defaults() -> Self {
        use Action::*;
        let pairs = [
            (Key::Char('j'), MoveDown),
            (Key::Char('k'), MoveUp),
            (Key::Char('h'), Collapse),
            (Key::Char('l'), Expand),
            (Key::Tab, CyclePanel),
            (Key::Char('1'), TabLocal),
            (Key::Char('2'), TabYoutube),
            (Key::Char('3'), TabSpotify),
            (Key::Space, PlayPause),
            (Key::Enter, PlayNow),
            (Key::Char('a'), Append),
            (Key::Char('d'), Remove),
            (Key::Char('c'), Clear),
            (Key::Char('n'), Next),
            (Key::Char('p'), Previous),
            (Key::Left, SeekBack),
            (Key::Right, SeekForward),
            (Key::Char('+'), VolumeUp),
            (Key::Char('-'), VolumeDown),
            (Key::Char('r'), Retry),
            (Key::Char('i'), Import),
            (Key::Char('/'), Search),
            (Key::Char('?'), Help),
            (Key::Esc, Escape),
            (Key::Char('v'), CycleView),
            (Key::Char('q'), Quit),
        ];
        Self {
            bindings: pairs.into_iter().collect(),
        }
    }

    /// Overrides remove the action's previous key, so a rebound action has one binding.
    pub fn with_overrides(overrides: &HashMap<String, String>) -> Self {
        let mut keymap = Self::defaults();
        let mut entries: Vec<(&String, &String)> = overrides.iter().collect();
        entries.sort();
        for (action_id, value) in entries {
            let Some(action) = Action::from_id(action_id) else {
                continue;
            };
            let Some(key) = Key::parse(value) else {
                continue;
            };
            keymap.bindings.retain(|_, bound| *bound != action);
            keymap.bindings.insert(key, action);
        }
        keymap
    }

    pub fn action_for(&self, key: Key) -> Option<Action> {
        self.bindings.get(&key).copied()
    }

    pub fn key_for(&self, action: Action) -> Option<Key> {
        self.bindings
            .iter()
            .find(|(_, bound)| **bound == action)
            .map(|(key, _)| *key)
    }

    pub fn bindings(&self) -> Vec<(Action, Key)> {
        let mut bindings: Vec<(Action, Key)> = self
            .bindings
            .iter()
            .map(|(key, action)| (*action, *key))
            .collect();
        bindings.sort_by_key(|(action, _)| *action);
        bindings
    }
}
