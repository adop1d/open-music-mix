# Proposal

## Why

Listening to music while working in a terminal currently means juggling three things: a browser tab for YouTube or Spotify, a separate player for local files, and a terminal multiplexer where the actual work happens. The browser alone costs hundreds of megabytes of RAM and pulls attention away from the keyboard. There is no single, lightweight, keyboard-driven client that covers local files, YouTube streaming and Spotify playlists in one place.

## What Changes

- Add a new Rust binary that runs a full-screen TUI (Ratatui + Crossterm) and is the only user-facing process; it starts no daemon of its own.
- Unify three audio sources behind one queue: local files through an external `mpd`, YouTube / YouTube Music through `yt-dlp` + `mpv`, and Spotify playlists imported through `spotdl`.
- Provide the panel layout from rmpc: library explorer, current queue, and a Now Playing panel with metadata, progress, elapsed/remaining time and album art.
- Render album art with the terminal's native graphics protocol (Kitty, Sixel, iTerm2) and fall back to half-blocks when none is available, without breaking layout over SSH or inside tmux.
- Import Spotify playlists, albums and single tracks by URL or ID using `spotdl` for metadata resolution only; audio is never downloaded from Spotify, cached permanently or redistributed.
- Reach import from inside the player (a modal bound to `i`) and from the command line (`--import <url>`), so a collection can be added without navigating the interface.
- Stream remote audio without prior downloads: `yt-dlp` resolves a stream URL, `mpv` decodes it and is controlled over its IPC socket; the temporary buffer is deleted when the application exits.
- Control everything from the keyboard with Vim-style defaults (`j`/`k`/`h`/`l`, `Space`, `n`, `p`, `/`) and a discoverable help overlay. Mouse support is optional. The default keymap also covers queue editing, source tabs, import, retry, volume and quit.
- Present all user-facing strings in English and Spanish from a single lookup table, with no strings hardcoded in widgets.
- Persist imported playlists, the queue and play history in the platform config directory as TOML/JSON, so a restart restores the session without querying any remote metadata service.
- Degrade per source: if `mpd`, `mpv`, `yt-dlp` or `spotdl` is missing, only the affected tab is disabled and the reason is shown in the UI.

Out of scope for this change: permanent download or bulk archiving of Spotify tracks; the user's own Spotify library, Liked Songs or any account-scoped collection, which requires OAuth2; Spotify Connect, account login or any Premium-only API; DSP spectrum visualisers; Apple Music, Deezer or SoundCloud sources.

## Capabilities

### New Capabilities

- `playback-queue`: the single queue that mixes entries from all three sources, its transport operations (play, pause, next, previous, seek, volume), source labelling and play history.
- `local-library`: browsing and playing the local MPD library (artists, albums, folders, saved MPD playlists) as a protocol client, including behaviour when the daemon is absent.
- `stream-playback`: resolving remote sources with `yt-dlp` and driving `mpv` over IPC, including buffering, temporary cache lifetime and playback error surfacing.
- `spotify-import`: importing Spotify playlists, albums and single tracks by URL or ID, matching tracks to YouTube sources, and storing the import result.
- `tui-shell`: the panel layout, focus model, Vim-style key bindings, list search, help overlay and English/Spanish localisation.
- `album-art`: terminal graphics capability detection and album art rendering, including the half-block fallback and asynchronous loading.
- `session-persistence`: the on-disk format and location of configuration, imported playlists, queue and history, including startup restore and atomic writes.

### Modified Capabilities

None. `openspec/specs/` is empty; every capability above is new.

## Decisions

- **2026-10-23** — One application, three sources, one queue. Sources: local, youtube, spotify. This change touches the queue abstraction by design, since the queue is the integration point.
- **2026-10-23** — Local playback stays behind an external `mpd` daemon; the TUI is a protocol client and never decodes local audio itself. Avoids writing an audio engine and keeps the local library indexed by a mature daemon.
- **2026-10-23** — Remote playback is `yt-dlp` → `mpv` over IPC. `mpv` already handles buffering, seeking and volume, so no in-process decoder or FFmpeg pipeline is written.
- **2026-10-23** — `spotdl` is used for metadata and matching only. Audio that would be downloaded from Spotify is never stored; the stream buffer is temporary and removed on exit. Sources: spotify, youtube.
- **2026-10-23** — `design.md` covers the process boundaries and failure modes; this proposal does not prescribe module structure.
- **2026-10-23** — Import is reachable from a modal bound to `i` and from `--import <url>`. Sources: spotify.
- **2026-10-23** — A spotify track is matched to a youtube video during import and its stream is resolved only when it starts, so imports are done once and playback stays instant.
- **2026-10-23** — Local playback adds the selected item to the MPD queue and plays it without clearing the queue, so other MPD clients keep their entries. Sources: local.
- **2026-10-23** — Album art comes from MPD or a folder image for local tracks and from a thumbnail written by `yt-dlp` for remote tracks. No tag-reading crate and no HTTP client are added.
- **2026-10-23** — YouTube search is `ytsearch` against youtube.com; `music.youtube.com` links are accepted as ordinary youtube URLs. Sources: youtube, spotify.
- **2026-10-23** — The two-second first-audio budget includes resolution, not just player start.
- **2026-10-23** — Volume is stored per source; the single visible control edits the active source.
- **2026-10-23** — History keeps the 200 most recent entries and records repeats as new entries.
- **2026-10-23** — A restored session loads paused and never auto-plays.
- **2026-10-23** — Local, youtube and spotify are tabs inside the explorer panel; the queue and Now Playing panels stay visible.

## Impact

- New Rust crate and binary in this repository (currently no source code). Runtime dependencies on external binaries `mpd`, `mpv`, `yt-dlp` and `spotdl`, all detected at startup and never bundled.
- New crates: `ratatui`, `crossterm`, `tokio`, `mpd`, `ratatui-image`, `serde`, `toml`. Album art decoding uses an image decoder crate behind `ratatui-image`.
- New files under the platform config directory (`~/.config/<app>/` on Linux/macOS): `config.toml`, `state.json`, `imports/` and optional `i18n/<lang>.toml` overrides. No files are written outside the user's config directory and the OS temp directory.
- New command-line surface: `--import <url>` and `--help`. Without arguments the binary starts the TUI as before.
- Network access is limited to `yt-dlp` and `spotdl` calls; the TUI itself makes no outbound requests.
- Tests must fake `mpd`, `mpv`, `yt-dlp` and `spotdl` through fixtures; the suite runs offline.
