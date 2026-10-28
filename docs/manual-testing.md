# Manual testing

## Environment

- macOS (darwin), Rust 1.95.0, edition 2024, tested 2026-10-27.
- External binaries are installed via Homebrew: `mpd` 0.24.17, `mpv` 0.41.0,
  `yt-dlp` 2026.10.21, `spotdl` 4.5.2 (pipx, Python 3.13), plus `ffmpeg`.
- MPD runs as a `brew services` user service with 746 indexed songs.
- The real-binary matrix in 8.4 was executed on this machine on 2026-10-27.
  Checks that were not run are marked NOT RUN, not BLOCKED.

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
| `cargo test` | 118 passed, 0 failed (17 suites) |

Test counts per suite: lib unit 2, resolver fixtures 8, album art 11, CLI/event
loop 6, config round-trip 4, end-to-end 1, local library 8, playback/queue 14,
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

## Real-binary matrix (8.4) — executed 2026-10-27

| Environment | Check | Result | Note |
| --- | --- | --- | --- |
| Local terminal | MPD library browse and local playback | PASS | 746 songs; queue `[local]`, `mpc status` `[playing]` |
| Local terminal | YouTube search, resolve, play | PASS | `ytdl://` + android client; see note below |
| Local terminal | Spotify import and playback | PASS | `1 matched / 1 tracks`; queue `[spotify]`, art loaded |
| Local terminal | Kitty album art | NOT RUN | tmux session had no Kitty graphics protocol |
| Local terminal | Half-block art fallback | PASS | block art rendered in tmux |
| Local terminal | First audio under 2 s | NOT MEASURED | stream startup varies by video and network |
| tmux | All of the above | PASS | local, YouTube, Spotify and half-block art |
| SSH | All of the above | NOT RUN | no SSH session used |

How this was run:

1. Started MPD with an indexed library and confirmed `mpc status` and
   `mpc stats` (746 songs).
2. Ran `target/release/omm` in tmux; played a local album, a YouTube query and
   an imported Spotify track. Confirmed the `[local]`, `[youtube]` and
   `[spotify]` labels, the playing marker, and that position advanced.
3. Ran `omm --import https://open.spotify.com/track/...` against a public
   track; the import printed `1 matched / 1 tracks` and wrote the import JSON.
4. Confirmed half-block album art in tmux. Kitty/SSH were not available.

YouTube note: the default yt-dlp web clients returned `HTTP 403 Forbidden` on
the signed `googlevideo` URL on this network. The `android` player client
returned `206` for the same URL, so `omm` passes
`--ytdl-raw-options=extractor-args=youtube:player_client=android` to mpv's
yt-dlp hook. This is a workaround for YouTube's current behaviour, not a
permanent guarantee; update `yt-dlp` if playback breaks again.

Spotify imports are metadata-only and limited to public collections. The
matched track still plays through YouTube, so it inherits the note above.
