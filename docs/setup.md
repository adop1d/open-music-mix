# Initial setup

This is the shortest path from a clean machine to a working `omm`: install the
external programs, start MPD with an indexed library, build `omm`, then check
each source once.

`omm` only needs MPD for the Local tab. `mpv` + `yt-dlp` enable YouTube, and
`spotdl` + `ffmpeg` enable Spotify imports. A missing optional binary disables
that source; the TUI still starts.

## 1. Install the dependencies

macOS (Homebrew):

- `brew install mpd mpc mpv yt-dlp ffmpeg pipx`
- `pipx install spotdl`
- `pipx ensurepath` (then open a new shell if `spotdl` is not found)

Debian/Ubuntu:

- `sudo apt install mpd mpc mpv yt-dlp ffmpeg pipx`
- `pipx install spotdl`
- `pipx ensurepath`

Rust 1.85 or newer is required (edition 2024). Install it with
[rustup](https://rustup.rs) or your package manager.

Check everything is on `PATH`:

- `mpd --version`
- `mpc --version`
- `mpv --version`
- `yt-dlp --version`
- `spotdl --version`
- `ffmpeg -version`

`mpc` is not used by `omm`; it is the quickest way to verify MPD.

## 2. Configure and start MPD

Create the data directories and a config at `~/.mpdconf`:

- `mkdir -p ~/.mpd/playlists`

```conf
music_directory "~/Music"
playlist_directory "~/.mpd/playlists"
db_file "~/.mpd/database"
log_file "~/.mpd/log"
state_file "~/.mpd/state"
bind_to_address "127.0.0.1"
port "6600"

audio_output {
    type "osx"
    name "CoreAudio"
}
```

On Linux, replace the output block with your sound server:

```conf
audio_output {
    type "pulse"
    name "PulseAudio"
}
```

Use `type "alsa"` with `device "default"` instead if you do not run
PulseAudio/PipeWire. `audio_output` must be a multiline block; MPD rejects the
inline `{ ... }` form.

Start MPD in the foreground:

- `mpd --no-daemon ~/.mpdconf`

Or let a service manager start it:

- macOS: `brew services start mpd`
- Linux: use your user service, or run the command above at login. The packaged
  system service often runs as the `mpd` user, which may not be able to read
  `~/Music`; a personal setup is simpler when MPD runs as you.

Index the library and confirm MPD sees it:

- `mpc update`
- `mpc ls | head`

`mpc update` can take a while on a large library. `mpc status` should connect
without errors. On macOS, pointing `music_directory` at `~/Music` also picks up
Apple Music files; DRM-protected files will not play, but MP3/FLAC/AAC files
will.

If MPD is not on port 6600, note the port for the next step.

## 3. Build omm

From the repository root:

- `cargo build --release`
- `./target/release/omm --help`

Or install it on `PATH`:

- `cargo install --path .`
- `omm --help`

## 4. First run

With MPD running:

- `./target/release/omm`

The first run creates the config directory and a default `config.toml`:

- macOS: `~/Library/Application Support/omm/config.toml`
- Linux: `~/.config/omm/config.toml`

If MPD uses another port, edit the file:

```toml
[mpd]
port = 6601
```

The equivalent environment overrides are `MPD_HOST` and `MPD_PORT`. To keep a
test session away from your real state, set `OMM_CONFIG_DIR`:

- `OMM_CONFIG_DIR=/tmp/omm-test ./target/release/omm`

## 5. Verify each source

1. **Local:** press `1`, expand an artist/album, and press `Enter`. The queue
   should show `[local]`, and `mpc status` should report `[playing]`.
2. **YouTube:** press `2`, press `/`, type a query, and press `Enter`. The first
   stream can take a few seconds while `yt-dlp` resolves it.
3. **Spotify:** import a public playlist, album, or track, then play a matched
   entry from tab `3`:

   - `./target/release/omm --import https://open.spotify.com/playlist/<id>`

   The import prints `name: matched / total` and then starts the TUI. Private
   collections are not supported.
4. **Album art:** Kitty, WezTerm, Ghostty and Konsole show full-resolution
   images through the Kitty graphics protocol. Other terminals, and narrow
   panels, fall back to half-blocks; this is expected.

## Troubleshooting

- **`No configuration file found` / `Failed to open "~/.mpdconf"`** — the config
  does not exist yet. Create it with the block in section 2, or pass an existing
  path explicitly (`mpd --no-daemon /path/to/mpd.conf`). MPD also searches
  `~/.mpd/mpd.conf`, `~/Library/Application Support/mpd/mpd.conf` (macOS), and
  `~/.config/mpd/mpd.conf` (Linux).
- **`brew services start mpd` fails with `Bootstrap failed: 5`** — an MPD is
  usually already running and holding port 6600, often a leftover
  `mpd --no-daemon` from an earlier test. Check the listener with
  `lsof -nP -iTCP:6600 -sTCP:LISTEN`, stop the old process (`pkill mpd`, or
  `brew services stop mpd` first), then run `brew services restart mpd`. Verify
  with `brew services list` (state `started`) and `mpc status`.
- **Connection refused / MPD is not available** — MPD is not running, or it is
  on another port. Check `mpc status` and `lsof -nP -iTCP:6600 -sTCP:LISTEN`.
- **Local tab is empty** — run `mpc update`, then `mpc ls`. Check that
  `music_directory` exists and is readable by the user running MPD.
- **Volume does nothing / `No mixer`** — the output has no hardware mixer.
  Playback still works; add `mixer_type "software"` inside the `audio_output`
  block if you want volume control.
- **YouTube fails with `HTTP error 403 Forbidden`** — `omm` asks mpv to use
  YouTube's `android` player client, which avoids the 403s seen with the default
  web clients on some networks. If it still fails, update `yt-dlp`
  (`brew upgrade yt-dlp` or `pipx upgrade yt-dlp`) and retry.
- **Spotify import fails** — check `ffmpeg` and `spotdl` versions, use a public
  URL, and retry. Imports have a 120-second timeout per collection.
- **No album art** — the terminal lacks a supported graphics protocol, or the
  panel is too narrow. Half-blocks are the fallback, not a failure.

More detail: [configuration.md](configuration.md) for config and state,
[imports.md](imports.md) for import limits, and
[manual-testing.md](manual-testing.md) for the verification record.
