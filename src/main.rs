//! Runtime bootstrap: config, state, worker, terminal and event loop.

use std::path::Path;
use std::process::ExitCode;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use omm::app::App;
use omm::art::ArtRenderer;
use omm::backends::{Backends, find_binary};
use omm::cli::{self, Cli};
use omm::event::Command;
use omm::event_loop;
use omm::i18n::I18n;
use omm::persist::{self, Config, imports, state as session};
use omm::process::{Collection, RealResolver, RealSpotifyImporter};
use omm::terminal;
use omm::ui;
use omm::worker::{Worker, import_blocking, workspace_for};

#[tokio::main]
async fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Ok(Cli::Help) => {
            print!("{}", cli::USAGE);
            ExitCode::SUCCESS
        }
        Ok(Cli::Tui) => run_tui(None).await,
        Ok(Cli::Import(url)) => run_tui(Some(url)).await,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

async fn run_tui(import: Option<String>) -> ExitCode {
    let dir = persist::config_dir();
    if let Err(error) = persist::ensure_layout(&dir) {
        eprintln!("{}: {error}", dir.display());
        return ExitCode::FAILURE;
    }
    let _ = Config::ensure_file(&dir);
    let (config, config_warnings) = Config::load(&dir);
    let (i18n, i18n_warnings) = I18n::load(config.ui.language.as_deref(), Some(&dir));
    let (mut restored, state_warnings) = session::load(&dir);
    restored.restore_states();
    let warnings: Vec<String> = config_warnings
        .into_iter()
        .chain(i18n_warnings)
        .chain(state_warnings)
        .collect();
    for warning in &warnings {
        eprintln!("omm: {warning}");
    }

    if let Some(url) = import {
        match import_now(&dir, &url) {
            Ok(collection) => eprintln!(
                "{}: {} matched / {} tracks",
                collection.name,
                collection.matched(),
                collection.tracks.len()
            ),
            Err(message) => {
                eprintln!("omm: {message}");
                return ExitCode::FAILURE;
            }
        }
    }

    let collections = imports::load_all(&dir);
    let renderer = ArtRenderer::detect();
    let backends = Backends::real(&config, workspace_for(&config));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let worker = Worker::new(backends, tx.clone(), dir.clone());
    for event in worker.probe_events(&|name| find_binary(name).is_some()) {
        let _ = tx.send(event);
    }
    let worker_task = tokio::spawn(worker.run(command_rx));
    let _ = command_tx.send(Command::LoadLibrary);

    let mut app = App::new(config, i18n)
        .with_collections(collections)
        .with_restored(restored)
        .with_art_renderer(renderer);
    app.config_dir = Some(dir.clone());
    if let Some(last) = warnings.last() {
        app.push_notice(last.clone());
    }

    terminal::install_panic_hook();
    let guard = match terminal::enter() {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!("omm: terminal unavailable: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut terminal = match Terminal::new(CrosstermBackend::new(std::io::stdout())) {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("omm: terminal unavailable: {error}");
            return ExitCode::FAILURE;
        }
    };
    let _ = terminal.clear();

    let ticker = event_loop::spawn_ticker(tx.clone(), event_loop::TICK);
    let _input = event_loop::spawn_input(tx.clone());

    let result = event_loop::run(&mut app, &mut rx, &command_tx, |app| {
        terminal.draw(|frame| ui::render(app, frame)).map(|_| ())
    })
    .await;
    ticker.abort();
    let _ = command_tx.send(Command::Shutdown);
    drop(command_tx);
    // Wait for the worker to quit mpv; otherwise the runtime can exit first.
    let _ = worker_task.await;
    let _ = session::save(&dir, &app.state());
    drop(terminal);
    drop(guard);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("omm: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `--import` runs before the TUI and reports on stderr.
fn import_now(dir: &Path, url: &str) -> Result<Collection, String> {
    let importer = RealSpotifyImporter::new();
    let resolver = RealResolver::new();
    import_blocking(dir, url, &importer, &resolver).map_err(|error| error.message())
}
