# Scripting muddie

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
| `packages/<name>/init.luau` | In profiles that enable the package |
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
| `panel(name, text)` | Shows text in a panel of its own beside the map; `panel(name)` removes it. For anything the server sends ready to display, such as a map it draws. |
| `gauge(name, value, max, options)` | Shows or updates a bar; `gauge(name)` removes it. `options` may give a `colour` (a name or `#rrggbb`), `style = "segments"` for a row of cells, and `low`, the percentage under which the numbers turn red. |
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
| `enable(group, enabled)` | Switches a group of triggers and aliases on or off. |
| `after(seconds, fn)`, `every(seconds, fn)` | Timers. |
| `cancel(id)` | Removes whatever the id belongs to; every function above returns one. |
| `bind(key, action)` | Runs a command or a function when a key is pressed: `bind("f5", "look")`, `bind("ctrl+shift+k", fn)`. `unbind(key)` removes it. |
| `separator(text)` | The text that separates commands typed on one line (`;` by default; doubled, it is typed literally). `nil` turns splitting off. |
| `remember(key, value)`, `recall(key)` | Values kept between sessions, per profile. |

The options table takes `group` (a name to switch several on and off
together) and `once` (remove after firing).

Keys are named in lower case with their modifiers in the order `ctrl`, `alt`,
`shift`: `f1`...`f35`, `ctrl+k`, `alt+shift+arrowup`. Of the main block, only
function keys and keys pressed with Ctrl or Alt can be bound, so typing is
never intercepted.

The numeric keypad has names of its own, whatever Num Lock says: `numpad0`
to `numpad9`, `numpadadd`, `numpadsubtract`, `numpadmultiply`,
`numpaddivide`, `numpaddecimal` and `numpadenter`. A bound keypad key no
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
