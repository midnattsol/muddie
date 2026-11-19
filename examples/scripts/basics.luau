-- A tour of the everyday script functions. Copy what you need into
-- ~/.config/muddie/profiles/<your profile>/scripts/.

-- Speak tells, and keep a copy of them in the chat panel.
trigger("^(?<who>\\w+) tells you: (?<what>.*)$", function(m, line)
    highlight("bright yellow")
    chat(line)
    speak(`{m.who} says {m.what}`, {interrupt = true})
end)

-- Hide a noisy line.
trigger("^The wind howls", function() gag() end)

-- "k orc" sends "kill orc".
alias("^k (.+)$", function(m) send("kill " .. m[1]) end)

-- Function keys for things done often.
bind("f1", "look")
bind("f2", "inventory")
bind("ctrl+h", function() send("cast heal me") end)

-- Walk with the numeric keypad. A bound keypad key stops typing its digit.
bind("numpad8", "north")
bind("numpad2", "south")
bind("numpad4", "west")
bind("numpad6", "east")
bind("numpad7", "northwest")
bind("numpad9", "northeast")
bind("numpad1", "southwest")
bind("numpad3", "southeast")
bind("numpadsubtract", "up")
bind("numpadadd", "down")
bind("numpad5", "look")

-- Count kills across sessions.
trigger("is DEAD!", function()
    local kills = (recall("kills") or 0) + 1
    remember("kills", kills)
    echo(`That makes {kills} kills.`)
end)

-- Combat triggers that only run while fighting.
trigger("^You are stunned", function() send("stand") end, {group = "combat"})
trigger("^You start fighting", function() enable("combat", true) end)
trigger("^You stop fighting", function() enable("combat", false) end)
enable("combat", false)
