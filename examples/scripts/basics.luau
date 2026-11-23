-- A tour of the everyday script functions. Copy what you need into
-- ~/.config/muddie/profiles/<your profile>/scripts/.

-- Speak tells, and keep a copy of them in a tab of the channels' panel.
trigger("^(?<who>\\w+) tells you: (?<what>.*)$", function(m, line)
    highlight("bright yellow")
    channel("Tells", line)
    speak(`{m.who} says {m.what}`, {interrupt = true})
end)

-- Hide a noisy line.
trigger("^The wind howls", function() gag() end)

-- "k orc" sends "kill orc".
alias("^k (.+)$", function(m) send("kill " .. m[1]) end)

-- Function keys for things done often.
macro("f1", "look")
macro("f2", "inventory")
macro("ctrl+h", function() send("cast heal me") end)

-- Walk with the numeric keypad. A bound keypad key stops typing its digit.
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad7", "northwest")
macro("numpad9", "northeast")
macro("numpad1", "southwest")
macro("numpad3", "southeast")
macro("numpadsubtract", "up")
macro("numpadadd", "down")
macro("numpad5", "look")

-- Count kills across sessions.
trigger("is DEAD!", function()
    local kills = (vars.kills or 0) + 1
    vars.kills = kills
    echo(`That makes {kills} kills.`)
end)

-- Combat triggers that only run while fighting.
trigger("^You are stunned", function() send("stand") end, {tags = {"combat"}})
trigger("^You start fighting", function() tags.enable("combat") end)
trigger("^You stop fighting", function() tags.disable("combat") end)
tags.disable("combat")
