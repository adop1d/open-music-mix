//! spotify-import spec: metadata only, public collections, offline afterwards.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use omm::app::App;
use omm::cli::{self, Cli};
use omm::event::{AppEvent, Command as AppCommand};
use omm::persist::imports;
use omm::process::spotdl::parse_spotdl_json;
use omm::process::{
    Collection, CollectionKind, FakeImporter, FakeMpd, FakeMpv, FakeResolver, ImportError,
    ImportedTrack, RealResolver, RealSpotifyImporter, parse_reference,
};
use omm::queue::{Locator, Source};
use omm::ui::keymap::Key;
use omm::worker::{Worker, import_blocking, match_tracks};

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/spotdl")
}

fn fixture_importer() -> RealSpotifyImporter {
    RealSpotifyImporter::with_program(script().to_string_lossy().into_owned())
}

fn empty_resolver() -> FakeResolver {
    FakeResolver::new()
}

#[test]
fn importing_a_playlist_url() {
    let dir = TempDir::new("import-playlist");
    let url = "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M";
    let collection = import_blocking(&dir, url, &fixture_importer(), &empty_resolver()).unwrap();

    assert_eq!(collection.kind, CollectionKind::Playlist);
    assert_eq!(collection.name, "Disintegration");
    assert_eq!(collection.tracks.len(), 3);
    assert_eq!(collection.id, "playlist-37i9dQZF1DXcBWIGoYBM5M");
    assert_eq!(imports::load_all(&dir).len(), 1);
    assert!(
        imports::load_all(&dir)[0]
            .tracks
            .iter()
            .all(|track| track.youtube_id.is_some())
    );
}

#[test]
fn importing_a_single_track() {
    let dir = TempDir::new("import-track");
    let url = "https://open.spotify.com/track/5T3h0hB0H0H0H0H0H0H0H0";
    let collection = import_blocking(&dir, url, &fixture_importer(), &empty_resolver()).unwrap();

    assert_eq!(collection.kind, CollectionKind::Track);
    assert_eq!(collection.name, "Plainsong");
    assert_eq!(collection.tracks.len(), 1);
    assert_eq!(collection.url, url);
}

#[test]
fn rejecting_invalid_input() {
    let mut app = app();
    assert!(!app.submit_import("https://example.com/not-spotify"));
    assert!(app.collections.is_empty());
    assert!(app.take_commands().is_empty());
    let notice = app.ui.notice.clone().unwrap();
    assert!(notice.contains("Invalid Spotify reference"), "{notice}");
    assert!(notice.contains("playlist, album or track"), "{notice}");
    assert!(parse_reference("https://example.com/not-spotify").is_err());
}

#[test]
fn importing_from_the_modal() {
    let mut app = app();
    press(&mut app, 'i');
    assert!(app.ui.import_open);
    type_text(
        &mut app,
        "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
    );
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("Import a Spotify collection"), "{frame}");

    press_key(&mut app, Key::Enter);
    assert!(!app.ui.import_open);
    let commands = app.take_commands();
    match commands.as_slice() {
        [AppCommand::Import { reference }] => {
            assert_eq!(reference.kind, CollectionKind::Playlist);
            assert_eq!(reference.id, "37i9dQZF1DXcBWIGoYBM5M");
        }
        other => panic!("expected Import, got {other:?}"),
    }

    app.handle(AppEvent::ImportProgress {
        name: "Disintegration".to_string(),
        matched: 0,
        total: 3,
    });
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("0 / 3"), "{frame}");
    assert!(frame.contains("Importing"), "{frame}");

    app.handle(AppEvent::ImportFinished {
        collection: collection(
            "playlist-37i9dQZF1DXcBWIGoYBM5M",
            "Disintegration",
            vec![track(
                "Plainsong",
                "The Cure Plainsong",
                Some("vid00000001"),
            )],
        ),
    });
    assert!(app.ui.import_status.is_none());
    assert_eq!(app.collections.len(), 1);
}

#[test]
fn importing_from_the_command_line() {
    let url = "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M";
    assert_eq!(
        cli::parse(["--import".to_string(), url.to_string()]),
        Ok(Cli::Import(url.to_string()))
    );

    let dir = TempDir::new("import-cli");
    let collection = import_blocking(&dir, url, &fixture_importer(), &empty_resolver()).unwrap();
    assert_eq!(collection.name, "Disintegration");

    // Without spotdl on PATH the command exits non-zero and explains why.
    let output = Command::new(env!("CARGO_BIN_EXE_omm"))
        .args(["--import", url])
        .env("OMM_CONFIG_DIR", dir.join("config"))
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("spotdl"), "{stderr}");
}

