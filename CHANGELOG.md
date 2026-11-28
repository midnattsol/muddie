# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.4](https://github.com/midnattsol/muddie/compare/v0.7.3...v0.7.4) - 2026-11-28

### Added

- *(core)* replace a part of a line
- *(plugins)* fetch a plugin's sounds from the web
- *(scripts)* offer actions in the palette alone
- *(sound)* mix sounds in muddie, on every system
- *(map)* walk the map with the keys that walk
- *(scripts)* lists, clipboard, rooms and quiet
- *(keys)* choose every key, from scripts too
- *(scripts)* say what a call to nothing called
- *(scripts)* find a module beside who asks
- *(scripts)* write definitions for editors
- *(scripts)* add what scripts of others can do
- *(scripts)* wait in the middle of a script
- *(scripts)* [**breaking**] set what is set as a field
- *(scripts)* listen to channels and what is sent
- *(scripts)* ask for gauges and listen to them
- *(scripts)* [**breaking**] ask a line for its text by name
- *(scripts)* [**breaking**] say how things are heard one way
- *(scripts)* [**breaking**] give triggers the line to change
- *(scripts)* [**breaking**] say what is not understood
- *(scripts)* [**breaking**] a table for each kind of thing
- *(scripts)* [**breaking**] listen to everything with on
- *(scripts)* [**breaking**] give what is made as an object
- *(gauges)* say how each is read aloud
- *(scripts)* say quietly on a channel
- *(scripts)* tell scripts whether a screen reader reads

### Documentation

- guide plugins and order the reference

### Fixed

- *(editor)* never save an unopened editor's rules
- *(core)* stop compressing when the server asks
- *(core)* let a session stop while it connects
- *(core)* read latin-1, long lines and cut codes
- *(core)* stop servers crashing msdp and mxp
- *(map)* keep rooms a busy file did not take
- *(rules)* tell a rule from a folder named alike
- *(script)* runaways, reruns and keys given wrong
- *(script)* keep globals and timers current
- *(gauges)* take how a session starts as no news
- *(scripts)* read a variable as it is now
- *(scripts)* let go of a session once closed
- *(sound)* keep the mix soon and a stuck player
- *(sound)* no number volumes and fade to the end
- *(sound)* bound loops and wildcard names
- *(sound)* build without warnings off linux
- *(speech)* read a silenced prompt no more

## [0.7.3](https://github.com/midnattsol/muddie/compare/v0.7.2...v0.7.3) - 2026-11-28

### Fixed

- *(scripts)* take fed text right after its line
- *(a11y)* keep up with a fight that goes on
- *(rules)* follow a gauge's maximum as its value
- *(rules)* tell a rule from a folder named alike
- *(core)* stop compressing when the server asks

## [0.7.2](https://github.com/midnattsol/muddie/compare/v0.7.1...v0.7.2) - 2026-11-28

### Fixed

- *(a11y)* f6 in the editor and the welcome, host
- *(files)* refuse names windows keeps for devices
- *(editor)* have back a deleted script byte by byte
- *(login)* send the name to a prompt with GA
- *(sound)* keep the mix soon and a stuck player

## [0.7.1](https://github.com/midnattsol/muddie/compare/v0.7.0...v0.7.1) - 2026-11-28

### Fixed

- *(a11y)* new profile, escape and who disconnects
- *(sessions)* keep what was typed, say what closed
- *(input)* complete a word after a no-break space
- *(plugins)* remove only what a plugin installed
- *(core)* let a session stop while it connects

## [0.7.0](https://github.com/midnattsol/muddie/compare/v0.6.0...v0.7.0) - 2026-11-28

### Added

- *(keys)* go through sessions with ctrl+tab

### Fixed

