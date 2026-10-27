//! Multi-source music TUI: local MPD, YouTube and Spotify in one queue.

pub mod app;
pub mod art;
pub mod backends;
pub mod cli;
pub mod event;
pub mod event_loop;
pub mod i18n;
pub mod persist;
pub mod process;
pub mod queue;
pub mod sources;
pub mod terminal;
pub mod ui;
pub mod worker;
