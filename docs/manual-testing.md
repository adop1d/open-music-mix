# Manual testing

## Environment

- macOS (darwin), Rust 1.95.0, edition 2024.
- External binaries are installed via Homebrew: `mpd` 0.24.17, `mpv` 0.41.0,
  `yt-dlp` 2026.10.21, `spotdl` 4.5.2 (pipx, Python 3.13), plus `ffmpeg`.
- The real-binary matrix in 8.4 has **not been executed yet**: it needs an
  interactive terminal, a running MPD with an indexed library, a public
  Spotify collection and optionally tmux/SSH. The rows below stay marked
  BLOCKED until each check is actually run.

## Automated checks

Run from a clean checkout, with networking disabled (the suite never opens a
socket; every external binary is faked):

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Recorded results:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | no issues found |
| `cargo test` | 115 passed, 0 failed (15 test binaries) |

Test counts per suite: resolver fixtures 7, album art 11, CLI/event loop 6,
config round-trip 4, end-to-end 1, local library 8, playback/queue 14,
queue operations 3, adapters 7, session persistence 9, Spotify import 14,
stream playback 10, traceability 1, TUI shell 20.

The suite uses in-memory fakes (`FakeMpd`, `FakeMpv`, `FakeResolver`,
`FakeImporter`), fixture scripts under `tests/fixtures/` (`yt-dlp`, `spotdl`)
and temporary directories only. No real daemon, player, downloader or Spotify
account is required, and no audio is downloaded.

### Scenario traceability (8.1)

`cargo test --test traceability` extracts every `#### Scenario:` name from the
change specs, slugifies it, and fails when no test function in `tests/` has that
name. It also asserts the scenario count is 86 with no duplicate slugs.

Verified by temporarily renaming `default_terminal_size` in
`tests/tui_shell.rs`:

- with the test removed: `test result: FAILED. 0 passed; 1 failed` (panic at
  `tests/traceability.rs:93` naming the missing scenario);
- after restoring the name: `test result: ok. 1 passed`.

## Real-binary matrix (8.4) — BLOCKED

Every row below requires binaries that are absent locally. They were not run
and must not be reported as passing.

| Environment | Check | Result | Note |
| --- | --- | --- | --- |
| Local terminal | MPD library browse and local playback | BLOCKED | `mpd`/`mpv` absent |
| Local terminal | YouTube search, resolve, play | BLOCKED | `yt-dlp`/`mpv` absent |
| Local terminal | Spotify import and playback | BLOCKED | `spotdl`/`yt-dlp` absent |
| Local terminal | Kitty album art | BLOCKED | depends on the runs above |
| Local terminal | Half-block art fallback | BLOCKED | depends on the runs above |
| Local terminal | First audio under 2 s | BLOCKED | covered by the paused-clock fake test only |
| tmux | All of the above | BLOCKED | binaries absent |
| SSH | All of the above | BLOCKED | binaries absent |

To execute the matrix on a machine with the binaries:

1. Start `mpd` with an indexed music library and confirm `mpc status` works.
2. Run `cargo run --release`; switch tabs with `1`/`2`/`3`, play a local album,
   then a YouTube query, then `--import` a public playlist and play a matched
   track. Confirm the `[local]`, `[youtube]` and `[spotify]` labels, the
   paused state after restart, and that `q` leaves the terminal restored.
3. Repeat inside `tmux` and over SSH; album art should downgrade to half blocks
   when graphics are unavailable, never break layout.
4. Confirm first audio starts before two seconds for a resolved stream.
5. Record each result in the table above.
