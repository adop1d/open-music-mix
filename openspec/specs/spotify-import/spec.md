# spotify-import Specification

## Purpose
Turns a Spotify playlist, album or track reference into a saved collection of track metadata that the player can queue and match to playable sources, without downloading Spotify audio.

## Requirements

### Requirement: Import from a Spotify reference

The player MUST accept a public Spotify URL or a bare playlist, album or track ID as import input, and MUST reject other input with a message naming the accepted formats.

#### Scenario: Importing a playlist URL

- **WHEN** the user submits `https://open.spotify.com/playlist/<id>`
- **THEN** the player imports that playlist as a spotify collection named after the playlist

#### Scenario: Importing a single track

- **WHEN** the user submits a Spotify track URL or ID
- **THEN** the player saves a collection containing that one track, named after the track

#### Scenario: Rejecting invalid input

- **WHEN** the user submits text that is not a Spotify URL or ID
- **THEN** the player shows a message naming the accepted formats and creates no collection

### Requirement: Import is reachable from the interface and the command line

The player MUST start an import from a modal bound to a key inside the interface and from a command-line argument, and both paths MUST produce the same result.

#### Scenario: Importing from the modal

- **WHEN** the user opens the import modal, pastes a playlist URL and confirms
- **THEN** the import starts and its progress is visible without leaving the interface

#### Scenario: Importing from the command line

- **WHEN** the player is started with `--import <playlist-url>`
- **THEN** that playlist is imported before the interface becomes interactive and appears in the explorer

### Requirement: Private collections are rejected

When a Spotify reference cannot be read without account authentication, the player MUST reject it with a message stating that only public collections are supported, and MUST NOT ask for credentials.

#### Scenario: Account-only playlist

- **WHEN** the user submits a playlist that is not publicly readable
- **THEN** the player reports that only public collections are supported and creates no collection

### Requirement: Import resolves metadata only

The import MUST obtain title, artists, album, duration and, when available, ISRC for each track, and MUST NOT download Spotify audio or bypass any protection mechanism.

#### Scenario: Imported tracks carry metadata

- **WHEN** a playlist with ten tracks is imported
- **THEN** each imported track has a title and at least one artist

#### Scenario: No Spotify audio is stored

- **WHEN** the import completes
- **THEN** no audio file from Spotify exists in the config directory or the temporary directory

### Requirement: No Spotify account required

The player MUST perform imports without asking the user for Spotify API credentials, an OAuth2 login or a running Spotify desktop client.

#### Scenario: First import on a clean machine

- **WHEN** the user imports a playlist with no Spotify credentials configured and no desktop client running
- **THEN** the import proceeds without prompting for credentials or login

### Requirement: Tracks carry their youtube match from import time

The import MUST store the youtube video matched to each track, and playback MUST resolve only that track's stream, so starting an imported entry performs no search.

#### Scenario: Playing an imported track

- **WHEN** the user plays an imported spotify track
- **THEN** the player resolves the stored youtube video, plays it, and the Now Playing panel labels it spotify

#### Scenario: No search when an imported entry starts

- **WHEN** an imported spotify entry starts playing
- **THEN** the player resolves a stream for the stored video and issues no search query

### Requirement: Unmatched tracks do not fail the import

When a track cannot be matched to a playable source, the import MUST keep it in the collection marked unavailable and MUST report how many tracks were matched and how many were not.

#### Scenario: Partially matched playlist

- **WHEN** eight of ten imported tracks match and two do not
- **THEN** the collection is saved with all ten entries, the two are marked unavailable, and a summary reports eight matched and two unmatched

### Requirement: Re-import updates the existing collection

Importing the same Spotify collection again MUST update that collection in place instead of creating a duplicate.

#### Scenario: Importing the same playlist twice

- **WHEN** the user imports a playlist that was imported earlier
- **THEN** the collection count stays the same and the entries reflect the playlist's current contents

### Requirement: Import results are available offline

A saved spotify collection MUST remain browsable after a restart without contacting Spotify.

#### Scenario: Browsing imports without network

- **WHEN** the player starts with no network access and the user opens an imported playlist
- **THEN** the playlist and its track metadata are listed from local storage
