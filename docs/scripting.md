# Muddie scripting reference

Español: [scripting.es.md](scripting.es.md). New to scripts? Start with
[first-scripts.md](first-scripts.md). To make a plugin, read
[plugins.md](plugins.md). Every function with its types, for an editor that
checks Luau, is in [muddie.d.luau](muddie.d.luau).

This is the reference: every function, what it does, what it takes and what
it returns. Scripts are written in [Luau](https://luau.org), a fast, typed
dialect of Lua 5.1.

There is one API for everyone. What a script shows is seen, and what it says
is heard. The same script works for someone looking at the screen and for
someone listening with a screen reader. Muddie gives you the data and the
tools; your scripts decide what to show and what to say.

Scripts are safe to share. They cannot read or write files, start programs,
or load code from outside the script folders. If a script runs for more than
half a second without stopping, Muddie stops it with an error and carries on.

## Contents

1. [Start here](#start-here)
   - [Where scripts go](#where-scripts-go)
   - [Running a script again](#running-a-script-again)
   - [A first script](#a-first-script)
2. [Which one do I use?](#which-one-do-i-use)
   - [To show or say something](#to-show-or-say-something)
   - [To react to something](#to-react-to-something)
   - [To keep data](#to-keep-data)
   - [To follow the player's room](#to-follow-the-players-room)
3. [React to what the MUD says](#react-to-what-the-mud-says)
   - [`trigger(pattern, fn, options)`](#triggerpattern-fn-options)
4. [Change how a line looks or sounds](#change-how-a-line-looks-or-sounds)
   - [Fields of a line](#fields-of-a-line)
   - [`line:hide()`](#linehide)
   - [`line:mute()`](#linemute)
   - [`line:read(text, options)`](#linereadtext-options)
   - [`line:replace(text)`](#linereplacetext)
   - [`line:paint(fg, bg)`](#linepaintfg-bg)
   - [`line:link(command, options)`](#linelinkcommand-options)
   - [`line:to_panel(name)`](#lineto_panelname)
5. [Show messages and speak](#show-messages-and-speak)
   - [`echo(text, options)`](#echotext-options)
   - [`speak(text, options)`](#speaktext-options)
   - [`notify(text, options)`](#notifytext-options)
   - [`tr(texts)`](#trtexts)
   - [`log(text)`](#logtext)
   - [`hush()`](#hush)
6. [Offer a list, and the clipboard](#offer-a-list-and-the-clipboard)
   - [`choose(items, fn, options)`](#chooseitems-fn-options)
   - [`action(name, fn, options)`](#actionname-fn-options)
   - [`clipboard.set(text)`](#clipboardsettext)
   - [`clipboard.get()`](#clipboardget)
7. [Choose how something is heard](#choose-how-something-is-heard)
8. [Send commands and make shortcuts](#send-commands-and-make-shortcuts)
   - [`send(text)`](#sendtext)
   - [`alias(pattern, fn, options)`](#aliaspattern-fn-options)
   - [`on("send", fn)`](#onsend-fn)
9. [Bind keys](#bind-keys)
   - [`macro(key, action, options)`](#macrokey-action-options)
10. [Change Muddie's own keys (shortcuts)](#change-muddies-own-keys-shortcuts)
   - [`shortcuts.<name>`](#shortcutsname)
   - [`shortcuts.all()`](#shortcutsall)
   - [Every shortcut](#every-shortcut)
11. [Do something later or every so often](#do-something-later-or-every-so-often)
   - [`after(seconds, fn, options)`](#afterseconds-fn-options)
   - [`every(seconds, fn, options)`](#everyseconds-fn-options)
12. [Wait in the middle of a script](#wait-in-the-middle-of-a-script)
   - [`wait(seconds)`](#waitseconds)
   - [`wait("line", pattern, options)`](#waitline-pattern-options)
   - [`wait(what, name, options)`](#waitwhat-name-options)
   - [`lines(options)`](#linesoptions)
   - [Where nothing can wait](#where-nothing-can-wait)
13. [Remember things (vars)](#remember-things-vars)
   - [`vars`](#vars)
   - [`expand(text)`](#expandtext)
   - [`on("var", name, fn)`](#onvar-name-fn)
14. [Show your character's state (gauges)](#show-your-characters-state-gauges)
   - [`gauges.set(name, gauge)`](#gaugessetname-gauge)
   - [`gauges.remove(name, options)`](#gaugesremovename-options)
   - [`gauges.get(name)`](#gaugesgetname)
   - [`gauges.all()`](#gaugesall)
   - [`on("gauge", name, fn, options)`](#ongauge-name-fn-options)
15. [Text panels and channels](#text-panels-and-channels)
   - [`panels.set(name, text)`](#panelssetname-text)
   - [`panels.clear(name)`](#panelsclearname)
   - [`panels.add(name, text, options)`](#panelsaddname-text-options)
   - [`panels.size(name)`](#panelssizename)
   - [`channels.say(name, text, options)`](#channelssayname-text-options)
   - [`channels.open(name, options)`](#channelsopenname-options)
   - [`channels.close(name)`](#channelsclosename)
   - [`channels.all()`](#channelsall)
16. [Play sounds](#play-sounds)
   - [`sounds.play(file, options)`](#soundsplayfile-options)
   - [`sounds.volume(group, volume)`](#soundsvolumegroup-volume)
   - [`sounds.fetch(names)`](#soundsfetchnames)
   - [`sounds.stop()`](#soundsstop)
17. [Talk to the MUD behind the scenes (GMCP, MSDP)](#talk-to-the-mud-behind-the-scenes-gmcp-msdp)
   - [`gmcp.get(package)`](#gmcpgetpackage)
   - [`gmcp.send(package, data)`](#gmcpsendpackage-data)
   - [`gmcp.support(module, ...)`](#gmcpsupportmodule-)
   - [`gmcp.merge(package, ...)`](#gmcpmergepackage-)
   - [`msdp.get(variable)`, `msdp.send(command, ...)`](#msdpgetvariable-msdpsendcommand-)
   - [`msdp.report(variable, ...)`](#msdpreportvariable-)
   - [`mssp.get()`](#msspget)
18. [React to other events (on)](#react-to-other-events-on)
   - [`on(what, name, fn, options)`](#onwhat-name-fn-options)
   - [`emit(name, data)`](#emitname-data)
19. [Turn things on and off](#turn-things-on-and-off)
   - [Options, places and tags](#options-places-and-tags)
   - [What `trigger`, `alias`, `macro`, `after`, `every` and `on` return](#what-trigger-alias-macro-after-every-and-on-return)
   - [`triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts`](#triggers-aliases-macros-timers-gauges-scripts)
   - [`tags`](#tags)
   - [What goes with a script](#what-goes-with-a-script)
20. [The session and the command line](#the-session-and-the-command-line)
   - [Fields of `session`](#fields-of-session)
   - [`session.completions(name, words)`](#sessioncompletionsname-words)
   - [`session.feed(text)`](#sessionfeedtext)
   - [`output.last(count)`](#outputlastcount)
21. [Map the game and walk it](#map-the-game-and-walk-it)
   - [Rooms and places](#rooms-and-places)
   - [Settings of the map](#settings-of-the-map)
   - [Following the player](#following-the-player)
     - [`map.visit(room)`](#mapvisitroom)
     - [`map.set_room(room)`](#mapset_roomroom)
     - [`map.here(place)`](#maphereplace)
     - [`map.arrive{name, exits, came}`](#maparrivename-exits-came)
     - [`map.add{name, exits, came}`](#mapaddname-exits-came)
     - [`map.learn`](#maplearn)
     - [`map.candidates()`](#mapcandidates)
     - [`map.failed()`](#mapfailed)
   - [Walking](#walking)
     - [`map.way(to, from)`](#mapwayto-from)
     - [`map.go(to)`](#mapgoto)
     - [`map.stop()`](#mapstop)
   - [Looking things up](#looking-things-up)
     - [`map.room(place)`](#maproomplace)
     - [`map.find(text, options)`](#mapfindtext-options)
     - [`map.exit(from, direction)`](#mapexitfrom-direction)
     - [`map.direction(word)`](#mapdirectionword)
     - [`map.count()`, `map.areas()`, `map.id(text)`](#mapcount-mapareas-mapidtext)
     - [`map.marks()`](#mapmarks)
   - [Changing the map](#changing-the-map)
     - [`map.mark(name, place)`](#mapmarkname-place)
     - [`map.edit_room(place, changes)`](#mapedit_roomplace-changes)
     - [`map.set_exit(from, direction, to, options)`](#mapset_exitfrom-direction-to-options)
     - [`map.remove_exit(from, direction)`, `map.remove_room(place)`](#mapremove_exitfrom-direction-mapremove_roomplace)
     - [`map.clear()`](#mapclear)
     - [`map.tidy(area)`](#maptidyarea)
     - [`map.undo()`](#mapundo)
     - [`map.show_area(area)`](#mapshow_areaarea)
   - [The map a MUD publishes](#the-map-a-mud-publishes)
     - [`map.import(address)`](#mapimportaddress)
     - [`map.newer(address, fn)`](#mapneweraddress-fn)
     - [`map.action(name, fn, options)`](#mapactionname-fn-options)
22. [Share code between scripts](#share-code-between-scripts)
   - [`require(name)`](#requirename)
23. [When something goes wrong](#when-something-goes-wrong)
24. [What scripts cannot do](#what-scripts-cannot-do)

## Start here

### Where scripts go

Scripts are text files ending in `.luau`, in the configuration folder:
`~/.config/muddie` on Linux, `%APPDATA%\muddie` on Windows.

| File | When it runs |
|---|---|
| `init.luau` | In every session |
| `profiles/<profile>/scripts/*.luau` | In that profile, in name order; a plugin's scripts are in the plugin's folder |
| files given with `--script` | In every session, last |

You can also write them inside Muddie: in the editor, **Scripts** lists every
script, to write it, run it, and turn it on or off.

### Running a script again

A script runs once, top to bottom, when Muddie loads it. What it sets up
(triggers, keys, gauges...) keeps working while you play. After you edit a
script, run it again: save it in the editor, press its run button, or choose
**Restart scripts** in the palette (Ctrl+K) to run every script again. To try
one line of Luau without a file, use **Run Luau…** in the palette.

### A first script

Save this as `init.luau` and choose **Restart scripts**.

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

[first-scripts.md](first-scripts.md) goes on from here, step by step.

## Which one do I use?

Several functions do similar jobs. These tables help you pick.

### To show or say something

| You want to... | Use |
|---|---|
| Show a line in the output and read it aloud | [`echo`](#echotext-options) |
| Say something aloud, without showing it | [`speak`](#speaktext-options) |
| Tell the player something from Muddie itself, heard even in another session | [`notify`](#notifytext-options) |
| Say something else in place of a line that came | [`line:read`](#linereadtext-options) |
| Leave a note only you, the author, will see | [`log`](#logtext) |

### To react to something

| When... | Use |
|---|---|
| A line of output matches a pattern | [`trigger`](#triggerpattern-fn-options) |
| You type a command that matches a pattern | [`alias`](#aliaspattern-fn-options) |
| You press a key | [`macro`](#macrokey-action-options) |
| Something that is not text happens: GMCP data, a gauge, a variable, connecting | [`on`](#onwhat-name-fn-options) |
| The next line, or the next lines, inside one function | [`wait("line")`](#waitline-pattern-options), [`lines`](#linesoptions) |
| Any of the first three, without writing code | A rule in the editor |

### To keep data

| You want it to last... | Use |
|---|---|
| While the script is loaded (lost when it runs again) | A `local` variable in the script |
| For good, saved with the profile | [`vars`](#vars) |

A script's `local` variables last while it is loaded:

- **They are kept** when the connection drops and comes back. Scripts are only
  told, with `on("disconnect")` and `on("connect")`. They are also kept when
  you change and save another script or a rule.
- **They are lost** when the script itself runs again (you save it, or press
  its run button), when every script runs again (**Restart scripts**, another
  profile, a plugin turned on or off), and when you close Muddie.

A script that runs again starts from its first line with nothing. That is the
place to ask the MUD again for anything it had already sent.

### To follow the player's room

| The server... | Use |
|---|---|
| Tells you the room, with its id (GMCP `Room.Info`, MSDP `ROOM`) | The standard plugin does it; or [`map.visit`](#mapvisitroom) |
| Tells you the id of a room the map already has | [`map.here`](#maphereplace) |
| Tells only the room's name and exits | [`map.arrive`](#maparrivename-exits-came) |
| Tells only the name and exits, and the map has no room there yet | [`map.add`](#mapaddname-exits-came) |

## React to what the MUD says

A trigger watches the game's output and runs your function when a line
matches a pattern. Use it to answer the game, warn yourself, or change how a
line looks or sounds.

### `trigger(pattern, fn, options)`

Calls `fn(m, line)` for each line of output that matches `pattern`.

**Parameters:**

- `pattern`: a regular expression, in the syntax of Rust's `regex`, so
  `\\d`, `(?<name>...)` and `(?i)` work.
- `fn(m, line)`: `m[0]` is the whole match, and `m[1]`, `m[2]`... are its
  groups. A named group is also `m.name`. `line` is
  [the line itself](#change-how-a-line-looks-or-sounds).

**Options:**

- `place`, `tags`: what turns it on and off (see
  [places and tags](#options-places-and-tags)).
- `once`: remove the trigger after it fires once.
- `prompt`: `true` matches prompts only; `false` matches everything except
  prompts. Leave it out to match both. A prompt is a line the MUD marks as a
  prompt, or text that has no line end and has stopped changing.

**Returns:** what it made (see [what is returned](#what-trigger-alias-macro-after-every-and-on-return)).

**Example:**

```lua
trigger("^You are hungry", function() send("eat bread") end,
        {place = "Survival/Eat", tags = {"survival"}})

trigger("^\\[spam\\]", function(m, line) line:hide() end)

-- A prompt that is not read, and what comes after it on its line is.
trigger("^HP \\d+> (.*)$", function(m, line) line:read(m[1]) end)
```

Triggers run in the order they were made. A line goes through every trigger
that matches it, not just the first. There is no priority.

**See also:** [`wait("line")`](#waitline-pattern-options) and
[`lines`](#linesoptions), to catch the next lines inside one function;
[`alias`](#aliaspattern-fn-options), for what you type.

## Change how a line looks or sounds

Every trigger gets the line it matched. While your function runs, you can
hide the line, recolour it, replace words, make a link, or say something else
in its place. If you keep the line for later, you can only read what it said.

### Fields of a line

| Field | What it is |
|---|---|
| `line.text` | What the line says |
| `line.prompt` | Whether it is a prompt: marked as one by the MUD, or without a line end |
| `line.spans` | The line as runs of text, each with `text`, `fg`, `bg`, `bold`, `italic`, `underline` and `inverse` |

**A trap:** a line is not its text. To use its words, use `line.text`:
`echo(line.text)`, `line.text:match("%d+")`, `line.text == "Done."`. Passing
the line where a text is expected gives an error that says so. But
`line == "Done."` is not an error in Luau: it is simply always false. Put
`--!strict` at the top of the script and
[the definitions](#when-something-goes-wrong) will catch it.

### `line:hide()`

Hides the line: it is neither shown nor read.

**Returns:** nothing.

**See also:** [`line:mute()`](#linemute), to show it without reading it.

### `line:mute()`

Shows the line but does not read it aloud when it arrives. It is still kept,
so you can read it back with the review keys.

**Returns:** nothing.

**See also:** [`line:hide()`](#linehide), to hide it as well.

### `line:read(text, options)`

Reads `text` instead of the line; the line is still shown as it is.

**Options:** `aloud` and `always` (see
[Choose how something is heard](#choose-how-something-is-heard)). Without
them, `text` is heard as the line would be.

**Returns:** nothing.

**See also:** [`line:replace`](#linereplacetext), to change what is shown
too.

### `line:replace(text)`

Puts `text` in place of the part that matched. The line is shown that way
and, unless you say otherwise with `line:read`, read that way.

**Returns:** whether it worked. It fails over a part that another trigger
already replaced.

### `line:paint(fg, bg)`

Recolours the part that matched.

**Parameters:** `fg` is the text colour and `bg` the background. A colour is
a name (`"red"`, `"bright blue"`), `"#rrggbb"`, or a palette number. `nil`
leaves that colour as it is.

**Returns:** nothing.

### `line:link(command, options)`

Turns the part that matched into a link that sends `command`. You can reach
it with the keyboard, like the links the MUD marks.

**Options:** `hint`, a text that says what the link is for.

**Returns:** whether it worked, as for `line:replace`.

**Example:**

```lua
-- Someone arrives: paint the line and make the name a link.
trigger("^(\\w+) arrives\\.$", function(m, line)
    line:paint("bright green")
    line:link("look " .. m[1], {hint = "look at them"})
end)

-- Shown, but not read aloud.
trigger("^You hear a distant bell\\.$", function(m, line) line:mute() end)
```

### `line:to_panel(name)`

Copies the line, with its colours, to the end of the text panel `name`.

**Returns:** nothing.

**See also:** [`panels.add`](#panelsaddname-text-options), to add your own
text to a panel.

## Show messages and speak

These functions put words in front of the player: in the output, in the
corner of the window, or only in their ears. To pick one, see
[Which one do I use?](#to-show-or-say-something)

### `echo(text, options)`

Shows a line in the output and reads it aloud.

**Parameters:** `text` can also be a table of texts by language, as for
[`tr`](#trtexts).

**Options:** `aloud` and `always` (see
[Choose how something is heard](#choose-how-something-is-heard)).

**Returns:** nothing.

To show a line without reading it, give `{aloud = "never"}`.

**See also:** [`speak`](#speaktext-options), [`notify`](#notifytext-options).

### `speak(text, options)`

Says something aloud and shows nothing.

**Parameters:** `text` can also be a table of texts by language.

**Options:** `aloud` and `always`.

**Returns:** nothing.

**Example:**

```lua
speak("You are dying!", {aloud = "cut", always = true})
```

**See also:** [`echo`](#echotext-options), [`hush`](#hush).

### `notify(text, options)`

Shows a notice to the player. It appears for a few seconds in the bottom
right corner, and then stays in the **Messages** list (the bell icon). It
never appears in the game's output.

A notice comes from Muddie, not from the game. So it is always read aloud,
even when reading the game is turned off or its session is not the one in
front.

**Parameters:** `text` can also be a table of texts by language.

**Options:** only `aloud`. To keep the notice from being read:
`{aloud = "never"}`.

**Returns:** nothing.

**See also:** [`echo`](#echotext-options), [`speak`](#speaktext-options).

### `tr(texts)`

Picks the text for the interface's language from a table keyed by language
code. If that language is missing, it uses the same language without the
country (`es` for `es_AR`); then English; then the first one given. Use it
for anything a shared script shows the player.

**Returns:** the text it picked.

**Example:**

```lua
notify(tr{en = "Map updated.", es = "Mapa actualizado."})
```

`echo`, `notify` and `speak` also take texts by language directly:
`speak{en = "You are dying", es = "Te mueres"}`.

### `log(text)`

Writes `text` to the standard output when Muddie was started with `--debug`,
and does nothing otherwise. Use it for messages only the script's author
needs.

**Returns:** nothing.

**See also:** [`muddie --debug`](#when-something-goes-wrong).

### `hush()`

Stops what is being read right now, in the session in front.

**Returns:** nothing.

## Offer a list, and the clipboard

Sometimes the player has to pick one thing among several: whom to reply to,
which event to read again, which object to look at. A list does that the
same way for everyone: it opens in the palette, the arrows go through it,
each item is read aloud, Enter chooses and Escape closes it. Typing narrows
the list down.

### `choose(items, fn, options)`

Offers `items` in a list, and calls `fn` with what the player chose.

**Parameters:**

- `items`: a list. Each item is a text, or `{text = ..., about = ...}`, where
  `about` says more of it; each can be a table of texts by language, as for
  [`tr`](#trtexts).
- `fn`: called with where the chosen item is in the list (from 1) and its
  text; with `nil` if the list was closed without choosing.

**Options:** `title`, what the list asks, as a text or texts by language.
Without it, the list says "Choose one".

**Returns:** nothing. `fn` is called later, when the player chooses.

A new list closes one still open: the script that offered that one gets
`nil`.

**Example:**

```lua
-- Choose whom to reply to, from those who told you something.
local senders = {}
trigger("^(\\w+) tells you: ", function(m)
    if not table.find(senders, m[1]) then table.insert(senders, m[1]) end
end)
alias("^replyto$", function()
    choose(senders, function(index, name)
        if name then vars.reply_to = name end
    end, {title = {en = "Reply to", es = "Responder a"}})
end)
```

**See also:** [`clipboard.set(text)`](#clipboardsettext).

### `action(name, fn, options)`

Offers to do something, chosen by `name` in the palette (Ctrl+K), while the
script is loaded: what is not of the map, as switching a mode or some keys.
A plugin's actions are listed under the plugin's name. `action(name)` takes
it away.

**Parameters:** `name` can be given by language, as for [`tr`](#trtexts).

**Options:** `checked`, true or false, makes it a switch: choosing it calls
`fn(on)` with what it is now.

**Returns:** nothing.

**Example:**

```lua
-- A switch in the palette, remembered.
action({en = "Read the channels", es = "Leer los canales"}, function(on)
    vars.read_channels = on
end, {checked = vars.read_channels ~= false})
```

**See also:** [`map.action(name, fn, options)`](#mapactionname-fn-options),
for what is done with the map, which is in the map's menu too.

### `clipboard.set(text)`

Puts `text` on the clipboard, for the player to paste anywhere.

**Returns:** nothing.

**Example:**

```lua
-- F7 copies your life, to paste it where you like.
macro("f7", function()
    local vitals = gmcp.get("Char.Vitals")
    if vitals then clipboard.set(`{vitals.hp}({vitals.maxhp})`) end
end)
-- "repg" says what Muddie copied last on the group channel.
alias("^repg$", function()
    local text = clipboard.get()
    if text then send("group " .. text) end
end)
```

**See also:** [`clipboard.get()`](#clipboardget).

### `clipboard.get()`

Gets the last text Muddie put on the clipboard: by a script, or by the
player with the review key that copies a line.

**Returns:** the text, or `nil` if Muddie has copied nothing yet.

Watch out: it does not read what other programs copied. A script someone
shares cannot see, for instance, a password you copied from elsewhere.

**See also:** [`clipboard.set(text)`](#clipboardsettext).

## Choose how something is heard

`speak`, `echo`, `line:read`, `panels.add`, `channels.say` and `gauges.set`
all take the same two options to say how they are heard. Most of the time you
need neither.

| Option | How it is heard |
|---|---|
| `aloud = "usual"` | The default. After what is being read. `line:read` is heard as the line would be; a gauge as its kind is (see [`gauges.set`](#gaugessetname-gauge)) |
| `aloud = "wait"` | After what is being read |
| `aloud = "cut"` | Right away, cutting off what is being read |
| `aloud = "never"` | Not at all |
| `aloud = "lowest"` | Gauges only: waits its turn, but cuts in when the gauge falls under its lowest step |
| `always = true` | Even while reading the game is turned off |

Everything a script says about the game follows the same rules as the game's
own lines:

- It is heard only in the session in front.
- It is not heard while reading the game is turned off, unless you give
  `always = true`.
- It is not heard if the settings, the panel or the channel say not to read
  it. `always` does not get past those.
- Words that were just read, as a line or on a channel, are not read again,
  unless the script asks for `"cut"`.

`notify` is different: it is Muddie talking, so it is heard in any session.

## Send commands and make shortcuts

Use `send` to send a command as if the player typed it, and `alias` to make
your own short commands. `on("send")` sees every command just before it
leaves, so it can change or stop it.

### `send(text)`

Sends a command to the MUD exactly as written. Aliases are not applied.

**Returns:** nothing.

**See also:** [`on("send")`](#onsend-fn), which sees it before it leaves.

### `alias(pattern, fn, options)`

When what you type matches `pattern`, calls `fn(m, input)` instead of
sending it.

**Parameters:** `m` holds the groups, as in a [trigger](#triggerpattern-fn-options);
`input` is what you typed.

**Options:** `place`, `tags` and `once`.

**Returns:** what it made (see [what is returned](#what-trigger-alias-macro-after-every-and-on-return)).

**Example:**

```lua
alias("^k (.+)$", function(m) send("kill " .. m[1]) end)
```

**See also:** [`macro`](#macrokey-action-options), for a key instead of a
command.

### `on("send", fn)`

Calls `fn(text)` with each command just before it goes to the MUD, wherever
it comes from: typed, or sent by a script, a rule, a walk or a link.

**Parameters:** what `fn` returns decides what is sent:

- a text: that is sent instead;
- `false`: nothing is sent;
- `nil`: the command goes as it is.

**Returns:** what it made, as [`on`](#onwhat-name-fn-options).

**Example:**

```lua
on("send", function(text)
    if text == "quit" then
        notify("Type it in full: quit!")
        return false
    end
    if text == "quit!" then return "quit" end
end)
```

It is not told the name and password the client logs in with, nor the
commands the function itself sends. Nothing can [wait](#where-nothing-can-wait)
inside it.

## Bind keys

A key binding runs commands or a function when you press a key. It is the
quickest way to walk with the numeric keypad or fire a spell with F5.

### `macro(key, action, options)`

Binds `key` to commands or a function: `macro("f5", "look")`,
`macro("ctrl+shift+k", fn)`.

**Parameters:**

- `key`: a key name (see below).
- `action`: a function, or commands, sent the way a rule sends them:
  separated by `;`, with each `$name` replaced by its variable, and without
  going through aliases.

**Options:** `place` and `tags`.

**Returns:** what it made (see [what is returned](#what-trigger-alias-macro-after-every-and-on-return)).

**Example:**

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad5", "look")
```

**On the map.** While you explore the map, a key whose macro sends only a
direction moves the room chosen that way instead of walking: in the MUD's
words or in English (`"este"`, `"e"`, `"east"`), as [`map.direction`](#mapdirectionword)
reads it. So the keys you walk with in the game walk on the map too. A
macro that calls a function, or sends something else, is left alone.

If you bind a key again under a different `place`, both bindings exist, and
the first one bound that is on is the one that runs. If you bind it again
under the same place, the new action replaces the old one.

**Key names.** Write them in lower case, with modifiers in the order `ctrl`,
`alt`, `shift`: `f1`...`f35`, `ctrl+k`, `alt+shift+up`. On the main keyboard
you can only bind function keys and keys pressed with Ctrl or Alt. That way a
binding never steals what you type.

**The numeric keypad** has names of its own: `numpad0` to `numpad9`,
`numpadadd`, `numpadsubtract`, `numpadmultiply`, `numpaddivide`,
`numpaddecimal` and `numpadenter`. With Num Lock off, the digit keys are
other keys, named after what is printed on them: `numpadup`, `numpaddown`,
`numpadleft`, `numpadright`, `numpadhome`, `numpadend`, `numpadpageup`,
`numpadpagedown`, `numpadclear` (the 5), `numpadinsert` and `numpaddelete`.
Once you bind a keypad key, it stops typing its digit.

## Change Muddie's own keys (shortcuts)

Every key Muddie uses for itself is a shortcut you can change: the palette,
the review keys that read the output, the keys that move on the map while you
explore it, the keys of the editor. The keys each has at first are in the
[table below](#every-shortcut). You can change them in **Settings → Keys**,
or from a script. A script is handy to keep your whole set of keys in one
file, share it with others, and let each of them adjust it.

Keys a script chooses go before those chosen in Settings while the script is
loaded. When the script is switched off or removed, the keys go back to what
they were. Settings shows which script chose each one.

### `shortcuts.<name>`

Reads or changes the keys of one shortcut, by its name.

- **Read it** to get the keys it has now, as a list of texts:
  `shortcuts.explore` gives `{"alt+e"}`.
- **Set it** to a key or a list of keys: `shortcuts.explore = "f9"`,
  `shortcuts.map_north = {"up", "k"}`.
- **Set it to `false`** to leave the shortcut without keys.
- **Set it to `nil`** to give it back the keys it had before the script.

Keys are written as in [`macro`](#macrokey-action-options), with `cmd` too
on a Mac. For `hush` you can also give a modifier alone: `"ctrl"`. Letter
case does not matter.

**Where each key can go:**

- A shortcut that works everywhere needs keys that do not type: held with
  Ctrl, Alt or Cmd, a function key, or a key of the numeric keypad.
- In the command line, keys that do not type will do too, such as the
  arrows, but not Enter.
- On the map, while you explore it, any key will do: the map has the
  keyboard to itself.
- Tab, Escape and Shift+F10 always move through the window. They cannot be
  taken.

**When keys collide:**

- If a script gives a shortcut keys that another one has where both work,
  the other one loses them.
- A [macro](#macrokey-action-options) on the same keys goes before Muddie's
  shortcut.
- While you explore the map, its keys go first.

**Returns:** reading it gives a list of texts.

Watch out: an unknown name, a key that does not exist, or a key that cannot go
where the shortcut works is an error that says which.

**Example:**

```lua
-- My keys: explore the map with F9, and read the lines with the keypad.
shortcuts.explore = "f9"
shortcuts.read_previous = "numpad8"
shortcuts.read_next = "numpad2"
-- On the map, the letters too, besides the arrows.
shortcuts.map_north = {"up", "k"}
shortcuts.map_south = {"down", "j"}
-- Ctrl no longer silences speech.
shortcuts.hush = false
```

**See also:** [`shortcuts.all()`](#shortcutsall), [`macro`](#macrokey-action-options).

### `shortcuts.all()`

Lists every shortcut, with the keys it has now. Give the list to an AI, or
read it aloud, to know what there is to change.

**Returns:** a list of tables, each with:

- `name`: its name, to use as `shortcuts.<name>`.
- `place`: where it works: `"everywhere"`, `"reading"`, `"line"` (the
  command line), `"map"` (exploring the map), `"editor"` or `"layout"`.
- `about`: what it does, in the language of the interface.
- `keys`: its keys now, as a list of texts.

**Example:**

```lua
for _, shortcut in shortcuts.all() do
    echo(`{shortcut.name}: {table.concat(shortcut.keys, ", ")} ({shortcut.about})`)
end
```

**See also:** [`shortcuts.<name>`](#shortcutsname).

### Every shortcut

These are the keys each shortcut has before anyone changes them. On a Mac,
`ctrl` here is `cmd`, and the keys held with `alt` and a letter or a figure
are held with `ctrl` instead; `shortcuts.all()` gives them as they are on
each system.

| Name | Where | Keys at first | What it does |
|---|---|---|---|
| `palette` | `everywhere` | `ctrl+k` | Palette |
| `settings` | `everywhere` | `ctrl+comma` | Settings |
| `new_session` | `everywhere` | `ctrl+t` | New session |
| `close_session` | `everywhere` | `ctrl+w` | Close session |
| `reconnect` | `everywhere` | `alt+r` | Reconnect |
| `switch_view` | `everywhere` | `ctrl+e` | Between the game and the editor |
| `data_view` | `everywhere` | `ctrl+d` | Between the game and the incoming data |
| `find` | `everywhere` | `ctrl+f` | Find in the output |
| `messages` | `everywhere` | `alt+n` | Messages |
| `gauges` | `everywhere` | `alt+0` | Read the gauges |
| `where` | `everywhere` | `alt+m` | Where am I |
| `explore` | `everywhere` | `alt+e` | Explore the map |
| `map` | `everywhere` | `ctrl+m` | Show or hide the map |
| `keys_help` | `everywhere` | `f1` | Say the keys to know |
| `next_zone` | `everywhere` | `f6` | Go to the output and each panel |
| `previous_zone` | `everywhere` | `shift+f6` | Go back through the panels |
| `hush` | `everywhere` | `ctrl` | Silence speech |
| `read_previous` | `reading` | `alt+up` | Read the previous line |
| `read_next` | `reading` | `alt+down` | Read the next line |
| `read_first` | `reading` | `alt+home` | Read the first line |
| `read_last` | `reading` | `alt+end` | Read the last line |
| `read_recent_1` | `reading` | `alt+1` | Read the most recent line |
| `read_recent_2` | `reading` | `alt+2` | Read the 2nd most recent line |
| `read_recent_3` | `reading` | `alt+3` | Read the 3rd most recent line |
| `read_recent_4` | `reading` | `alt+4` | Read the 4th most recent line |
| `read_recent_5` | `reading` | `alt+5` | Read the 5th most recent line |
| `read_recent_6` | `reading` | `alt+6` | Read the 6th most recent line |
| `read_recent_7` | `reading` | `alt+7` | Read the 7th most recent line |
| `read_recent_8` | `reading` | `alt+8` | Read the 8th most recent line |
| `read_recent_9` | `reading` | `alt+9` | Read the 9th most recent line |
| `copy_read` | `reading` | `alt+c` | Copy the line read last |
| `follow_link` | `reading` | `alt+o` | Follow the link of the line read last |
| `panel_previous` | `reading` | `alt+shift+left` | Choose the previous panel of text to read |
| `panel_next` | `reading` | `alt+shift+right` | Choose the next panel of text to read |
| `panel_read_previous` | `reading` | `alt+shift+up` | Read the previous line of the panel |
| `panel_read_next` | `reading` | `alt+shift+down` | Read the next line of the panel |
| `panel_read_first` | `reading` | `alt+shift+home` | Read the first line of the panel |
| `panel_read_last` | `reading` | `alt+shift+end` | Read the last line of the panel |
| `panel_recent_1` | `reading` | `alt+shift+1` | Read the most recent line of the panel |
| `panel_recent_2` | `reading` | `alt+shift+2` | Read the 2nd most recent line of the panel |
| `panel_recent_3` | `reading` | `alt+shift+3` | Read the 3rd most recent line of the panel |
| `panel_recent_4` | `reading` | `alt+shift+4` | Read the 4th most recent line of the panel |
| `panel_recent_5` | `reading` | `alt+shift+5` | Read the 5th most recent line of the panel |
| `panel_recent_6` | `reading` | `alt+shift+6` | Read the 6th most recent line of the panel |
| `panel_recent_7` | `reading` | `alt+shift+7` | Read the 7th most recent line of the panel |
| `panel_recent_8` | `reading` | `alt+shift+8` | Read the 8th most recent line of the panel |
| `panel_recent_9` | `reading` | `alt+shift+9` | Read the 9th most recent line of the panel |
| `panel_copy_read` | `reading` | `alt+shift+c` | Copy the line of the panel read last |
| `history_previous` | `line` | `up` | Earlier command |
| `history_next` | `line` | `down` | Later command |
| `history_search` | `line` | `ctrl+r` | Search the commands typed before |
| `map_north` | `map` | `up`, `8`, `numpad8`, `numpadup` | Follow the exit north |
| `map_south` | `map` | `down`, `2`, `numpad2`, `numpaddown` | Follow the exit south |
| `map_west` | `map` | `left`, `4`, `numpad4`, `numpadleft` | Follow the exit west |
| `map_east` | `map` | `right`, `6`, `numpad6`, `numpadright` | Follow the exit east |
| `map_northwest` | `map` | `7`, `numpad7`, `numpadhome` | Follow the exit north-west |
| `map_northeast` | `map` | `9`, `numpad9`, `numpadpageup` | Follow the exit north-east |
| `map_southwest` | `map` | `1`, `numpad1`, `numpadend` | Follow the exit south-west |
| `map_southeast` | `map` | `3`, `numpad3`, `numpadpagedown` | Follow the exit south-east |
| `map_up` | `map` | `pageup`, `numpadsubtract` | Follow the exit up |
| `map_down` | `map` | `pagedown`, `numpadadd` | Follow the exit down |
| `map_in` | `map` | `numpaddivide` | Follow the exit in |
| `map_out` | `map` | `numpadmultiply` | Follow the exit out |
| `map_again` | `map` | `space`, `numpad5`, `numpadclear` | Say the room again |
| `map_home` | `map` | `home` | Back to your room |
| `map_back` | `map` | `backspace` | Back to the room before |
| `map_go` | `map` | `enter`, `numpadenter` | Go to the room chosen |
| `map_way` | `map` | `shift+enter` | Say the way to the room chosen |
| `map_help` | `map` | `f1` | Say all the map's keys |
| `map_info` | `map` | `ctrl+i` | All about the room |
| `map_near` | `map` | `ctrl+l` | What is near |
| `map_find` | `map` | `ctrl+f` | Find on the map |
| `map_mark` | `map` | `ctrl+b` | Mark the room |
| `map_note` | `map` | `ctrl+n` | Write a note on the room |
| `move_up` | `editor` | `alt+up` | Move the rule up |
| `move_down` | `editor` | `alt+down` | Move the rule down |
| `move_out` | `editor` | `alt+left` | Move the rule out of its folder |
| `rename` | `editor` | `f2` | Rename |
| `delete` | `editor` | `delete` | Delete |
| `select_all` | `editor` | `ctrl+a` | Select all visible items |
| `undo` | `editor` | `ctrl+z` | Undo |
| `redo` | `editor` | `ctrl+y`, `ctrl+shift+z` | Redo |
| `panel_up` | `layout` | `alt+up` | Move the panel up |
| `panel_down` | `layout` | `alt+down` | Move the panel down |
| `panel_side_back` | `layout` | `alt+left` | Move the panel to the side before |
| `panel_side_on` | `layout` | `alt+right` | Move the panel to the next side |


## Do something later or every so often

Timers run a function after a delay, or again and again. Use them for things
like saving every minute.

### `after(seconds, fn, options)`

Calls `fn()` once, after that many seconds.

**Options:** `place` and `tags`.

**Returns:** what it made; `made:left()` gives the seconds left.

**See also:** [`wait(seconds)`](#waitseconds), to pause inside a function
instead.

### `every(seconds, fn, options)`

Calls `fn()` every that many seconds, until you remove it. While it is turned
off, it skips its turns.

**Options:** `place` and `tags`.

**Returns:** what it made; `made:left()` gives the seconds until the next
call.

**Example:**

```lua
local saving = every(60, function() send("save") end)
after(5, function() saving:remove() end)
```

## Wait in the middle of a script

When something takes several steps, write them one after another. A function
can pause until a line arrives, a gauge drops, or some seconds pass, and then
carry on. This is much simpler than triggers and timers that start each
other.

### `wait(seconds)`

Pauses the function for that many seconds. The rest of the client and the
other scripts keep going.

**Returns:** nothing.

### `wait("line", pattern, options)`

Pauses until a later line matches `pattern`.

**Options:**

- `timeout`: the most seconds to wait.
- `prompt`: as in a [trigger](#triggerpattern-fn-options).

**Returns:** what a trigger gets: the groups and the line. `nil` if
`timeout` passes first.

**Example:**

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

**See also:** [`lines`](#linesoptions), for several lines;
[`trigger`](#triggerpattern-fn-options).

### `wait(what, name, options)`

Pauses until whatever `on(what, name, ...)` listens to arrives:
`wait("gauge", "HP", {under = 30, timeout = 10})`. Leave out `name` for what
takes none.

**Options:** `timeout`, and those of [`on`](#onwhat-name-fn-options).

**Returns:** what `on` would pass to its function, or `true` if that is
nothing. `nil` if `timeout` passes first.

### `lines(options)`

Gives you the next lines one by one, in a `for` loop.

**Options:** it stops at the first of these, and you must give at least one:

- `till`: a line matches this pattern (that line is not included);
- `count`: this many lines have come;
- `timeout`: no line comes for this many seconds.

**Returns:** a function for `for`, which gives a [line](#fields-of-a-line)
each time.

**See also:** [`wait("line")`](#waitline-pattern-options).

### Where nothing can wait

A wait ends when its script is stopped or run again.

You cannot wait inside `on("send", ...)`, inside a module while `require` is
loading it, or inside a function handed to another function, such as the one
you give `table.sort`.

To wait without holding up the rest of your code, wrap it:
`coroutine.wrap(function() ... end)()` runs its contents, waits included,
alongside the rest.

## Remember things (vars)

Variables in `vars` are saved with the profile, so they survive closing
Muddie. Use them for a target, a list of friends, or anything your scripts
must not forget. For what lasts only while a script is loaded, see
[To keep data](#to-keep-data).

### `vars`

The profile's variables, as a table. They are the same ones you see under
**Variables** in the editor. Set a field to keep it, read it to get it as it
is now, and set it to `nil` to forget it.

**Example:**

```lua
vars.target = "orc"          -- keeps it
send("kill " .. vars.target) -- reads it, as it is now
vars.target = nil            -- forgets it
for name, value in vars do echo(`{name} = {value}`) end
```

**A trap:** a table stored in a variable is saved as it was when you stored
it. If you change the table afterwards, store it again:

```lua
local bag = vars.bag or {}
bag.gold = 5
vars.bag = bag
```

**See also:** [`on("var")`](#onvar-name-fn), [`expand`](#expandtext).

### `expand(text)`

Fills variables into `text`, as in the commands of rules: `$name` becomes
the value of that variable, `$name.key` a field of a table, and `$name.1` an
item of a list.

**Returns:** the text with the variables filled in.

### `on("var", name, fn)`

Calls `fn(value, name)` when the variable changes, whether a script, a
trigger or the editor changed it.

**Returns:** what it made, as [`on`](#onwhat-name-fn-options).

A gauge made in the editor can also follow a variable, without a script.

## Show your character's state (gauges)

A gauge shows a piece of your character's state: health as a bar, a light
for "poisoned", a list of effects. Gauges sit in gauge panels, and their
changes are read aloud.

A gauge is data first. Whoever made it (the editor, a plugin, a script or the
MUD), any script can read it and listen to it, whether a panel shows it or
not. With a screen reader, the gauge panels are skipped; press Alt+0 to hear
what they show, and changes are said as they happen.

### `gauges.set(name, gauge)`

Shows a gauge, or updates it if it exists.

**Parameters:** `gauge` is a table with these fields:

| Field | What it is |
|---|---|
| `kind` | `"bar"` (the default), `"cells"`, `"number"`, `"light"`, `"text"` or `"effects"` |
| `value`, `max` | Its level. A bar and cells need both; a number needs `value` |
| `on` | For a light: whether it is lit |
| `text` | For text: what it says |
| `items` | For effects: what the character has, as a list (or a table by name) of names or of `{name = "Poisoned", about = "1 damage a second", colour = "red"}` |
| `colour`, `off_colour` | A name or `#rrggbb`; `off_colour` is a light's colour while it is off |
| `steps` | The colour it takes under each percentage: `{{under = 60, colour = "yellow"}, {under = 30, colour = "red"}}` |
| `aloud`, `always` | How its changes are heard. `"usual"`: a level cuts in when it falls under a step, and other changes wait their turn. Also `"lowest"`, `"wait"`, `"cut"` and `"never"` |
| `panel` | The gauge panel to show it in, by name. The panel is made if it does not exist. Without it, the first gauge panel there is |
| `place`, `tags` | What turns it on and off, as for a trigger |

**Returns:** nothing.

**Example:**

```lua
gauges.set("HP", {value = 40, max = 100,
                  steps = {{under = 30, colour = "red"}}, aloud = "lowest"})
gauges.set("Poisoned", {kind = "light", on = true, colour = "green"})
gauges.set("States", {kind = "effects", items = {"Blessed", "Stunned"}})
```

When a level's change is said:

- when it falls under one of its steps; then not again until it rises clearly
  above that step;
- and again for each further tenth it falls while under a step.

A light, a text or effects are said whenever what they say changes.

**See also:** [`on("gauge")`](#ongauge-name-fn-options),
[`gauges.get`](#gaugesgetname).

### `gauges.remove(name, options)`

Removes the gauge.

**Options:** `place` and `tags`, the same ones it was set with.

**Returns:** nothing.

### `gauges.get(name)`

Gets a gauge as it is now, by its name or its place.

**Returns:** a table with `name`, `place` and `kind`, plus, depending on the
kind: `value`, `max` and `percent`; `on`; `text`; or `items`, a list of
`{name, about, colour}`. `nil` if there is no such gauge; a gauge that is
turned off counts as missing.

### `gauges.all()`

Gets every gauge.

**Returns:** a list, each gauge as `gauges.get` gives it.

### `on("gauge", name, fn, options)`

Calls `fn(gauge, change)` when the gauge changes. Leave out `name` to hear
about every gauge. A gauge being made does not count as a change.

**Parameters:** `gauge` is how it is now, as `gauges.get` gives it.
`change.was` is how it was before. For effects, `change.came` lists those
that arrived and `change.gone` those that left.

**Options:**

- `under = 30`: only when it falls under 30 %. It fires once, and not again
  until the gauge has come back 5 % above that.
- `over = 90`: when it reaches 90 %.
- `place`, `tags`.

**Returns:** what it made, as [`on`](#onwhat-name-fn-options).

**Example:**

```lua
on("gauge", "HP", function() sounds.play("alarm.ogg") end, {under = 30})

on("gauge", "States", function(gauge, change)
    for _, state in change.came do speak(state.name) end
end)
```

## Text panels and channels

A text panel is a box beside the output where a script can put text: a loot
list, a map the server draws, a log of one kind of message. Channels are the
MUD's chat channels, each in its own tab, so conversation does not get lost
in the fight.

### `panels.set(name, text)`

Replaces everything in a text panel with `text`. Use it for something the
server sends ready to show, such as a map it draws.

**Parameters:** `name` is the panel. It is made if none has that name.
`"map"` always means the map panel, whatever it is called, and `"channels"`
means the channels panel.

**Returns:** nothing.

**See also:** [`panels.add`](#panelsaddname-text-options), to add without
replacing.

### `panels.clear(name)`

Empties the panel.

**Returns:** nothing.

### `panels.add(name, text, options)`

Adds `text` at the end of a panel, keeping what was there, and reads it aloud
after the panel's name.

**Options:** `aloud` and `always`.

**Returns:** nothing.

**Example:**

```lua
trigger("^You find (.+)\\.$", function(m) panels.add("Loot", m[1]) end)

trigger("^(\\w+) chats: (.*)$", function(m, line)
    channels.say("chat", `{m[1]}: {m[2]}`)
    line:hide()
end)
```

**See also:** [`channels.say`](#channelssayname-text-options),
[`line:to_panel`](#lineto_panelname).

### `panels.size(name)`

Tells how many characters fit in a panel right now. Without a name, the map
panel.

**Returns:** `{columns, rows}`, or `nil` if the panel has not been on screen
yet.

**See also:** `on("panel")` in [`on`](#onwhat-name-fn-options), which tells
you when the size changes.

### `channels.say(name, text, options)`

Adds `text` as said on the MUD channel `name`, which has its own tab in a
panel, and reads it aloud after the channel's name.

**Options:**

- `panel`: the panel the channel goes in, by name. An existing panel of that
  name is used as it is; otherwise one is made. Without it, the channels
  panel.
- `aloud` and `always`. Use `aloud = "never"` for what was said before the
  player arrived.

**Returns:** nothing.

If two scripts say the same message at the same moment, it counts once.

**See also:** [`panels.add`](#panelsaddname-text-options).

### `channels.open(name, options)`

Makes the channel's tab before anyone speaks on it.

**Options:** `panel`, as for `channels.say`.

**Returns:** nothing.

### `channels.close(name)`

Removes the channel's tab, as when the character leaves the channel.

**Returns:** nothing.

### `channels.all()`

Gets the channels there are.

**Returns:** a list of their names.

## Play sounds

Sounds give quick cues without words: a beep when you are hit, an alarm when
health is low, the wind of a forest. Put your files in the profile's `sounds`
folder. Muddie reads WAV, OGG, MP3, FLAC and M4A, and plays them the same on
Linux, Windows and Mac.

### `sounds.play(file, options)`

Plays a sound, by path or by name in the profile's `sounds` folder.

**Options:**

- `volume`, from 0 to 1, and `loop`, to play it again and again.
- `start` and `finish`: where to start and stop within the sound, in seconds.
- `fade_in` and `fade_out`: how many seconds it takes to come up, and to die
  away before `finish`.
- `pan`: where it is heard, from -1, all left, to 1, all right; 0, in the
  middle, as it is if not given. Put a sound where something is: a door
  opening to the west, on the left.
- `group`: the group of sounds it is in, as `"ambient"` or `"alerts"`, whose
  volume [`sounds.volume`](#soundsvolumegroup-volume) sets.

**Returns:** the sound:

- `sound:stop()` stops it.
- `sound:playing()` says whether it is still playing.
- `sound:volume(volume)` makes it louder or quieter as it plays, from 0 to 1.

**Example:**

```lua
trigger("^You are hit", function() sounds.play("hit.ogg", {volume = 0.5}) end)
```

```lua
-- The wind of a forest, quietly, on the left; louder as the fight ends.
local wind = sounds.play("wind.ogg", {loop = true, volume = 0.3, pan = -0.6, group = "ambient"})
trigger("^You won the fight", function() wind:volume(0.6) end)
```

**See also:** [`sounds.volume(group, volume)`](#soundsvolumegroup-volume).

### `sounds.volume(group, volume)`

Sets how loud a group of sounds is, from 0 to 1: those playing at once, and
those played after. Without `volume`, says how loud it is. The sounds the MUD
plays itself are in the group `"mud"`.

**Returns:** how loud the group is now; 1 if nothing set it.

When the script that set it goes, the group is as loud as before. To keep
the player's choice, keep it in [`vars`](#vars) and set it again as the
script starts.

**Example:**

```lua
-- "ambient 20" sets the background sounds to 20 %.
alias("^ambient (%d+)$", function(m)
    vars.ambient = tonumber(m[1]) / 100
    sounds.volume("ambient", vars.ambient)
end)
```

### `sounds.fetch(names)`

Fetches, in the background, the sounds of a plugin that has them on the web
(its `plugin.toml` says where, with `sounds`), so that none comes late the
first time it plays. Only those not kept, or changed since, are fetched; more
than three are told of, as they start and when all are here.

**Parameters:** `names`, a list of the sounds' names inside that folder.

**Returns:** nothing.

**Example:**

```lua
sounds.fetch({"Combate/Alerta vida 30.wav", "Comunicaciones/Tel.wav"})
```

A plugin's script plays them as any other, with
[`sounds.play`](#soundsplayfile-options): the player's own file of that name
first, if there is one, else the one kept, else it is fetched and played as
it comes. Nothing is fetched while **Download sounds** is off in the profile.

### `sounds.stop()`

Stops every sound.

**Returns:** nothing.

## Talk to the MUD behind the scenes (GMCP, MSDP)

Many MUDs send data you never see in the text, such as your health or the
room you are in. They use protocols called GMCP and MSDP. Use this data to
fill gauges and the map without reading the text. To react when data
arrives, use `on("gmcp", ...)` or `on("msdp", ...)` (see
[`on`](#onwhat-name-fn-options)).

### `gmcp.get(package)`

Gets the data last received for exactly that package.

**Returns:** the data, or `nil` if none has come.

### `gmcp.send(package, data)`

Sends a GMCP message to the server.

**Returns:** nothing.

### `gmcp.support(module, ...)`

Asks the server for GMCP modules, as `"Name version"`, besides those the
client asks for itself (`Core` and `Client.Media`). Some servers only send
what you ask for. The request lasts while the script is loaded: when it is
turned off or removed, the server is told the modules are no longer wanted.

**Returns:** nothing.

**Example:**

```lua
gmcp.support("Char 1", "Room 1")
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("HP", {value = vitals.hp, max = vitals.maxhp})
end)

msdp.report("HEALTH", "HEALTH_MAX")
on("msdp", "HEALTH", function(value) echo("health " .. value) end)
```

### `gmcp.merge(package, ...)`

For packages where the server sends only what changed, such as a
`Char.Status` with just the health. What arrives is merged into what
`gmcp.get` and the gauges keep, including nested tables. Listeners still
receive only what arrived.

**Returns:** nothing.

### `msdp.get(variable)`, `msdp.send(command, ...)`

The same as `gmcp.get` and `gmcp.send`, for MSDP.

**Returns:** `msdp.get` returns the value, as a string; `msdp.send`, nothing.

### `msdp.report(variable, ...)`

Asks the server to send these MSDP variables whenever they change, while the
script is loaded.

**Returns:** nothing.

### `mssp.get()`

Gets the server's MSSP status.

**Returns:** a table of texts.

## React to other events (on)

`on` runs a function when something happens that is not a line of text: GMCP
data arrives, a variable changes, you connect, another script says
something.

### `on(what, name, fn, options)`

Calls `fn` when `what` happens. For events that take no `name`, write
`on(what, fn)`.

**Parameters:**

| `what` | `name` | `fn` gets |
|---|---|---|
| `"gmcp"` | a package; it also covers those under it (`"Char"` covers `Char.Vitals`) | the data, and the package |
| `"msdp"` | a variable | the value (a string), and the variable |
| `"var"` | a variable of [`vars`](#vars) | the value, and the name |
| `"gauge"` | a gauge, or none for all | the gauge, and how it changed (see [`on("gauge")`](#ongauge-name-fn-options)) |
| `"channel"` | a channel, or none for all | `{channel, text}` |
| `"emit"` | what another script says | what it said, and the name |
| `"send"` | none | each command on its way to the MUD (see [`on("send")`](#onsend-fn)) |
| `"room"` | none | the room the player is now in, as [`map.room`](#maproomplace) gives it, and the id of the one before (`nil` at first) |
| `"connect"`, `"disconnect"` | none | nothing |
| `"focus"` | none | whether the session is now in front, in a window that is in front |
| `"reader"` | none | whether a screen reader now reads Muddie |
| `"mxp"` | none | `{name, value}`, when the MUD names a value with MXP |
| `"panel"` | none | `{name, map, columns, rows}`, when a panel that shows a drawing settles on a new size; `map` is true for the map panel |

**A move to another room** is told once the map knows the player is there, however it knows it: the standard plugin, a script, or a walk.

```lua
-- The note of a room is said as you come into it.
on("room", function(room)
    if room.note ~= "" then speak(room.note) end
end)
```

**Options:** `place` and `tags`; for a gauge, also `under` and `over`.

**Returns:** what it made, just like a trigger (see
[what is returned](#what-trigger-alias-macro-after-every-and-on-return)).

**Example:**

```lua
on("connect", function() send("look") end)
```

If `on` does not know `what`, the error lists what it can listen to.
Changes to a variable, a gauge or a channel, and messages from another
script, are passed on once the script that made them has finished. So you
always get the whole change, not half of it.

**See also:** [`wait(what, name)`](#waitwhat-name-options), to wait for one
of these inside a function.

### `emit(name, data)`

Sends a message to the other scripts, which listen with
`on("emit", name, fn)`. This is how a plugin tells others what they may want
to know.

**Returns:** nothing.

**Example:**

```lua
-- In one script:
emit("fight", {with = "orc"})
-- In another:
on("emit", "fight", function(data) speak(`Fighting {data.with}`) end)
```

## Turn things on and off

Anything a script makes can be turned on and off: during a fight you may want
combat triggers and keys, and outside it you may not. Give things a place or
tags, then turn whole groups on and off at once.

### Options, places and tags

Options always go last, in a table: `trigger(pattern, fn, {once = true})`.
An option a function does not take gives an error that lists the ones it
does take. Muddie never ignores an option silently.

Triggers, aliases, keys, timers, listeners and gauges all take two options:

- `place`, such as `"Folder/Name"`: where it lives. Turning a folder off
  turns off everything inside it.
- `tags`, a list of tags or a single one. Turning a tag off turns off
  everything that carries it.

Something runs only while its place, every folder above it, and every tag it
carries are all on.

Rules made in the editor have their folder and name as their place:
`Combat/Fight back`. Two rules with the same name get a number:
`Combat/Fight back #2`. A script's place is its path among the profile's
scripts, without the extension: `Mapping/rooms`.

### What `trigger`, `alias`, `macro`, `after`, `every` and `on` return

Each returns the thing it made, which you can keep in a variable:

| Method | What it does |
|---|---|
| `made:enable()`, `made:disable()`, `made:toggle()` | Turn it on or off; return whether it is now on |
| `made:enabled()` | Whether it is on |
| `made:remove()` | Removes it; returns whether it was there |
| `made:left()` | For a timer, the seconds it has left |

### `triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts`

Turn things of one kind on or off by their place, or a whole folder at once,
for the session. This works whether they were made in a script or in the
editor.

**Functions:** each has `enable(place)`, `disable(place)`, `toggle(place)`,
`enabled(place)` and `list()`.

**Returns:** `toggle` and `enabled` return whether it is on; `list` returns
the places there are; the others, nothing.

**Example:**

```lua
triggers.disable("Combat")
macros.enable("Combat/Kick")
local on = scripts.toggle("Mapping")
```

### `tags`

The same functions, by tag: `tags.disable("combat")` turns off everything of
every kind that carries the tag.

### What goes with a script

When a script is turned off, removed or run again, everything it made goes
with it:

- its triggers, aliases, keys, timers, listeners and gauges;
- the text it put in panels, and the channels it opened;
- its sounds, which stop;
- what it set on the map and the session, which goes back to how it was:
  `map.learn`, `map.directions`, `map.names`, `map.native`, `map.persist`,
  `map.language`, `session.separator`, `session.completions`, `gmcp.merge`,
  `gmcp.support` and `msdp.report`.

## The session and the command line

`session` tells your script about the current connection: where it is
connected, whether a screen reader is running, what is typed in the command
line. A few functions help with typing and testing.

### Fields of `session`

| Field | What it is |
|---|---|
| `session.connected` | Whether the session is connected |
| `session.host` | The server it connects to |
| `session.profile` | The name of its profile, or `nil` |
| `session.reader` | Whether a screen reader is reading Muddie |
| `session.focused` | Whether the session is in front, in a window that is in front |
| `session.input` | What is typed in the command line. Set it to put text there. It is `nil` while a password is being typed, and then it cannot be set either |
| `session.reading` | Whether the game is read aloud. Set it to `false` to keep it quiet, as a quiet mode does; `true` or `nil` reads it again. The player can switch it too; when the script that set it goes, the game is read again |
| `session.separator` | What separates several commands typed on one line: `";"` unless you set it. `false` turns splitting off; `nil` brings back `";"`. Type it twice to send it as it is |

### `session.completions(name, words)`

Adds words for Tab to offer, besides the words seen in the output. Each
`name` keeps its own list; `session.completions(name)` removes that list.

**Returns:** nothing.

**Example:**

```lua
session.completions("friends", {"Ana", "Luis"})
if session.reader then echo("A screen reader is reading Muddie.") end
```

### `session.feed(text)`

Handles `text` as if the MUD had sent it. Use it to test what your script
does with a line.

**Returns:** nothing.

### `output.last(count)`

Gets what the last `count` lines of the output say. Without `count`, only
the last line.

**Returns:** a list of texts, newest last.

## Map the game and walk it

Muddie keeps a map of the rooms you visit and can walk you to any of them.
The **Standard GMCP and MSDP** plugin builds the map from GMCP `Room.Info`
and MSDP `ROOM`. For a MUD that describes rooms some other way, or only in
the text, a script feeds the map itself and can turn the standard loader
off. To pick the function that fits your MUD, see
[To follow the player's room](#to-follow-the-players-room).

### Rooms and places

You give a room as a table with an `id`. It can also have `name`, `area`,
`exits` (each direction to a room id), `x`, `y`, `z` and `colour` (a name,
`"#rrggbb"` or a palette number). A room given without a colour keeps the one
it had.

An id is a number, or the text the server uses to name the room. You can pass
what a MUD sends in `Room.Info` or `ROOM` as it comes: `num` or `vnum` work
as `id`, `zone` as `area`, and `coords` as the position, in any letter case.

Some functions take a place (`map.room`, `map.here`, `map.mark`,
`map.edit_room`, `map.remove_room`, `map.way`, `map.go`). A place is a room's
id, or a mark or a room name as the player would type it. If several rooms
match, the nearest one is used.

### Settings of the map

These are fields: set them, and read them back. Setting one to `nil` puts it
back as it was.

| Field | What it does |
|---|---|
| `map.native` | `false` turns the standard loader off while the script is loaded |
| `map.persist` | `false` keeps the map in memory only, for servers that send the map themselves |
| `map.learn` | `true` makes the map learn as the player moves (see [`map.learn`](#maplearn)) |
| `map.language` | The language the MUD is played in, by code or name (`"es"`). The map uses that language's words for directions |
| `map.directions` | The MUD's own words for directions, on top of those of its language: `{n = {"nte"}, s = "sur"}`. The first word of each is the one walks send |
| `map.names` | How the MUD writes its room names: `{region = ":", exits = "[]"}`. The region comes first, up to the `region` text; the exits come after, between the characters of `exits` |

### Following the player

#### `map.visit(room)`

Says the player is now in `room`, adding the room or updating it.

**Returns:** the room's id.

**Example:**

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

**See also:** [`map.here`](#maphereplace), [`map.arrive`](#maparrivename-exits-came).

#### `map.set_room(room)`

Adds or updates a room without moving the player there.

**Returns:** the room's id.

#### `map.here(place)`

Says the player is in that room, which must already be on the map.

**Returns:** whether the map has the room.

**See also:** [`map.visit`](#mapvisitroom).

#### `map.arrive{name, exits, came}`

For servers that do not say which room you are in. Says the player is in a
room with this name and these exits, reached through the exit `came`.

**Returns:** the room's id, or `nil` while the map is not sure which room it
is.

**See also:** [`map.add`](#mapaddname-exits-came), [`map.learn`](#maplearn),
[`map.candidates`](#mapcandidates).

#### `map.add{name, exits, came}`

For servers that do not say which room you are in. Use it when the player
went through `came` to a place where the map has no room. It looks for the
room among those the map has, or makes a new one. The player can undo the
rooms added this way.

**Returns:** the room's id and `"found"` or `"made"`. If it cannot, `nil`
and the reason:

- `"lost"`: the map does not know for sure where the player was;
- `"known"`: that exit already leads to a room;
- `"alike"`: several rooms could be the one;
- `"unnamed"`.

#### `map.learn`

A setting: set it to `true` and the map learns from the rooms passed to
`map.arrive`. It never learns from a single sighting, nor from a room it is
not sure the player was in. It:

- adds rooms it does not have where the player walks by compass directions;
- corrects the names of rooms it had under another name, once the next move
  confirms it;
- adds exits a room shows that the map is missing;
- makes an exit lead to the room it really leads to.

Each change is announced, and can be undone like any change the map makes on
its own.

#### `map.candidates()`

Gets the rooms the player might be in while the map cannot tell which.

**Returns:** a list of room ids; empty when the map knows, or when it has no
idea.

#### `map.failed()`

Tells the map that the server said a move failed. The moves already sent are
treated as not done, and a walk stops.

**Returns:** nothing.

### Walking

#### `map.way(to, from)`

Works out the way from `from` to `to`. Without `from`, it starts where the
player is.

**Returns:** a list of commands, or `nil` if there is no way.

#### `map.go(to)`

Walks the player there, waiting for each move to be confirmed before the
next.

**Returns:** nothing.

**Example:**

```lua
alias("^home$", function() map.go("home") end)
```

#### `map.stop()`

Stops a walk, as **Stop going** does.

**Returns:** nothing.

### Looking things up

#### `map.room(place)`

Gets a room; without `place`, the player's room.

**Returns:** a table with `id`, `name`, `area`, `x`, `y`, `z`, `exits`
(direction to room id), `cost` (what entering it costs a route), `avoid`,
`colour` (`"#rrggbb"` or `nil`), `note` (`""` if none), `renamed` (its name
was given in place of the map's), `added` (the map added it on its own),
`shape` and `symbol`. `nil` if there is no such room.

#### `map.find(text, options)`

Finds rooms the way **Find rooms…** does: by mark, note or name.

**Options:** `exact = true`: only rooms named exactly `text`, as the MUD
names them.

**Returns:** a list of room ids, nearest first.

#### `map.exit(from, direction)`

Gets an exit of a room.

**Returns:** a table with `to`, `cost`, `blocked` (kept off routes),
`command`, and `custom` (one the player added); `nil` if there is no such
exit.

#### `map.direction(word)`

Tells which direction a word means. The word can be in the MUD's language or
in English.

**Returns:** the direction as the map stores it (`"n"`, `"ne"`... `"u"`,
`"d"`, `"in"`, `"out"`), or `nil` if it is not a direction.

#### `map.count()`, `map.areas()`, `map.id(text)`

**Returns:** `map.count()`, how many rooms there are; `map.areas()`, a list
of the areas' names; `map.id(text)`, the number used for a text id.

#### `map.marks()`

Gets the marks.

**Returns:** a table of marks by name, each with its room's id.

### Changing the map

#### `map.mark(name, place)`

Marks a room with `name`, as **Mark…** does. `map.mark(name)` removes the
mark.

**Returns:** whether the room (or the mark) existed.

#### `map.edit_room(place, changes)`

Changes only the fields given in `changes`, leaving the rest.

**Parameters:** `changes` can have:

- `name`: a name in place of the map's. It stays even when the server
  describes the room again or the map is imported again.
- `note`, `cost`, `avoid`, `x`, `y`, `z`, `colour`.
- `shape`: `"square"`, `"circle"`, `"diamond"` or `"hexagon"`.
- `symbol`: one or two characters written in the room.

`false` brings back the map's own colour or shape, removes the cost, or
removes the symbol.

**Returns:** whether the room existed.

#### `map.set_exit(from, direction, to, options)`

Adds an exit of the player's own, or changes where an exit leads. With `to`
set to `nil`, it only changes what `options` give.

**Options:** `cost`, `blocked`, or `command` (such as `"open door;n"`).

**Returns:** whether the room and the exit existed.

#### `map.remove_exit(from, direction)`, `map.remove_room(place)`

Remove an exit, or a room.

**Returns:** whether it existed.

#### `map.clear()`

Forgets every room.

**Returns:** nothing.

#### `map.tidy(area)`

Redraws an area from scratch, if that draws it better; without `area`, the
player's area.

**Returns:** how many rooms moved. If it moved none, it left the area as it
was, and returns as a second value
`{now = {wrong_exits, stacked}, ["then"] = {wrong_exits, stacked}}`.

#### `map.undo()`

Undoes the last change the map made on its own.

**Returns:** whether there was one.

#### `map.show_area(area)`

Shows an area on the map, just to look at it, without walking there.

**Returns:** nothing.

### The map a MUD publishes

#### `map.import(address)`

Imports a map made with Mudlet from `address` (a `.dat` file, or a zip with
one, on the web or on disk), replacing the current map. What the player set
is kept: renamed rooms, notes, marks, rooms to avoid, their own exits, and
the rooms the map added.

**Returns:** nothing.

#### `map.newer(address, fn)`

Checks, without downloading it, whether the map at `address` has changed
since it was imported, and calls `fn()` if it has. Use it to tell the
player, rather than replace their map without asking.

**Returns:** nothing.

#### `map.action(name, fn, options)`

Adds an entry called `name` to the map's menu and the palette, which runs
`fn()`. It stays while the script is loaded; a plugin's entries appear under
the plugin's name. `map.action(name)` removes the entry.

**Parameters:** `name` can be given by language, as for [`tr`](#trtexts).

**Options:** `checked = false` makes the entry a switch: choosing it calls
`fn(on)` with its new state.

**Returns:** nothing.

## Share code between scripts

When several scripts need the same helpers, put them in a module and load it
with `require`. This is how a plugin's scripts share code.

### `require(name)`

Loads `name.luau`, or `name/init.luau`. It looks in the folder of the script
that asks first, then in the folders of the other loaded scripts. A module is
loaded only once.

**Returns:** what the module returns.

**Example:**

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

**A trap:** in a plugin's folder, every `.luau` file also runs as a script,
modules included. So a module should only return what it has, and not make
triggers, keys or anything else.

## When something goes wrong

- An error shows as `script.luau:12: what is wrong`, and the editor keeps it.
  Calling something that does not exist names it:
  `there is no function gauges.sett`.
- An option a function does not take, something `on` does not know, a wrong
  key name or a wrong colour: each gives an error that lists what is valid.
- [`muddie.d.luau`](muddie.d.luau) has every function with its types. Load it
  in an editor with luau-lsp (as its definitions file), or pass it to
  `luau-analyze`. Typos show as you type. With `--!strict` at the top of the
  script, comparing a line with a text shows too.
- `muddie --debug` prints everything that happens to the standard output:
  - the text: `TEXT` for a line, `PROMPT` for a line the MUD marks as a
    prompt, `PARTIAL` for text without an end yet;
  - what is sent: `SENT`;
  - GMCP and the other protocols;
  - what scripts pass to `log`, and their errors: `SCRIPT`.

  `muddie --debug | tee muddie.log` saves it to a file. Passwords are never
  shown.

## What scripts cannot do

These limits are on purpose. Scripts cannot:

- Make their own windows, labels or bars. Use a text panel or a gauge
  instead: they are heard as well as seen.
- Read or write files, start programs, or download whatever they like. A
  script from someone else must not be able to do harm. Use `vars` to keep
  data.
- Add their own commands or settings to the palette. Use aliases, keys and
  `vars` instead.
- Order triggers by priority. Triggers run in the order they were made, and
  `wait` follows things that span several lines.
- Store their own data on each room of the map.
