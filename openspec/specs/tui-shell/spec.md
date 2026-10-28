# tui-shell Specification

## Purpose
Defines what the user sees and how they drive the player: a panel layout, keyboard-first navigation with Vim defaults, discoverable help, search, and user-facing text in English and Spanish.

## Requirements

### Requirement: Three-panel layout

The interface MUST show an explorer panel, a queue panel and a Now Playing panel, and MUST keep the queue and Now Playing panels visible when the terminal is narrower than 80 columns or shorter than 24 rows.

#### Scenario: Default terminal size

- **WHEN** the terminal is at least 80 columns by 24 rows
- **THEN** the explorer, queue and Now Playing panels are all visible

#### Scenario: Small terminal

- **WHEN** the terminal is 60 columns by 18 rows
- **THEN** the queue and Now Playing panels remain visible and the explorer panel is hidden with its content still reachable by keyboard

### Requirement: Source tabs inside the explorer

The explorer panel MUST present the local, youtube and spotify sources as tabs, and switching tabs MUST NOT hide the queue or Now Playing panels.

#### Scenario: Switching to the youtube tab

- **WHEN** the local tab is active and the user selects the youtube tab
- **THEN** the explorer shows the youtube search view and the queue and Now Playing panels stay visible

#### Scenario: Disabled tab is still reachable

- **WHEN** the local source is unavailable
- **THEN** its tab remains selectable and shows the reason instead of a list

### Requirement: Vim-style key bindings

The player MUST provide `j` and `k` to move down and up, `h` and `l` to collapse and expand or switch panels, `Space` to play or pause, `n` for next, `p` for previous and `/` for search, and MUST let the user override these bindings in TOML.

#### Scenario: Moving in a list

- **WHEN** a list is focused and the user presses `j`
- **THEN** the selection moves one row down without scrolling the panel header out of view

#### Scenario: Overriding a binding

- **WHEN** the configuration binds next to `]`
- **THEN** `]` advances the queue and the default `n` no longer does

#### Scenario: Every action reachable without a mouse

- **WHEN** the user works with only a keyboard
- **THEN** every queue, library, import and playback action available in the interface can be triggered

### Requirement: Complete default keymap

Beyond Vim navigation, the default keymap MUST cover playing the selection, appending to the queue, removing an entry, cycling panels, switching source tabs, opening import, retrying a failed entry, changing volume, opening help and quitting.

#### Scenario: Queue editing from the keyboard

- **WHEN** an explorer item is selected and the user presses `a`
- **THEN** the item is appended to the queue without starting playback

#### Scenario: Removing an entry

- **WHEN** a queue entry is selected and the user presses `d`
- **THEN** that entry is removed from the queue

#### Scenario: Changing volume

- **WHEN** the user presses `+` or `-`
- **THEN** the volume of the active source changes by the configured step and no other source's volume changes

#### Scenario: Retrying a failed entry

- **WHEN** a failed entry is selected and the user presses `r`
- **THEN** the player retries that entry and clears its failed state on success

### Requirement: Discoverable help

The player MUST show a help overlay listing the currently active key bindings and MUST close it without changing playback state.

#### Scenario: Opening help during playback

- **WHEN** a track is playing and the user opens help
- **THEN** the overlay lists the active bindings and playback keeps running underneath

#### Scenario: Closing help

- **WHEN** the help overlay is open and the user dismisses it
- **THEN** the previous panel regains focus with its selection unchanged

### Requirement: Global search

The player MUST filter the focused list by a case-insensitive query entered after `/`, and MUST restore the unfiltered list when the query is cleared.

#### Scenario: Filtering a queue

- **WHEN** the queue panel is focused, the user presses `/` and types `cure`
- **THEN** only entries whose title or artist contains `cure` are shown

#### Scenario: Clearing the query

- **WHEN** the user clears the search query
- **THEN** the full list is shown again in its original order

### Requirement: English and Spanish interface

Every user-facing string MUST come from a single lookup table available in English and Spanish, a missing translation MUST fall back to English rather than showing a blank or a raw key, and a user-provided `i18n/<language>.toml` MUST override the built-in strings.

#### Scenario: Spanish interface

- **WHEN** the configured language is Spanish
- **THEN** panel titles, help entries and error messages appear in Spanish

#### Scenario: Missing translation

- **WHEN** a string has no Spanish entry
- **THEN** that string appears in English and the rest of the interface stays in Spanish

#### Scenario: User override file

- **WHEN** the config directory contains `i18n/es.toml` that replaces the queue panel title
- **THEN** the overridden text is shown and every other Spanish string is unchanged

#### Scenario: Invalid override file

- **WHEN** `i18n/es.toml` cannot be parsed
- **THEN** the built-in Spanish strings are used and a warning names the file

### Requirement: Interface stays responsive

The player MUST accept input and redraw while import, resolution or metadata requests are in progress.

#### Scenario: Typing during a slow import

- **WHEN** an import is running and the user navigates another panel
- **THEN** the selection keeps moving and the interface redraws without waiting for the import to finish
