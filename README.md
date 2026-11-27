# Muddie

A modern, accessible MUD client written in Rust.

- **Made to be heard as well as seen.** Every line is read by the screen
  reader you use, and so reaches a braille display, or by the system's voice;
  there are keys to review earlier output, and the interface is exposed to
  screen readers. It works the same on Linux, Windows and macOS.
- **Scriptable in Luau**, in a sandbox: triggers, aliases, timers, key
  bindings, GMCP and MSDP handlers, and a map you can feed from any source.
  Scripts someone shares with you cannot touch your files.
- **No scripting needed** for the common things: triggers, aliases and keys
  can be made in a window.
- **An automapper** that draws as you walk, finds routes, and walks them one
  confirmed step at a time.
- **Sessions in tabs**, profiles with their own scripts and map, logging,
  reconnection, search, clickable links, themes, and English and Spanish.
- **Protocols:** GMCP, MSDP, MSSP, MCCP2, MCCP3, MXP, MSP, TLS.

Leer en [español](README.es.md).

## Installing

Each release has, on the
[releases page](https://github.com/midnattsol/muddie/releases):

- **Windows**: `muddie-windows-x86_64-setup.exe`, an installer that puts
  Muddie in the Start menu and in Settings to be removed.
- **macOS**: `muddie-macos-aarch64.dmg`; drag Muddie to Applications. It is
  not signed by a developer known to Apple, so the first time open it with
  Open in the menu of a right click.
- **Debian and Ubuntu**: `muddie_*.deb`, opened with the software installer
  or `sudo apt install ./muddie_*.deb`.
- **Any Linux, Windows or macOS**: an archive to unpack and run as it is.

## Building

```
cargo build --release -p muddie
```

The binary is `target/release/muddie`. Building needs CMake and a C++
compiler; on Linux also the development files of D-Bus and Speech Dispatcher
(`sudo apt install cmake libdbus-1-dev libspeechd-dev pkg-config` on Debian
and Ubuntu), without which the program is built mute. Sound uses `pw-play`,
`paplay`, `ffplay` or `aplay`, whichever is installed.

Besides `cargo test`, `dbus-run-session -- tests/orca/run.sh
target/release/muddie` checks Muddie with Orca in a desktop of its own that
nobody sees or hears (it needs Orca, Xvfb and xdotool); the CI runs it every
night.

## Installing on Linux from the source

```
./install.sh
```

It builds Muddie and installs it for your user in `~/.local`: the program,
its entry in the menu of applications, and its icon. `./uninstall.sh` takes
it away again and keeps your profiles and maps. To install it for everyone:
`PREFIX=/usr/local sudo -E ./install.sh --no-build` after building.

The icon is made from `assets/icon.png` by `assets/make-icons.sh`.

## Using

```
muddie mud.example.org 4000
```

See the [manual](docs/manual.md), [your first scripts](docs/first-scripts.md),
the [scripting reference](docs/scripting.md),
the guide to [making a plugin](docs/plugins.md) and the
[example scripts](examples/scripts).

## State of the project

Muddie is new. It is developed and tested on Linux; the Windows and macOS
builds compile but have had little use, and reports from screen reader users
are especially welcome.

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or
the [MIT licence](LICENSE-MIT), at your option.