- *(palette)* keep a script's choice on no match
- *(plugins)* safe folders, sound names and index
- *(script)* runaways, reruns and keys given wrong
- *(keys)* repeat keypad keys held down
- *(voice)* read tells holding a short line read
- *(a11y)* tell tabs selected and bars how full
- *(a11y)* read headers in order, keep the focus
- *(settings)* never overwrite unreadable settings
- *(gauges)* put them in rows that do not jump
- *(core)* read latin-1, long lines and cut codes
- *(core)* stop servers crashing msdp and mxp
- *(map)* keep rooms a busy file did not take
- *(script)* keep globals and timers current
- *(sound)* no number volumes and fade to the end
- *(sound)* bound loops and wildcard names

## [0.6.0](https://github.com/midnattsol/muddie/compare/v0.5.0...v0.6.0) - 2026-11-28

### Added

- *(plugins)* fetch a plugin's sounds from the web
- *(scripts)* offer actions in the palette alone
- *(sound)* mix sounds in muddie, on every system
- *(map)* walk the map with the keys that walk
- *(scripts)* lists, clipboard, rooms and quiet
- *(keys)* choose every key, from scripts too

### Documentation

- write the guides with care
- guide plugins and order the reference

### Fixed

- *(sound)* build without warnings off linux

## [0.5.0](https://github.com/midnattsol/muddie/compare/v0.4.0...v0.5.0) - 2026-11-26

### Added

- *(map)* say where you are from a room's menu
- *(debug)* tell prompts from lines
- *(scripts)* find a module beside who asks
- *(scripts)* write definitions for editors
- *(scripts)* add what scripts of others can do
- *(scripts)* [**breaking**] set what is set as a field
- *(gauges)* leave panels out of screen readers
- *(scripts)* listen to channels and what is sent
- *(scripts)* ask for gauges and listen to them
- *(scripts)* [**breaking**] say how things are heard one way
- *(scripts)* [**breaking**] give triggers the line to change
- *(scripts)* [**breaking**] say what is not understood
- *(scripts)* [**breaking**] a table for each kind of thing
- *(scripts)* [**breaking**] listen to everything with on
- *(gauges)* say how each is read aloud
- *(scripts)* say quietly on a channel
- *(core)* replace a part of a line
- *(scripts)* say what a call to nothing called
- *(scripts)* wait in the middle of a script
- *(scripts)* [**breaking**] ask a line for its text by name
- *(scripts)* [**breaking**] give what is made as an object

### Fixed

- *(plugins)* clear what an update does not have
- *(keyboard)* go back from the command line
- *(gauges)* wrap where they do not fit
- *(gauges)* take how a session starts as no news
- *(gauges)* do not say one switched off
- *(a11y)* read effects by name with the rest
- *(channels)* read a message once
- *(a11y)* tell each gauge once
- *(scripts)* speak only while the game is read
- *(speech)* read a silenced prompt no more
- *(scripts)* read a variable as it is now
- *(scripts)* let go of a session once closed

## [0.4.0](https://github.com/midnattsol/muddie/compare/v0.3.0...v0.4.0) - 2026-11-25

### Added

- *(plugins)* download official plugins

### Fixed

- *(mapview)* draw rooms always in the same order
- *(mapview)* colour each way of an exit by itself
- *(a11y)* keep noise from screen readers
- *(scripts)* let go of a session once closed

## [0.3.0](https://github.com/midnattsol/muddie/compare/v0.2.0...v0.3.0) - 2026-11-24

### Added

- *(scripts)* tell scripts whether a screen reader reads
- *(a11y)* say first steps on welcome, keys with F1
- *(a11y)* fewer stops in panels, F6 between places
- *(plugins)* bring included plugins up to date
- *(a11y)* read the output as a text, reached with tab

### Fixed

- *(a11y)* tab goes between line and output
- *(plugins)* on before connecting, clear when switched
- *(a11y)* reach Create, read messages by keyboard

## [0.2.0](https://github.com/midnattsol/muddie/compare/v0.1.0...v0.2.0) - 2026-11-23

### Added

- *(keys)* rebind shortcuts, warn on key clashes
- *(a11y)* move among tabs with the arrow keys

### Fixed

- *(keys)* match a chosen Cmd shortcut on macOS
- *(a11y)* ctrl silences every session's voice
