# Muddie manual

## Starting

```
muddie                               open the window and ask where to connect
muddie HOST PORT [--tls] [--insecure]   connect straight away
muddie --profile NAME                load a saved profile and connect
```

`--speech` speaks with the system's voice, whatever the settings say,
`--script FILE` runs an extra script (it can be
repeated), and `--insecure` accepts a self-signed TLS certificate.

`--debug` shows on the standard output, as it happens, everything that
happens in the run, one line each: what goes over the protocols, under its
name and way (`GMCP<` for what arrived by GMCP, `GMCP>` for what was sent,
and so `MSDP`, `MSSP`, `MXP` and `TELNET` for the options offered and
answered), the text of the game, what was sent, what the client said, the
scripts' errors and what they note with `log()`, and where the map has the
player. To keep it as well, `muddie --debug | tee muddie.log`. Passwords
are never shown: neither what is typed while the server hides it or right
after it asks for one, nor what the profile logs in with, nor a password
sent by GMCP. What is said on the channels is, so mind whom you give it to.

## The window

At the top, at the left the views of a session, **Game**, **Editor**
(Ctrl+E) and **Layout**, where the panels are arranged; in the middle the field that opens the **palette**; at the right
the **menu** (the three lines): a new session, the settings, the profiles,
the keyboard shortcuts and what Muddie is. At the bottom, the sessions as tabs, what the
one on screen is doing, and the **bell** of the messages the client has given
in it, marked while some are new: all it said can be read there again. What
the client says (connected, the map taken, a plugin installed...) is shown
for a few seconds over the bottom right corner, where it moves nothing, and
read aloud; it is never put among the lines of the game, which are the
game's. Over the corner too is what waits for an answer, such as a plugin
offered for the MUD.

A session's tab says its name and, with a dot, how its connection is doing.
A right click on it opens its menu: connect or reconnect, disconnect, edit
its profile (or save it as one, if it has none), and close it.

The palette, **Ctrl+K**, reaches everything: type a few letters of what you
want, such as `timers`, `map`, `theme` or the name of a profile, and press
Enter. It goes to the views and the sections of the editor, opens the
settings at the part wanted, shows and hides panels, adds them, connects to
a profile, and runs the commands of the client. Everything the menu does is
in it. It says how many it found; **↑** and **↓** go through them one by
one, round the ends, **Page Up** and **Page Down** a page at a time, and
**Ctrl+Home** and **Ctrl+End** to the first and the last, each said aloud
with where it is; the wheel scrolls it too. Wherever there is more to
scroll to, a faint bar says so.

## Sessions and profiles

Each session has its own connection, scripts and map. The sessions are the
tabs along the bottom of the window, each with a dot that is green while it is
connected. The `+` beside them, or **New session** in the menu, opens another;
sessions in the background keep running and mark their tab when something
arrives.

A new session opens on a welcome screen. Type an address such as
`mud.example.org:4000` and press Enter to connect, or pick one of your saved
profiles below it. A right click on one of them, or on it in the Profiles
window, edits, duplicates, renames or deletes it. **More options** there
keeps the address typed as a new profile, asking only its name, and opens it
to be filled in.

Profiles have their own window, **Profiles**, in the menu: all of them in a
list at the left, with their colour, where they lead and a dot on those a
session plays with, and the one chosen at the right. Its page has its name,
**Connect**, its menu (duplicate, rename, delete) and three tabs:

- **Connection**: the **Server** (host, port, encryption: none, TLS, or TLS
  without verifying the certificate; and the encoding) and the **Login**
  (the character, and its password, kept in the system keyring).
- **Session**: **Auto-reconnect** when the connection drops; **Keep-alive**,
  off or every so often, so that an idle connection is not closed; the
  **Session log**, off, as text or as HTML; and **Download sounds** the MUD
  asks for.
- **Plugins**: those **Included** with Muddie, and those **Installed**, with
  **Install from git** and the address of their git repository. Each has a switch
  that enables it in that profile; its menu has the rest, and **Update all**
  updates them all. Only install what you trust: a plugin can send commands
  as you.

