-- For a MUD that sends each area complete, here in an imaginary GMCP
-- package "Area.Map". Nothing is saved: the server is the source of truth.

map.native = false
map.persist = false

on("gmcp", "Area.Map", function(area)
    map.clear()
    for _, room in area.rooms do
        map.set_room({
            id = room.id, name = room.name, area = area.name,
            x = room.x, y = room.y, z = room.z, exits = room.exits,
        })
    end
end)

on("gmcp", "Area.Position", function(id)
    local room = map.room(id)
    if room then map.visit(room) end
end)

-- A MUD that draws its own map in text can simply be shown as it comes.
on("gmcp", "Area.Ascii", function(text) panels.set("Map", text) end)
