-- Maps a MUD that has no GMCP by reading each room from the text.
-- The patterns below are an example; change them to match your MUD.
--
-- It expects rooms to look like this:
--   [ Market Square ]
--   Exits: north=Gate east=Alley

map.native(false)

local title
trigger("^\\[ (.+) \\]$", function(m) title = m[1] end)

trigger("^Exits: (.+)$", function(m)
    if not title then return end
    local exits = {}
    for direction, to in m[1]:gmatch("(%a+)=(%a+)") do
        exits[direction] = to
    end
    -- The title stands in for a room number, which this MUD does not give.
    map.visit({id = title, name = title, area = "world", exits = exits})
end)