What is changed there is kept as it is changed, with no button to press, and
a session playing with that profile takes it. While the session on screen is
connected otherwise than its profile now says, the profile is marked
**Changes pending** and its **Connect** button is **Reconnect**. **New
profile**, under the list, makes an empty one. A profile also has its own scripts, triggers, map and remembered values,
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
| Enter | Sends the line. Several commands can be separated by `;`. With **Keep the command after sending it**, in the settings, the line stays, selected: Enter sends it again, and typing replaces it |
| Up, Down | Earlier and later lines; with something typed, only those starting with it |
| Ctrl+R | Searches the history for what is typed; again goes further back |
| Tab | Completes the word being typed from recent output |
| Ctrl+F | Searches the scrollback; Enter goes to the previous match, Shift+Enter to the next |

Text is chosen in the output by dragging over it, and Ctrl+C copies it; the
keyboard stays in the command line meanwhile, and the output holds still
while text is being chosen. A right click on the output copies what is
chosen, the line under the pointer or all of it, or searches it; the
palette (Ctrl+K) has **Copy all output** too.

While scrolled back, the latest lines stay in view at the bottom. Web
addresses in the output can be clicked, as can links the server marks.

## Commands

Lines starting with `/` and one of these words are for Muddie itself;
anything else is sent to the MUD.

**Ctrl+K** opens them in a palette over the window, with the game in view
behind it, together with the places to go, the panels, the settings and
the rest, in groups under their names. Type to find one; Up and Down
choose, which is also read aloud with where it is in the list, and Enter
runs it. One that needs something more, such as the room to walk to, is
completed instead, to type that after it; Tab completes without running.
Escape, or a click anywhere else, closes it. Its foot says these keys.
Typed in the command line after the `/`, a command is carried out the same.

What a command answers is shown where it was asked: run from the palette,
in the palette, which stays open to read it; typed in the command line,
over the corner, as the client says anything. A list, of the rooms found,
the tags or the plugins, is shown in the palette under the command, to
choose from: Enter on a room walks to it, and on a tag or a plugin switches
it.

| Command | What it does |
|---|---|
| `/help` | Opens the palette of these commands |
| `/lua <code>` | Runs a line of Luau |
| `/reload` | Restarts the scripts and rules, as **Restart scripts** does in the editor |
| `/log` | Starts or stops logging the session to a file |
| `/tag`, `/tag <name> [on\|off]` | Lists the tags; switches one on or off for this session |
| `/map where` | Says where the map has you, and the exits |
| `/map here [number]` | Tells the map which room you are in; alone, lists the rooms it is unsure between |
| `/map find <text>` | Lists rooms whose name contains the text |
| `/map travel <place>` | Goes to a room by number, mark, name, or area and name: of several, the nearest |
| `/map route <place>` | Says the way to a place, without going |
| `/map exits` | Says where each exit of this room leads, to go through one by choosing it |
| `/map near [moves]` | Lists what is up to three moves away (or as many as given), with the way |
| `/map info [place]` | Says all the map knows of a room: the current one if none is given |
| `/map mark [name]`, `/map unmark <name>` | Names this room, to travel to it by that name; alone, lists the names |
| `/map areas` | Lists the areas, with how many rooms each has |
| `/map import <file or address>` | Takes a map made with Mudlet in place of this one |
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

**Editor**, at the left of the top bar, takes the place of the game
while you work on it; **Game**, Escape or Ctrl+E goes back, and the session
keeps running meanwhile. The sections are on the left, then the list of the
current one and the selected item's details; drag the gap between those two
to trade their widths. Changes are saved by themselves.

A trigger, an alias, a macro and a timer differ in what sets them off; what
they then do is the same choice for all four:

- **Commands**: what to send, one command per line.
- **Luau**: a few lines of Luau instead, in an editor that numbers the
  lines, colours the code and marks the line of a mistake with what is wrong.

