# Your first scripts

Español: [first-scripts.es.md](first-scripts.es.md).

This guide is for you if you have never written a script. You will make a
few small ones, step by step, each useful in a real game. You do not need to
know how to program: every step shows the code, says what it does, and says
what you will see and hear.

When you want every detail of a function, look it up in the
[scripting reference](scripting.md). When you want to share your work with
other players, read [how to make a plugin](plugins.md).

## Contents

1. [Where scripts go](#where-scripts-go)
2. [Your first alias](#your-first-alias)
3. [Remembering things](#remembering-things)
4. [Your first trigger](#your-first-trigger)
5. [Saying things aloud, well](#saying-things-aloud-well)
6. [Keys and timers](#keys-and-timers)
7. [Doing things in steps](#doing-things-in-steps)
8. [Your character's state](#your-characters-state)
9. [Turning things on and off](#turning-things-on-and-off)
10. [When something goes wrong](#when-something-goes-wrong)
11. [Which one do I use?](#which-one-do-i-use)

## Where scripts go

A script is a small text file with instructions for Muddie. Muddie runs it
when the session starts, and it keeps working while you play.

Scripts are written in Luau, a simple language. You only need a few words of
it, and this guide shows them as they come.

To make a script:

1. Open the **Editor** with Ctrl+E.
2. Go to the **Scripts** section.
3. Press **+** and choose **Empty script**. The **+** also offers examples
   to start from.
4. Write the code. A moment after you stop typing, Muddie saves the script
   and runs it. The triangle beside its name runs it at once.

Press Ctrl+E again, or Escape, to go back to the game. The session keeps
running while you are in the editor.

Two more ways to run code:

- **Run Luau…**, in the palette (Ctrl+K), runs one line on the spot. It is
  handy to try something out.
- **Restart scripts**, in the palette, runs every script again from the
  start.

A script in the editor belongs to your profile. If you want a script in
every profile, write it in `init.luau`, in Muddie's configuration folder.

## Your first alias

An alias is a shortcut for something you type. You type a short word, and
Muddie sends the long command for you.

Say you are tired of typing `kill orc`. With this alias, you type `k orc`:

```lua
alias("^k (.+)$", function(m)
    send("kill " .. m[1])
end)
```

What each part means:

- `"^k (.+)$"` is the pattern: what you type must start with `k` and a
  space. The `(.+)` takes whatever comes after.
- `m[1]` is what `(.+)` took: `orc`.
- `send(...)` sends the command to the game.
- `..` joins two texts: `"kill "` and `"orc"` make `"kill orc"`.

Type `k orc`, and the game gets `kill orc`.

Mind the space inside `"kill "`. Without it, the game would get `killorc`:

```lua
-- Wrong: sends "killorc", with no space.
alias("^k (.+)$", function(m) send("kill" .. m[1]) end)
```

## Remembering things

`vars` keeps things for you, such as who you are fighting. What you put in
`vars` stays there when you close Muddie. You can also see it in the
**Variables** section of the editor.

These two aliases let you choose a target once, and then attack it with
one letter:

```lua
-- "t orc" chooses the target.
alias("^t (.+)$", function(m)
    vars.target = m[1]
    echo("Target: " .. m[1])
end)

-- "a" attacks it.
alias("^a$", function()
    if vars.target then
        send("kill " .. vars.target)
    else
        echo("Choose a target first, with t.")
    end
end)
```

`echo` shows a line in the output. If a screen reader reads Muddie, it reads
that line too.

The `if ... then ... else ... end` checks that there is a target before
attacking. Without it, `a` with no target would be an error.

## Your first trigger

A trigger reacts to what the game says. When a line matches its pattern,
Muddie runs your code.

### Reacting to a line

When the game says you are hungry, eat:

```lua
trigger("^You are hungry", function()
    send("eat bread")
end)
```

The `^` means "at the start of the line". So the trigger fires on
`You are hungry.` but not on `The orc says: You are hungry`.

### Changing how a line looks and sounds

A trigger also gets the line itself, as `line`. With it, you can change how
the line is shown and read:

```lua
-- Paint tells in yellow.
trigger("tells you", function(m, line)
    line:paint("yellow")
end)

-- Hide the advertising lines.
trigger("^\\[Ad\\]", function(m, line)
    line:hide()
end)

-- Read a long line shorter, and show it as it is.
trigger("^You slash the (\\w+) with your sword", function(m, line)
    line:read("Hit " .. m[1])
end)
```

`line:read` changes only what is heard: the screen still shows the whole
line.

### A line is not its text

To compare what a line says, use `line.text`. Comparing `line` itself never
matches, and Luau will not warn you:

```lua
trigger("^Done", function(m, line)
    -- Wrong: a line is never equal to a text, so this never runs.
    if line == "Done." then send("rest") end
    -- Right: line.text is what the line says.
    if line.text == "Done." then send("rest") end
end)
```

## Saying things aloud, well

Muddie gives you three ways to tell yourself something:

| Function | Shown | Read aloud |
|---|---|---|
| `echo` | In the output | Yes |
| `speak` | No | Yes |
| `notify` | In the bottom right corner, then under the bell | Yes, even in another session or with the game's speech off |

Each takes `aloud`, to say when it is heard:

- `aloud = "cut"` cuts short what is being read. Use it for what is urgent.
- `aloud = "wait"` waits until the reader is quiet. This is what happens if
  you say nothing.
- `aloud = "never"` is not heard at all.

And `always = true` is heard even when you have turned the game's speech
off. Keep it for what you must never miss:

```lua
trigger("^You are bleeding badly", function()
    speak("Bleeding!", {aloud = "cut", always = true})
end)
```

### Say what changes, not everything

The more a script talks, the less you hear the game. This script says every
line again, so you hear everything twice:

```lua
-- Too much: every line is said again.
trigger(".", function(m, line)
    speak(line.text)
end)
```

This one says only what matters, once, in a few words:

```lua
-- Better: one short word, only when it happens.
trigger("^The (\\w+) arrives", function(m)
    speak(m[1] .. " here")
end)
```

## Keys and timers

### Walking with the keypad

`macro` runs a command when you press a key. With these, the numeric keypad
walks:

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad5", "look")
```

You can use the function keys too, and keys with Ctrl or Alt:
`macro("f5", "score")`, `macro("ctrl+h", "heal")`.

### Doing something every so often

`every` runs code again and again. This saves your character every five
minutes:

```lua
every(300, function()
    send("save")
end)
```

`after` runs code once, after a while: `after(10, function() send("stand") end)`.

## Doing things in steps

Some things take several steps: drink a potion, wait to see if it worked,
try again. `wait` lets you write them one after another:

```lua
alias("^cure$", function()
    send("drink potion")
    local m = wait("line", "^You feel better", {timeout = 3})
    if m then
        echo("Cured.")
        return
    end
    -- It did not work in 3 seconds: try once more.
    send("drink potion")
end)
```

`wait("line", ...)` stops this alias until the game says `You feel better`,
or until 3 seconds go by. Meanwhile, the rest of Muddie and your other
scripts keep working.

`wait(2)` waits two seconds.

## Your character's state

A gauge shows a value, such as your life, as a bar. It also speaks when the
value drops. With a screen reader, you hear all your gauges with Alt+0.

Many games send your life over GMCP, a channel of data that comes with the
text. If your game sends it in `Char.Vitals`, this makes a gauge of it:

```lua
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("Life", {value = vitals.hp, max = vitals.maxhp})
end)
```

`on` listens for something, here the GMCP message `Char.Vitals`. The names
`hp` and `maxhp` change from game to game. To see what yours sends, start
Muddie with `--debug` (see [When something goes wrong](#when-something-goes-wrong)).

Now a warning when life falls under 30 %. It fires once, and not again until
your life has come back up:

```lua
on("gauge", "Life", function()
    speak("Life low!", {aloud = "cut"})
end, {under = 30})
```

## Turning things on and off

Sometimes you want a script to rest: no automatic eating while you are in a
quest, say. Give it a place, and switch the place off.

```lua
trigger("^You are hungry", function()
    send("eat bread")
end, {place = "Food/Eat"})

alias("^nofood$", function()
    triggers.disable("Food")
    echo("Automatic eating off.")
end)

alias("^food$", function()
    triggers.enable("Food")
    echo("Automatic eating on.")
end)
```

`place = "Food/Eat"` puts the trigger in a folder called `Food`. Switching
the folder off switches off everything in it.

Tags do the same across kinds: give `tags = {"combat"}` to triggers, aliases
and keys, and `tags.disable("combat")` switches them all off at once.

## When something goes wrong

Everybody makes mistakes. Muddie tells you where yours are.

- **The error message.** A mistake shows in the bottom right corner, such as
  `combat.luau:12: there is no function gauges.sett`. It gives the script,
  the line, and what is wrong. The editor marks that line too.
- **Trying a trigger without the game.** In **Run Luau…**, type
  `session.feed("You are hungry.")`. Muddie takes the text as if the game had
  sent it, and your triggers react.
- **Seeing everything.** Start Muddie with `muddie --debug`. It shows, in the
  terminal, every line that comes, every command sent, every GMCP message and
  every error. To keep it in a file: `muddie --debug | tee muddie.log`.
- **Catching typos before running.** [muddie.d.luau](muddie.d.luau) lists
  every function. Give it to `luau-analyze`, or to an editor with luau-lsp,
  and it marks a misspelt name as you write it.

## Which one do I use?

| You want to… | Use |
|---|---|
| Type less | An alias: `alias` |
| React to what the game says | A trigger: `trigger` |
| Run something with a key | A key: `macro` |
| Do something every so often, or later | A timer: `every`, `after` |
| Remember something, even after closing | `vars` |
| Show a line | `echo` |
| Say something without showing it | `speak` |
| Tell yourself something important | `notify` |
| Do things in steps | `wait` |
| Show your life, energy and so on | A gauge: `gauges.set` |
| React to data the game sends | `on` |
| Switch things off for a while | `place`, `tags`, `triggers.disable` |

### Where to go next

- The [scripting reference](scripting.md) has every function, with all its
  options. For example: [`trigger`](scripting.md#triggerpattern-fn-options),
  [`speak`](scripting.md#speaktext-options),
  [`wait`](scripting.md#waitline-pattern-options) and
  [`gauges.set`](scripting.md#gaugessetname-gauge).
- [How to make a plugin](plugins.md) shows how to gather your scripts and
  share them with every player of your game.
