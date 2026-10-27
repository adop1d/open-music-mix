# Spotify imports

An import fetches Spotify **metadata only** and matches it to YouTube. No
Spotify audio is downloaded, stored or played.

## What happens

1. The reference is parsed (`parse_reference`): playlist, album, track URL,
   `spotify:` URI or a bare 22-character ID. Anything else is rejected with a
   notice listing the accepted forms.
2. `spotdl <url> --save-file <temp>` runs with a 120 s timeout. The JSON file is
   parsed and mapped to title, artists, album, duration and ISRC.
3. The collection is saved to `<config>/imports/<kind>-<id>.json` **before**
   matching, so an interrupted import stays browsable.
4. Each track is matched with `yt-dlp ytsearch1:"artist title"` (at most four
   concurrent searches). A candidate is accepted when its duration is within
   5 s of the Spotify duration; a missing duration is accepted as-is.
5. Matched tracks store the YouTube video ID. Unmatched tracks remain in the
   collection marked unavailable and are skipped when playing.
6. Re-importing the same collection overwrites the stored file in place, so the
   collection count does not grow.

## Playback

An imported entry plays through the YouTube path: the stored video ID is passed
straight to the resolver, so no search is issued at start time. The queue keeps
the `spotify` label.

## Limits

- Only public collections work. Account-only or private playlists are rejected
  (`Only public collections are supported`); there is no login or credential
  prompt, and credentials are never read.
- Metadata quality depends on `spotdl`; matching depends on `yt-dlp` and the
  search result. Region-restricted or deleted videos stay unavailable.
- `spotdl` and `yt-dlp` must be on `PATH`; otherwise importing fails with a
  message naming the missing binary.
- Imports need network access. Browsing and playing already-stored imports works
  offline.

## Run it

- `./target/release/omm --import https://open.spotify.com/playlist/<id>`
- or start the TUI and press `i`, paste the URL, press `Enter`.

Watching it work with a recorded fixture (offline, no Spotify account):

- `cargo test --test spotify_import importing_a_playlist_url`

The test drives `RealSpotifyImporter` against `tests/fixtures/spotdl`, which
replays `tests/fixtures/playlist.json` and writes no audio.