The editor fits what it holds, from three lines for commands and eight for
Luau up to twenty, and scrolls past that. The grip under it makes it taller
or shorter, by dragging it or with the arrow keys, and a double click or
Home fits it to its contents again; the height chosen is kept for every
editor of the same kind. The button at the right of **Do** opens the editor
over the whole window, and Escape closes it.

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
  to complete given by the MUD, and gauges. A script is saved, and run, a
  moment after you stop typing; the triangle does it at once, and under its
  name it says when it ran, or what kept it from running. Only that script
  is run again: the others, and the rules, go on with all they keep, and
  saving a rule does not run the scripts again either. The switch beside it
  says whether a script is run by itself when a session starts; switched
  off it is kept, and can still be run by hand with the triangle.

**Folders** keep each section in order: triggers, aliases, macros, timers
and scripts. Make one with the folder button beside **+**; it is named as it
is made, and named again from its menu (a right click), with a double click
or with **F2**. A click on a folder chooses it, and its details show what it
holds, its switch, and buttons to add a rule or a folder in it, to rename it
and to delete it; a click on its arrow opens or closes it. The button beside the
folder button closes every folder, or opens them all when all are closed. Folders are closed at first,
but for those the one chosen is in; those opened stay open, for each
profile, from one session to the next, and a search or a tag shows all it
finds in them.
Drag a rule onto a folder to put it there, or between two rules to put it in
their place; a folder is dragged into another the same way, or onto the top
or bottom edge of one to go beside it. While what is dragged is in a folder,
the toolbar becomes a strip to let it go on, to take it out of every folder.
What it would do is written by the pointer; the list scrolls at its edges, a
closed folder held over opens, and letting go out of the list, or Escape,
gives the drag up. **Alt+↑** and **Alt+↓** move the one chosen, a rule or a
folder, among those beside it, and **Alt+←** out of the folder it is in; a
folder's menu and its details move it into another. Its details have the folder too, to
choose. Deleting a folder leaves what it held in the folder it was in. A
folder of scripts is a folder on the disk, in the profile's `scripts`. Each
folder has a switch: switched off, nothing in it runs, however deep.

**Restart scripts**, under the sections, runs every script and rule again as
when a session starts, keeping the variables; under it, when that was done
and how many failed, and the scripts that failed are marked in the list.
`/reload` does the same.

The list is a tree, and answers the keys a tree does while it has the
keyboard (Tab stops on it once, at the one chosen) or nothing has:
**↑** and **↓** go to the line above or below, folders included, **Home**
and **End** to the first and the last, **←** closes a folder or goes to the
folder around, **→** opens a folder or goes into it, **Enter** opens or
closes it, **F2** renames what is chosen, and **Del** deletes it, a folder
too.

Every change to the rules can be undone with **Ctrl+Z** and redone with
**Ctrl+Y** or **Ctrl+Shift+Z**, in a field too, where what is typed without
stopping is one step; what is undone is chosen and said. A script or a
variable deleted is had back from the bar that says it was deleted. The
palette has **Undo** and **Redo** while the editor is open. Escape leaves a field being typed in
before it leaves the editor. Several things are chosen as in a file manager: **Ctrl+click** adds one or
takes it away, **Shift+click** chooses all from the one chosen before,
**Ctrl+A** all that is shown, **Shift+↑** and **Shift+↓** one more, and
**Escape** leaves one. With several chosen, the details show what they have
in common, and what is changed there is changed in all of them: their
switch, their folder, their tags; deleting them deletes them all, to be had
back at once with **Undo**. Dragged, they go together. A right click on a
row offers the same: switch, move to a folder, rename, delete. Its menu acts
on the row it is opened on, ringed while it is open, or on all that is
chosen if the row is among them, and leaves what is chosen, and its details,
as they were. Menus open as the right button is pressed, and Escape closes
an open menu alone.

**Tags** switch together what belongs together whatever its kind and its
folder: a trigger, two aliases and a macro tagged *combat* go off together
when *combat* is switched off. Give tags in the details of each thing, by
typing them or choosing among those there are. The tags are listed under the
sections, each with its switch and how many carry it; choosing one narrows
the lists to what carries it.

