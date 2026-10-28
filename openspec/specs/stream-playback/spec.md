# stream-playback Specification

## Purpose
Plays remote entries on demand by resolving them to an audio stream and handing that stream to a player process, so nothing is downloaded ahead of time and no browser is involved.

## Requirements

### Requirement: Remote resolution without a browser

The player MUST resolve a youtube URL, video ID or search query into a playable audio stream, and MUST NOT open or require a web browser.

#### Scenario: Resolving a search query

- **WHEN** the user submits the query `the cure disintegration` for the youtube source
- **THEN** the player resolves the best matching result to a stream and starts playback

#### Scenario: Resolving a youtube URL

- **WHEN** the user submits a youtube watch URL
- **THEN** that exact video is resolved and played without an intermediate search

### Requirement: Playback is driven through the player process

The player MUST send the resolved stream to the external player process and MUST drive pause, resume, seek and volume through that process's control channel.

#### Scenario: Pausing a remote entry

- **WHEN** a youtube entry is playing and the user presses pause
- **THEN** the player process pauses and the progress indicator stops advancing

#### Scenario: Seeking a remote entry

- **WHEN** the user seeks a remote entry to 90 seconds
- **THEN** the player process continues from 90 seconds and the progress indicator matches that position

### Requirement: First audio within two seconds

For a reachable youtube or spotify entry, the player MUST produce audible output within two seconds of the play command, excluding the first launch of the player process in a session. The measurement includes source resolution.

#### Scenario: Normal playback start

- **WHEN** the user plays a resolvable youtube entry while the player process is already running
- **THEN** audio output begins within two seconds of the play command, covering resolution and playback start

### Requirement: Temporary audio buffer only

Audio fetched for remote playback MUST be written only to the operating system temporary directory or held in memory, and MUST be removed when the entry changes or the application exits.

#### Scenario: Track change removes the previous buffer

- **WHEN** a remote entry ends and the following entry starts
- **THEN** no audio file belonging to the previous entry remains in the temporary directory

#### Scenario: Exit removes buffered audio

- **WHEN** the application exits while a remote entry is playing
- **THEN** the temporary audio files written during the session are gone

### Requirement: Streaming degrades only remote sources

When the resolver or the player process is missing or crashes, the player MUST disable only the youtube and spotify sources, MUST show the reason, and MUST keep the local source operable.

#### Scenario: Resolver not installed

- **WHEN** the player starts and no `yt-dlp` binary is found
- **THEN** the youtube and spotify tabs are disabled with a message naming the missing binary and the local tab remains usable

#### Scenario: Player process crashes

- **WHEN** the player process exits unexpectedly during remote playback
- **THEN** the current entry is marked as failed and the interface keeps accepting input

### Requirement: Resolution errors are actionable

When resolution fails, the player MUST show a message identifying the failing entry and the reason class: missing binary, no results, or an unreachable source.

#### Scenario: No results for a query

- **WHEN** the resolver returns no results for a query
- **THEN** the player shows a no-results message for that query and leaves the queue unchanged
