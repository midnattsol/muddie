# Muddie manual

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

With a character name saved, Muddie answers the first prompt with it. The
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

Lines starting with `/` and one of these words are for Muddie itself;
anything else is sent to the MUD.

**Ctrl+K** opens them in a palette over the window, with the game in view
behind it: type to find one, Up and Down choose, which is also read aloud,
and Enter runs it. One that needs something more, such as the room to walk
to, is completed instead, to type that after it; Tab completes without
running. Escape, or a click anywhere else, closes it. Typed in the command
line after the `/`, a command is carried out the same.

What a command answers is not put among the lines of the game. A line of it
is said inside the command line for a moment; if something could not be
done, it stays there until you type. A list, of the rooms found, the groups
or the plugins, is shown in the palette, to choose from: Enter on a room
walks to it, and on a group or a plugin switches it.

| Command | What it does |
|---|---|
| `/help` | Opens the palette of these commands |
| `/lua <code>` | Runs a line of Luau |
| `/reload` | Runs the scripts and rules again |
| `/log` | Starts or stops logging the session to a file |
| `/group`, `/group <name> [on\|off]` | Lists the groups of rules; switches one on or off for this session |
| `/map where` | Says where the map has you, and the exits |
| `/map find <text>` | Lists rooms whose name contains the text |
| `/map walk <number or name>` | Walks to a room |
| `/map stop` | Stops walking |
| `/map avoid [number]` | Keeps routes out of a room (the current one), or allows them again |
| `/map lock <direction>` | Keeps routes off an exit of this room, or allows it again |
| `/map tidy` | Draws the current area again from scratch |
| `/map show`, `/map hide` | Shows or hides the map panel |
| `/plugin list` | Lists the installed plugins |
| `/plugin install <git address> [name]` | Installs a plugin and enables it in this profile |
| `/plugin update <name>` | Brings a plugin up to date |
| `/plugin enable <name>`, `/plugin disable <name>` | Switches a plugin on or off in this profile |
| `/plugin remove <name>` | Deletes a plugin |

These are in English whatever the language of the interface, and Tab
completes them.

## The editor: triggers, aliases, macros, timers and scripts

**Editor**, at the right of the top bar, takes the place of the game
while you work on it; **Game**, Escape or Ctrl+E goes back, and the session
keeps running meanwhile. The sections are on the left, then the list of the
current one and the selected item's details; drag the gap between those two
to trade their widths. Changes are saved by themselves.

A trigger, an alias, a macro and a timer differ in what sets them off; what
they then do is the same choice for all four:

- **Commands**: what to send, one command per line.
- **Lua**: a few lines of Luau instead, in an editor that numbers the
  lines, colours the code and marks the line of a mistake with what is wrong.

The three, and what else there is:

- A **trigger** goes off when a line of output matches. Besides its action
  it can say something, play a sound, colour the matched text or hide the
  line, or show it in a panel. `%1`, `%2`… in its commands are the groups of
  its pattern; a script has them in `m`.
- An **alias** goes off when you type a command; `%1` is the rest of what
  was typed.
- A **macro** goes off when a key is pressed. Press **Record a key** and then
  the key: function keys, combinations with Ctrl or Alt, and the keys of the
  numeric keypad can be used. The keypad's digits are one key with Num Lock
  on and another with it off, so it can walk with Num Lock off and type
  numbers with it on; record the key in the state you will use it in.
- A **timer** goes off after a while: every so many seconds, or once.
- **Scripts** are Luau code that is always loaded, for what no rule can do:
  building the map, acting on what the MUD reports. They are the profile's
  `.luau` files. The `+` offers examples to start from: a map the MUD draws
  itself, a map it sends whole, a map read from the room descriptions, words
  to complete given by the MUD, and gauges.

Rules can be put in **groups**, to switch a whole way of playing on and off
at once: a group holds triggers, aliases, macros and timers alike. Give a
rule its group in its details; the groups are listed under the sections, each
with the switch of how it starts a session, and choosing one narrows the lists
to it. `/group <name> on` and `/group <name> off` switch one for the session
in course, as a script does with `group.enable(name)` and its like, without
changing what is saved. The same key can be bound once per group, so that it
does one thing in one mode and another in another.

**Variables** are values with a name that are kept: who you are hunting, how
many you have killed. They are listed and changed in the editor, each with
its type, by the name a script knows it by: `string`, `number`, `boolean`,
`array` or `table`. Each type is edited as what it is: a switch for a
boolean, a row per item of an array, a key and a value per field of a table.
In the commands of a rule or a macro, `$name` is replaced by the value, and
`$name.key` or `$name.1` by what is inside a table or an array; a script
reads and changes it as `vars.name`. They belong to the session: with a saved
profile they are kept with it, and without one they last until the session is
closed, going into the profile if it is then saved.

A mistake in one rule or script is reported with its name and does not stop
the others. Triggers and aliases have a box to try them out: paste a line
from the MUD, or type a command, and it says whether it matches and what it
would do.

They are saved in the profile's `rules.toml`. For anything more, see
[scripting](scripting.md).

## Panels

Around the output there can be panels: the map, the gauges, and panels of
text. A profile starts with one for the MUD's channels above, the map at the
right and the gauges below. **Preferences** lists them, each to show or hide.