Something runs while it, every folder it is in and every tag it carries are
switched on; its details say which of them keeps it from running, if one
does. The switches in the editor are what is saved, and how a session
starts. `/tag <name> on` and `/tag <name> off` switch a tag for the session in
course, as a script does with `tags.disable(name)`, and a script switches
any folder or rule by its place, as `triggers.disable("Combat")`; what is
saved does not change. The same key can be bound by several macros, so that
it does one thing while one is on and another while another is: the first
that is on is the one that runs.

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
text. A profile starts with the map at the right and the gauges below; the
MUD's channels get a panel of their own, **Channels**, under the map, when
the first of them comes. A panel is called what it was named, whatever the
language of the interface: the usual ones are in English.

Every panel has a header: six dots to drag it by, its name (or the names of
the panels that share its place, as tabs), the tools of its kind, and its
menu (the three dots). They are arranged on the game by dragging, or in
**Layout**, beside **Game** and **Editor** in the top bar.

**Layout** lists the panels by the side they are on, with **+** to add one,
and shows a picture of the window with the one chosen marked; clicking a
panel in the picture chooses it too. For the one chosen: its name, whether
it is shown, its side, its place there, the width or height of its side,
the panel it shares a place with as a tab, what goes with its kind, and
removing it. **Arrangements** puts every panel at once as **Classic** (the
map and the panels of text at the right, the gauges below), **Wide screen**
(the map at the left) or **The game alone** (all hidden, kept to show
again). With the keyboard, the arrows go through the panels, and Alt with
Up or Down moves the one chosen on its side, Alt with Left or Right to
another side, saying where it is now. Escape goes back to the game; a
panel's menu has **Arrange the panels…** to come here with it chosen.

- **Drag** a panel by its dots to another side of the window, or to another
  panel: on its upper or lower half (left or right, above and below the
  output) to go before or after it, on its middle to share its place as a
  tab. Drag the edge between two panels, or between a panel and the output,
  to size them.
- Its **menu** has its name, what goes with its kind (the channels for one
  of text, editing the gauges, the areas of the map), hiding it and removing
  it.
- **A whole side** can be put away, with all its panels, from the arrow
  to the edge in the header of its first panel. It leaves a thin strip
  with the arrow that brings the side back as it was, where the other one
  was, and the names of its panels: any of them brings it back too, with
  that panel on show. A panel shown or added on a side put away
  brings the side back too; **Ctrl+M** shows or hides the map alone.
- **Side panels at full height**, in the palette, decides who gets the
  corners: with it, the panels beside the output reach the top and the
  bottom of the window.
- Panels are **added** from **Layout** or the palette: of text, the map, or
  gauges. There is one map; of the others, as many as wanted. The palette
  lists them all too, each to show or hide.
- A tab that gets something while another is on show is marked.

The layout is kept with the profile.

What a panel of text shows comes from elsewhere. A trigger can show the lines
it matches in a panel, and hide them from the output so that they are only
there: that is how tells, or combat, get a panel of their own. A script can
write to one with `panel_add(name, text)`, or put a whole text in it with
`panel(name, text)`. A panel can also be told to show the MUD's channels,
from its menu.

The **gauges** are made and changed in the editor, under **Gauges**; the
menu of a panel of gauges, and the palette, lead there. A gauge made there
can also be pressed where it is shown, to change it over the game in the
same details, with the way to the editor. Each gauge says which panel it is shown in, so that yours can be below
and an enemy's beside the map, and they are shown in the order of the
editor's list: drag one there, or press Alt+Up or Alt+Down, to move it. A gauge
follows something the MUD reports, and is one of these kinds:

- **Bar** and **Cells**: how full something is, of its maximum.
- **Number**: a number as it is, such as gold or level.
- **Light**: on or off, for what is so or is not: hidden, poisoned.
- **Text**: what the MUD says, as words: a stance, a target.

