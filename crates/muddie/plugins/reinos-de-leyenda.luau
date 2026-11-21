-- Reinos de Leyenda (rlmud.org), as it is played: the character's gauges
-- and states, what is said on its channels, each in its tab, and the map,
-- which the MUD publishes made with Mudlet, following the player on it.
-- All by GMCP, in packages of the MUD's own; this is what it sends, as a
-- session shows:
--
--   on logging in    Char.Status, Char.Status.Estados.set, Char.Ficha,
--                    Char.Stats, Comm.Canales, Room.Actual once
--   on each step     the room in its text as "Name [exits]", then
--                    Room.Movimiento "este"; or Room.MovimientoErroneo
--                    "sudeste" for a move that could not be made
--   as it happens    Char.Status (what changed only), Char.Status.Estados
--                    .add and .remove, Comm.MensajeCanal, and
--                    Comm.EnciendeCanal and Comm.ApagaCanal
--
-- The MUD sends nothing over GMCP until asked from inside the game:
--     consentir gmcp on
-- If nothing has come a while after connecting, this says so.

supports("Char 1")
supports("Comm 1")
supports("Room 1")

local heard = false

-- Text, as it comes or inside a table of the MUD's.
local function text_of(value)
    if type(value) == "string" then
        return value
    end
    if type(value) == "table" then
        for _, field in { "nombre", "name", "canal", "salida", "direccion", 1 } do
            if type(value[field]) == "string" then
                return value[field]
            end
        end
    end
    return nil
end

---------------------------------------------------------------------------
-- Gauges, for what is urgent in a fight: life, energy and faith (and the
-- shield, while there is one). Char.Status gives each as {min = now, max =
-- most}, at first all of them and then only what changed, so what came
-- before is kept. All alike: green, yellow under 60 %, red under 30 %, a
-- whole bar. Faith and the shield are shown only for a character who has
-- them; experience is not, the game shows it well enough.

local GAUGES = {
    { key = "pvs", name = "Vida", always = true },
    { key = "pe", name = "Energía", always = true },
    { key = "fe", name = "Fe" },
    { key = "escudo", name = "Escudo" },
}

local LOOK = {
    colour = "#5fb85f",
    style = "bar",
    steps = { { 60, "#e0c040" }, { 30, "#e05050" } },
}

local status = {}

on_gmcp("Char.Status", function(data, package)
    -- Char.Status.Estados comes here too: that is below.
    if package ~= "Char.Status" or type(data) ~= "table" then
        return
    end
    heard = true
    for key, value in pairs(data) do
        if type(value) == "table" then
            local known = status[key] or {}
            for field, number in pairs(value) do
                known[field] = number
            end
            status[key] = known
        end
    end
    for _, shown in GAUGES do
        local value = status[shown.key]
        if value and value.max then
            if shown.always or value.max > 0 then
                gauge(shown.name, value.min or 0, value.max, LOOK)
            else
                gauge(shown.name)
            end
        end
    end
end)

---------------------------------------------------------------------------
-- States: each a light among the gauges, by its name. They are not said
-- aloud: the game says so itself, in its text. A state is {nombre, corto,
-- tipo, ayuda}.

local states = {}

local function state_on(state)
    local name = text_of(state)
    if not name or states[name] then
        return
    end
    states[name] = true
    gauge(name, 1, 1, { style = "light", colour = "#e0a040" })
end

local function state_off(state)
    local name = text_of(state)
    if not name or not states[name] then
        return
    end
    states[name] = nil
    gauge(name)
end

on_gmcp("Char.Status.Estados.set", function(data)
    for name in pairs(states) do
        state_off(name)
    end
    if type(data) == "table" then
        for key, state in pairs(data) do
            state_on(type(state) == "table" and state or key)
        end
    end
end)

on_gmcp("Char.Status.Estados.add", state_on)
on_gmcp("Char.Status.Estados.remove", state_off)

---------------------------------------------------------------------------
-- Channels: each in its tab, made before anyone speaks on it, and taken
-- away when the character leaves it. What was said before logging in comes
-- too.

local function said(data)
    heard = true
    if type(data) == "table" and data.canal and data.mensaje then
        channel(data.canal, data.mensaje)
    end