#[test]
fn account_only_playlist() {
    let dir = TempDir::new("import-private");
    let (tx, mut rx) = event_channel();
    let importer = FakeImporter::failing(ImportError::Private);
    let backends = fakes(
        FakeMpd::with_library(vec![], vec![], vec![]),
        FakeMpv::default(),
        FakeResolver::new(),
        importer,
        workspace_in(&dir),
    );
    let mut worker = Worker::new(backends, tx, dir.to_path_buf());
    let reference =
        parse_reference("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M").unwrap();

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(worker.handle_command(AppCommand::Import { reference }));

    let mut app = app();
    let mut failed = false;
    for event in events(&mut rx) {
        if matches!(event, AppEvent::ImportFailed { .. }) {
            failed = true;
        }
        app.handle(event);
    }
    assert!(failed);
    let notice = app.ui.notice.clone().unwrap();
    assert!(notice.contains("only public collections"), "{notice}");
    assert!(app.collections.is_empty());
    assert!(imports::load_all(&dir).is_empty());
}

#[test]
fn imported_tracks_carry_metadata() {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/playlist.json"),
    )
    .unwrap();
    let collection = parse_spotdl_json(&text).unwrap();
    assert_eq!(collection.name, "Disintegration");
    for track in &collection.tracks {
        assert!(!track.title.is_empty());
        assert_eq!(track.artists, vec!["The Cure".to_string()]);
        assert_eq!(track.album.as_deref(), Some("Disintegration"));
        assert!(track.duration.is_some());
        assert!(track.query.contains(&track.title));
    }
}

#[test]
fn no_spotify_audio_is_stored() {
    let dir = TempDir::new("import-no-audio");
    import_blocking(
        &dir,
        "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
        &fixture_importer(),
        &empty_resolver(),
    )
    .unwrap();

    let audio = ["mp3", "m4a", "ogg", "opus", "flac", "wav", "webm"];
    let mut files = Vec::new();
    collect_files(&dir, &mut files);
    for file in files {
        let extension = file
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_lowercase();
        assert!(
            !audio.contains(&extension.as_str()),
            "audio file {}",
            file.display()
        );
        assert_eq!(extension, "json", "unexpected file {}", file.display());
    }
}

fn collect_files(directory: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn first_import_on_a_clean_machine() {
    let dir = TempDir::new("import-clean");
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    let collection = import_blocking(
        &dir,
        "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
        &fixture_importer(),
        &empty_resolver(),
    )
    .unwrap();
    assert!(dir.join("imports").is_dir());
    assert_eq!(
        imports::find(&dir, &collection.id).unwrap().name,
        "Disintegration"
    );
    // No credential file or prompt is created.
    let mut files = Vec::new();
    collect_files(&dir, &mut files);
    assert!(
        files
            .iter()
            .all(|file| file.extension().is_some_and(|value| value == "json"))
    );
}

#[test]
fn playing_an_imported_track() {
    let dir = TempDir::new("import-play");
    let mut app = app();
    app.queue.append(
        Source::Spotify,
        meta("Plainsong", "The Cure"),
        Locator::YoutubeId("vid00000001".to_string()),
    );
    let id = app.queue.entries[0].id;
    app.play_index(0);
    app.handle(AppEvent::PlaybackStarted { entry_id: id });
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("[spotify]"), "{frame}");
    assert!(frame.contains("Plainsong"), "{frame}");

    let (tx, mut rx) = event_channel();
    let resolver = FakeResolver::new();
    let resolver_handle = resolver.clone();
    let mpv = std::sync::Arc::new(std::sync::Mutex::new(FakeMpv::default()));
    let mpv_handle = mpv.clone();
    let backends = omm::backends::Backends {
        mpd: shared_mpd(FakeMpd::with_library(vec![], vec![], vec![])),
        mpv,
        resolver: std::sync::Arc::new(resolver),
        importer: std::sync::Arc::new(FakeImporter::new(Collection {
            id: "empty".to_string(),
            name: "empty".to_string(),
            kind: CollectionKind::Playlist,
            url: String::new(),
            tracks: vec![],
        })),
        workspace: std::sync::Arc::new(workspace_in(&dir)),
    };
    let mut worker = Worker::new(backends, tx, dir.to_path_buf());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(worker.handle_command(AppCommand::PlayRemote {
            entry_id: id,
            source: Source::Spotify,
            locator: Locator::YoutubeId("vid00000001".to_string()),
            volume: 70,
        }));
    for event in events(&mut rx) {
        app.handle(event);
    }
    assert_eq!(resolver_handle.resolve_count(), 1);
    assert_eq!(mpv_handle.lock().unwrap().played.len(), 1);
    assert!(render(&mut app, 80, 24).contains("Plainsong"));
}

