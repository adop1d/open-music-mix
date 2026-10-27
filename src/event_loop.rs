//! Input task, 250 ms tick and one render per batch of events.

use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyEventKind};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::app::App;
use crate::event::{AppEvent, Command};
use crate::ui::keymap::Key;

pub const TICK: Duration = Duration::from_millis(250);

/// Emits a tick so progress, art and time-based redraws keep moving.
pub fn spawn_ticker(tx: UnboundedSender<AppEvent>, tick: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick);
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    })
}

/// Blocking terminal read on its own thread; a thread (unlike `spawn_blocking`)
/// never delays process exit. Resizes repaint through a tick.
pub fn spawn_input(tx: UnboundedSender<AppEvent>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(Event::Key(key))
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    if let Some(key) = Key::from_event(key)
                        && tx.send(AppEvent::Key(key)).is_err()
                    {
                        return;
                        return;
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    if tx.send(AppEvent::Tick).is_err() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    })
}

/// Everything already queued rides in one batch, so one render serves it.
pub async fn next_batch(rx: &mut UnboundedReceiver<AppEvent>) -> Option<Vec<AppEvent>> {
    let first = rx.recv().await?;
    let mut batch = vec![first];
    while let Ok(event) = rx.try_recv() {
        batch.push(event);
    }
    Some(batch)
}

pub fn flush(app: &mut App, commands: &UnboundedSender<Command>) {
    for command in app.take_commands() {
        let _ = commands.send(command);
    }
}

/// Handles one batch and renders once. False when the session ended.
pub async fn pump<F>(
    app: &mut App,
    rx: &mut UnboundedReceiver<AppEvent>,
    commands: &UnboundedSender<Command>,
    mut draw: F,
) -> io::Result<bool>
where
    F: FnMut(&mut App) -> io::Result<()>,
{
    let Some(batch) = next_batch(rx).await else {
        return Ok(false);
    };
    for event in batch {
        app.handle(event);
        flush(app, commands);
        if app.quit {
            return Ok(false);
        }
    }
    draw(app)?;
    Ok(true)
}

pub async fn run<F>(
    app: &mut App,
    rx: &mut UnboundedReceiver<AppEvent>,
    commands: &UnboundedSender<Command>,
    mut draw: F,
) -> io::Result<()>
where
    F: FnMut(&mut App) -> io::Result<()>,
{
    while pump(app, rx, commands, &mut draw).await? {}
    Ok(())
}