Say where the value is, and the maximum for those that have one: a GMCP
package and the fields inside it (`Char.Vitals.health.current`), or the name
of a MSDP variable; **Incoming data**, in the editor, shows what there is. Choose its colour, and for
those with a maximum the colours it takes as it gets low: yellow under 60 %,
red under 30 %, as many levels as wanted, kept in order. A light can have a
colour for when it is off as well, to say which of two things is so: red for
poisoned, green for not. Alt+0 reads the gauges aloud.

### Plugins that come with Muddie

Muddie itself knows the ways of no MUD in particular: those are plugins, in
each profile's **Plugins** tab in the Profiles window, switched on and off
per profile like the ones you install. Two come with it.

- **Standard GMCP and MSDP** makes something of what many MUDs send the same
  way: `Char.Vitals` as gauges, `Room.Info` as the map, `Comm.Channel.Text`
  as channels. It is on in a new profile; switched off, the client makes
  nothing of them by itself, and scripts still get them.
- **Reinos de Leyenda** shows the channels of that MUD, each in its tab, and
  gives it its map: the one the MUD publishes, taken as soon as the plugin
  is on and brought up to date when it changes. It follows you on it from
  the rooms the MUD shows, and walks in Spanish. It
  is off until asked for. On connecting to that MUD it is offered in a
  notice over the bottom right corner: **Enable** switches it on there and then, and **Not now**
  puts the offer away for that profile. The MUD sends nothing over GMCP until
  you type `consentir gmcp on` in the game; if nothing has come a minute and
  a half after connecting, the plugin says so.

Switching one does not run your scripts again. **Customize**, in the menu
of one written in Luau, makes it yours: its code becomes a script of the profile's,
opened in the editor to change at will (to send the channels to another
panel, say), and the one that came is switched off so as not to run twice.

### Channels

What is said on the channels of a MUD (chat, guild, tells...) goes to the
panel of channels, **Channels**, made under the map when the first of them
comes. It has a tab for each
channel, and one for all of them together; a tab turns yellow, with a dot,
when something is said on it that you have not looked at. Any text panel can
be one of channels: **Show the MUD's channels here**, in its menu.

A MUD that sends its channels the usual way over GMCP (`Comm.Channel.Text`)
fills them by itself. For one that does it its own way, a script says which
channel a line is of with `channel(name, text)`; for one that only sends
text, a trigger can.

A script can also say which panel a channel goes to, by the panel's name:
`channel(name, text, {panel = "Talk"})`. A panel of that name that you made
yourself is used as it is, wherever you put it; if there is none, one is
made. So the public channels can be in one panel and whispers in another,
each with its own tabs. The panels go by their names: one renamed is another
panel to the script.

## Incoming data: what the MUD sends

**Incoming data**, the last section of the editor (Ctrl+D goes straight to
it), shows what the MUD sends besides text, with GMCP or MSDP, as it comes: each
package or variable with its values one by one, the message as it came, and
those before it. It is where to look before making a gauge or writing a
script. **Copy the path** takes what leads to a value, such as
`Char.Vitals.health.current`, and **Make a gauge** makes one that follows a
number, with the maximum that goes with it if there is one beside it.

At the top are the **protocols**: lit, those in use; dim, those the MUD
offered and Muddie does not use; fainter, those it did not offer. A MUD that
offers neither GMCP nor MSDP sends only text, and what is said on its
channels comes as lines of the game, for triggers to pick out. Besides GMCP
and MSDP, what comes by MSSP, the tags of MXP, the sounds of MSP and what
comes with an option Muddie does not know are listed too; choosing a protocol
lists what came by it alone.

Nothing is kept while the view is closed, so that it costs nothing: the list
starts when it is opened. **Record in the background** goes on
keeping it, to see what comes on connecting, and **Clear** empties the list.

## The map

The map draws itself as you walk, on servers that report rooms through GMCP
or MSDP, and is saved per profile. Scripts can map other servers, or take
the map a MUD publishes made with Mudlet, as the plugin of Reinos de Leyenda
does; `/map import` takes one from a file of your own.

On servers that do not say which room you are in, only its name and exits,
the map works it out from those and the exit you took. Where several rooms
are alike, as in a desert, it says it is not sure and follows each it could
be until a move tells them apart; `/map here` lists them, to say which.

