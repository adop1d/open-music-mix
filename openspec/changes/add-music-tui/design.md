# Design

## Context

See `proposal.md` for motivation. This repository has no source code yet, so every module below is new. The relevant constraints come from `openspec/config.yaml`: one Rust binary, Ratatui for the interface, three sources with independent failure, a single queue, and offline tests. The specs in `specs/` define observable behaviour; this document only explains how the pieces fit.

## Goals / Non-Goals

**Goals:**

- One event loop that never blocks on process or network I/O, with all slow work in workers.
- A single queue model that is source-agnostic, plus one adapter per source.
- Every external process behind a trait so the suite runs offline with fixtures.
- Failure of any one binary or connection is contained to its source.
- A first playable version whose module boundaries hold up when more sources are added.

**Non-Goals:**

- Windows support in this iteration; the mpv control channel uses a Unix socket. Windows-shaped code may exist but is untested.
- The user's own Spotify library, Liked Songs or any account-scoped collection: reading them requires OAuth2, which is out of scope.
- An in-process audio decoder, an FFmpeg pipeline, or spectrum analysis.
- Forcing a specific audio codec; `bestaudio` is accepted as the resolver returns it.
- Offline caching of streamed audio, background prefetch of the next track, or gapless playback.
- Plugin system, remote control API, session sharing, or multiple simultaneous outputs.
- Retroactive migration of previous state files; this is the first state format.

## Decisions

### Process and runtime model

One Tokio runtime with a single owner of application state: the main loop. Everything else communicates with it through `mpsc::Sender<AppEvent>`:

```
crossterm input task ─┐
tick (250 ms) ────────┤
mpd connection task ──┼──> mpsc<AppEvent> ──> App::handle() ──> render()
mpv ipc task ─────────┤                              │
resolver tasks ───────┘                     mpsc<Command> per worker
```

- `App` holds `Queue`, `PlaybackState`, per-source `SourceStatus`, transient UI state (focus, filter, overlay) and an i18n handle. It never awaits.
- Commands to sources are enqueued; workers own their connection or child process and send results back as events.
- Rendering happens on a tick and after processing a batch of events, never on a raw keypress path that touches disk or network.
- A single `AppEvent` enum (`SourceStatusChanged`, `QueueUpdated`, `PlaybackProgress`, `ImportProgress`, `ResolutionFailed`, …) keeps worker/main coupling explicit.

Alternative considered: one thread per source with shared `Arc<Mutex<App>>`. Rejected because it serialises the render loop against slow paths and makes partial UI states hard to reason about.

### Queue model

```rust
enum Source { Local, Youtube, Spotify }

struct TrackMeta { title: String, artists: Vec<String>, album: Option<String>, duration: Option<Duration> }

struct Entry {
    id: EntryId,
    source: Source,
    meta: TrackMeta,
    locator: Locator,      // LocalPath | MpdUri | YoutubeId | SpotifyTrackId
    state: EntryState,     // Pending | Ready | Playing | Failed(reason) | Unavailable
}

struct Queue { entries: Vec<Entry>, position: Option<usize>, history: VecDeque<HistoryItem> /* capped at 200 */ }
```

`Queue` is the only queue; MPD's own queue is not used as the source of truth. When a local entry plays, the adapter appends that URI with `addid` and starts it with `playid`, then stops it on pause. It never clears the daemon queue, so entries queued by other MPD clients survive. `next`/`previous`, history and source labels are therefore identical across sources while MPD remains a playback target, not a second queue to reconcile.

`Locator` is deliberately separate from `meta`: the queue and history must be restorable without resolving anything (spec: session-persistence, "Startup does not resolve remote entries").

### Source adapters

A shared trait keeps the main loop uniform:

```rust
trait SourceAdapter: Send {
    async fn play(&mut self, entry: &Entry) -> Result<()>;
    async fn pause(&mut self) -> Result<()>;
    async fn resume(&mut self) -> Result<()>;
    async fn seek(&mut self, position: Duration) -> Result<()>;
    async fn set_volume(&mut self, percent: u8) -> Result<()>;
}
```

