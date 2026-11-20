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
| `packages/<name>/init.luau` | In profiles that enable that plugin |
| `profiles/<profile>/scripts/*.luau` | In that profile, in name order |
| files given with `--script` | In every session, last |

Type `/reload` to start the scripts afresh after editing them, and
`/lua <code>` to run a line of Luau on the spot.

A script can be split into modules: `require("name")` loads `name.luau` or
`name/init.luau` from the folder of any loaded script, once, and returns what
the module returns.

## Sending and showing

| Function | What it does |
|---|---|
| `send(text)` | Sends a command to the MUD. Aliases are not applied. |
| `echo(text)` | Shows a line in the output, and speaks it if speech is on. |
| `speak(text, {interrupt = true})` | Says something through speech. |
| `chat(text)` | Adds a line to the chat panel. Colour codes are allowed. |
| `panel(name, text)` | Puts `text` in a panel, in place of what it showed; `panel(name)` clears it. For anything the server sends ready to display, such as a map it draws. The panel is made, as one of text, if there is none of that name; `"map"` means the map's panel whatever it is called, and `"chat"` the one of the channels. |
| `panel_add(name, text)` | Adds `text` at the end of a panel, which keeps what it had. In a trigger, `to_panel(name)` adds the line being looked at, with its colours. |
| `channel(name, text, options)` | Adds `text` as said on the channel `name` of the MUD, which has a tab of its own in a panel. `options.panel` names that panel: the one of that name is used as it is, and one is made if there is none; with no panel named, it is the panel of channels. `channel(name)` makes the tab before anyone speaks. |
| `gauge(name, value, max, options)` | Shows or updates a gauge; `gauge(name)` removes it. `options` may give a `colour` (a name or `#rrggbb`), a `style` (`"bar"`, `"cells"`, `"number"`, `"light"`, which is on when `value` is not 0 and takes the colour `off` while it is not, or `"text"`, which shows `options.text`), and `steps`, the colours it takes under each percentage: `{{60, "yellow"}, {30, "red"}}`. |
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
        {group = "survival", once = false})

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
| `group.enable(name)`, `group.disable(name)`, `group.toggle(name)` | Switch a group on or off: its triggers, aliases, key bindings and timers, whether made in a script or in the editor. For the session; what is saved is what the editor says. |
| `group.enabled(name)`, `group.list()` | Whether a group is on; the groups there are. |
| `enable(group, enabled)` | The same switch, as one function. |
| `after(seconds, fn, options)`, `every(seconds, fn, options)` | Timers. `{group = name}` makes one pass its turn while the group is off. |
| `cancel(id)` | Removes whatever the id belongs to; every function above returns one. |
| `bind(key, action, options)` | Runs a command or a function when a key is pressed: `bind("f5", "look")`, `bind("ctrl+shift+k", fn)`. With `{group = name}` the key does that only while the group is on, and can do something else in another group. `unbind(key)` removes it. |
| `separator(text)` | The text that separates commands typed on one line (`;` by default; doubled, it is typed literally). `nil` turns splitting off. |
| `vars` | The variables, as a table: `vars.target = "orc"` keeps a value, `vars.target` reads it, `vars.target = nil` forgets it, and `for name, value in vars do` goes through them. They are those of the editor, kept with the session's profile. A table kept in one must be assigned whole to be kept: `local bag = vars.bag; bag.gold = 5; vars.bag = bag`. |
| `expand(text)` | `text` with each `$name` replaced by that variable's value, `$name.key` by a field of a table and `$name.1` by an item of an array, as the commands of rules and key bindings are. |
| `remember(key, value)`, `recall(key)` | The same values, as functions. |

The options table takes `group` (a name to switch several on and off
together) and `once` (remove after firing).

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
bind("numpad8", "north")
bind("numpad2", "south")
bind("numpad4", "west")
bind("numpad6", "east")
bind("numpad9", "up")
bind("numpad3", "down")
bind("numpad5", "look")
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
| `supports(module, ...)` | Announces GMCP modules to the server besides the usual ones, as `"Name version"`: `supports("Map.Render 1")`. Some servers only send what was announced. |
| `on_event(name, fn)` | Calls `fn` when something happens in the client: `"connect"`, `"disconnect"`, `"mxp"` with `{name, value}` when the MUD names a value with MXP (a variable, an entity, or what a flagged element held, such as `RoomName`), or `"panel"` with `{columns, rows}`, the size in characters of the panel beside the output, once it stops changing. For a server that draws its map to fit. |
| `completions(name, words)` | Words for Tab to offer, besides those seen in the output. Each name keeps its own list; `completions(name)` removes it. |
| `mssp()` | The server's MSSP status as a table. |

## The map

The built-in loader maps from GMCP `Room.Info` and MSDP `ROOM`. For a MUD
that reports rooms some other way, or only in text, a script feeds the map
itself, and can turn the built-in loader off.

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
(direction to room id) and `x`, `y`, `z`. An id is a number, or the text the
server identifies the room with.

| Function | What it does |
|---|---|
| `map.visit(room)` | The player is now in `room`. |
| `map.set_room(room)` | Adds or updates a room without moving there. |
| `map.native(false)` | Turns the built-in loader off. |
| `map.persist(false)` | Keeps the map in memory only, for servers that send it themselves. |
| `map.clear()` | Forgets every room. |
| `map.room(id)` | A room as a table, the current one if `id` is omitted. Also has `weight` and `avoid`. |
| `map.find(text)` | Rooms whose name contains `text`. |
| `map.path(to, from)` | The commands to walk, or `nil`. |
| `map.walk(to)` | Walks there, one confirmed step at a time. |
| `map.set_exit(from, direction, to, options)` | Adds an exit of your own or changes where one leads. |
| `map.edit_exit(from, direction, options)` | Changes an exit's `cost`, `locked` or `command` (`"open door;n"`). |
| `map.exit(from, direction)` | An exit as a table. |
| `map.remove_exit(from, direction)`, `map.remove_room(id)` | Remove. |
| `map.move(id, x, y, z)` | Moves a room on the grid. |
| `map.set_weight(id, weight)`, `map.avoid(id, true)` | How routes treat a room. |
| `map.relayout(area)` | Draws an area again from scratch. |
| `map.areas()`, `map.id(text)` | Area names; the number used for a text id. |