**Preferences > Edit the layout…** is where they are made and placed, on the game
itself:

- **Add a panel** makes one: of text, the map, or gauges. There is one map;
  of the others, as many as wanted.
- Drag a panel by its handle (the six dots) to another side of the window, or
  press one of the four small pictures beside its name. Drag its edge to size
  it.
- Several panels can share a side: those beside the output are stacked, those
  above or below it stand in a row. Drop a panel on the upper or lower half of
  another (the left or right half, above and below the output) to put it
  before or after it, and drag the edge between two to give one more room.
  The dots beside the name move it too, for those who would rather not drag.
- Panels can share one place as **tabs**, with their names along its top to
  choose between them: drop a panel on the middle of another, or use the
  dots. A tab that gets something while another is on show has a dot.
- **Tall sides** decides who gets the corners: with it, the panels beside
  the output reach the top and the bottom of the window, and those above
  and below fit between them.
- Its name can be changed there, and the cross removes it.
- **Done**, or Escape, leaves the layout mode. The layout is kept with the
  profile.

What a panel of text shows comes from elsewhere. A trigger can show the lines
it matches in a panel, and hide them from the output so that they are only
there: that is how tells, or combat, get a panel of their own. A script can
write to one with `panel_add(name, text)`, or put a whole text in it with
`panel(name, text)`. A panel can also be told to show the MUD's channels,
from the dots beside its name in the layout mode.

The **gauges** are made and changed in the editor, under **Gauges**; the
dots at the corner of a panel of gauges and **Preferences > Gauges…** lead
there. Each gauge says which panel it is shown in, so that yours can be below
and an enemy's beside the map, and they are shown in the order of the
editor's list: drag one there, or press Alt+Up or Alt+Down, to move it. A gauge
follows something the MUD reports, and is one of these kinds:

- **Bar** and **Cells**: how full something is, of its maximum.
- **Number**: a number as it is, such as gold or level.
- **Light**: on or off, for what is so or is not: hidden, poisoned.
- **Text**: what the MUD says, as words: a stance, a target.

Say where the value is, and the maximum for those that have one: a GMCP
package and the fields inside it (`Char.Vitals.health.current`), or the name
of a MSDP variable; **Debug** shows what there is. Choose its colour, and for
those with a maximum the colours it takes as it gets low: yellow under 60 %,
red under 30 %, as many levels as wanted, kept in order. A light can have a
colour for when it is off as well, to say which of two things is so: red for
poisoned, green for not. Alt+0 reads the gauges aloud.

## Debugging: what the MUD sends

**Debug**, beside Game and Editor at the right of the top bar (Ctrl+D),
shows what the MUD sends besides text, with GMCP or MSDP, as it comes: each
package or variable with its values one by one, the message as it came, and
those before it. It is where to look before making a gauge or writing a
script. **Copy the path** takes what leads to a value, such as
`Char.Vitals.health.current`, and **Make a gauge** makes one that follows a
number, with the maximum that goes with it if there is one beside it.

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
| The buttons beside the map's title | Find where you are, tidy the area, stop walking; the dots hold the rest: show every area, avoid this room, hide the map |

A room marked to avoid is never walked through, and a locked exit is never
used. Both are kept when the server reports the room again.

## Speech

With **Speech** on, Muddie speaks each new line through the system's speech:
speech-dispatcher on Linux, the system voices on Windows and macOS.

| Key | What it does |
|---|---|
| Ctrl | Silences speech |
| Alt+Up, Alt+Down | Reads the previous and next line of output |
| Alt+Home, Alt+End | Reads the first and last line |
| Alt+1 ... Alt+9 | Reads the most recent lines |
| Alt+Shift with the same keys | The same for the chat panel |
| Alt+0 | Reads the gauges |

With **Voice through the screen reader** set in the settings, Muddie hands
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

**Preferences > Settings…**, in sections:

- **General**: the language, and the size the interface is drawn at.
- **Appearance**: the theme (Dark, Graphite, Mocha, Dusk, Tokyo Night, Nord,
  Light, High contrast), and for the text of the MUD its colours, its font
  (MesloLGS NF unless another is chosen: JetBrains Mono, which also comes
  with the program, or any monospaced font installed), its size and its line
  spacing, with a sample.
- **Output**: lines kept, timestamps.
- **Sessions**: whether a sent command stays in the command line, logging,
  reconnection, keep-alive.
- **Speech**: rate, language, screen reader.
- **Plugins**: sets of scripts made by others. Install one from the address
  of its git repository; each has a switch that enables it in the profile of
  the session, and can be updated or removed. Only install what you trust: a
  plugin can send commands as you.

They are saved in `settings.toml`.

## Languages

The interface follows the language of the system; **Settings** can fix it.
English and Spanish come with the program. Translations are gettext
catalogues: to add a language, copy `po/muddie.pot` from the sources to
`<language code>.po` (`fr.po`, `pt_BR.po`), translate it with any gettext
editor such as Poedit, and put it in the `locale` folder of the configuration
(`~/.config/muddie/locale`). It then appears among the languages. A file there
also replaces a built-in translation of the same code.
