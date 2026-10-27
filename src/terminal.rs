//! Terminal lifecycle: raw mode, alternate screen and unconditional restore.

use std::io::{self, Write};

use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

/// Written by `Drop` even when the caller returns an error or panics.
pub const RESTORE: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1049l";

pub struct TerminalGuard<W: Write> {
    out: W,
    raw: bool,
}

impl<W: Write> TerminalGuard<W> {
    /// For tests: no raw mode, but the restore sequence is still written.
    pub fn new(out: W) -> Self {
        Self { out, raw: false }
    }

    pub fn with_raw(out: W) -> Self {
        Self { out, raw: true }
    }

    pub fn writer(&mut self) -> &mut W {
        &mut self.out
    }
}

impl<W: Write> Drop for TerminalGuard<W> {
    fn drop(&mut self) {
        if self.raw {
            let _ = disable_raw_mode();
        }
        let _ = self.out.write_all(RESTORE);
        let _ = self.out.flush();
    }
}

pub fn enter() -> io::Result<TerminalGuard<std::io::Stdout>> {
    enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen, Hide)?;
    Ok(TerminalGuard::with_raw(out))
}

pub fn leave(guard: &mut TerminalGuard<std::io::Stdout>) -> io::Result<()> {
    execute!(guard.writer(), Show, LeaveAlternateScreen)?;
    Ok(())
}

pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let mut out = std::io::stdout();
        let _ = out.write_all(RESTORE);
        let _ = out.flush();
        default(info);
    }));
}