Each room is drawn in its colour: a map taken from Mudlet keeps the colours
of its rooms, which in most MUDs follow the land (sand, forest, rock), and
scripts can colour rooms too. The room you are in glows and is ringed; while
the map is unsure, each room you may be in is ringed. During a walk, the way
still to go is drawn over the map, and the room it ends in is ringed. A line
joins rooms that lead to each other; one with an arrow at its end goes one
way only, a dashed one is an exit off the compass (`enter portal`), and a
short stub leads to a room not drawn. Inside a room, a triangle up or down
marks a way up or down, and a dot an exit off the compass. A room to avoid
is struck through in red, and a locked exit is red. Zoomed in far enough,
each room's name is written under it. When you move, the view glides to
the new room. The list of areas only shows when the map has more than one.
A map taken from Mudlet before this version of Muddie read its colours is
taken again the next time the plugin asks for it.

Everything the map shows can be said: the `/map` commands above tell where
you are, what is around, where each exit leads and the way to anywhere, and
`/map travel` goes there. Typed in the palette, the name of a place lists
the places of the map that have it, under **Places**, each once with where
it is and how many moves away, the nearest first, and the names given to
rooms with `/map mark`: Enter goes there, and Shift with Enter says the way
without going. Going somewhere says how far, and the regions the way goes
through; nothing of what the game says on the way is left unread. Escape in
the empty command line stops the walk.

Where a room is, its region, is its area; a plugin can teach how its MUD
names rooms instead, as the one of Reinos de Leyenda does: there the region
is what a name begins with, as Anduar in "Anduar: Plaza Mayor".
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

Muddie reads each new line aloud, the same way on every system. In the
settings, **Speak with** chooses what reads it:

- **The screen reader** (the default): the one you use, Orca, NVDA, JAWS or
  VoiceOver, with its own voice and settings, and on your braille display
  too. While no screen reader runs, nothing is said, so a sighted player
  hears nothing they did not ask for. Open or close one while Muddie runs and
  it says which it speaks with now.
- **The system's voice**: speaks whether or not a screen reader runs, at the
  speed and in the language set below it.
- **Nothing**.

**Switch speech off** in the palette silences it for the session, and
**Switch speech on** brings it back, with the system's voice if there is no
screen reader. What is said on the MUD's channels is read too, after the
channel's name; **Read the channels** in the settings turns that off, and
**Read aloud** in the menu of a channel's tab, for that channel alone, in
that profile. **Interrupt with what is new** has each new line cut short the
one being said, for games where what came before is soon old.

| Key | What it does |
|---|---|
| Ctrl | Silences speech |
| Alt+Up, Alt+Down | Reads the previous and next line of output |
| Alt+Home, Alt+End | Reads the first and last line |
| Alt+1 ... Alt+9 | Reads the most recent lines |
| Alt+Shift with the same keys | The same for the panel of channels |
| Alt+0 | Reads the gauges |
| Alt+M | Says where you are by the map, and how far a walk has left |

On a Mac, Option types characters (@ on a Spanish keyboard is Option+2),
so the shortcuts held with Alt and a letter or a figure are held with Ctrl
there (Ctrl+1 … Ctrl+9, Ctrl+0, Ctrl+M), those held with Alt and an arrow
with Option, Reconnect is Cmd+R, and those written here with Ctrl, Cmd. The
window of keyboard shortcuts shows them as they are on each system.

Everything can be done from the keyboard. Tab goes through the window as
it is read, top to bottom; Shift+F10 or the menu key opens what a right
click opens; the arrows move an edge between panels once Tab is on it, and
the new size is said; and opening a dialog with the keyboard puts it on
its first control. In an editor of code Tab inserts a tab; Escape and then
Tab move on to the next control.

A gauge is said as soon as it falls under one of its warnings (a step
where it changes colour, or its low mark), as it is seen turning colour.

The review keys cut short what is being said, so moving through the lines
is quick. Sending a command silences what was still queued. For the system's
voice, set the language (`es`, `en`...) to that of the MUD, so its text is
read with the right voice.

