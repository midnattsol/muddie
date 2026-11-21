-- Reinos de Leyenda (rlmud.org): what is said on its channels, which it
-- sends with GMCP in packages of its own, each channel in its tab; and the
-- map, which the MUD publishes made with Mudlet.
--
-- The MUD sends nothing over GMCP until asked from inside the game:
--     consentir gmcp on
-- If nothing has come a while after connecting, this says so.

supports("Comm 1")

local heard = false

on_gmcp("Comm.MensajeCanal", function(data)
    heard = true
    if data.canal and data.mensaje then
        channel(data.canal, data.mensaje)
    end
end)

-- The channels the character is on: each has its tab before anyone
-- speaks on it.
on_gmcp("Comm.Canales", function(data)
    heard = true
    for name in pairs(data) do
        channel(name)
    end
end)

-- The map. The MUD publishes it for Mudlet: it is taken as soon as the
-- plugin is on, and shown, and on each connection brought up to date if
-- it has changed.
local MAP = "https://mudlet.reinosdeleyenda.es/Mapa_Mudlet.zip"
map.import(MAP)

-- Directions as the MUD says them, long and short; the first is what a
-- walk sends.
map.directions({
    n = { "norte", "n" },
    ne = { "noreste", "ne" },
    nw = { "noroeste", "no" },
    e = { "este", "e" },
    w = { "oeste", "o" },
    s = { "sur", "s" },
    se = { "sudeste", "se" },
    sw = { "sudoeste", "so" },
    u = { "arriba", "ar" },
    d = { "abajo", "ab" },
    ["in"] = { "dentro", "de" },
    out = { "fuera", "fu" },
})

-- The MUD does not say which room the player is in: it says the move made,
-- then shows the room as "Name [exits]". The map works out the room from
-- both, following the player.
local came = nil

on_gmcp("Room.Movimiento", function(way)
    heard = true
    if type(way) == "string" then
        came = way
    end
end)

on_gmcp("Room.MovimientoErroneo", function()
    came = nil
    map.stop()
end)

on_gmcp("Room.Actual", function(shown)
    heard = true
    if type(shown) ~= "string" then
        return
    end
    local name, list = shown:match("^(.-)%s*(%b[])%s*$")
    local exits = nil
    if name then
        exits = {}
        -- Doors are marked on their exits with | and -.
        for exit in (list:gsub("[|%-]", "")):gmatch("[^%[%],%s]+") do
            table.insert(exits, exit)
        end
    else
        name = shown
    end
    map.arrive({ name = name, exits = exits, came = came })
    came = nil
end)

-- Long enough to have logged in; once per connection.
on_event("connect", function()
    heard = false
    came = nil
    map.import(MAP)
    after(90, function()
        if not heard then
            echo("Reinos de Leyenda no manda los canales ni dónde estás hasta que escribas en el juego: consentir gmcp on")
        end
    end)
end)
