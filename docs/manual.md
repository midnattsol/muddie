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

Each session has its own connection, scripts and map. The sessions are the
tabs along the bottom of the window, each with a dot that is green while it is
connected. The `+` beside them, or **Session > New session**, opens another;
sessions in the background keep running and mark their tab when something
arrives.

A new session opens on a welcome screen. Type an address such as
`mud.example.org:4000` and press Enter to connect, or pick one of your saved
profiles below it. **More options…** there,
or **Session > Connection…**, opens the window with everything about a
connection: host, port, TLS, text encoding, and the character to log in as.
Type a profile name and press **Save profile** to keep it. A profile also has its own scripts, triggers, map and remembered values,
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
| Enter | Sends the line. Several commands can be separated by `;`. The line stays, selected: Enter sends it again, and typing replaces it (this can be switched off in the settings) |
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

**Automation**, at the right of the top bar, takes the place of the game
while you work on it; **Game**, Escape or Ctrl+E goes back, and the session
keeps running meanwhile. The sections are on the left, then the list of the
current one and the selected item's details; drag the gap between those two
to trade their widths. Changes are saved by themselves.

A trigger, an alias, a macro and a timer differ in what sets them off; what
they then do is the same choice for all four:

- **Commands**: what to send, one command per line.
- **Lua script**: a few lines of Luau instead, in an editor that numbers the
  lines, colours the code and marks the line of a mistake with what is wrong.

The three, and what else there is:

- A **trigger** goes off when a line of output matches. Besides its action
  it can say something, play a sound, colour the matched text or hide the
  line. `%1`, `%2`… in its commands are the groups of its pattern; a script
  has them in `m`.
- An **alias** goes off when you type a command; `%1` is the rest of what
  was typed.
- A **macro** goes off when a key is pressed. Press **Record a key** and then
  the key: function keys, combinations with Ctrl or Alt, and the keys of the
  numeric keypad can be used.
- A **timer** goes off after a while: every so many seconds, or once.
- A **gauge** is a bar that follows a number the MUD reports, such as your
  health. Say where the value and its maximum are: a GMCP package and the
  fields inside it (`Char.Vitals.health.current`), or the name of a MSDP
  variable. Choose its colour, whether it is drawn as a row of cells, and the
  level under which its numbers turn red. Alt+0 reads the gauges aloud.
- **Modules** are Luau code that is always loaded, for what no rule can do:
  building the map, acting on what the MUD reports. They are the profile's
  `.luau` files. The `+` offers examples to start from: a map the MUD draws
  itself, a map it sends whole, a map read from the room descriptions, words
  to complete given by the MUD, and gauges.

A mistake in one rule or module is reported with its name and does not stop
the others. Triggers and aliases have a box to try them out: paste a line
from the MUD, or type a command, and it says whether it matches and what it
would do.

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
| The area list, − and + | Looks at another area or level; **Back to me** returns |
| The buttons beside the map's title | Find where you are, tidy the area, show every area on the level, stop walking |

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

With **Voice through the screen reader** set in the settings, muddie hands
each line to your screen reader to announce instead of speaking itself. That
is the way to read the MUD on a braille display, and it uses the screen
reader's own voice and settings.

Sending a command silences what was still queued. The rate and the language
of the voice are in the settings; set the language (`es`, `en`...) to that of
the MUD, so its text is read with the right voice.

## Sound

Scripts and triggers can play sound files, and requests of the MUD Sound
Protocol are played. Sounds are looked for by name in the profile's `sounds`
folder and then in `sounds` in the configuration folder.

## Settings

**View > Settings…**, in sections: appearance (theme, language, text size and
font; the themes are Dark, Graphite, Mocha, Dusk, Tokyo Night, Nord, Light and
High contrast), output (lines kept, timestamps), sessions (logging, reconnection,
keep-alive) and speech (rate, language, screen reader). They are saved in
`settings.toml`, which also remembers whether the chat and map panels are
shown.

## Protocols

Telnet with NAWS, TTYPE/MTTS, CHARSET and EOR/GA prompts; GMCP, MSDP and
MSSP; MCCP2 and MCCP3 compression; MXP formatting and links; MSP sounds; TLS.