- **Local** — `mpd` crate over TCP or Unix socket (`MPD_HOST`/`MPD_PORT`, plus config override). Holds one reconnecting connection; `idle` subsystem events are forwarded as `PlaybackProgress`/`StatusChanged`. No local decoding, no writes to the MPD database.
- **Youtube / Spotify** — both use the same remote adapter because a spotify entry resolves to a youtube video (see below). The adapter keeps one long-lived `mpv` process and one `yt-dlp` invocation per track.

### mpv control

`mpv` is spawned once per session with an idle loop:

```
mpv --idle=yes --no-video --audio-display=no --no-ytdl \
    --input-ipc-server=<state-tmp>/mpv.sock \
    --cache=yes --cache-secs=20 --demuxer-max-bytes=64MiB
```

- Playback is `{"command": ["loadfile", "<resolved-url>", "replace"]}` over the JSON IPC socket; pause/seek/volume use `set_property`; position and duration arrive as `property-change` events.
- `--no-ytdl` is required: resolution is always ours, so the pipeline is deterministic and a failure is attributable to one step.
- The socket lives under the OS temp directory and is removed on exit, together with the cache directory (`--cache-dir=<tmp>/audio`).
- If mpv dies: the adapter marks the source failed, the current entry as failed, and spawns a replacement on the next play command, at most once per track.

Alternative considered: letting mpv call yt-dlp itself (`--ytdl-format`). Rejected: no seam for tests, no per-step error classes, and no way to enforce the two-second budget separately for resolution and playback.

### Resolution and matching

- One backend: `ytsearch` against youtube.com. `music.youtube.com` links are accepted and treated as ordinary youtube URLs; playlists are never expanded during playback (`--no-playlist`), only the requested video is played.
- A youtube entry resolves with `yt-dlp --no-playlist --no-warnings --get-url -f bestaudio/best <url-or-ytsearch1:<query>>`, 10 s timeout. The same call reports the thumbnail URL used by the art layer.
- A spotify entry carries a stored `YoutubeId` from import time, so playback is the same URL fetch as youtube and never a search.
- Import runs `spotdl save <url> --save-file <tmp>.json` (metadata only; the `save` subcommand does not download audio), then for each track resolves a candidate with `yt-dlp ytsearch1:"<artist> - <title> audio"` under a concurrency cap of 4.
- Match acceptance: candidate duration within ±5 s of the Spotify duration, else the track is stored `Unavailable` with its search query kept for a later retry. ISRC is stored but not used as a search key, because YouTube search does not index it.
- Track metadata is persisted before matching starts, so an aborted import still leaves a browsable collection.
- Import shape: playlist and album references become a collection named after them; a single track reference becomes a one-entry collection named after the track.
- A reference that cannot be read without authentication is rejected as "public collections only"; no credential prompt exists.
- Entry points: a modal bound to `i` and the `--import <url>` argument call the same importer. The CLI form runs the import before the terminal becomes interactive and reports failures on stderr.
- Restoring a session never resolves: entries keep their stored `Locator` and only `play` triggers a fetch (spec: startup does not resolve remote entries).
- The two-second budget is measured from the `play` command to the player's first `playback-restart` event, resolution included. The test injects a resolver with a fixed delay and a paused clock and asserts the event lands under two seconds of virtual time.

### State and persistence

- Directory from the `directories` crate (`ProjectDirs::config_dir()` → `~/.config/<app>/` on Linux, `~/Library/Application Support/<app>/` on macOS). Justification: the standard library has no platform config-directory API and hand-rolling env-var precedence invites drift.
- Files: `config.toml`, `state.json`, `imports/<collection-id>.json`, and optional `i18n/<language>.toml` overrides.
- `state.json` holds queue entries with their `Locator`, the position, the 200-entry history and per-source volumes. Restore never auto-plays: the current entry loads as `Paused` and no process call is issued until the user presses play.
- Writes: serialise to `<file>.tmp` in the same directory, flush and `fsync`, then `rename`. Version field `format = 1`.
- On parse failure: rename to `<file>.corrupt-<timestamp>`, start with defaults, emit a warning event. On a newer `format`: leave the file untouched, start empty, warn.
- Config merge: defaults → file → environment overrides. Invalid values produce a warning and the default for that key only.

