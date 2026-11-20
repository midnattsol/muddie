# Muddie

A modern, accessible MUD client written in Rust.

- **Made to be heard as well as seen.** Every line can be spoken through the
  system's speech, with keys to review earlier output, and the interface is
  exposed to screen readers.
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

## Building

```
cargo build --release -p muddie
```

The binary is `target/release/muddie`. On Linux, speech needs
speech-dispatcher running, and sound uses `pw-play`, `paplay`, `ffplay` or
`aplay`, whichever is installed; nothing extra is needed to build.

## Installing on Linux

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

See the [manual](docs/manual.md), the [scripting reference](docs/scripting.md)
and the [example scripts](examples/scripts).

## State of the project

Muddie is new. It is developed and tested on Linux; the Windows and macOS
builds compile but have had little use, and reports from screen reader users
are especially welcome.

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or
the [MIT licence](LICENSE-MIT), at your option.
