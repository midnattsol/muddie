-- What many MUDs send the same way: the map from GMCP Room.Info or MSDP
-- ROOM, and the channels from Comm.Channel.Text. The gauges of Char.Vitals
-- and of the MSDP vitals are this plugin's rules, in the editor.

gmcp.support("Char 1", "Room 1", "Comm.Channel 1")
msdp.report("HEALTH", "HEALTH_MAX", "MANA", "MANA_MAX", "MOVEMENT", "MOVEMENT_MAX", "ROOM")

-- The room the player is in, unless a script of the profile draws the map
-- its own way.
local function arrived(room)
    if map.native() and type(room) == "table" then
        map.visit(room)
    end
end
on("gmcp", "Room.Info", arrived)
on("msdp", "ROOM", arrived)

-- What is said on a channel, in its tab.
on("gmcp", "Comm.Channel.Text", function(said)
    if type(said) == "table" and said.text then
        channels.say(said.channel or "", said.text)
    end
end)