## Sound

Scripts and triggers can play sound files. A MUD can ask for sounds and music
too, in three ways that go the same: the MUD Sound Protocol (`!!SOUND`,
`!!MUSIC`), the `sound` and `music` tags of MXP, and GMCP's `Client.Media`.
Sounds are looked for by name in the profile's `sounds` folder and then in
`sounds` in the configuration folder; one of a kind (`T=combat`) may be in the
folder of that kind, and a name with `*` or `?` in it is any of the files it
stands for.

A sound you do not have is fetched from where the MUD says it is on the web,
kept in the cache folder (`~/.cache/muddie/media`) and played; next time it is
already there. Only sound files are fetched, only over HTTP or HTTPS, and none
larger than 25 MB. Fetching reaches a server the MUD names, as a web page
does; **Settings > Download the sounds a MUD asks to play** switches it off,
and then only the sounds you have are played.

A sound with more priority stops those with less, and is not played over one
with more. There is one music at a time, which goes on if it is asked for
again. A sound can be played several times or until stopped, and stopped by
name, kind, tag or key. Starting part way in, ending early, and coming up or
dying away are done where `ffplay` is installed; otherwise the sound is played
whole.

## What a MUD can do: the protocols

Muddie agrees with a MUD on what both can do when it connects; **Incoming data** shows
what was agreed. Nothing here needs setting up.

- **GMCP**, **MSDP**, **MSSP**: data beside the text, which gauges, the map
  and scripts use. With MSDP, the MUD is asked which variables it has and then
  for all of them.
- **MXP**: text the MUD marks up. Links to click: one that sends a command,
  one that puts it in the command line to be finished, one to the web, and
  one that offers several commands, of which a click sends the first and the
  other button shows them all; pointing at a link says what it is for. Bold,
  italics, colours, rules and paragraphs. The tags and names a MUD defines
  for itself. Values it names, which scripts hear as the `"mxp"` event with
  `{name, value}`; gauges that follow them, shown with the others; frames,
  which are text panels by their name, and text sent to them; sounds; and the
  name and password of the profile when the MUD asks for them. A picture is
  shown as its name, which opens it. Where a MUD asks to go on at another
  address, that is said and left to you.
- **CHARSET**: the encoding of the text, agreed with the MUD and then used to
  read it. The one set in the profile is asked for if there is one.
- **MCCP** compresses what is sent each way; **NAWS** tells the MUD how much
  text fits, and again when that changes; **MTTS** and **MNES** tell it what
  the client is: its name, version, colours, and whether a screen reader is
  in use.

## Settings

**Settings**, in the menu (Ctrl+,), are one window with a tab for each of
its pages; each profile's are in the Profiles window. A row says what it
is at the left, with what it does under it when that needs saying, and has
its control at the right.

- **General**: the language, and the interface scale.
- **Appearance**: the theme (Dark, Graphite, Mocha, Dusk, Tokyo Night, Nord,
  Light, High contrast), and for the text of the MUD its palette, its font
  (MesloLGS NF unless another is chosen: JetBrains Mono, which also comes
  with the program, or any monospaced font installed), its size and its line
  spacing, with a sample.
- **Text**: the scrollback, timestamps, and whether a sent command stays in
  the command line.
- **Speech**: what reads aloud, whether new lines interrupt, whether the
  channels are read, and the speed and language of the system's voice.

A session without a profile connects, keeps its connection and logs as a new
profile would. Muddie's settings are saved in `settings.toml`, each profile
in its `profile.toml`.

## Languages

The interface follows the language of the system; **Settings** can fix it.
English and Spanish come with the program. Translations are gettext
catalogues: to add a language, copy `po/muddie.pot` from the sources to
`<language code>.po` (`fr.po`, `pt_BR.po`), translate it with any gettext
editor such as Poedit, and put it in the `locale` folder of the configuration
(`~/.config/muddie/locale`). It then appears among the languages. A file there
also replaces a built-in translation of the same code.
