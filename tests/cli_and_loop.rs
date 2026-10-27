mod common;

use std::io::Write;
use std::process::Command;

use omm::cli::{Cli, parse};
use omm::event::AppEvent;
use omm::event_loop::pump;
use omm::terminal::{self, TerminalGuard};
use omm::ui::keymap::Key;
use tokio::sync::mpsc::unbounded_channel;

use common::*;

#[test]
fn terminal_restore_on_drop() {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut guard = TerminalGuard::new(&mut buffer);
        guard.writer().write_all(b"frame").unwrap();
    }
    assert_eq!(buffer, b"frame\x1b[0m\x1b[?25h\x1b[?1049l");
    assert_eq!(
        &buffer[buffer.len() - terminal::RESTORE.len()..],
        terminal::RESTORE
    );
}

#[tokio::test]
async fn event_loop_keeps_order_and_renders_once() {
    let mut app = app();
    let (tx, mut rx) = unbounded_channel();
    let (commands, mut command_rx) = unbounded_channel();
    let mut draws = 0;

    tx.send(AppEvent::Key(Key::Char('2'))).unwrap();
    tx.send(AppEvent::Key(Key::Char('1'))).unwrap();
    tx.send(AppEvent::Tick).unwrap();
    let continued = pump(&mut app, &mut rx, &commands, |_| {
        draws += 1;
        Ok(())
    })
    .await
    .unwrap();

    assert!(continued);
    assert_eq!(draws, 1, "a batch must produce exactly one frame");
    assert_eq!(app.tab, omm::queue::Source::Local, "events in order");
    assert!(command_rx.try_recv().is_err());

    tx.send(AppEvent::Tick).unwrap();
    tx.send(AppEvent::Tick).unwrap();
    pump(&mut app, &mut rx, &commands, |_| {
        draws += 1;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(draws, 2);
}

#[tokio::test]
async fn quit_ends_the_loop_without_rendering() {
    let mut app = app();
    let (tx, mut rx) = unbounded_channel();
    let (commands, mut command_rx) = unbounded_channel();
    let mut draws = 0;
    tx.send(AppEvent::Key(Key::Char('q'))).unwrap();

    let continued = pump(&mut app, &mut rx, &commands, |_| {
        draws += 1;
        Ok(())
    })
    .await
    .unwrap();

    assert!(!continued);
    assert_eq!(draws, 0);
    assert!(matches!(
        command_rx.try_recv(),
        Ok(omm::event::Command::Shutdown)
    ));
}

#[test]
fn cli_help() {
    assert_eq!(parse(["--help".to_string()]), Ok(Cli::Help));
    assert_eq!(parse(["-h".to_string()]), Ok(Cli::Help));

    let output = Command::new(env!("CARGO_BIN_EXE_omm"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "{stdout}");
    assert!(stdout.contains("--import"), "{stdout}");
}

#[test]
fn cli_unknown_flag() {
    assert!(parse(["nope".to_string()]).is_err());
    assert!(parse(["--import".to_string()]).is_err());

    let output = Command::new(env!("CARGO_BIN_EXE_omm"))
        .arg("--nope")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown argument"), "{stderr}");
    assert!(stderr.contains("Usage:"), "{stderr}");
}

#[test]
fn cli_import_flag() {
    let url = "https://open.spotify.com/album/2guirTSEqLizK7j9i1MTTZ";
    assert_eq!(
        parse(["--import".to_string(), url.to_string()]),
        Ok(Cli::Import(url.to_string()))
    );
    assert_eq!(
        parse([format!("--import={url}")]),
        Ok(Cli::Import(url.to_string()))
    );
    assert_eq!(parse(Vec::<String>::new()), Ok(Cli::Tui));
}
