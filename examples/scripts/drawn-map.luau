-- For a MUD that draws the map itself and sends the drawing, here in a
-- GMCP package "Map.Render" with the lines of the drawing. The client
-- only shows it, in the panel beside the output, and tells the MUD how
-- much room there is so the drawing fits.

map.native(false)
support_gmcp("Map.Render 1")

-- The size in characters of the map's panel, whenever it changes.
on_event("panel", function(size)
    if not size.map then
        return
    end
    send_gmcp("Map.Render.Size", {
        width = size.columns,
        height = math.clamp(size.rows, 5, 60),
    })
end)

on_gmcp("Map.Render", function(render)
    panel("Map", table.concat(render.lines, "\n"))
end)
