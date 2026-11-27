# Muddie scripting reference

Español: [scripting.es.md](scripting.es.md). To make a plugin, start with
[plugins.md](plugins.md). Every function and its types, for an editor that
checks Luau: [muddie.d.luau](muddie.d.luau).

Scripts are written in [Luau](https://luau.org), a fast, typed dialect of Lua
5.1. There is one API for everyone: what a script shows is seen, what it says
is heard, and the same script serves who looks at the screen and who listens
with a screen reader. Muddie gives the data and the tools; what is shown and
said is up to the scripts.

Scripts run in a sandbox: they cannot read or write files, start programs or
load code from outside the script folders, so a script someone shares can do
no harm. One that runs for more than half a second at a time is stopped with
an error, and the client carries on.

## Contents

1. [Getting started](#getting-started)
2. [Showing and saying](#showing-and-saying)
3. [What is heard](#what-is-heard)
4. [Triggers](#triggers)
5. [The line](#the-line)
6. [Aliases, keys and timers](#aliases-keys-and-timers)
7. [What a script makes, and switching it](#what-a-script-makes-and-switching-it)
8. [Waiting](#waiting)
9. [Listening: on](#listening-on)
10. [Variables](#variables)
11. [Gauges](#gauges)
12. [Panels and channels](#panels-and-channels)
13. [Sounds](#sounds)
14. [The session and the command line](#the-session-and-the-command-line)
15. [Data from the MUD](#data-from-the-mud)
16. [The map](#the-map)
17. [Modules](#modules)
18. [Mistakes, and checking a script](#mistakes-and-checking-a-script)
19. [What there is not](#what-there-is-not)

## Getting started

### Where scripts live

All paths are under the configuration folder: `~/.config/muddie` on Linux,
`%APPDATA%\muddie` on Windows.

| File | When it runs |
|---|---|
| `init.luau` | In every session |
| `profiles/<profile>/scripts/*.luau` | In that profile, in name order; those of a plugin in the plugin's folder |
| files given with `--script` | In every session, last |

In the palette (Ctrl+K), **Restart scripts** runs them afresh after editing,
and **Run Luau…** runs a line of Luau on the spot. In the editor, **Scripts**
has each script, to write, run and switch.

### A first script

```lua
-- Say who tells you something, cutting in, and paint the name.
trigger("^(?<who>\\w+) tells you: (.*)$", function(m, line)
    line:paint("bright yellow")
    speak(`{m.who} says {m[2]}`, {aloud = "cut"})
end)

-- Keys of the numeric keypad walk.
macro("numpad8", "north")
macro("numpad2", "south")

-- Health as a bar, from what the MUD sends over GMCP.
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("HP", {value = vitals.hp, max = vitals.maxhp})
end)
```

### What a script keeps, and when it loses it

What a script holds in its own variables lasts while it is loaded:

- **Kept** when the connection is lost and made again (scripts are only told,
  with `on("disconnect")` and `on("connect")`), and when another script or a
  rule is changed and saved.
- **Lost** when the script itself is run again, by saving it or with its run
  button; when everything is run again (**Restart scripts**, another profile, a
  plugin switched); and when Muddie is closed. A script run again starts from
  its first line with nothing: that is where to ask the MUD for what it had
  sent.

What must outlast all of that goes in [`vars`](#variables).

What a script makes and sets goes with it when it is switched off, removed or
run again: its triggers, aliases, keys, timers, listeners and gauges; the text
it put in panels and the channels it opened; its sounds, stopped; and what it
set of the map and the session (`map.learn`, `map.directions`, `map.names`,
`map.native`, `map.persist`, `map.language`, `session.separator`,
`session.completions`, `gmcp.merge`, `gmcp.support`, `msdp.report`), back as it
was.

### Options, places and tags

Options always go last, in a table: `trigger(pattern, fn, {once = true})`. An
option a function does not take is an error that says which it takes, never
left out without a word.

Everything that can be switched takes `place` and `tags`:

- `place`, as `"Folder/Name"`: what it is switched by. Switching a folder off
  switches off everything in it.
- `tags`, a list of tags or one alone: each switches it with everything else
  that carries it.

Something runs while its place, every folder above it and every tag it
carries are on. A rule of the editor has as its place its folder and its name
(`Combat/Fight back`; two alike are told apart by a number,
`Combat/Fight back #2`). A script's place is where it is among the profile's scripts, with no
extension: `Mapping/rooms`.

## Showing and saying

### `send(text)`

Sends a command to the MUD as it is: aliases are not applied.

### `echo(text, options)`

Shows a line in the output, and reads it aloud. Takes `aloud` and `always`
(see [What is heard](#what-is-heard)).

### `speak(text, options)`

Says something aloud and shows nothing. Takes `aloud` and `always`.

```lua
speak("You are dying!", {aloud = "cut", always = true})
```

### `notify(text, options)`

Tells the player something, as the client tells its own: for a few seconds over
the bottom right corner, and under the bell after; never in the output. It is
Muddie talking, not the game, so it is heard in any session and with the game
not read aloud, unless it is `{aloud = "never"}`.

### `log(text)`

Shows something on the standard output of a run started with `--debug`;
nothing otherwise. For what only whoever writes the script wants to see.

### `tr(texts)`

The text in the language of the interface, of those given by language code:
else that of its language without the country (`es` for `es_AR`), else
English, else the first. For what a shared script shows the player.

```lua
notify(tr{en = "Map updated.", es = "Mapa actualizado."})
```

`echo`, `notify` and `speak` take the texts by language themselves, too:
`speak{en = "You are dying", es = "Te mueres"}`.

### `hush()`

Silences what is being read, in the session on screen.

## What is heard

`speak`, `echo`, `line:read`, `panels.add`, `channels.say` and `gauges.set`
say how they are heard in the same way, in their options:

| Option | Heard |
|---|---|
| `aloud = "usual"` | As it is if nothing is said: after what is being read; `line:read` as the line would be; a gauge as its kind is (see [`gauges.set`](#gaugessetname-gauge)) |
| `aloud = "wait"` | After what is being read |
| `aloud = "cut"` | Cutting short what is being read |
| `aloud = "never"` | Not at all |
| `aloud = "lowest"` | Of a gauge only: waiting, but cutting in under its lowest step |
| `always = true` | Also while the player has the game not read aloud |

Everything of the game goes through one door, with the same rules as its lines:

- It is heard only in the session on screen.
- It is not heard while the player has the game not read aloud, unless it is
  `always`.
- It is not heard if the settings, the panel or the channel are told not to be
  read; `always` does not get past those.
- The same words just read, as a line or of a channel, are not read again,
  unless a script asks for them to cut in.

`notify` is not of the game: it is heard in any session.

## Triggers

### `trigger(pattern, fn, options)`

Calls `fn(m, line)` for each line of the output that matches `pattern`, a
regular expression (Rust's `regex`, so `\\d`, `(?<name>...)` and `(?i)` work).
`m[0]` is the whole match, `m[1]`... its groups, and a named group is also
`m.name`. `line` is [the line](#the-line), to change how it is shown and heard.

Options: `place`, `tags`, `once` (removed after it fires once) and `prompt`:
`true`, for prompts only; `false`, for all but prompts; not given, for both. A
prompt is what the MUD marks as one, or text left without an end, once it stops
changing.

```lua
trigger("^You are hungry", function() send("eat bread") end,
        {place = "Survival/Eat", tags = {"survival"}})

trigger("^\\[spam\\]", function(m, line) line:hide() end)

-- A prompt that is not read, and what comes after it on its line is.
trigger("^HP \\d+> (.*)$", function(m, line) line:read(m[1]) end)
```

Triggers run in the order they were made; a line goes through every one that
matches.

## The line

A trigger is given the line it matched, to do with it what it wants while it
looks at it. Kept for later, it can only be asked what it said.

### Fields

| Field | What it is |
|---|---|
| `line.text` | What it says |
| `line.prompt` | Whether it is a prompt: marked so by the MUD, or left without an end |
| `line.spans` | The line as runs of text with `text`, `fg`, `bg`, `bold`, `italic`, `underline` and `inverse` |

### `line:hide()`

It is neither shown nor read.

### `line:mute()`

It is shown, and kept to be read back with the review keys, but not read aloud
as it comes.

### `line:read(text, options)`

`text` is read in its place, as the line would be, or as `aloud` and `always`
say.

### `line:replace(text)`

Puts `text` in place of what matched: shown so and, unless told otherwise,
read so. Returns whether it could: not over what another trigger replaced.

### `line:paint(fg, bg)`

Recolours what matched. A colour is a name (`"red"`, `"bright blue"`),
`"#rrggbb"` or a palette number; `nil` leaves that one as it is.

### `line:link(command, options)`

Makes what matched a link that sends `command`, reached with the keyboard as
those the MUD marks. `{hint = "..."}` says what it is for. Returns whether it
could, as `line:replace`.

### `line:to_panel(name)`

Adds the line to a panel of text, with its colours.

### A line is not its text

Its text is always `line.text`: `echo(line.text)`, `line.text:match("%d+")`,
`line.text == "Done."`. Giving the line where a text is wanted is an error that
says so; but `line == "Done."` is no error in Luau, only never true. With
`--!strict` at the top of the script, [the definitions](#mistakes-and-checking-a-script)
catch it.

## Aliases, keys and timers

### `alias(pattern, fn, options)`

Calls `fn(m, input)` instead of sending what is typed, when it matches
`pattern`. Options: `place`, `tags` and `once`.

```lua
alias("^k (.+)$", function(m) send("kill " .. m[1]) end)
```

### `macro(key, action, options)`

Runs commands or a function when a key is pressed: `macro("f5", "look")`,
`macro("ctrl+shift+k", fn)`. Commands are sent as a rule's are: separated by
`;`, with `$name` put in, not through aliases. Options: `place` and `tags`.

A key bound again under another `place` does that while it is on: the first
bound that is on is the one that runs. Bound again under the same place, it
does the new thing instead.

Keys are named in lower case with their modifiers in the order `ctrl`, `alt`,
`shift`: `f1`...`f35`, `ctrl+k`, `alt+shift+up`. Of the main block, only
function keys and keys pressed with Ctrl or Alt can be bound, so typing is
never taken.

The numeric keypad has names of its own: `numpad0` to `numpad9`, `numpadadd`,
`numpadsubtract`, `numpadmultiply`, `numpaddivide`, `numpaddecimal` and
`numpadenter`. With Num Lock off, the digits are other keys, named by what is
written on them: `numpadup`, `numpaddown`, `numpadleft`, `numpadright`,
`numpadhome`, `numpadend`, `numpadpageup`, `numpadpagedown`, `numpadclear` (the
5), `numpadinsert` and `numpaddelete`. A bound keypad key no longer types its
digit.

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad5", "look")
```

### `after(seconds, fn, options)`

Calls `fn()` once, after so many seconds. Options: `place` and `tags`.

### `every(seconds, fn, options)`

Calls `fn()` again and again, every so many seconds. Switched off, it lets its
turns pass.

```lua
local saving = every(60, function() send("save") end)
after(5, function() saving:remove() end)
```

## What a script makes, and switching it

### What `trigger`, `alias`, `macro`, `after`, `every` and `on` return

What they made:

| Method | What it does |
|---|---|
| `made:enable()`, `made:disable()`, `made:toggle()` | Switch it; return whether it is now on |
| `made:enabled()` | Whether it is on |
| `made:remove()` | Takes it away; returns whether it was there |
| `made:left()` | Of a timer, the seconds it has left |

### `triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts`

Switch the things of a kind by their place, or a folder with all it holds,
whether made in a script or in the editor, for the session:

```lua
triggers.disable("Combat")
macros.enable("Combat/Kick")
local on = scripts.toggle("Mapping")
```

Each has `enable(place)`, `disable(place)`, `toggle(place)` (returns whether it
is now on), `enabled(place)` and `list()`, the places there are.

### `tags`

The same for tags: `tags.disable("combat")` switches off everything that
carries the tag, of every kind.

## Waiting

What takes several steps is written one after another, rather than as triggers
and timers that set off each other.

```lua
alias("^cure$", function()
    send("drink potion")
    local m = wait("line", "^You feel better", {timeout = 3})
    if not m then return echo("It did nothing.") end
    wait(2)
    send("drink potion")
end)

trigger("^You are carrying:$", function()
    local things = {}
    for line in lines({till = "^$", timeout = 2}) do table.insert(things, line.text) end
    vars.things = things
end)
```

### `wait(seconds)`

Stops there for a while; the rest of the client and of the scripts goes on.

### `wait("line", pattern, options)`

Stops until a line after this one matches, and returns what a trigger is
given: the groups and the line. Options: `timeout`, in seconds, after which it
returns `nil`; and `prompt`, as a trigger's.

### `wait(what, name, options)`

Stops until what `on(what, name, ...)` listens to comes, and returns what it
would be given (`true`, if that is nothing). Options: `timeout`, and those of
`on`: `wait("gauge", "HP", {under = 30, timeout = 10})`.

### `lines(options)`

The lines that come next, one by one, in a `for`: until one matches `till`,
which is not among them; until there have been `count`; or until none comes in
`timeout` seconds. It needs one of the three.

### Where nothing can wait

What waits goes when its script is stopped or run again. Nothing can wait in
`on("send", ...)`, in a module while it is being required, or in a function
given to another, as to `table.sort`. `coroutine.wrap(function() ... end)()`
runs what is in it, waits and all, beside the rest.

## Listening: on

### `on(what, name, fn, options)`

Calls `fn` when something comes. Returns what it made, as a trigger. Options:
`place` and `tags`; for a gauge also `under` and `over`. Something it does not
know to listen to is an error that says what there is.

| `what` | `name` | `fn` is given |
|---|---|---|
| `"gmcp"` | a package: also those under it (`"Char"` covers `Char.Vitals`) | the data, and the package |
| `"msdp"` | a variable | the value (a string), and the variable |
| `"var"` | a variable of [`vars`](#variables) | the value, and the name |
| `"gauge"` | a gauge, or none for all | the gauge, and how it changed (see [`on("gauge")`](#ongauge-name-fn-options)) |
| `"channel"` | a channel, or none for all | `{channel, text}` |
| `"emit"` | what another script says | what it said, and the name |
| `"send"` | none | each command on its way to the MUD (see [`on("send")`](#onsend-fn)) |
| `"connect"`, `"disconnect"` | none | nothing |
| `"focus"` | none | whether the session is now in front, in a window that is |
| `"reader"` | none | whether a screen reader now reads Muddie |
| `"mxp"` | none | `{name, value}`, when the MUD names a value with MXP |
| `"panel"` | none | `{name, map, columns, rows}`, when a panel that shows a drawing settles its size; `map` is true for the map's panel |

What changes of a variable, a gauge, a channel or another script is told once
the script that changed it is done, so it is told whole.

### `emit(name, data)`

Says something to the other scripts, which listen with `on("emit", name, fn)`.
A plugin tells so what others may want to know.

```lua
-- In one script:
emit("fight", {with = "orc"})
-- In another:
on("emit", "fight", function(data) speak(`Fighting {data.with}`) end)
```

### `on("send", fn)`

Calls `fn(text)` with each command about to be sent to the MUD, wherever it
comes from: typed, or sent by a script, a rule, a walk or a link. What it
returns is sent instead; `false`, nothing is; `nil`, it goes as it is. Not
told of the name and password the client logs in with, nor of what such a
function sends itself.

```lua
on("send", function(text)
    if text == "quit" then
        notify("Type it in full: quit!")
        return false
    end
    if text == "quit!" then return "quit" end
end)
```

## Variables

### `vars`

The variables, as a table, kept with the profile: they outlast the session.
They are those of the editor's **Variables**.

```lua
vars.target = "orc"          -- keeps it
send("kill " .. vars.target) -- reads it, as it is now
vars.target = nil            -- forgets it
for name, value in vars do echo(`{name} = {value}`) end
```

A table kept in one is kept as it is when it is given: change it, and give it
again.

```lua
local bag = vars.bag or {}
bag.gold = 5
vars.bag = bag
```

### `expand(text)`

`text` with each `$name` replaced by that variable's value, `$name.key` by a
field of a table and `$name.1` by an item of a list, as the commands of rules
are.

### `on("var", name, fn)`

Calls `fn(value, name)` when the variable changes, by a script, a trigger or
the editor. A gauge of the editor can follow a variable too.

## Gauges

A gauge is data first: whatever made it (the editor, a plugin, a script or the
MUD), a script can ask for it and listen to it, whether a panel shows it or not.
With a screen reader the panels of gauges are not there for it; what they show
is read with Alt+0, and said as it changes.

### `gauges.set(name, gauge)`

Shows a gauge, or updates it. What it is given:

| Field | What it is |
|---|---|
| `kind` | `"bar"` (as none is said), `"cells"`, `"number"`, `"light"`, `"text"` or `"effects"` |
| `value`, `max` | Its level: a bar and cells need both; a number, `value` |
| `on` | Of a light: whether it is lit |
| `text` | Of text: what it says |
| `items` | Of effects: what the character is in, a list (or a table by name) of names or `{name = "Poisoned", about = "1 damage a second", colour = "red"}` |
| `colour`, `off_colour` | A name or `#rrggbb`; `off_colour` is a light's while it is off |
| `steps` | The colours it takes under each percentage: `{{under = 60, colour = "yellow"}, {under = 30, colour = "red"}}` |
| `aloud`, `always` | How its changes are heard. `"usual"`: a level cuts in as it falls under a step, and the rest wait their turn; also `"lowest"`, `"wait"`, `"cut"`, `"never"` |
| `panel` | The panel of gauges it is shown in, by name: made if there is none; the first there is if none is named |
| `place`, `tags` | What switches it, as a trigger |

A level is said as it falls under one of its steps, once until it rises clear
of it, and each tenth more it falls under one; a light, words or effects are
said when what they say changes.

```lua
gauges.set("HP", {value = 40, max = 100,
                  steps = {{under = 30, colour = "red"}}, aloud = "lowest"})
gauges.set("Poisoned", {kind = "light", on = true, colour = "green"})
gauges.set("States", {kind = "effects", items = {"Blessed", "Stunned"}})
```

### `gauges.remove(name, options)`

Removes it. Options: `place` and `tags`, those it was set with.

### `gauges.get(name)`

What the gauge is now, by its name or its place, or `nil`: a table with
`name`, `place` and `kind`, and by its kind, `value`, `max` and `percent`;
`on`; `text`; or `items`, a list of `{name, about, colour}`. One switched off
is as if it were not there.

### `gauges.all()`

All of them, in a list, each as `gauges.get` gives it.

### `on("gauge", name, fn, options)`

Calls `fn(gauge, change)` when the gauge changes, as it is now; with no name,
for every gauge. `change.was` is what it was; of effects, `change.came` and
`change.gone` are lists of those that came and those gone. What a gauge is as
it is made is not a change.

`{under = 30}`: only as it falls under 30 %, once, until it comes back 5 % over
it. `{over = 90}`: as it reaches 90 %.

```lua
on("gauge", "HP", function() sounds.play("alarm.ogg") end, {under = 30})

on("gauge", "States", function(gauge, change)
    for _, state in change.came do speak(state.name) end
end)
```

## Panels and channels

### `panels.set(name, text)`

Puts `text` in a panel of text, in place of what it showed: for what the
server sends ready to show, as a map it draws. The panel is made if there is
none of that name; `"map"` is the map's panel whatever it is called, and
`"channels"` the panel of channels.

### `panels.clear(name)`

Empties it.

### `panels.add(name, text, options)`

Adds `text` at the end of a panel, which keeps what it had, and reads it aloud
after the panel's name. Takes `aloud` and `always`.

### `panels.size(name)`

How many characters fit in a panel, as `{columns, rows}`, at once; `nil` if it
has not been on the screen. With no name, the map's panel. `on("panel")` tells
when it changes.

### `channels.say(name, text, options)`

Adds `text` as said on the channel `name` of the MUD, which has a tab of its
own in a panel, and reads it aloud after the channel's name. Options:

- `panel`: the panel the channel goes in, by name. One of that name is used as
  it is, and one is made if there is none; with none, the panel of channels.
- `aloud` and `always`: `aloud = "never"` for what was said before the player
  came.

The same message said twice at once, by two scripts, is one.

### `channels.open(name, options)`

Makes the channel's tab before anyone speaks on it. Takes `panel`.

### `channels.close(name)`

Takes the tab away, as when the character leaves the channel.

### `channels.all()`

The names of the channels there are.

## Sounds

### `sounds.play(file, options)`

Plays a sound, by path or by name in the profile's `sounds` folder. Options:

- `volume`, from 0 to 1, and `loop`.
- Where the system's player can (Linux): `start` and `finish`, where in it to
  start and to end, and `fade_in` and `fade_out`, how long it takes to come up
  and to die away, in seconds. On Windows and Mac it plays whole.

Returns the sound: `sound:stop()`, and `sound:playing()`, whether it plays.

### `sounds.stop()`

Stops every sound.

## The session and the command line

### Fields of `session`

| Field | What it is |
|---|---|
| `session.connected` | Whether the session is connected |
| `session.host` | Where it connects to |
| `session.profile` | The name of its profile, or `nil` |
| `session.reader` | Whether a screen reader is reading Muddie |
| `session.focused` | Whether the session is in front, in a window that is |
| `session.input` | What is typed in the command line; set it to put something there. `nil` while it is a password, which is not set either |
| `session.separator` | What separates commands typed on one line: `";"` as none is set; `false` turns splitting off; `nil` gives back `";"`. Doubled, it is typed as it is |

### `session.completions(name, words)`

Words for Tab to offer, besides those seen in the output. Each name keeps its
own list; `session.completions(name)` removes it.

### `session.feed(text)`

Takes `text` as if the MUD had sent it: to try what a script does with it.

### `output.last(count)`

What the last `count` lines of the output say, the newest last; the last one,
if `count` is not given.

## Data from the MUD

```lua
gmcp.support("Char 1", "Room 1")
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("HP", {value = vitals.hp, max = vitals.maxhp})
end)

msdp.report("HEALTH", "HEALTH_MAX")
on("msdp", "HEALTH", function(value) echo("health " .. value) end)
```

### `gmcp.get(package)`

The data last received for exactly that package.

### `gmcp.send(package, data)`

Sends a GMCP message.

### `gmcp.support(module, ...)`

Asks the server for GMCP modules besides the client's own (`Core` and
`Client.Media`), as `"Name version"`. Some servers only send what was asked
for. Asked for while the script is loaded: switched off or removed, the server
is told it no longer wants them.

### `gmcp.merge(package, ...)`

For packages the server sends only what changed of, as a `Char.Status` with
only the life that changed: what comes is merged into what `gmcp.get` and the
gauges keep, tables inside too. Listeners still get what came.

### `msdp.get(variable)`, `msdp.send(command, ...)`

The same for MSDP. Values arrive as strings.

### `msdp.report(variable, ...)`

Asks the server to report MSDP variables as they change, while the script is
loaded.

### `mssp.get()`

The server's MSSP status, as a table.

## The map

The **Standard GMCP and MSDP** plugin maps from GMCP `Room.Info` and MSDP
`ROOM`. For a MUD that tells rooms some other way, or only in text, a script
feeds the map itself, and can turn the standard loader off.

```lua
-- A MUD with no GMCP: the room from its title and exits lines.
map.native = false
local title
trigger("^\\[ (.+) \\]$", function(m) title = m[1] end)
trigger("^Exits: (.+)$", function(m)
    local exits = {}
    for direction, to in m[1]:gmatch("(%a+)=(%a+)") do exits[direction] = to end
    map.visit({id = title, name = title, area = "town", exits = exits})
end)
```

### Rooms and places

A room is given as a table with `id` and, optionally, `name`, `area`, `exits`
(direction to room id), `x`, `y`, `z` and `colour` (a name, `"#rrggbb"` or a
palette number; a room given without one keeps the colour it had). An id is a
number, or the text the server tells the room by. What a MUD sends in
`Room.Info` or `ROOM` can be given as it comes: `num` or `vnum` for `id`,
`zone` for `area`, `coords` for where it is, in any letter case.

A place, where a function takes one (`map.room`, `map.here`, `map.mark`,
`map.edit_room`, `map.remove_room`, `map.way`, `map.go`), is a room's id, or a
mark or a name as the player types them, the nearest of several.

### Settings of the map

They are fields: set them, and read them back. `nil` puts one back as it was.

| Field | What it does |
|---|---|
| `map.native` | `false` turns the standard loader off while the script is loaded |
| `map.persist` | `false` keeps the map in memory only, for servers that send it themselves |
| `map.learn` | `true` has the map learn as the player goes (see [`map.learn`](#maplearn)) |
| `map.language` | The language the MUD is played in, by code or name (`"es"`): the map takes its words for directions |
| `map.directions` | Words of the MUD's own for directions, over those of its language: `{n = {"nte"}, s = "sur"}`; the first of each is what walks send |
| `map.names` | How the MUD names its rooms: `{region = ":", exits = "[]"}`, the region first, up to `region`, and the exits after, between the characters of `exits` |

### Following the player

#### `map.visit(room)`

The player is now in `room`, which is added or updated. Returns its id.

#### `map.set_room(room)`

Adds or updates a room without moving there. Returns its id.

#### `map.here(place)`

The player is in that room, one the map has. Returns whether it has it.

#### `map.arrive{name, exits, came}`

For servers that do not say which room it is: the player is in a room of this
name and exits, reached by the exit `came`. Returns the room's id, or `nil`
while unsure.

#### `map.add{name, exits, came}`

For servers that do not say which room it is, when the player went by `came`
where the map has no room: finds the room among those the map has, or makes it.
Returns its id and `"found"` or `"made"`, or `nil` and why not: `"lost"`, the
map does not know for certain where the player was; `"known"`, the exit leads
to a room already; `"alike"`, several rooms could be it; `"unnamed"`. The
player can undo the rooms added.

#### `map.learn`

Set to `true`, the map learns from the rooms `map.arrive` is given, never from
one sighting alone, nor from a room it does not know for certain the player was
in: it adds the rooms it has not where the player goes by the compass; puts
right the names of rooms it named otherwise, once the next move bears it out;
adds the exits a room shows that it lacks; and has an exit lead to the room it
really leads to. Each change is told, and undone as one of the map's own.

#### `map.candidates()`

The ids of the rooms the player may be in while the map cannot tell which;
empty when it can, or has no idea.

#### `map.failed()`

The server said a move failed: the moves sent did not happen, and a walk stops.

### Walking

#### `map.way(to, from)`

The commands to walk from `from` (where the player is, if not given) to `to`,
or `nil`.

#### `map.go(to)`

Goes there, one confirmed move at a time.

#### `map.stop()`

Stops a walk, as **Stop going** does.

### Looking things up

#### `map.room(place)`

A room as a table, the player's if none is given: `id`, `name`, `area`, `x`,
`y`, `z`, `exits` (direction to room id), `cost` (what entering it costs a
route), `avoid`, `colour` (`"#rrggbb"` or `nil`), `note` (`""` for none),
`renamed` (its name is one given in place of the map's), `added` (the map
added it of itself), `shape` and `symbol`.

#### `map.find(text, options)`

The ids of the rooms `text` finds, as **Find rooms…** finds them: by mark,
note or name, the nearest first. With `{exact = true}`, of those called just
`text`, as the MUD names them.

#### `map.exit(from, direction)`

An exit as a table: `to`, `cost`, `blocked` (kept off routes), `command` and
`custom` (one of the player's own).

#### `map.direction(word)`

The direction a word says, as the map keeps it (`"n"`, `"ne"`... `"u"`, `"d"`,
`"in"`, `"out"`), in the MUD's words or in English; `nil` if it says none.

#### `map.count()`, `map.areas()`, `map.id(text)`

How many rooms there are; the names of the areas; the id used for a text id.

#### `map.marks()`

The marks, by name, each with its room's id.

### Changing the map

#### `map.mark(name, place)`

Marks a room with `name`, as **Mark…** does; `map.mark(name)` forgets the mark.
Returns whether there was the room, or the mark.

#### `map.edit_room(place, changes)`

Changes what `changes` gives of a room, leaving the rest: `name` (in place of
the map's: the server telling the room again, or the map imported again, leaves
it), `note`, `cost`, `avoid`, `x`, `y`, `z`, `colour`, `shape` (`"square"`,
`"circle"`, `"diamond"` or `"hexagon"`) and `symbol` (a character or two
written in it). `false` gives back the map's own colour or shape, no cost, or
no symbol. Returns whether there was the room.

#### `map.set_exit(from, direction, to, options)`

Adds an exit of the player's own, or changes where one leads; with `to` `nil`,
changes only what `options` give of it: `cost`, `blocked` or `command`
(`"open door;n"`). Returns whether there was the room, and the exit.

#### `map.remove_exit(from, direction)`, `map.remove_room(place)`

Remove one.

#### `map.clear()`

Forgets every room.

#### `map.tidy(area)`

Draws an area again from scratch, the player's if none is given, if that draws
it better. Returns how many rooms moved; none if it was left as it was, and
then, as a second value, `{now = {wrong_exits, stacked}, ["then"] = {wrong_exits, stacked}}`.

#### `map.undo()`

Undoes the last change the map made of itself. Returns whether there was one.

#### `map.show_area(area)`

Shows an area on the map, to look at it, without going.

### The map a MUD publishes

#### `map.import(address)`

Imports the map made with Mudlet at `address` (a `.dat`, or a zip with one; on
the web or on disk) in place of this one. What the player set stays: rooms
renamed, notes, marks, rooms to avoid and exits of their own, and the rooms the
map added.

#### `map.newer(address, fn)`

Finds out, without taking it, whether the map at `address` changed since it
was taken, and calls `fn()` if it did: to tell the player, rather than change
the map unasked.

#### `map.action(name, fn, options)`

Offers to do `fn()` with the map, chosen by `name` in the map's menu and the
palette, while the script is loaded; a plugin's under the plugin's name. `name`
can be given by language, as to `tr`. With `{checked = false}` it is a switch:
choosing it calls `fn(on)` with what it is now. `map.action(name)` takes it
away.

## Modules

### `require(name)`

Loads `name.luau`, or `name/init.luau`, from the folder of the script that asks
(or else from that of any other loaded), once, and returns what it returns. The
scripts of a plugin share their modules so.

```lua
-- util.luau
local util = {}
function util.percent(value, max) return math.floor(value * 100 / max) end
return util
```

```lua
-- another script, beside it
local util = require("util")
echo(`{util.percent(30, 120)} %`)
```

A module in a plugin's folder is also run as a script, as every `.luau` there
is: make a module only return what it has, and not make things.

## Mistakes, and checking a script

- A mistake is shown as `script.luau:12: what is wrong`, and kept for the
  editor. A call to what there is not says what: `there is no function
  gauges.sett`.
- An option a function does not take, something `on` does not know, a key that
  is not one or a colour that is not one are mistakes that say what there is.
- [`muddie.d.luau`](muddie.d.luau) has every function with its types, for an
  editor with luau-lsp (as its definitions file) or for `luau-analyze`: what is
  misspelt is seen as it is written, and, with `--!strict` at the top of the
  script, a line compared with a text.
- `muddie --debug` shows, on the standard output, everything that happens: the
  text (`TEXT`, a line; `PROMPT`, one the MUD marks; `PARTIAL`, what has no end
  yet), what is sent (`SENT`), GMCP and the other protocols, what scripts
  `log`, and their mistakes (`SCRIPT`). `muddie --debug | tee muddie.log` keeps
  it. Passwords are never shown.

## What there is not

On purpose, scripts cannot:

- Make windows, labels or bars of their own: a panel of text or a gauge, which
  are heard as well as seen, do that.
- Read or write files, start programs or download what they like: a script of
  someone else's must not be able to do harm. `vars` keeps data.
- Add their own commands or settings to the palette: aliases, keys and `vars`
  do that.
- Order triggers by priority: they run in the order they were made, and
  `wait` follows what comes in several lines.
- Keep data of their own on each room of the map.
