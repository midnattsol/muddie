# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