#[test]
fn no_search_when_an_imported_entry_starts() {
    let dir = TempDir::new("import-no-search");
    let (tx, _rx) = event_channel();
    let resolver = FakeResolver::new();
    let handle = resolver.clone();
    let backends = omm::backends::Backends {
        mpd: shared_mpd(FakeMpd::with_library(vec![], vec![], vec![])),
        mpv: shared_mpv(FakeMpv::default()),
        resolver: std::sync::Arc::new(resolver),
        importer: std::sync::Arc::new(FakeImporter::new(Collection {
            id: "empty".to_string(),
            name: "empty".to_string(),
            kind: CollectionKind::Playlist,
            url: String::new(),
            tracks: vec![],
        })),
        workspace: std::sync::Arc::new(workspace_in(&dir)),
    };
    let mut worker = Worker::new(backends, tx, dir.to_path_buf());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(worker.handle_command(AppCommand::PlayRemote {
            entry_id: omm::queue::EntryId(0),
            source: Source::Spotify,
            locator: Locator::YoutubeId("vid00000001".to_string()),
            volume: 70,
        }));

    assert_eq!(handle.search_count(), 0);
    assert_eq!(handle.resolve_count(), 1);
}

#[test]
fn partially_matched_playlist() {
    let spotify: Vec<ImportedTrack> = (0..10)
        .map(|index| {
            let mut track = track(
                &format!("Song {index}"),
                &format!("q{index}"),
                Some(&format!("vid{index:08}")),
            );
            track.duration = Some(std::time::Duration::from_secs(200));
            track
        })
        .collect();
    let mut imported = collection("playlist-partial", "Ten Songs", spotify);
    // Default candidate has a mismatching duration, so unmatched queries stay unmatched.
    let mut resolver = FakeResolver::new()
        .with_candidate("wrongduration", Some(std::time::Duration::from_secs(500)));
    for index in 0..8 {
        resolver = resolver.with_candidate_for(
            format!("q{index}"),
            format!("vid{index:08}"),
            Some(std::time::Duration::from_secs(202)),
        );
    }

    match_tracks(&mut imported, &resolver);

    assert_eq!(imported.matched(), 8);
    assert_eq!(imported.unmatched(), 2);
    assert!(imported.tracks[8].unavailable);
    assert!(imported.tracks[8].youtube_id.is_none());
    assert_eq!(
        imported.tracks[7].youtube_id.as_deref(),
        Some("vid00000007")
    );
}

#[test]
fn importing_the_same_playlist_twice() {
    let dir = TempDir::new("import-twice");
    let first = collection(
        "playlist-same",
        "Disintegration",
        vec![
            track("Plainsong", "The Cure Plainsong", Some("vid00000001")),
            track("Lovesong", "The Cure Lovesong", Some("vid00000002")),
        ],
    );
    let second = collection(
        "playlist-same",
        "Disintegration",
        vec![track(
            "Plainsong",
            "The Cure Plainsong",
            Some("vid00000001"),
        )],
    );
    imports::save(&dir, &first).unwrap();
    imports::save(&dir, &second).unwrap();

    let loaded = imports::load_all(&dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].tracks.len(), 1);

    let mut app = app();
    app.handle(AppEvent::ImportFinished { collection: first });
    app.handle(AppEvent::ImportFinished {
        collection: second.clone(),
    });
    assert_eq!(app.collections.len(), 1);
    assert_eq!(app.collections[0].tracks.len(), second.tracks.len());
}

#[test]
fn browsing_imports_without_network() {
    let dir = TempDir::new("import-browse");
    let saved = collection(
        "playlist-browse",
        "Disintegration",
        vec![
            track("Plainsong", "The Cure Plainsong", Some("vid00000001")),
            track("Lovesong", "The Cure Lovesong", None),
        ],
    );
    imports::save(&dir, &saved).unwrap();

    let mut app = App::new(Default::default(), omm::i18n::I18n::english())
        .with_collections(imports::load_all(&dir));
    press(&mut app, '3');
    assert_eq!(app.tab, Source::Spotify);
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("Disintegration"), "{frame}");
    press(&mut app, 'l');
    let frame = render(&mut app, 80, 24);
    assert!(frame.contains("Plainsong"), "{frame}");
    assert!(frame.contains("[unavailable]"), "{frame}");
}

// Keep RealResolver in the import list used by the CLI tests' counterparts.
#[allow(dead_code)]
fn real_resolver() -> RealResolver {
    RealResolver::new()
}
