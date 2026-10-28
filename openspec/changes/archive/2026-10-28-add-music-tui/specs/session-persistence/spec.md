# Spec Delta

## Purpose

Keeps the session and its settings on disk so that the queue, imports and history come back after a restart, without contacting any remote service just to start up.

## ADDED Requirements

### Requirement: State lives in the user config directory

The player MUST store its configuration and state under the platform's user configuration directory, and MUST NOT write files outside that directory and the operating system temporary directory.

#### Scenario: First run creates the state directory

- **WHEN** the player starts and no configuration exists
- **THEN** it creates its directory under the user configuration directory with default configuration and an empty state

#### Scenario: No writes in the working directory

- **WHEN** the player runs from a project directory and the user changes settings
- **THEN** the project directory contains no files created by the player

### Requirement: Session state is restored at startup

The player MUST restore the queue with its position, the play history, imported spotify collections and user settings at startup, without making network requests.

#### Scenario: Queue restored after restart

- **WHEN** the player exits with a queue of five entries positioned at the third and starts again
- **THEN** the queue shows the same five entries with the third selected

#### Scenario: Startup does not resolve remote entries

- **WHEN** the restored queue contains youtube and spotify entries
- **THEN** the player restores them from stored metadata and resolves no stream until the user presses play

#### Scenario: Restored session starts paused

- **WHEN** the player restarts after playing a track
- **THEN** the current entry is selected and shown as paused, and no audio starts until the user plays

### Requirement: Writes are atomic

The player MUST write state by replacing the previous file with a complete new file, so that an interruption never leaves truncated configuration or state.

#### Scenario: Interrupted write

- **WHEN** the process is killed while saving state
- **THEN** the previous state file is still valid and the player starts with it on the next run

### Requirement: A corrupt state file does not prevent startup

When a state or configuration file cannot be parsed, the player MUST move it aside, start with defaults, and inform the user which file was discarded.

#### Scenario: Corrupt state

- **WHEN** the state file contains invalid JSON
- **THEN** the player renames it, starts with an empty queue and shows a message naming the discarded file

#### Scenario: Invalid configuration value

- **WHEN** the configuration sets the volume to an out-of-range value
- **THEN** the default volume is used, the rest of the configuration is applied and a warning is shown

### Requirement: Stored state declares its version

Every state file MUST declare a format version, and a file written by a newer unsupported version MUST be preserved while the player starts with defaults.

#### Scenario: Newer state version

- **WHEN** the state file declares a version the player does not support
- **THEN** the player leaves the file untouched, starts with defaults and shows a message explaining the version mismatch
