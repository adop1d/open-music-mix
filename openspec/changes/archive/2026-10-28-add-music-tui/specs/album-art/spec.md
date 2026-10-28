# Spec Delta

## Purpose

Shows the real cover of the current track inside the terminal, using whichever graphics protocol the terminal supports and degrading to a block rendering when none is available.

## ADDED Requirements

### Requirement: Graphics protocol detection

At startup the player MUST detect whether the terminal supports Kitty, Sixel or iTerm2 graphics and MUST use the detected protocol for album art, falling back to half-blocks when none is detected.

#### Scenario: Kitty terminal

- **WHEN** the player starts in a terminal that advertises the Kitty graphics protocol
- **THEN** album art is drawn using that protocol

#### Scenario: Unsupported terminal

- **WHEN** the player starts in a terminal that supports none of the graphics protocols
- **THEN** album art is drawn with half-blocks and no protocol escape sequences are emitted

### Requirement: Art loads without blocking

Album art MUST be fetched and decoded asynchronously, and the panel MUST show a placeholder until the art is ready.

#### Scenario: Art available

- **WHEN** a track starts and its cover is available locally
- **THEN** the placeholder is replaced by the cover without delaying input handling

#### Scenario: Slow art fetch

- **WHEN** the cover takes longer than the track's metadata to arrive
- **THEN** metadata and progress are shown immediately and the cover appears when ready

### Requirement: Art sources by source type

For local tracks the player MUST use the picture served by the MPD daemon, or a cover image found in the album folder, and for youtube and spotify tracks the thumbnail reported by the resolver. The player MUST NOT read audio tags itself.

#### Scenario: Local art from the daemon

- **WHEN** a local track plays and the MPD daemon serves a picture for it
- **THEN** that picture is displayed

#### Scenario: Local art from the folder

- **WHEN** the daemon serves no picture and the album folder contains a cover image
- **THEN** that image is displayed

#### Scenario: Remote art from the resolver

- **WHEN** a youtube or spotify entry plays and the resolver reports a thumbnail
- **THEN** the thumbnail is fetched to the temporary directory and displayed

### Requirement: Missing art shows a placeholder

When a track has no cover, or the cover cannot be decoded, the player MUST show a placeholder identifying the track and MUST NOT show an error state.

#### Scenario: Track without embedded art

- **WHEN** a track has no cover in its metadata or its folder
- **THEN** the Now Playing panel shows the placeholder with the track title

#### Scenario: Corrupt image

- **WHEN** the cover file exists but cannot be decoded
- **THEN** the placeholder is shown and playback continues

### Requirement: Art respects the panel area

Rendered art MUST fit inside the Now Playing panel, MUST be redrawn after a terminal resize, and MUST NOT write escape sequences outside that area.

#### Scenario: Resizing the terminal

- **WHEN** the terminal is resized while art is displayed
- **THEN** the art is redrawn to fit the new panel size without leaving fragments

#### Scenario: Narrow panel

- **WHEN** the Now Playing panel is narrower than the minimum width for the detected protocol
- **THEN** the player switches to half-block rendering for that panel
