# Tasks

## 1. Bootstrap

- [x] 1.1 Create the crate with the module tree from design.md (`app`, `queue`, `sources`, `process`, `persist`, `ui`, `i18n`, `art`) and verify `cargo build` succeeds with all modules present
- [x] 1.2 Add the dependencies listed in design.md (`ratatui`, `crossterm`, `tokio`, `mpd`, `ratatui-image`, `image`, `serde`, `serde_json`, `toml`, `directories`) and verify `cargo build` and `cargo clippy -- -D warnings` both succeed
- [x] 1.3 Implement terminal lifecycle (raw mode, alternate screen, restore on normal exit, error and panic) and verify a unit test with a fake writer receives the restore sequence from `Drop`
- [x] 1.4 Implement the event loop skeleton: input task, 250 ms tick, `mpsc<AppEvent>` channel, render after each batch, and verify a test with a paused clock processes two events in order and renders once per tick
- [x] 1.5 Write `README.md` with build, run, `--import` usage and required external binaries (`mpd`, `mpv`, `yt-dlp`, `spotdl`) and verify the documented commands from a clean checkout
- [x] 1.6 Implement argument parsing for `--import <url>` and `--help` without a parser crate, and verify `--help` exits 0 and an unknown flag exits non-zero with a message

## 2. Local source (MPD)

- [x] 2.1 Define `MpdBackend` (connect, list artists/albums/folders/playlists, play, pause, status) plus an in-memory `FakeMpd` and verify status and disconnect events reach the app through the fake
- [x] 2.2 Implement the real MPD client over the `mpd` crate with configurable host/port/socket and reconnect with backoff, and verify it against `FakeMpd` at the adapter level so no daemon is needed
- [x] 2.3 Implement the four local browser views and verify tests cover "Artist view lists albums" and "Empty local library"
- [x] 2.4 Implement local playback delegation (`addid` + `playid`, pause and stop, never clearing the daemon queue) and verify "Playing a local track", "Pausing local playback" and "Existing daemon queue is preserved" pass against `FakeMpd`
- [x] 2.5 Implement per-source status handling for missing binary, unreachable daemon and mid-playback disconnect, and verify "Daemon not installed" and "Daemon disconnects during playback" pass while youtube and spotify stay enabled
- [x] 2.6 Implement case-insensitive filtering of the focused local list and verify "Search filters the artist list" preserves original order

## 3. Queue and transport

- [x] 3.1 Implement `Queue`, `Entry`, `Locator`, `EntryState` and mutation operations, and verify unit tests cover append, remove, reorder and clear plus "Removing the current entry" and "Clearing the queue stops playback"
- [x] 3.2 Implement transport dispatch across adapters (`play`, `pause`, `next`, `previous`, `seek`, `volume`) and verify "Next advances within the queue", "Previous at the first entry", "Seek on a remote entry" and "Volume change applies to the active source"
- [x] 3.3 Implement failed-entry handling with mark-and-skip semantics, and verify "Failed resolution skips to the next entry" and "Last entry fails"
- [x] 3.4 Implement history recording (newest first, capped at 200, one entry per start including repeats) and verify "Playing tracks records history", "History is bounded", "Replaying a track adds a new entry" plus the in-memory half of "History survives a restart" (restore is covered in 7.4)

## 4. Remote playback (yt-dlp + mpv)

- [x] 4.1 Define the `Resolver` trait and implement the `yt-dlp --get-url` call with a 10 s timeout, `--no-playlist` and error classification (missing binary, no results, unreachable), and verify fixture-driven tests for URL, query, no-results and missing-binary cases
- [x] 4.2 Define `MpvBackend` and implement the long-lived `mpv --idle` process with JSON IPC over a temp Unix socket (`set_property`/`loadfile`, `property-change` events), and verify command and event mapping against `FakeMpv`
- [x] 4.3 Wire the remote adapter for youtube and spotify entries (play, pause, seek, volume) and verify "Pausing a remote entry" and "Seeking a remote entry"
- [x] 4.4 Configure the temporary buffer (`--cache-dir` under the OS temp directory) and implement cleanup on track change and on exit, and verify "Track change removes the previous buffer" and "Exit removes buffered audio" by asserting the temp directory is empty
- [x] 4.5 Implement mpv crash handling (entry marked failed, source flagged, single respawn on next play), and verify "Player process crashes" and "Resolver not installed" with the youtube/spotify tabs disabled and local still usable
- [x] 4.6 Record resolution and first-audio timestamps and verify "Normal playback start" with a fixed-delay fake resolver and a paused clock, asserting the first-audio event lands under two seconds including resolution
- [x] 4.7 Implement the no-results path and verify "No results for a query" shows the message and leaves the queue unchanged

## 5. Spotify import

