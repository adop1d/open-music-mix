# local-library Specification

## Purpose
Lets the user browse and play the music already indexed by their MPD daemon, presenting the daemon as the local source without taking over file indexing or audio decoding.

## Requirements

### Requirement: Browse the local library

The player MUST list the local library by artist, album, folder and MPD saved playlist, and MUST show each view without waiting for the MPD daemon to finish any other operation.

#### Scenario: Artist view lists albums

- **WHEN** the MPD daemon reports an artist with two albums and the user opens that artist
- **THEN** both albums are listed under the artist

#### Scenario: Empty local library

- **WHEN** the MPD daemon reports no tracks
- **THEN** the local tab shows an empty-state message and remains navigable

### Requirement: Local playback is delegated to MPD

The player MUST send local playback commands to the MPD daemon, MUST NOT decode local audio itself, and MUST NOT clear the daemon's existing queue when starting a local item.

#### Scenario: Playing a local track

- **WHEN** the user plays a local album with play
- **THEN** the player sends the album to the MPD daemon, the daemon starts playback and the Now Playing panel reflects the daemon's state

#### Scenario: Existing daemon queue is preserved

- **WHEN** the MPD daemon holds three entries queued by another client and the user plays a local album
- **THEN** the album is added and started and those three entries remain in the daemon's queue

#### Scenario: Pausing local playback

- **WHEN** a local track is playing and the user presses pause
- **THEN** the MPD daemon reports the pause and the panel shows the paused state

### Requirement: MPD unavailable degrades only the local source

When the MPD daemon is missing, unreachable or disconnects, the player MUST disable the local tab, MUST show the reason, and MUST keep the youtube and spotify sources operable.

#### Scenario: Daemon not installed

- **WHEN** the player starts and no `mpd` binary is found
- **THEN** the local tab is disabled with a message stating that MPD is not available and the youtube and spotify tabs remain usable

#### Scenario: Daemon disconnects during playback

- **WHEN** the MPD daemon disconnects while a local track is playing
- **THEN** the player marks the local source as disconnected and keeps the interface responsive

### Requirement: Local list search

The player MUST filter the currently loaded local view by a case-insensitive text query without sending the query to the MPD daemon and without changing the daemon's state.

#### Scenario: Search filters the artist list

- **WHEN** the local artist list is focused and the user types `cure`
- **THEN** only artists whose name contains `cure` remain visible and the original order is preserved
