# Making a Muddie plugin

Español: [plugins.es.md](plugins.es.md).

This guide takes you from nothing to a plugin that other players can install.
You need no earlier experience with plugins. When you want to look up a
function, go to [scripting.md](scripting.md). When you want the exact types,
go to [muddie.d.luau](muddie.d.luau).

## Contents

1. [What a plugin is for](#what-a-plugin-is-for)
2. [Your first plugin](#your-first-plugin)
3. [Try it while you write it](#try-it-while-you-write-it)
4. [The files in detail](#the-files-in-detail)
5. [How Muddie installs and updates it](#how-muddie-installs-and-updates-it)
6. [Doing it well](#doing-it-well)
7. [Recipes](#recipes)
8. [Publishing it](#publishing-it)
9. [Writing a plugin with an AI](#writing-a-plugin-with-an-ai)

## What a plugin is for

A plugin teaches Muddie how one MUD works. It brings that MUD's gauges, its
channels, its map, and what is worth saying aloud. Muddie itself knows no MUD
in particular, so all of this lives in plugins.

Make a plugin when you want to share that work with every player of the MUD.
A few scripts in your own profile serve only you. A plugin sets everything up
by itself: the player switches it on, or says yes when Muddie offers it on
connecting, and nothing else is needed.

### What is inside

A plugin is a folder. The folder's name is the plugin's id. It holds these
files and nothing else:

| File | What it holds |
|---|---|
| `plugin.toml` | The plugin's name, what it does, and which MUDs it is for |
| `*.luau` | Its scripts. You need at least one |
| `rules.toml` | Optional. Gauges, triggers, aliases, keys and timers, the same rules the editor makes |
| `template.toml` | Optional. A profile ready to play the MUD, offered in **New profile** |

### The three kinds

All three kinds are written the same way. They differ in where they come from:

- **Included**: Muddie comes with the **Standard GMCP and MSDP** plugin. It
  handles what many MUDs send in the same way.
- **Official**: these live in the
  [muddie-plugins](https://github.com/midnattsol/muddie-plugins) repository.
  Muddie downloads them and keeps them up to date by itself.
- **From git**: any repository with a plugin at its top. The player installs
  it with **Install a plugin…** in the palette, giving the repository's
  address.

## Your first plugin

This section builds a whole plugin in four files. Copy it, change the names,
and you have a starting point for your own MUD.

The example MUD is called Example and lives at `mud.example.org`. It sends
life and energy over GMCP in `Char.Vitals`. It sends channel messages in
`Comm.Message`. The plugin's folder is `example-mud/`.

**Step 1.** Write `plugin.toml`. It says what the plugin is and when to offer it:

```toml
name = "Example MUD"
about = { en = "Its gauges and channels.", es = "Sus indicadores y canales." }
hosts = ["mud.example.org"]
names = ["example"]
muddie = "0.5.0"
```

**Step 2.** Write `rules.toml` with the gauges. Each gauge follows what the
MUD sends without any code. The player can change these gauges in the editor as if
they had made them.

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

**Step 3.** Write `example.luau`. It asks the MUD for GMCP and puts each channel in its
own tab:

```lua
-- Example MUD: its channels, each in its tab. Its gauges are in rules.toml.
gmcp.support("Char 1", "Comm 1")

on("gmcp", "Comm.Message", function(message)
    if type(message) == "table" and message.channel and message.text then
        channels.say(message.channel, message.text)
    end
end)
```

**Step 4.** Write `template.toml`. With it, **New profile** offers the MUD ready to
play:

```toml
name = "Example MUD"
host = "mud.example.org"
port = 4000
plugins = ["example-mud"]
```

That is all. When a player connects to `mud.example.org`, Muddie offers the
plugin. Once it is on, the player sees the gauges and hears them when they
fall. Each channel gets its own tab, and Muddie reads each message aloud after
the channel's name.

## Try it while you write it

You can run your plugin in Muddie long before you publish it. Follow these
steps:

1. Put the plugin's folder inside the `plugins` folder of Muddie's
   configuration folder. On Linux that is `~/.config/muddie/plugins/example-mud/`.
   On Windows it is `%APPDATA%\muddie\plugins\example-mud\`. Muddie treats it
   as a plugin from git.
2. Open the Profiles window, go to the profile's **Plugins** tab, and switch
   the plugin on. Its scripts appear in **Scripts**, in a folder with its
   name. Its gauges appear in **Gauges**.
3. Each time you change the plugin's files, open its menu in the **Plugins**
   tab and choose **Install again**. The profile then gets the files as they
   are now.

For quicker tries, change the installed copy in the editor and run it there.
When it works, copy the change back to the plugin's folder.

### Seeing what happens

These tools show you what the MUD sends and what your plugin does:

- **`session.feed("text")`** pretends the MUD sent that text. Type it in
  **Run Luau…** in the palette. Use it to try a trigger without connecting.
- **`muddie --debug | tee muddie.log`** prints everything that happens. You
  see the text (`TEXT`, `PROMPT`, `PARTIAL`), what is sent, each GMCP message
  (`GMCP<`), what scripts write with `log`, and script mistakes (`SCRIPT`).
  Look here first to learn what a MUD sends and to find what goes wrong.
- **Script mistakes** show up in the corner. The editor keeps them too, with
  the file and the line.
- **[muddie.d.luau](muddie.d.luau)** lists every function with its types.
  Give it to an editor with luau-lsp, or to `luau-analyze`. A misspelt name is
  then marked as soon as you write it.

## The files in detail

This section explains every field of every file. Come back to it when the
first plugin is not enough.

### `plugin.toml`

This file tells Muddie what the plugin is and when to offer it.

```toml
name = { en = "Example MUD", es = "Ejemplo" }   # a text, or one by language
about = "What it does, in a sentence."          # the same
hosts = ["mud.example.org"]   # part of the address of the MUDs it is for
names = ["example"]           # or the names they give themselves (MSSP)
default_on = false            # on in every profile until switched off
muddie = "0.5.0"              # the oldest Muddie it works with
```

- **`name` and `about`** appear in the **Plugins** tab and in the offer. Each
  is a text, or a table by language code (`en`, `es`, and so on), the same as
  [`tr`](scripting.md#trtexts) takes. The English name is also the name of the
  folder the plugin is installed in, in each profile.
- **`hosts` and `names`** decide when Muddie offers the plugin. Muddie offers
  it when the address contains one of `hosts`. It also offers it when the MUD
  says over MSSP that its name is one of `names`.
- **`default_on`** switches the plugin on in every profile until the player
  switches it off. For a plugin for one MUD, leave it off. Muddie offers it
  anyway when the player connects to that MUD.
- **`muddie`** is the oldest Muddie the plugin works with. An older Muddie
  keeps the version of the plugin it already has.
- **No other field is allowed.** Any other field is a mistake, and Muddie
  refuses the plugin.

### `rules.toml`

This file holds the rules the plugin brings. Use rules for what the player
will want to change in the editor, such as the colours and steps of a gauge.
Do everything else in a script.

The file is written like a profile's own `rules.toml`. Each rule goes into a
folder named after the plugin. `folder = "Combat"` puts a rule into a folder
called Combat inside that one. Every kind of rule takes `name`, `folder`,
`tags` and `enabled`.

#### Gauges

A gauge is written as `[[gauge]]`. It shows a value the MUD sends.

| Field | What it is |
|---|---|
| `name` | The gauge's name, as shown and said aloud |
| `value` | Where the value comes from: a GMCP path (`Char.Vitals.hp`), an MSDP variable, or a variable in `vars` |
| `max` | Where the maximum comes from, or a number |
| `kind` | `"bar"`, `"cells"`, `"number"`, `"light"`, `"text"` or `"effects"` |
| `colour`, `off_colour` | A colour name or `#rrggbb` |
| `steps` | The colour below each percentage, such as `[{ under = 30, colour = "#e05050" }]` |
| `aloud` | `"usual"`, `"lowest"`, `"wait"`, `"cut"` or `"never"`, as in [`gauges.set`](scripting.md#gaugessetname-gauge) |
| `always` | `true` makes it heard even when reading the game aloud is off |
| `panel` | The name of the gauge panel it appears in. Muddie makes the panel if it does not exist |

#### Triggers

A trigger is written as `[[trigger]]`. It reacts to a line from the MUD.

- **`pattern`** is text the line contains. With `regex = true`, it is a
  regular expression instead.
- Then say what the trigger does, with one or more of these:
  - **`send`**: commands to send, one per line or separated by `;`. Use `%1`
    and so on for what matched, and `$name` for a variable.
  - **`speak`**: text to say aloud.
  - **`sound`**: a sound to play.
  - **`highlight`**: a colour for the line.
  - **`panel`**: a panel to show the line in.
  - **`gag = true`**: hides the line.
  - **`silence = true`**: shows the line but does not read it.
  - **`code`** with **`luau = true`**: Luau code. It gets `m` and `line`, the
    same as a trigger's function in a script.

#### Aliases, keys and timers

- **An alias**, `[[alias]]`, reacts to what the player types. Its `pattern` is
  the first word typed, or a regular expression with `regex = true`. It does
  `send` or `code`.
- **A key**, `[[macro]]`, reacts to a key press. Its `key` is a name such as
  `"f5"`, `"ctrl+k"` or `"numpad8"`. It does `send` or `code`.
- **A timer**, `[[timer]]`, runs after some time. Its `seconds` says how long.
  `repeat = true` makes it run again and again. It does `send` or `code`.

### `template.toml`

This file is a ready-made profile for the MUD. **New profile** offers it in
the list of MUDs to play.

```toml
name = "Example MUD"        # the game, which the profile is named after
host = "mud.example.org"
port = 4000
tls = false
encoding = "utf-8"          # optional: as the MUD says, if not given
language = "es"             # optional: the language the game is played in
plugins = ["example-mud"]   # the plugins switched on, by their ids
```

### The scripts

The scripts are where the plugin does its work. Keep these rules in mind:

- **Name them in English, with no spaces.** For example `example.luau`,
  `aloud.luau` and `prompt.luau`. Muddie runs them in the order of their
  names.
- **They are installed in the player's profile**, in a folder named after the
  plugin. The player can read and change them in the editor. When the plugin
  is switched off, its scripts are switched off too.
- **Share code with modules.** `require("prompt")` loads `prompt.luau` from
  the plugin's own folder. It loads it once, for all the plugin's scripts.
  Muddie also runs every `.luau` in the folder as a script, so a module should
  only return what it holds.
- **Put everything that is heard in its own script**, `aloud.luau`. The
  player can then switch off what is heard and keep what is shown. See
  [Doing it well](#doing-it-well).

## How Muddie installs and updates it

You do not need to do anything for this to happen. Read it to know what the
player ends up with.

- **When the player switches it on** in a profile, Muddie installs the plugin
  in that profile. The scripts go into a folder with the plugin's name among
  the profile's scripts. The rules go into a folder with the plugin's name
  among the profile's rules. The gauges go into the panels they name. From
  then on, all of it belongs to the player, who can change it.
- **When the player switches it off**, those folders are switched off. When
  the player switches it on again, anything they had switched off inside stays
  off.
- **When there is a new version**, official or from git, Muddie installs it
  as the profile is opened. It never updates in the middle of a game.
  - Scripts and rules the new version no longer has are removed.
  - A script the player changed is kept next to the new one, as
    `name.luau.before-update`.
  - Anything the player added stays.
- **Install again**, in the plugin's menu, puts back everything it installed,
  exactly as the plugin comes.
- **In a session with no profile**, the plugin runs as it comes until the
  session ends.

## Doing it well

These are the habits that make a plugin pleasant, for players who look and for
players who listen.

- **It sets everything up by itself.** When switched on, the plugin asks the
  MUD for what it needs (`gmcp.support`). It sets up its gauges and panels. It
  tells the player once about anything they must do in the game. The player
  never sets anything up by hand.
- **Send what the MUD says to data, not to the screen.** Data means gauges,
  channels, the map and `vars`. A panel is only one way to look at that data.
  A player who listens gets the same data: they ask for the gauges with Alt+0,
  and hear them when they change.
- **Make what is heard from the data, not from what is shown.** Write it in
  `aloud.luau`, and listen to what the MUD sends, with `on("gmcp")` and
  triggers. Never listen to a gauge or a panel, because the player may delete
  it.
- **Keep the noise down.** Say what changes, not everything again. For states,
  say the one that arrives and the one that goes, not the whole list.
  - Say what matters with `aloud = "cut"`, and the rest after what is being
    read.
  - Use `line:mute()` or `line:read(shorter)` for lines that say too much.
  - Keep `always = true` for what is urgent.
- **Write every text in English and Spanish**, as a table by language:
  `notify{en = ..., es = ...}`, `tr{...}`. The player sees each text in the
  language of the interface.
- **Tell the player a thing once.** Some things the player must know, such as
  a module to switch on in the game or a newer map. Say each of them once.
  Remember in `vars` that you said it, if it should stay said.
- **Remember the player's choices in `vars`**, under names that belong to the
  plugin, such as `vars.example_learning`.
- **Leave the player's keys alone.** A plugin for a MUD does not change
  Muddie's keys: the player learns them once, for every MUD. Keys go in a
  plugin of their own, which the player chooses (see
  [Share a set of keys](#share-a-set-of-keys)).
- **Keep the MUD out of Muddie.** If the plugin needs something Muddie does
  not have, ask for a new API function. It should be useful to every MUD.

## Recipes

Each recipe solves one common need. Copy the code and change the GMCP names
and patterns to match your MUD.

### Show life and energy

Write the gauges in `rules.toml`, as in [your first plugin](#your-first-plugin).

Some MUDs send only what changed. For example, they send only the life when
only the life changed. Ask Muddie to merge each message with what came before,
so the gauges always have every value:

```lua
gmcp.merge("Char.Status")
```

### Show the character's states

This script shows the states the character is in as a gauge of effects:

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

To say aloud which states arrive and which go, add this to `aloud.luau`. It
reads the same GMCP data, not the gauge:

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

### Put each channel in its tab

`channels.say` puts a message in its channel's tab and reads it aloud. Messages
from before the player arrived are kept but not read:

```lua
on("gmcp", "Comm.Message", function(message)
    channels.say(message.channel, message.text)
end)
-- What was said before the player came: kept, not read.
on("gmcp", "Comm.History", function(message)
    channels.say(message.channel, message.text, {aloud = "never"})
end)
```

### Follow the player on the map

Most MUDs send `Room.Info`. If yours does, the Standard plugin already draws
the map, and you need nothing.

If your MUD says which room the player is in with GMCP of its own, switch off
the standard map and tell the map each room:

```lua
map.native = false
on("gmcp", "Room.Info", function(room)
    map.visit({id = room.vnum, name = room.name, area = room.zone, exits = room.exits})
end)
```

If your MUD only writes the room in its text, the map follows the player by
the room's name and exits. It can also learn what it lacks:

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

To let the player choose whether the map learns, add an option. It appears in
the map's menu and in the palette, and Muddie remembers it:

```lua
map.action({en = "Learn the map as you go", es = "Aprender el mapa sobre la marcha"},
    function(on)
        vars.example_learning = on
        map.learn = on
    end,
    {checked = map.learn})
```

### Keep the prompt quiet

This trigger stops the prompt from being read. If something follows the prompt
on the same line, it reads only that:

```lua
trigger("^HP:\\d+ SP:\\d+> ?(.*)$", function(m, line)
    if m[1] == "" then line:mute() else line:read(m[1]) end
end)
```

In some MUDs each player designs their own prompt. Then learn the prompt as
the game shows it, keep it in `vars`, and build the pattern from it. The
Reinos de Leyenda plugin in muddie-plugins does this in its `prompt.luau`.

### Tell the player something once

This script waits 90 seconds after connecting. If no GMCP vitals have arrived,
it tells the player how to switch them on, and remembers in `vars` that it
did:

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

### Share a set of keys

A plugin can be nothing but keys: a set that someone made to their liking,
for others to install and adjust. It has no `hosts`, so Muddie offers it for
no MUD; each player switches it on in the profiles they want.

`plugin.toml`:

```toml
name = "Keypad keys"
about = { en = "Explore and read with the keypad.", es = "Explorar y leer con el teclado numérico." }
muddie = "0.5.0"
```

`keys.luau`:

```lua
-- keys.luau: Muddie's keys, as this set has them. Change what you like;
-- what is not here keeps Muddie's own keys.
local mine = {
    explore = "numpad0",
    read_previous = "numpad8",
    read_next = "numpad2",
    read_last = "numpad5",
    map_north = {"up", "numpad8"},
    map_south = {"down", "numpad2"},
}
for name, keys in mine do
    shortcuts[name] = keys
end
```

Every key Muddie has, by its name, is in
[the reference](scripting.md#every-shortcut). Whoever installs it changes the
list in the editor, in **Scripts**, and switches it off to get Muddie's keys
back.

## Publishing it

There are two ways to share your plugin. Pick one.

### In a repository of your own

1. Put the plugin's files at the top of the repository.
2. Give players its address. They install it with **Install a plugin…** in
   the palette.
3. Players bring it up to date with **Update a plugin…**.

### As an official plugin

Official plugins reach every Muddie player, and Muddie updates them by itself.

1. Put the plugin in a folder named after its id, in a copy of
   [muddie-plugins](https://github.com/midnattsol/muddie-plugins). There the
   plugin must also have `rules.toml` and `template.toml`, and no folders
   inside it.
2. Run the repository's checks:

```sh
python3 tools/check.py   # the files of every plugin
python3 tools/api.py     # the use of the API, with luau-analyze
python3 tools/pack.py    # plugins.zip, as Muddie downloads it
```

3. Open a pull request.

Once it is merged, release-please makes a release, and every Muddie picks it
up by itself.

## Writing a plugin with an AI

An AI can write a good plugin if you give it the right material. Follow these
steps:

1. Give it this guide, [plugins.md](plugins.md).
2. Give it the reference, [scripting.md](scripting.md), and the types,
   [muddie.d.luau](muddie.d.luau).
3. Give it **a log of a real session**, made with `muddie --debug | tee
   session.log`. In that session, log in, walk around, fight once, and use the
   channels. The log shows what the MUD sends, line by line and over GMCP. No
   description can replace it. Remove your password if you typed it where the
   game shows it.
4. Give it the Reinos de Leyenda plugin from muddie-plugins, as an example of
   a complete plugin.
5. Ask it for the plugin. For example:

   > Make a Muddie plugin for the MUD in session.log, following plugins.md and
   > scripting.md, using only the functions in muddie.d.luau. Gauges for what it
   > sends over GMCP, its channels each in a tab, the map following the player,
   > and, in aloud.luau, only what changes said aloud. Every text in English and
   > Spanish. Give me each file whole.

6. Try the result as in [Try it while you write it](#try-it-while-you-write-it).
7. When something fails, give the AI the mistake as Muddie shows it, and the
   lines of the log around it.

Before you use the plugin, check that:

- [ ] `plugin.toml` has only the fields above, and `muddie = "0.5.0"` or newer.
- [ ] The scripts have English names with no spaces, and what is heard is in `aloud.luau`.
- [ ] It uses only functions from `muddie.d.luau`, and `luau-analyze` finds nothing misspelt.
- [ ] Nothing is said twice: a script does not repeat what a gauge already says.
- [ ] Every text the player sees is in English and Spanish.
- [ ] Switched on in a new profile, it works with nothing done by hand.