end

on_gmcp("Comm.MensajeCanal", said)
on_gmcp("Comm.MensajeCanalHistorico", said)

on_gmcp("Comm.Canales", function(data)
    heard = true
    for name in pairs(data) do
        channel(name)
    end
end)

on_gmcp("Comm.EnciendeCanal", function(data)
    local name = text_of(data)
    if name then
        channel(name)
    end
end)

on_gmcp("Comm.ApagaCanal", function(data)
    local name = text_of(data)
    if name then
        channel_close(name)
    end
end)

---------------------------------------------------------------------------
-- The map. The MUD publishes it for Mudlet: it is taken as soon as there is
-- a session to take it into, and shown, and on each connection brought up
-- to date if it has changed.

local MAP = "https://mudlet.reinosdeleyenda.es/Mapa_Mudlet.zip"
map.import(MAP)

-- How its rooms are named: the region first, as in "Anduar: Plaza Mayor",
-- and some with their exits after, as in "Desierto de Sharframna [n,s]".
map.names({ region = ":", exits = "[]" })

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

-- Following the player. The MUD tells three things, each used for what it
-- is good for:
--   Room.Movimiento "este"   that a move was made, and by which exit: what
--                            moves the player on the map, each one as it
--                            comes, in order, whatever the lag;
--   "Name [exits]"           the room, as the game writes it in its text
--                            just before that move, and again on looking:
--                            what checks the map is right, and puts it
--                            right if it is not;
--   Room.Actual              the room, once, on logging in.
-- The room written is taken with the move that follows it; one written
-- with no move after it, as on looking, is taken by itself a moment later.

local shown = nil

local function exits_of(list)
    local exits = {}
    -- Doors are marked on their exits with | and -.
    for exit in (list:gsub("[|%-]", "")):gmatch("[^%[%],%s]+") do
        table.insert(exits, exit)
    end
    return exits
end

local function step(seen)
    -- A move by an exit the map does not have, with no room written to
    -- check it by: it is said, and the map stays where it was.
    local here = map.room()
    if seen.came and not seen.name and here and not map.exit(here.id, seen.came) then
        notify(`El mapa no tiene la salida {seen.came} desde {here.name}: no te ha seguido.`)
    end
    local now = map.arrive(seen)
    -- Shown when Muddie is started with --debug.
    log(`Reinos: name={seen.name} exits={table.concat(seen.exits or {}, ",")} came={seen.came} -> {now}`)
end

-- The room as the game writes it: only a name the map has a room of, so
-- that no other line that ends in brackets is taken for one. The prompt
-- ends in no new line, so with lag the room comes after it on its line,
-- as "] Anduar: Arco Este [s,e,n,o]": what is before the name is let go.
trigger([[^(.*?) \[([A-Za-z|,\-]+)\]\s*$]], function(m)
    local name = m[1]
    while #map.named(name) == 0 do
        name = name:match("^.-[%]>]%s+(.+)$")
        if not name then
            return
        end
    end
    local this = { name = name, exits = exits_of(m[2]) }
    shown = this
    after(0.2, function()
        if shown == this then
            shown = nil
            step(this)
        end
    end)
end, { name = "Reinos de Leyenda/Sala escrita" })

on_gmcp("Room.Movimiento", function(way)
    heard = true
    local seen = shown or {}
    shown = nil
    seen.came = text_of(way)
    step(seen)
end)

-- A move that could not be made: it did not happen, and a walk stops.
on_gmcp("Room.MovimientoErroneo", function()
    shown = nil
    map.stop()
end)

on_gmcp("Room.Actual", function(room)
    heard = true
    room = text_of(room)
    if not room then
        return
    end
    local name, list = room:match("^(.-)%s*(%b[])%s*$")
    step({ name = name or room, exits = list and exits_of(list) })
end)

-- Long enough to have logged in; once per connection.
on_event("connect", function()
    heard = false
    shown = nil
    status = {}
    map.import(MAP)
    after(90, function()
        if not heard then
            notify("Reinos de Leyenda no manda la vida, los canales ni dónde estás hasta que escribas en el juego: consentir gmcp on")
        end
    end)
end)