- [x] 5.1 Define `SpotifyImporter` and implement `spotdl save` into a temp file with JSON parsing and a 120 s timeout, and verify a fixture-driven test imports a recorded playlist without invoking a download
- [x] 5.2 Implement URL/ID parsing for playlist, album and track references, and verify "Importing a playlist URL", "Importing a single track" (one-entry collection named after the track) and "Rejecting invalid input"
- [x] 5.3 Implement metadata mapping (title, artists, album, duration, ISRC) and verify "Imported tracks carry metadata" plus "No Spotify audio is stored" by asserting no audio files exist in either directory
- [x] 5.4 Implement track matching with `yt-dlp ytsearch1`, ±5 s duration tolerance and a concurrency cap of 4, and verify "Partially matched playlist" reports 8 matched / 2 unmatched and marks the rest unavailable
- [x] 5.5 Implement rejection of account-only collections without any credential prompt and verify "Account-only playlist" and "First import on a clean machine"
- [x] 5.6 Implement collection storage written before matching starts plus update-in-place on re-import, and verify "Importing the same playlist twice" keeps the count stable and "Browsing imports without network" after a restart
- [x] 5.7 Verify "Playing an imported track" resolves through the youtube path and keeps the spotify label, and document import behaviour and limits in `docs/imports.md` with a runnable example
- [x] 5.8 Implement lazy stream resolution for spotify entries from the stored video ID and verify "No search when an imported entry starts" asserts no resolver search is issued
- [x] 5.9 Implement `--import <url>` end-to-end (import before the TUI starts, result on stderr, non-zero exit on failure) and verify "Importing from the command line"

## 6. Interface, i18n and album art

- [x] 6.1 Implement the three-panel layout with responsive collapse and source tabs inside the explorer, and verify `TestBackend` tests at 80×24 ("Default terminal size"), 60×18 ("Small terminal"), "Switching to the youtube tab" and "Disabled tab is still reachable"
- [x] 6.2 Implement the full default keymap from design.md with TOML overrides plus the help overlay generated from it, and verify "Moving in a list", "Overriding a binding", "Queue editing from the keyboard", "Removing an entry", "Changing volume", "Retrying a failed entry" and "Opening/closing help"
- [x] 6.3 Add a test that asserts every `Action` has at least one binding and that no action is mouse-only, verifying "Every action reachable without a mouse"
- [x] 6.4 Implement `/` search with clear/restore and verify "Filtering a queue" and "Clearing the query"
- [x] 6.5 Implement the embedded i18n tables with Spanish/English lookup, English fallback and per-key user overrides from `i18n/<language>.toml`, and verify "Spanish interface", "Missing translation", "User override file" and "Invalid override file"
- [x] 6.6 Implement album art detection through `ratatui-image` with a fake picker and verify "Kitty terminal" and "Unsupported terminal" (half-blocks, no graphics escapes in output)
- [x] 6.7 Implement asynchronous art fetch per source (MPD picture, folder image, resolver thumbnail written with `--write-thumbnail`) with placeholder, missing-art and corrupt-image handling, and verify "Local art from the daemon", "Local art from the folder", "Remote art from the resolver", "Art available", "Slow art fetch", "Track without embedded art" and "Corrupt image"
- [x] 6.8 Implement art fitting and redraw on resize with protocol downgrade for narrow panels, and verify "Resizing the terminal" and "Narrow panel"
- [x] 6.9 Verify "Typing during a slow import" by blocking a fake importer and asserting selection still moves and the frame advances
- [x] 6.10 Implement the import modal bound to `i` (URL input, validation message, progress and result) and verify "Importing from the modal"

## 7. Persistence and configuration

- [x] 7.1 Implement config loading (defaults, file merge, per-key validation warnings) and verify "Invalid configuration value" leaves the rest of the config applied
- [x] 7.2 Implement atomic state writes (temp file in the same directory, flush, rename) with `format = 1`, and verify "Interrupted write" leaves the previous file readable after a simulated abort
- [x] 7.3 Implement corrupt-file quarantine and newer-version preservation, and verify "Corrupt state" and "Newer state version" name the affected file and start with defaults
- [x] 7.4 Implement startup restore of queue, position, history and imported collections without any resolution call, and verify "Queue restored after restart", "Startup does not resolve remote entries", "Restored session starts paused" and "History survives a restart"
- [x] 7.5 Resolve the config directory through `directories` and verify "First run creates the state directory" and "No writes in the working directory" using a temporary `HOME`
- [x] 7.6 Document every config key, the state format and the rollback procedure in `docs/configuration.md`, and verify each documented key round-trips through a test fixture

## 8. Integration checks

- [x] 8.1 Add a scenario-traceability check that fails when a `#### Scenario:` in this change's specs has no test named after it, and verify it by temporarily removing one test
- [x] 8.2 Write an end-to-end test with all fakes: start, import a playlist, build a queue across three sources, play, exit and restart, asserting queue, position and history are restored
- [x] 8.3 Run `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` with networking disabled, and record the commands and results in `docs/manual-testing.md`
- [ ] 8.4 Execute the manual matrix against real `mpd`, `mpv`, `yt-dlp` and `spotdl` locally, in tmux and over SSH, including art fallback and the two-second budget, and record each result in `docs/manual-testing.md`

## Workflow follow-up

- Archive the change after review and after the manual matrix is recorded.
- Verify the archived specs match the behaviour shipped.
