# muddie manual

## Starting

```
muddie                               open the window and ask where to connect
muddie HOST PORT [--tls] [--insecure]   connect straight away
muddie --profile NAME                load a saved profile and connect
```

`--speech` turns speech on, `--script FILE` runs an extra script (it can be
repeated), and `--insecure` accepts a self-signed TLS certificate.

## Sessions and profiles

Each tab is a session with its own connection, scripts and map. The `+` on
the tab strip opens another; sessions in the background keep running and
mark their tab when something arrives.

The **Connection** window holds where to connect and how: host, port, TLS,
text encoding, and the character to log in as. Type a profile name and press
**Save profile** to keep it; saved profiles are chosen from the **Profile**
list. A profile also has its own scripts, triggers, map and remembered values,
all under `profiles/<name>/` in the configuration folder (`~/.config/muddie`
on Linux, `%APPDATA%\muddie` on Windows).

With a character name saved, muddie answers the first prompt with it. The
password is kept in the system's keyring, never in a file, and is sent when
the server hides typing.

The text encoding is `auto` for nearly every MUD: UTF-8, with Latin-1 for
anything that is not valid UTF-8. Set it only for servers that use something
else, such as `big5` or `koi8-r`.

## Typing

| Key | What it does |
|---|---|
| Enter | Sends the line. Several commands can be separated by `;` |
| Up, Down | Earlier and later lines; with something typed, only those starting with it |
| Ctrl+R | Searches the history for what is typed; again goes further back |
| Tab | Completes the word being typed from recent output |
| Ctrl+F | Searches the scrollback; Enter goes to the previous match, Shift+Enter to the next |

While scrolled back, the latest lines stay in view at the bottom. Web
addresses in the output can be clicked, as can links the server marks.

## Commands

Lines starting with `/` and one of these words are for muddie itself;
anything else is sent to the MUD.

| Command | What it does |
|---|---|
| `/lua <code>` | Runs a line of Luau |
| `/reload` | Starts the scripts and rules afresh |
| `/log` | Starts or stops logging the session to a file |
| `/where` | Says where the map has you, and the exits |
| `/find <text>` | Lists rooms whose name contains the text |
| `/go <number or name>` | Walks to a room |
| `/stop` | Stops walking |
| `/avoid [number]` | Keeps routes out of a room (the current one), or allows them again |
| `/lock <direction>` | Keeps routes off an exit of this room, or allows it again |
| `/tidy` | Draws the current area again from scratch |
| `/packages` | Lists installed script packages |
| `/install <git address> [name]` | Installs a package and enables it in this profile |
| `/update <name>` | Updates a package |
| `/enable <name>`, `/disable <name>` | Switches a package on or off in this profile |

## Triggers, aliases and keys without scripting

The **Triggers** button opens a window to make them. A trigger reacts to a
line of output: it can send commands, say something, play a sound, colour the
matched text or hide the line. An alias replaces a typed command; `%1` in what
it sends is the rest of what was typed. A key sends commands when pressed.
Function keys and combinations with Ctrl or Alt can be used, such as `f5` or
`ctrl+k`.

They are saved in the profile's `rules.toml`. For anything more, see
[scripting](scripting.md).

## The map

The map draws itself as you walk, on servers that report rooms through GMCP
or MSDP, and is saved per profile. Scripts can map other servers.

| Action | What it does |
|---|---|
| Click a room | Walks there, one step at a time, waiting for each to succeed |
| Right-click a room | Walk here, avoid in routes, tidy the area |
| Drag | Moves the view; the wheel zooms |
| Shift+drag a room | Moves it to another cell |
| Area, Down, Up | Looks at another area or level; **Back to me** returns |
| All areas | Draws every area on the level |

A room marked to avoid is never walked through, and a locked exit is never
used. Both are kept when the server reports the room again.

## Speech

With **Speech** on, muddie speaks each new line through the system's speech:
speech-dispatcher on Linux, the system voices on Windows and macOS.

| Key | What it does |
|---|---|
| Ctrl | Silences speech |
| Alt+Up, Alt+Down | Reads the previous and next line of output |
| Alt+Home, Alt+End | Reads the first and last line |
| Alt+1 ... Alt+9 | Reads the most recent lines |
| Alt+Shift with the same keys | The same for the chat panel |
| Alt+0 | Reads the gauges |

Sending a command silences what was still queued. The rate and the language
of the voice are in the settings; set the language (`es`, `en`...) to that of
the MUD, so its text is read with the right voice.

## Sound

Scripts and triggers can play sound files, and requests of the MUD Sound
Protocol are played. Sounds are looked for by name in the profile's `sounds`
folder and then in `sounds` in the configuration folder.

## Settings

**Settings**, at the right of the tab strip: theme (dark, light, high
contrast), language, text size and font, lines kept, timestamps, logging,
reconnection, keep-alive, and the rate and language of speech. They are saved in `settings.toml`.

## Protocols

Telnet with NAWS, TTYPE/MTTS, CHARSET and EOR/GA prompts; GMCP, MSDP and
MSSP; MCCP2 and MCCP3 compression; MXP formatting and links; MSP sounds; TLS.
