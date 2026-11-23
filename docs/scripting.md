# Scripting Muddie

Scripts are written in [Luau](https://luau.org), a fast, typed dialect of Lua
5.1. They run in a sandbox: a script cannot read or write files, start
programs or load code from outside the script folders, so a script someone
shares with you can do no harm to your computer. A script that runs for more
than half a second is stopped with an error, and the client carries on.

## Where scripts live

All paths are under the configuration folder: `~/.config/muddie` on Linux,
`%APPDATA%\muddie` on Windows.

| File | When it runs |
|---|---|
| `init.luau` | In every session |
| `profiles/<profile>/scripts/*.luau` | In that profile, in name order: those of its plugins in the folder of each |
| files given with `--script` | In every session, last |

In the palette (Ctrl+K), **Restart scripts** starts the scripts afresh
after editing them, and **Run Lua…** runs a line of Luau on the spot.

A script can be split into modules: `require("name")` loads `name.luau` or
`name/init.luau` from the folder of any loaded script, once, and returns what
the module returns.

A plugin is a git repository with its `.luau` scripts at the top; a
`rules.toml`, written as a profile's, with the rules it brings, of any kind,
each gauge with `panel`, the name of the panel it goes in; and a
`plugin.toml` that says what it is:

```toml
name = "My MUD"
about = { en = "Its gauges and its map.", es = "Sus indicadores y su mapa." }
default_on = false              # on in every profile until switched off
hosts = ["mymud.example.org"]   # offered when connecting to these…
names = ["my mud"]              # …or to a MUD that says it is called so
```

`name` and `about` are a text, or one by language as for `tr`; with no
`plugin.toml`, the plugin is called as its folder. Those that come with
Muddie are written the same way, in `crates/muddie/plugins`. Installed in a profile, its
scripts go in a folder of its name among the profile's, and its gauges in a
folder of that name among the profile's gauges.

## Sending and showing

### What a script keeps, and when it loses it

What a script holds in its variables lasts while it is loaded. It is kept
when the connection is lost and made again: scripts are not run again for
that, only told with `"disconnect"` and `"connect"`. It is kept when another
script, or a rule, is changed and saved: only what changed is run again. It
is lost when the script itself is run again, by saving it or with its run
button, when everything is run again (**Restart scripts**, another profile, a plugin
switched), and when Muddie is closed. A script that is run again starts
from its first line with nothing: that is where to ask a MUD for whatever
it had sent. What must outlast all of this is kept in `vars`.

What a script makes and sets goes with it when it is switched off or
removed, as when it is run again: its triggers, aliases, macros, timers,
handlers and gauges; the panels it put text in and the channels it opened,
emptied and closed; its sounds, stopped; and what it set of the map and the
session (`map.learn`, `map.directions`, `map.names`, `map.native`,
`map.persist`, `map.language`, `separator`, `merge_gmcp`, `support_gmcp`,
`report_msdp`, `completions`), back as they were before any script set them.

| Function | What it does |
|---|---|
| `send(text)` | Sends a command to the MUD. Aliases are not applied. |
| `echo(text)` | Shows a line in the output, and speaks it if speech is on. |
| `notify(text)` | Tells the user something of the script's, as the client tells its own: for a few seconds over the bottom right corner, and under the bell after; never in the output. |
| `tr{en = "Map", es = "Mapa"}` | The text in the language of the interface, of those given by language code: else that of its language without the country (`es` for `es_AR`), else English, else the first by code. For what a script shows the user, as a plugin that is shared does. |
| `log(text)` | Shows something on the standard output of a run started with `--debug` (see the manual); nothing otherwise. |
| `speak(text, {interrupt = true})` | Says something through speech. |
| `panel(name, text)` | Puts `text` in a panel, in place of what it showed; `panel(name)` clears it. For anything the server sends ready to display, such as a map it draws. The panel is made, as one of text, if there is none of that name; `"map"` means the map's panel whatever it is called, and `"channels"` the one of the channels. |
| `add_to_panel(name, text)` | Adds `text` at the end of a panel, which keeps what it had. In a trigger, `to_panel(name)` adds the line being looked at, with its colours. |
| `channel(name, text, options)` | Adds `text` as said on the channel `name` of the MUD, which has a tab of its own in a panel. `options.panel` names that panel: the one of that name is used as it is, and one is made if there is none; with no panel named, it is the panel of channels. `channel(name)` makes the tab before anyone speaks, and `close_channel(name)` takes it away, as when the character leaves the channel. |
| `gauge(name, value, max, options)` | Shows or updates a gauge; `gauge(name)` removes it. `options` are named as a gauge's in the editor's rules: `kind` (`"bar"`, `"cells"`, `"number"`, `"light"`, which is on when `value` is not 0, `"text"`, which shows `options.text`, or `"effects"`, which shows `options.items`: a list, or a table by name, of what the character is in, each a name or `{name = "Poisoned", about = "1 damage a second", colour = "red"}`), `colour` (a name or `#rrggbb`), `off_colour`, a light's while it is off, and `step`, the colours it takes under each percentage, `{{under = 60, colour = "yellow"}, {under = 30, colour = "red"}}`. Also `panel`, the panel of gauges it is shown in, by its name (made if there is none; the first there is if none is named), and `place` and `tags`, which switch it as they do a trigger. |
| `play(file, {volume = 0.8, loop = false})` | Plays a sound, by path or by name in the profile's `sounds` folder. |
| `stop_sounds()` | Stops every sound. |

## Triggers, aliases and timers

```lua
-- Patterns are regular expressions. m[0] is the whole match, m[1]... its
-- groups, and named groups are also m.name.
trigger("^(?<who>\\w+) tells you: (.*)$", function(m, line)
    highlight("bright yellow")              -- recolour what matched
    speak(`{m.who} says {m[2]}`, {interrupt = true})
end)

trigger("^You are hungry", function() send("eat bread") end,
        {place = "Survival/Eat", tags = {"survival"}})

trigger("^\\[spam\\]", function() gag() end) -- hide the line

alias("^k (.+)$", function(m) send("kill " .. m[1]) end)

local id = every(60, function() send("save") end)
after(5, function() cancel(id) end)
```

| Function | What it does |
|---|---|
| `trigger(pattern, fn, options)` | Calls `fn(m, line)` for each output line that matches. Unterminated prompts count once they stop changing. |
| `alias(pattern, fn, options)` | Calls `fn(m, input)` instead of sending typed input that matches. |
| `gag()` | In a trigger: hides the line. |
| `highlight(fg, bg)` | In a trigger: recolours the matched text. A colour is a name (`"red"`, `"bright blue"`), `"#rrggbb"` or a palette number. |
| `line_spans()` | In a trigger: the line as runs of text with `fg`, `bg`, `bold`, `italic`, `underline` and `inverse`. |
| `triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts` | Switch one thing of that kind by its place, or a folder with all it holds: `triggers.disable("Combat")`, `macros.enable("Combat/Kick")`, `scripts.toggle("Mapping")`. Each has `enable(place)`, `disable(place)`, `toggle(place)` (which returns whether it is now on), `enabled(place)` and `list()`, the places there are. Whether made in a script or in the editor; for the session, as what is saved is what the editor says. |
| `tags` | The same for tags: `tags.disable("combat")` switches off everything that carries the tag, of every kind. |
| `after(seconds, fn, options)`, `every(seconds, fn, options)` | Timers. Switched off, one lets its turns pass. |
| `cancel(id)` | Takes away what the id belongs to, and says whether there was something: `trigger`, `alias`, `after`, `every`, `macro`, `on_gmcp`, `on_msdp`, `on_var` and `on_event` each return one. |
| `macro(key, action, options)` | Runs commands or a function when a key is pressed: `macro("f5", "look")`, `macro("ctrl+shift+k", fn)`. Commands are sent as a rule's are: separated by `;`, with `$name` put in, not through aliases. A key bound again under another `place` does that while it is on and those bound before are not: the first bound that is on is the one that runs. Bound again under the same place, it does the new thing instead. |
| `separator(text)` | The text that separates commands typed on one line (`;` by default; doubled, it is typed literally). `separator(false)` turns splitting off, and `separator()` gives back `;`. |
| `vars` | The variables, as a table: `vars.target = "orc"` keeps a value, `vars.target` reads it, `vars.target = nil` forgets it, and `for name, value in vars do` goes through them. They are those of the editor, kept with the session's profile. A table kept in one must be assigned whole to be kept: `local bag = vars.bag; bag.gold = 5; vars.bag = bag`. |
| `expand(text)` | `text` with each `$name` replaced by that variable's value, `$name.key` by a field of a table and `$name.1` by an item of an array, as the commands of rules are. |

The options table takes `place`, as `Folder/Name`, to switch it by;
`tags`, a list of tags (or one alone) that switch it with everything else
that carries one of them; and, for triggers and aliases, `once`, to remove it
after it fires. A rule of the editor has as its place its folder and its
name, or what it is made of if it has none: `Combat/Fight back`; two alike in
one folder are told apart by a number, `Combat/Fight back #2`. A script's
place is where it is among the profile's scripts, with no extension:
`Mapping/rooms`. Something runs while its place, every folder above it and
every tag it carries are switched on.

Keys are named in lower case with their modifiers in the order `ctrl`, `alt`,
`shift`: `f1`...`f35`, `ctrl+k`, `alt+shift+arrowup`. Of the main block, only
function keys and keys pressed with Ctrl or Alt can be bound, so typing is
never intercepted.

The numeric keypad has names of its own: `numpad0` to `numpad9`,
`numpadadd`, `numpadsubtract`, `numpadmultiply`, `numpaddivide`,
`numpaddecimal` and `numpadenter`. With Num Lock off the digits and the point
are other keys, named by what is then written on them: `numpadup`,
`numpaddown`, `numpadleft`, `numpadright`, `numpadhome`, `numpadend`,
`numpadpageup`, `numpadpagedown`, `numpadclear` (the 5), `numpadinsert` and
`numpaddelete`. So the keypad can walk with Num Lock off and type numbers
with it on, or do one thing in each. A bound keypad key no
longer types its digit, which makes the keypad a good place for movement:

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad9", "up")
macro("numpad3", "down")
macro("numpad5", "look")
```

## Data from the server

```lua
on_gmcp("Char.Vitals", function(vitals)
    gauge("HP", vitals.hp, vitals.maxhp)
end)

on_msdp("HEALTH", function(value) echo("health " .. value) end)
send_msdp("REPORT", "HEALTH", "MANA")
```

| Function | What it does |
|---|---|
| `on_gmcp(package, fn)` | Calls `fn(data, package)` for that package and those under it (`"Char"` covers `Char.Vitals`). |
| `gmcp(package)` | The data last received for exactly that package. |
| `send_gmcp(package, data)` | Sends a GMCP message. |
| `on_msdp(variable, fn)`, `msdp(variable)`, `send_msdp(command, ...)` | The same for MSDP. Values arrive as strings. |
| `on_var(name, fn)` | Calls `fn(value, name)` when a variable of `vars` changes, by a script, a trigger or the editor: once the script that changed it is done, so it sees all it changed. A gauge can follow a variable the same way. |
| `merge_gmcp(package, ...)` | For packages the server sends only what changed of, as `Char.Status` with only the life that changed: what comes is merged into what `gmcp(package)` and the gauges keep, tables inside merged too. Handlers still get what came. |
| `report_msdp(variable, ...)` | Asks the server to report MSDP variables as they change: `report_msdp("HEALTH", "HEALTH_MAX")`. Asked for while the script is loaded, as `support_gmcp`. |
| `support_gmcp(module, ...)` | Announces GMCP modules to the server besides the client's own (`Core` and `Client.Media`), as `"Name version"`: `support_gmcp("Map.Render 1")`. Some servers only send what was announced. They are asked for while the script is loaded: when it is switched off or removed, the server is told it no longer wants them. |
| `on_event(name, fn)` | Calls `fn(data, name)` when something happens in the client: `"connect"`, `"disconnect"`, `"mxp"` with `{name, value}` when the MUD names a value with MXP (a variable, an entity, or what a flagged element held, such as `RoomName`), or `"panel"` with `{name, map, columns, rows}`: the size in characters of a panel that shows a drawing, once it stops changing, after connecting and after the script is run. `map` is true for the map's panel, whatever it is called. |
| `panel_size(name)` | The size in characters of a panel, as `{columns, rows}`, at once; `nil` if it has not been on the screen. With no name, the map's panel. |
| `completions(name, words)` | Words for Tab to offer, besides those seen in the output. Each name keeps its own list; `completions(name)` removes it. |
| `mssp()` | The server's MSSP status as a table. |

Each `on_` function takes, last, options with `tags`: while one of its tags
is off, it is told nothing. An option a function does not take is an error,
rather than left out without a word.

## The map

The **Standard GMCP and MSDP** plugin maps from GMCP `Room.Info` and MSDP
`ROOM`. For a MUD that reports rooms some other way, or only in text, a
script feeds the map itself, and can turn the standard loader off.

```lua
-- A MUD with no GMCP: read the room from its title and exits lines.
map.native(false)
local title
trigger("^\\[ (.+) \\]$", function(m) title = m[1] end)
trigger("^Exits: (.+)$", function(m)
    local exits = {}
    for direction, to in m[1]:gmatch("(%a+)=(%a+)") do exits[direction] = to end
    map.visit({id = title, name = title, area = "town", exits = exits})
end)
```

A room is a table with `id` and, optionally, `name`, `area`, `exits`
(direction to room id), `x`, `y`, `z` and `colour` (a name, `"#rrggbb"` or a
palette number, as for `highlight`; a room given without one keeps the
colour it had). An id is a number, or the text the
server identifies the room with. What a MUD sends in `Room.Info` or `ROOM`
can be given as it comes: `num` or `vnum` for `id`, `zone` for `area`,
`coords` for where it is, in any letter case.

| Function | What it does |
|---|---|
| `map.visit(room)` | The player is now in `room`, which is added or updated. Returns its number. |
| `map.set_room(room)` | Adds or updates a room without moving there. Returns its number. |
| `map.native(false)` | Turns the standard loader off while the script is loaded; `map.native()` says whether it is on. |
| `map.persist(false)` | Keeps the map in memory only, for servers that send it themselves. |
| `map.clear()` | Forgets every room. |
| `map.room(id)` | A room as a table, the current one if `id` is omitted. Also has `exits` (direction to room number), `cost`, what entering it costs a route besides the move, as an exit's `cost`, `avoid`, `colour` (`"#rrggbb"`, or `nil`), `note`, what the user wrote of it (`""` for none), `renamed`, whether its name is one given in place of the map's, `added`, whether the map added it of itself (see `map.add`), and `shape` and `symbol`, how it is drawn. |
| `map.find(text, {exact = true})` | The numbers of the rooms whose name contains `text`; with `exact`, of those called just `text`, as the MUD names them. |
| `map.way(to, from)` | The commands to walk, or `nil`. A place is a room's id, or a mark or name as typed in **Go to…**, the nearest of several. |
| `map.go(to)` | Goes there, one confirmed move at a time. `map.stop()` stops, as **Stop going** does. |
| `map.failed()` | The server said a move failed: the moves sent did not happen, and a walk stops. |
| `map.arrive{name, exits, came}` | For servers that do not say which room it is: the player is in a room of this name and exits, reached by the exit `came`. Returns the room's number, or `nil` while unsure. |
| `map.here(id)` | The player is in that room, one the map has. Returns whether it has it. |
| `map.learn(true)` | For servers that do not say which room it is, has the map learn as the player goes from the rooms `map.arrive` is given, never on one sighting alone, nor from a room it does not know for certain the player was in: it adds the rooms it has not where the player goes by the compass, as `map.add` does; puts right the names of rooms it followed the player through by the way and the exits though it named them otherwise, once the next move is to a room as it has it; adds the exits a room shows that it lacks, leading to rooms not explored; and has an exit that led to a room beside the one it had lead there, once the next move bears it out. Exits it has that are not shown are only counted, for **Check the map**. Each change is told and undone as one of the map's own. Off unless asked for; `false` switches it off. |
| `map.candidates()` | The numbers of the rooms the player may be in while the map cannot tell which; empty when it can, or has no idea. |
| `map.add{name, exits, came}` | For servers that do not say which room it is, when the player went by `came` where the map has no room: finds the room among those the map has, or makes it, and has the exit lead there and the player be there. Returns the room's number and `"found"` or `"made"`, or `nil` and why not: `"lost"`, the map does not know for certain which room the player was in; `"known"`, the exit leads to a room already; `"alike"`, rooms the map has could be the one and it cannot tell which; `"unnamed"`. By a compass exit the room is looked for that way; by any other, as a portal, which could lead anywhere, any room with that name and exits refuses it. A room made is drawn the way of the exit, or beside the room left; its exits lead to unexplored rooms until taken, the way back too. The user can undo the rooms added, the last first, and taking the map again keeps them, or joins each with the room the map taken has, once it has it. |
| `map.language("es")` | The language the MUD is played in, by its code or name: the map takes its words for directions, and exits kept in them, as a map taken from Mudlet may keep "fu" for out, become those directions. A profile's own choice comes first. |
| `map.directions{n = {"nte"}, ...}` | Words of the MUD's own for directions, over those of its language; the first of each is what walks send from then on. |
| `map.names{region = ":", exits = "[]"}` | How the MUD names its rooms: the region first, up to `region`, and the exits after, between the characters of `exits`. Unless taught, the region is the area and names are compared whole; `map.names()` goes back to that. |
| `map.import(address)` | Takes the map made with Mudlet at `address` (a `.dat`, or a zip with one; web or disk) in place of this one; one taken from there before, only if it changed, or if this version of Muddie reads more of Mudlet's maps than the one that took it. What the user set stays: rooms renamed, notes, marks, rooms to avoid and exits of their own; and so do the rooms the map added, each joined with the room the map taken has where it has it, reached from a room by the same exit, with the same name and exits. |
| `map.newer(address, fn)` | Finds out, without taking it, whether the map at `address` changed since the map was taken from there, and calls `fn()` if it did: to tell the user, rather than change the map unasked. |
| `map.action(name, fn, {checked = false})` | Offers to do `fn()` with the map, chosen by `name` in the map's menu and the palette, while the script is loaded. `name` can be given by language, as to `tr`, and is shown in the language the interface is in at the time; the menu shows them under the name of the script, a plugin's under the plugin's. With `checked`, it is a switch: the menu ticks it while on, the palette says whether it is, and choosing it switches it and calls `fn(on)` with what it is now. `map.action(name)` takes it away. |
| `map.count()` | How many rooms the map has. |
| `map.set_exit(from, direction, to, options)` | Adds an exit of your own, or changes where one leads; with `to` `nil`, changes only what `options` give of the exit there: `cost`, `blocked` or `command` (`"open door;n"`). Returns whether there was the room, and the exit. |
| `map.exit(from, direction)` | An exit as a table: `to`, `cost`, `blocked` (kept off routes), `command` and `custom`, whether it is one of your own. |
| `map.direction(word)` | The direction a word says, as the map keeps it (`"n"`, `"ne"`... `"u"`, `"d"`, `"in"`, `"out"`), in the MUD's words or in English; `nil` if it says none. |
| `map.remove_exit(from, direction)`, `map.remove_room(id)` | Remove. |
| `map.marks()` | The marks, by name, each with its room's id. |
| `map.mark(name, id)` | Marks a room with `name`, as **Mark…** does; `map.mark(name)` forgets the mark. Returns whether there was the room, or the mark. |
| `map.edit_room(id, changes)` | Changes what `changes` gives of a room, leaving the rest: `name`, in place of the one it has, such as one taken wrong, which the server reporting it or the map taken again leaves (the room's table says `renamed`); `note`, its note, empty for none; `cost` and `avoid`, how routes treat it; `x`, `y`, `z`, where it is; `colour`, `shape` (`"square"`, `"circle"`, `"diamond"` or `"hexagon"`) and `symbol`, a character or two written in it, how it is drawn, which are kept so too. `false` gives back the map's own colour or shape, no cost, or no symbol. Returns whether there was the room. |
| `map.tidy(area)` | Draws an area again from scratch, the player's if none is given, if that draws it better: no more exits pointing the wrong way, nor rooms on another's cell, than now. Returns how many rooms moved; none if it was left as it was, and then, as a second value, how it is drawn and how it would be: `{now = {wrong_exits, stacked}, ["then"] = {wrong_exits, stacked}}`. |
| `map.undo()` | Undoes the last change the map made of itself, as **Undo the map's last change** does. Returns whether there was one. |
| `map.show_area(area)` | Shows an area on the map, to look at it, without going, as **Show an area…** does. |
| `map.areas()`, `map.id(text)` | Area names; the number used for a text id. |
