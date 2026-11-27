# Making a Muddie plugin

Español: [plugins.es.md](plugins.es.md). Every function a plugin can use is in
[scripting.md](scripting.md); their types, in [muddie.d.luau](muddie.d.luau).

A plugin teaches Muddie the ways of one MUD: its gauges, its channels, its map,
what is worth saying aloud. Muddie itself knows no MUD in particular; all of
that is plugins. A player switches a plugin on, or accepts it when Muddie
offers it on connecting, and it sets everything up by itself.

## Contents

1. [What a plugin is](#what-a-plugin-is)
2. [A plugin in five minutes](#a-plugin-in-five-minutes)
3. [Trying it as you write it](#trying-it-as-you-write-it)
4. [The files](#the-files)
5. [How Muddie installs and updates it](#how-muddie-installs-and-updates-it)
6. [Writing it well](#writing-it-well)
7. [Recipes](#recipes)
8. [Publishing it](#publishing-it)
9. [Writing a plugin with an AI](#writing-a-plugin-with-an-ai)

## What a plugin is

A folder, whose name is the plugin's id, with nothing but these files:

| File | What it is |
|---|---|
| `plugin.toml` | What it is: its name, what it does, which MUDs it is for |
| `*.luau` | Its scripts, at least one |
| `rules.toml` | The rules it brings, as the editor's: gauges, triggers, aliases, keys, timers. Optional |
| `template.toml` | A profile ready to play the MUD, for **New profile**. Optional |

There are three kinds, all written alike:

- **Included** in Muddie: the **Standard GMCP and MSDP** plugin, for what many
  MUDs send the same way.
- **Official**: those of the [muddie-plugins](https://github.com/midnattsol/muddie-plugins)
  repository, which Muddie downloads and keeps up to date by itself.
- **From git**: any repository with a plugin at its top, which a player
  installs with **Install a plugin…** in the palette and its address.

## A plugin in five minutes

A plugin for a MUD called Example, at `mud.example.org`, that sends life and
energy over GMCP in `Char.Vitals`, and messages of its channels in
`Comm.Message`. The folder is `example-mud/`.

`plugin.toml`:

```toml
name = "Example MUD"
about = { en = "Its gauges and channels.", es = "Sus indicadores y canales." }
hosts = ["mud.example.org"]
names = ["example"]
muddie = "0.5.0"
```

`rules.toml`, with its gauges: they follow what the MUD sends by themselves,
and the player can change them in the editor as their own.

```toml
[[gauge]]
name = "Life"
value = "Char.Vitals.hp"
max = "Char.Vitals.maxhp"
kind = "bar"
colour = "#5fb85f"
steps = [{ under = 60, colour = "#e0c040" }, { under = 30, colour = "#e05050" }]
aloud = "lowest"
panel = "Points"

[[gauge]]
name = "Energy"
value = "Char.Vitals.ep"
max = "Char.Vitals.maxep"
kind = "bar"
colour = "#5f8fd8"
aloud = "wait"
panel = "Points"
```

`example.luau`, what it does:

```lua
-- Example MUD: its channels, each in its tab. Its gauges are in rules.toml.
gmcp.support("Char 1", "Comm 1")

on("gmcp", "Comm.Message", function(message)
    if type(message) == "table" and message.channel and message.text then
        channels.say(message.channel, message.text)
    end
end)
```

`template.toml`, so that **New profile** offers the MUD ready to play:

```toml
name = "Example MUD"
host = "mud.example.org"
port = 4000
plugins = ["example-mud"]
```

That is all. On connecting to `mud.example.org`, Muddie offers the plugin;
switched on, the gauges are shown and heard as they fall, and each channel has
its tab, read aloud after its name.

## Trying it as you write it

1. Put the folder in the `plugins` folder of the configuration folder:
   `~/.config/muddie/plugins/example-mud/` on Linux,
   `%APPDATA%\muddie\plugins\example-mud\` on Windows. Muddie takes it as one
   from git.
2. Switch it on in the profile's **Plugins** tab, in the Profiles window. Its
   scripts are installed in the profile, in **Scripts**, in a folder of its
   name; its gauges in **Gauges**.
3. After changing the plugin's files, **Install again**, in its menu in the
   **Plugins** tab, puts it back as it now is. To try a change faster, change
   the installed copy in the editor and run it; then carry it to the plugin.

To see what it does:

- `session.feed("text")`, in **Run Luau…** of the palette, takes the text as if
  the MUD had sent it: to try a trigger without the MUD.
- `muddie --debug | tee muddie.log` shows everything that happens: the text
  (`TEXT`, `PROMPT`, `PARTIAL`), what is sent, each GMCP message (`GMCP<`),
  what scripts `log`, and their mistakes (`SCRIPT`). It is the first thing to
  look at to learn what a MUD sends, and to find what goes wrong.
- A mistake in a script is shown over the corner and kept in the editor, with
  its file and line.
- [muddie.d.luau](muddie.d.luau) has every function with its types: given to
  an editor with luau-lsp, or to `luau-analyze`, a misspelt name is seen as it
  is written.

## The files

### `plugin.toml`

```toml
name = { en = "Example MUD", es = "Ejemplo" }   # a text, or one by language
about = "What it does, in a sentence."          # the same
hosts = ["mud.example.org"]   # part of the address of the MUDs it is for
names = ["example"]           # or the names they give themselves (MSSP)
default_on = false            # on in every profile until switched off
muddie = "0.5.0"              # the oldest Muddie it works with
```

- `name` and `about` are shown in the **Plugins** tab and in the offer. A text,
  or a table by language code (`en`, `es`...), as [`tr`](scripting.md#trtexts)
  takes; the English name is the folder it is installed in, in each profile.
- `hosts` and `names` say when to offer it: when the address contains one of
  `hosts`, or the MUD says over MSSP that it is called one of `names`.
- `default_on`: on in every profile unless switched off. For a plugin of one
  MUD, leave it off: it is offered when that MUD is connected to.
- `muddie`: a Muddie older than this keeps the version of the plugin it has.
- Nothing else: anything else is a mistake, and the plugin is not taken.

### `rules.toml`

The rules it brings, written as a profile's own `rules.toml`. Each goes in a
folder named as the plugin; `folder = "Combat"` puts it in a folder inside
that one. Every kind takes `name`, `folder`, `tags` and `enabled`.

A gauge, `[[gauge]]`:

| Field | What it is |
|---|---|
| `name` | Its name, as shown and said |
| `value` | Where its value is: a GMCP path (`Char.Vitals.hp`), an MSDP variable or a variable of `vars` |
| `max` | Where its maximum is, or a number |
| `kind` | `"bar"`, `"cells"`, `"number"`, `"light"`, `"text"` or `"effects"` |
| `colour`, `off_colour` | A name or `#rrggbb` |
| `steps` | `[{ under = 30, colour = "#e05050" }]`: the colour under each percentage |
| `aloud` | `"usual"`, `"lowest"`, `"wait"`, `"cut"` or `"never"`, as in [`gauges.set`](scripting.md#gaugessetname-gauge) |
| `always` | `true` to be heard with the game not read aloud |
| `panel` | The panel of gauges it is shown in, by name: made if there is none |

A trigger, `[[trigger]]`: `pattern` (text the line contains, or a regular
expression with `regex = true`), and what it does: `send` (commands, one per
line or separated by `;`, with `%1`... for what matched and `$name` for
variables), `speak`, `sound`, `highlight` (a colour), `panel` (shows the line
there), `gag = true` (hides it), `silence = true` (shows it and does not read
it), or `code` with `luau = true` (Luau, given `m` and `line` as a trigger's
function is).

An alias, `[[alias]]`: `pattern` (the first word of what is typed, or a
regular expression with `regex = true`), and `send` or `code`. A key,
`[[macro]]`: `key` (`"f5"`, `"ctrl+k"`, `"numpad8"`), and `send` or `code`. A
timer, `[[timer]]`: `seconds`, `repeat = true` to go again and again, and
`send` or `code`.

What is better done in a script, do in a script: rules are for what the player
will want to change in the editor, as the colours and the steps of a gauge.

### `template.toml`

```toml
name = "Example MUD"        # the game, which the profile is named after
host = "mud.example.org"
port = 4000
tls = false
encoding = "utf-8"          # optional: as the MUD says, if not given
language = "es"             # optional: the language the game is played in
plugins = ["example-mud"]   # the plugins switched on, by their ids
```

**New profile** offers it among the MUDs to play.

### The scripts

- **Name them in English, with no spaces**: `example.luau`, `aloud.luau`,
  `prompt.luau`. They are run in the order of their names.
- They are installed in the player's profile, in a folder of the plugin's
  name, to read and change in the editor. Switching the plugin off switches
  them off.
- **Modules**: `require("prompt")` loads `prompt.luau` from the plugin's own
  folder, once, for all its scripts. Every `.luau` there is also run as a
  script, so a module only returns what it has.
- **Keep what is heard in a script of its own**, `aloud.luau`, which the
  player can switch off alone and keep what is shown (see
  [Writing it well](#writing-it-well)).

## How Muddie installs and updates it

- **Switched on** in a profile, the plugin is installed in it: its scripts in a
  folder of its name among the profile's scripts, its rules among the
  profile's, in a folder of its name; its gauges in the panels they name.
  They are the player's to change.
- **Switched off**, those folders are switched off; switched on again, what the
  player switched off in them stays so.
- **A new version**, official or from git, is installed by itself as the
  profile is opened, never in the middle of a game. What it had before and no
  longer has goes, scripts and rules; a script the player changed is kept
  beside it, as `name.luau.before-update`; what the player added stays.
- **Install again**, in its menu, puts back what it installed as it comes.
- In a session with no profile, it runs as it comes, until the session ends.

## Writing it well

- **It does everything by itself.** Switched on, the plugin asks the MUD for
  what it needs (`gmcp.support`), sets its gauges and panels, and tells the
  player, once, anything they must do in the game. Nothing to set up by hand.
- **Data, not screen.** What the MUD says goes to data (gauges, channels,
  the map, `vars`); a panel is only one way to see it. Who listens gets the
  same: gauges are asked for with Alt+0 and said as they change.
- **What is heard comes from the data, not from what is shown.** Write it in
  `aloud.luau`, listening to what the MUD sends (`on("gmcp")`, triggers), never
  to a gauge or a panel the player may delete.
- **Little noise.** Say what changes, not all again: the state that comes and
  the one that goes, not all of them. Say what matters with `aloud = "cut"`,
  the rest after what is being read. Use `line:mute()` or `line:read(shorter)`
  for lines that say too much. Leave `always = true` for what is urgent.
- **Every text in English and Spanish**, by language: `notify{en = ..., es = ...}`,
  `tr{...}`. The player sees them in the language of the interface.
- **Tell a thing once.** What the player must know (a module to switch on in
  the game, a newer map) is said once, and remembered in `vars` if it is to
  stay said.
- **Remember the player's choices in `vars`**, under names of the plugin's own:
  `vars.example_learning`.
- **Nothing of the MUD in Muddie.** If the plugin needs something Muddie does
  not have, that is a function of the API to ask for, of use to every MUD.

## Recipes

### Gauges from GMCP

In `rules.toml`, as in [the example](#a-plugin-in-five-minutes). A package
that sends only what changed (only the life, when only the life changed) is
merged with what came before, for the gauges to have it whole:

```lua
gmcp.merge("Char.Status")
```

### The states the character is in, as a gauge of effects

```lua
local states = {}

local function show()
    local items = {}
    for _, state in states do
        table.insert(items, {name = state.name, about = state.about})
    end
    gauges.set("States", {kind = "effects", items = items, aloud = "never",
                          panel = "States", place = "Example MUD/States"})
end

on("gmcp", "Char.States", function(data)
    states = {}
    for _, state in data do
        table.insert(states, {name = state.name, about = state.help})
    end
    show()
end)
```

And what changes of them, in `aloud.luau`, from the same data:

```lua
local known = nil

on("gmcp", "Char.States", function(data)
    local now = {}
    for _, state in data do now[state.name] = true end
    if known then
        for name in now do
            if not known[name] then speak(`{name}.`) end
        end
        for name in known do
            if not now[name] then speak({en = `No longer: {name}.`, es = `Ya no: {name}.`}) end
        end
    end
    known = now
end)

on("disconnect", function() known = nil end)
```

### Channels

```lua
on("gmcp", "Comm.Message", function(message)
    channels.say(message.channel, message.text)
end)
-- What was said before the player came: kept, not read.
on("gmcp", "Comm.History", function(message)
    channels.say(message.channel, message.text, {aloud = "never"})
end)
```

### The map

If the MUD sends `Room.Info` as most do, the Standard plugin maps it. If it
says which room by GMCP of its own:

```lua
map.native = false
on("gmcp", "Room.Info", function(room)
    map.visit({id = room.vnum, name = room.name, area = room.zone, exits = room.exits})
end)
```

If it only writes the room in its text, the map follows by name and exits,
and can learn what it lacks:

```lua
map.language = "es"
map.names = {region = ":", exits = "[]"}
map.learn = vars.example_learning ~= false

trigger("^(.+) \\[([a-z,]+)\\]$", function(m)
    local exits = {}
    for exit in m[2]:gmatch("[^,]+") do table.insert(exits, exit) end
    map.arrive({name = m[1], exits = exits})
end)
```

A choice for the player, in the map's menu and the palette, remembered:

```lua
map.action({en = "Learn the map as you go", es = "Aprender el mapa sobre la marcha"},
    function(on)
        vars.example_learning = on
        map.learn = on
    end,
    {checked = map.learn})
```

### The prompt

Keep it from being read, and read what comes after it on its line:

```lua
trigger("^HP:\\d+ SP:\\d+> ?(.*)$", function(m, line)
    if m[1] == "" then line:mute() else line:read(m[1]) end
end)
```

When each player makes their own prompt, learn it as the game says it, keep it
in `vars`, and make the pattern from it. The plugin of Reinos de Leyenda, in
muddie-plugins, does so in its `prompt.luau`.

### Telling the player once

```lua
on("connect", function()
    after(90, function()
        if not gmcp.get("Char.Vitals") and not vars.example_gmcp_told then
            vars.example_gmcp_told = true
            notify({en = "Type 'gmcp on' in the game for your gauges.",
                    es = "Escribe 'gmcp on' en el juego para tus indicadores."})
        end
    end)
end)
```

## Publishing it

- **In a repository of your own**: put the plugin's files at its top. A player
  installs it with **Install a plugin…**, in the palette, and its address;
  **Update a plugin…** brings it up to date.
- **As an official plugin**: open a pull request to
  [muddie-plugins](https://github.com/midnattsol/muddie-plugins), with the
  plugin in a folder of its id. There it must have `rules.toml` and
  `template.toml` too, and no folders inside. Its tools check it before it
  goes in:

  ```sh
  python3 tools/check.py   # the files of every plugin
  python3 tools/api.py     # the use of the API, with luau-analyze
  python3 tools/pack.py    # plugins.zip, as Muddie downloads it
  ```

  Once merged, a release is made with release-please, and every Muddie takes
  it by itself.

## Writing a plugin with an AI

An AI can write a plugin with these files and a session of the MUD. Give it:

1. This guide, [plugins.md](plugins.md).
2. The reference, [scripting.md](scripting.md), and the types,
   [muddie.d.luau](muddie.d.luau).
3. **A log of a real session**, `muddie --debug | tee session.log`: logging
   in, walking, a fight, the channels. It shows what the MUD sends, line by
   line and over GMCP, which no description replaces. Take out your password
   if you typed it where the game shows it.
4. The plugin of Reinos de Leyenda, in muddie-plugins, as an example of a
   whole one.

And ask it, for example:

> Make a Muddie plugin for the MUD in session.log, following plugins.md and
> scripting.md, using only the functions in muddie.d.luau. Gauges for what it
> sends over GMCP, its channels each in a tab, the map following the player,
> and, in aloud.luau, only what changes said aloud. Every text in English and
> Spanish. Give me each file whole.

Then try it as in [Trying it as you write it](#trying-it-as-you-write-it), and
give it back what goes wrong: the mistake as Muddie shows it, and the lines of
the log around it. Check before using it:

- [ ] `plugin.toml` has only the fields above, and `muddie = "0.5.0"` or newer.
- [ ] Scripts named in English, with no spaces; what is heard in `aloud.luau`.
- [ ] Only functions of `muddie.d.luau`; `luau-analyze` finds nothing misspelt.
- [ ] Nothing said twice: what a gauge says is not said again by a script.
- [ ] Every text the player sees in English and Spanish.
- [ ] Switched on in a new profile, it works with nothing done by hand.