### Terminal interface

- Layout: three vertical panes on wide terminals (explorer 30%, queue 35%, now playing 35%). The explorer holds the source tabs `1` local, `2` youtube, `3` spotify; switching tabs does not touch the other panes. Under 80×24 the explorer collapses and remains reachable by focus cycling.
- Widgets are stateless renderers over `&App`; all state lives in `App`, which keeps snapshot tests simple with `TestBackend`.
- Keymap: `HashMap<Action, KeyChord>` built from defaults and overridden by `[keys]` in TOML. The help overlay is generated from this map, so it cannot drift from the active bindings.

| Key | Action |
|---|---|
| `j` / `k` | move down / up |
| `h` / `l` | collapse / expand in tree views; switch panel in flat lists |
| `Tab` | cycle panels |
| `1` / `2` / `3` | local / youtube / spotify tab |
| `Space` | play or pause |
| `Enter` | play the selection now |
| `a` | append the selection to the queue |
| `d` | remove the selected queue entry |
| `c` | clear the queue |
| `n` / `p` | next / previous |
| `←` / `→` | seek back / forward 5 s |
| `+` / `-` | volume step on the active source |
| `r` | retry a failed entry |
| `i` | open the import modal |
| `/` | search the focused list |
| `?` | help overlay |
| `Esc` | close overlay or clear the query |
| `q` | quit and save state |

- Volume is stored per source (`[playback] volume_local`, `volume_remote`); the single visible control in Now Playing writes to the active source only.
- i18n: embedded `en.toml` and `es.toml` (`include_str!`) parsed into `HashMap<&'static str, String>`. A user file `i18n/<language>.toml` is merged over the built-in table key by key; an unparsable override is ignored with a warning. `t(key)` looks up the active language, then English, then returns the key. Language from `[ui] language`, else `LC_ALL`/`LANG`, else English. Embedding avoids a compile-time exhaustive match, which would make the "missing translation falls back to English" scenario impossible to exercise.

### Album art

- `ratatui-image` `Picker::from_query_stdio()` detects Kitty, Sixel and iTerm2 by querying the terminal; failure means half-blocks. Half-block rendering is also forced when the panel is narrower than the protocol's minimum.
- Local fetch order: the picture served by the MPD daemon (`albumart`), then a cover image in the album folder (`cover.*`, `folder.*`, `front.*`). Audio tags are never read, so no tag parser is added.
- Remote fetch: after resolution, `yt-dlp --skip-download --write-thumbnail -o <tmp>/art/%(id)s` writes the thumbnail to the temporary directory; no HTTP client is added. The file disappears with the rest of the temp cache on exit.
- Decoding runs in `spawn_blocking`; an LRU of 20 decoded images keyed by `(source, album, art-path)` bounds memory. Placeholder is the track title; missing or corrupt art never produces an error dialog.

### Process boundaries and failure modes

| Process | Discovery | Control channel | Failure mode in UI | Test seam |
|---|---|---|---|---|
| `mpd` | `PATH` (`which mpd`); connection settings from config/env | MPD protocol over TCP/Unix socket (`mpd` crate) | Local tab disabled with reason; youtube/spotify unaffected; reconnect with backoff | `FakeMpd` implementing the adapter trait over an in-memory script |
| `mpv` | `PATH`; version logged at start | JSON IPC over Unix socket | Current entry failed, source marked failed, respawn once on next play | `FakeMpv` exposing the same request/event surface as a channel fixture |
| `yt-dlp` | `PATH`; version logged at start | Subprocess stdout (one URL) | Remote sources disabled with "yt-dlp not found"; no crash | Fixture binary or injected `Resolver` returning canned results/errors |
| `spotdl` | `PATH`; import tab disabled when missing | Subprocess writing the metadata JSON file | Import tab disabled with reason; existing imports stay browsable | `FakeImporter` returning recorded `.spotdl` JSON |

