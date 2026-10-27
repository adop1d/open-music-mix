# Configuration, state and rollback

## Location

`config_dir()` resolves, in order:

- `OMM_CONFIG_DIR` when set and non-empty (used by the tests).
2. The platform config directory for `omm`:
  `~/Library/Application Support/omm` (macOS),
  `~/.config/omm` (Linux),
  `%APPDATA%\omm` (Windows).
3. `./.omm` only when the platform directory cannot be determined.

The first run creates the directory, `imports/` and a default `config.toml`.
The program never writes to the working directory.

## `config.toml` keys

All keys are optional; missing keys use the default. An unparsable file is
renamed to `config.corrupt-<unix-time>` and defaults are used with a warning.

| Key | Default | Valid values |
| --- | --- | --- |
| `ui.language` | unset (English) | `en`, `es`; anything else warns and falls back to English |
| `ui.theme` | `default` | free text, reserved for display themes |
| `playback.volume_local` | `70` | 0–100 |
| `playback.volume_remote` | `70` | 0–100 |
| `playback.volume_step` | `5` | 1–50 |
| `mpd.host` | `127.0.0.1` | hostname or IP |
| `mpd.port` | `6600` | 1–65535 (0 warns and resets) |
| `mpd.socket` | unset | path to a Unix socket; overrides host/port when set |
| `mpd.password` | unset | MPD password |
| `paths.cache_dir` | unset (OS temp dir) | directory used for the audio buffer and art; removed on exit |
| `keys.<action>` | see below | one key: a single character, `space`, `enter`, `esc`, `tab`, `up`, `down`, `left`, `right`, `backspace` |

Invalid values warn once at startup and fall back **per key**; the rest of the
file still applies. Warnings are printed to stderr and the last one is shown in
the Now Playing panel.

Environment overrides, applied after the file: `MPD_HOST` (a path starting with
`/` becomes `mpd.socket`) and `MPD_PORT`. `OMM_CONFIG_DIR` selects the
directory.

## Key bindings

`[keys]` maps an action id to a key. Rebinding removes the action's previous
key, so an action always has exactly one binding.

| Action id | Default | Action id | Default |
| --- | --- | --- | --- |
| `move_down` | `j` | `seek_forward` | `right` |
| `move_up` | `k` | `volume_up` | `+` |
| `collapse` | `h` | `volume_down` | `-` |
| `expand` | `l` | `retry` | `r` |
| `cycle_panel` | `tab` | `import` | `i` |
| `tab_local` | `1` | `search` | `/` |
| `tab_youtube` | `2` | `help` | `?` |
| `tab_spotify` | `3` | `escape` | `esc` |
| `play_pause` | `space` | `cycle_view` | `v` |
| `play_now` | `enter` | `quit` | `q` |
| `append` | `a` | `seek_back` | `left` |
| `remove` | `d` | `next` | `n` |
| `clear` | `c` | `previous` | `p` |

Example: `next = "]"` under `[keys]` unbinds `n` and binds `]`.

Each documented key is verified to round-trip by
`tests/config_roundtrip.rs`; that suite also iterates all 26 actions.

## Text overrides

Place `i18n/es.toml` (or `i18n/en.toml`) next to `config.toml`. Keys are the
flat identifiers used by the program (`panel_queue`, `action_next`, `msg_*`).
Values override the built-in table only; missing keys fall back to English and
then to the key name. A file that does not parse is ignored with a warning that
names the path.

## State format

`state.json` is written atomically (temp file, flush, rename) on exit and has
`format = 1`:

- `format`: integer schema version.
- `queue`: entries with `id`, `source` (`local`/`youtube`/`spotify`), `meta`
  (`title`, `artists`, `album`, `duration`), `locator` and `state`.
- `position`: index of the current entry, or null.
- `history`: newest-first list of `{source, title, timestamp}`; capped at 200.
- `volumes`: `{local, remote}`.
- `tab`: last visible source.

Startup restores the queue without resolving anything. Playback starts paused:
the first `Space` loads the restored entry instead of resuming audio.

A corrupt `state.json` is renamed to `state.corrupt-<unix-time>` and the session
starts empty. A file with `format` greater than 1 is left untouched and the
session starts empty with a warning naming the file; never downgrade it in
place.

## Rollback procedure

1. Quit the TUI so the final state is written.
2. Copy `state.json` aside if you want to keep it: `cp state.json state.bak`.
3. To forget the session, delete `state.json`. To forget one import, delete its
   file from `imports/`.
4. To undo a configuration change, edit or delete `config.toml`; deleting it
   makes the next start write defaults.
5. To undo a bad translation override, delete the file under `i18n/`.
6. To return to an older schema, delete `state.json` (or restore an older
   backup) — the program refuses to rewrite newer formats.

Imports are independent of the queue: restoring an older `state.json` never
deletes stored collections.
