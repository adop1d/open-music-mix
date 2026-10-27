# oom

One keyboard-driven terminal queue for local MPD music, YouTube streams and
Spotify playlists. Local files play through MPD; everything remote is resolved
with `yt-dlp` and played by a long-lived `mpv` process. Spotify is metadata
only: `spotdl` fetches the track list, then each track is matched to YouTube.

## Requirements

- Rust 1.85 or newer (edition 2024)
- `mpd` running locally for the Local tab
- `mpv`, `yt-dlp` and `spotdl` on `PATH` for YouTube and Spotify
- A terminal with Kitty graphics, iTerm2 inline images, Sixel, or plain
  half-blocks. Narrow panels automatically fall back to half-blocks.

Missing binaries disable only the sources that need them; the TUI still starts
and the queue still works.

## Build and run

From a clean checkout:

- `cargo build --release`
- `./target/release/omm`

During development: `cargo run --release`.

Import a Spotify playlist, album or track before the TUI starts:

- `./target/release/omm --import https://open.spotify.com/playlist/<id>`
- `./target/release/omm --import spotify:album:<id>`

The result is printed on stderr (`name: matched / total`) and the TUI starts
afterwards. Any failure exits non-zero.

- `` `omm --help` prints usage.``

## Keys

`?` opens the help overlay, generated from the active keymap so it always
matches. Defaults: `j`/`k` move, `h`/`l` collapse/expand, `Tab` cycles panels,
`1`/`2`/`3` switch sources, `Space` play/pause, `Enter` play now, `a` append,
`d` remove, `c` clear, `n`/`p` next/previous, arrows seek, `+`/`-` volume,
`r` retry, `i` import, `/` filter, `v` cycle the local view, `q` quit.

Every binding can be replaced in `config.toml` (see
[docs/configuration.md](docs/configuration.md)).

## Where state lives

- The config directory comes from the platform (`~/Library/Application Support/omm`
on macOS, `~/.config/omm` on Linux, `%APPDATA%\omm` on Windows).
- Set `OMM_CONFIG_DIR` to override it.
`state.json`, `i18n/<language>.toml` overrides and `imports/<id>.json`.
Nothing is written to the working directory.

## Tests

- `cargo test` runs the offline suite (no network, no real `mpd`/`mpv`/`yt-dlp`/`spotdl`).
- `cargo fmt --check`
- `cargo clippy -- -D warnings`

More detail: [docs/imports.md](docs/imports.md) for import behaviour and limits,
[docs/configuration.md](docs/configuration.md) for config/state/rollback and
[docs/manual-testing.md](docs/manual-testing.md) for the verification record.
