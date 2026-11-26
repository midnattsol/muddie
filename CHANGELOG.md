# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