Every boundary is a trait (`MpdBackend`, `MpvBackend`, `Resolver`, `SpotifyImporter`), and the real implementations are thin wrappers over the process or socket. That is what makes "one test per scenario, no network" achievable.

### Module layout

```
src/main.rs            runtime bootstrap, terminal setup/teardown
src/cli.rs             argument parsing (--import, --help), no argument parser crate
src/app.rs             App state, event handling, command dispatch
src/queue/             queue, entry, history
src/sources/           local, remote, spotify adapters
src/process/           mpd, mpv, yt-dlp, spotdl boundaries
src/persist/           config, state, atomic file IO
src/ui/                layout, widgets, help overlay
src/i18n/              en.toml, es.toml, lookup
src/art.rs             picker, fetch, decode, LRU
tests/                 scenario tests with fakes and fixtures
```

Each file stays under the 800-line cap; `ui/` and `sources/` are directories from the start for that reason.

### Dependencies and justification

| Crate | Why it is needed | Alternative rejected |
|---|---|---|
| `ratatui`, `crossterm` | Fixed by project context | — |
| `tokio` | async runtime, child processes, channels | threads + blocking IO (loses timeouts, complicates cancellation) |
| `mpd` | protocol client for the local source | hand-written protocol (duplicates a maintained parser) |
| `ratatui-image` + `image` | graphics protocols, half-block fallback, decoding | hand-written Kitty/Sixel encoders |
| `serde`, `serde_json`, `toml` | config, state, import fixtures | hand-rolled parsers |
| `directories` | platform config directory | `dirs` (same role, fewer maintained guarantees) or manual env handling |

No HTTP client is added: every network call goes through `yt-dlp` or `spotdl`, which own their own transport. No audio tag parser is added either: local art arrives through MPD or a folder image. No logging framework in this iteration; startup diagnostics go to the UI and `stderr` is captured only for error messages.

## Risks / Trade-offs

- **Matching quality (spotify → youtube)** → duration tolerance plus a stored query; unmatched tracks are visible and retryable rather than silently dropped.
- **A stored youtube match can disappear later** (video removed or region-blocked) → the entry fails with its failure reason, and `r` retries it: first the stored video, then a fresh search from the kept query.
- **Import cost grows linearly with playlist size** (one `yt-dlp` search per track) → concurrency cap 4, metadata saved before matching, and progress shown; a 100-track playlist is seconds-to-a-minute, not instant.
- **`spotdl` is the most brittle dependency** (its bundled Spotify credentials can change) → failures surface as an import error class; imported collections remain readable offline, so the damage is bounded to the import action.
- **Kitty graphics inside tmux require passthrough** and detection may misreport → capability is verified by the terminal's query response, and any failure falls back to half-blocks on the next resize.
- **Two-second first-audio budget depends on the network and on mpv already running** → mpv is started with the app and kept idle; the budget excludes that first launch, as the spec states. If the budget is missed, the progress indicator still moves and a debug overlay exposes resolution and startup timings.
- **Long-lived mpv process holds an open audio device** → volume and pause are always routed to mpv, never to the system mixer, so the app never leaves a second audio path behind.
- **State format v1 is unversioned in behaviour** (only the `format` integer) → adding fields is backward compatible with `serde(default)`; a breaking change gets `format = 2` and the preservation rule already defined.
- **Keyboard-only coverage** is easy to regress when adding a mouse-friendly widget → the help overlay test asserts every `Action` in the keymap has a binding, so an action cannot ship unreachable.

## Migration Plan

Greenfield, so there is no data or code to migrate:

1. Land the binary and default config; the first run creates `~/.config/<app>/`.
2. Rollback is removing the binary and, optionally, the config directory; no system service or database is involved.
3. No feature flags; the change ships as one release.

## Open Questions

None that affect specs, approach or task breakdown. The remaining unknowns (exact LRU bound, cache byte cap, import concurrency) are tuning values with defaults set above and no observable contract beyond the specs.
