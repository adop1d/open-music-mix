# Spec Delta

## Purpose

Holds every track the user plays, regardless of source, in a single ordered queue with predictable transport behaviour and a history of what was played.

## ADDED Requirements

### Requirement: Single queue across sources

The player MUST keep one ordered queue whose entries come from the local, youtube and spotify sources, and each entry MUST carry the source it came from.

#### Scenario: Queue mixes entries from all three sources

- **WHEN** the user appends a local track, a youtube track and a spotify track
- **THEN** the queue contains three entries in that order, each labelled local, youtube and spotify respectively

#### Scenario: Source remains visible while playing

- **WHEN** a spotify entry is the current track
- **THEN** the Now Playing panel shows the spotify label next to the track title

### Requirement: Transport control

The player MUST support play, pause, next, previous, seek and volume commands, applied to the current entry through the mechanism of its source.

#### Scenario: Next advances within the queue

- **WHEN** the queue has more entries after the current one and the user presses next
- **THEN** the following entry starts playing and the queue position advances by one

#### Scenario: Previous at the first entry

- **WHEN** the first entry is playing and the user presses previous
- **THEN** playback restarts the first entry from its beginning and the queue position does not change

#### Scenario: Seek on a remote entry

- **WHEN** a spotify or youtube entry is playing and the user seeks to a timestamp
- **THEN** playback continues from that timestamp without restarting the entry

#### Scenario: Volume change applies to the active source

- **WHEN** the user changes the volume while a local entry is playing
- **THEN** the new volume is applied to the local source only and the volume shown for remote entries is unchanged

### Requirement: Queue mutation

The player MUST allow removing an entry, clearing the queue and moving an entry to another position, preserving the current entry unless it is the one removed.

#### Scenario: Removing the current entry

- **WHEN** the user removes the entry that is currently playing and later entries exist
- **THEN** the next entry starts playing

#### Scenario: Clearing the queue stops playback

- **WHEN** the user clears the queue while a track is playing
- **THEN** playback stops, the queue becomes empty and the Now Playing panel shows no current track

### Requirement: Play history

The player MUST record each entry that starts playing with its source, title and a timestamp, MUST expose the history newest first, and MUST keep the 200 most recent entries.

#### Scenario: Playing tracks records history

- **WHEN** three entries play in sequence
- **THEN** the history lists those three entries with the most recent first

#### Scenario: History survives a restart

- **WHEN** the player is closed and started again
- **THEN** the history still contains the entries recorded before the restart

#### Scenario: History is bounded

- **WHEN** more than 200 entries have played
- **THEN** the history contains the 200 most recent entries and the oldest ones are gone

#### Scenario: Replaying a track adds a new entry

- **WHEN** the same entry starts playing twice
- **THEN** the history contains two entries for it

### Requirement: Unavailable entries are skipped

When an entry cannot be played, the player MUST mark it as failed in the queue and MUST continue with the following entry instead of stopping the queue.

#### Scenario: Failed resolution skips to the next entry

- **WHEN** the current youtube entry fails to resolve and a following entry exists
- **THEN** the failed entry is marked as failed in the queue and the following entry starts playing

#### Scenario: Last entry fails

- **WHEN** the last remaining entry fails to play
- **THEN** the queue keeps the failed entry marked and the player enters the paused state without exiting
