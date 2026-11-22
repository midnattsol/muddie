-- Reinos de Leyenda (rlmud.org), as it is played: the character's states,
-- what is said on its channels, each in its tab, and the map, which the
-- MUD publishes made with Mudlet, following the player on it. Its gauges
-- are among those of the editor.
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
-- Gauges: they are among the gauges of the editor, made from Char.Status,
-- life, energy, faith and the shield. Char.Status gives each as {min = now,
-- max = most}, at first all of them and then only what changed: it is
-- merged into what came before, for the gauges to have it whole.

gmcp_merge("Char.Status")
on_gmcp("Char", function()
    heard = true
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
-- The map. The MUD publishes it for Mudlet: it is taken when the session
-- has no map yet, and after that only when the user asks for it, from the
-- map's menu or the palette, so that the map never changes unasked. On
-- connecting it is looked whether the MUD's has changed, to say so. Taken
-- again, what the user set on the map stays: the names given to rooms,
-- notes, marks, rooms to avoid and exits of their own.

local MAP = "https://mudlet.reinosdeleyenda.es/Mapa_Mudlet.zip"
local SYNC = "Sincronizar con el mapa oficial"

map.action(SYNC, function()
    map.import(MAP)
end)

-- How its rooms are named: the region first, as in "Anduar: Plaza Mayor",
-- and some with their exits after, as in "Desierto de Sharframna [n,s]".
map.names({ region = ":", exits = "[]" })

-- It is played in Spanish: its words for directions are those of the
-- language, long and short, "norte" and "n" to "fuera" and "fu". One of its
-- own would go after it, with map.directions.
map.language("es")

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

---------------------------------------------------------------------------
-- Putting the map's names right. The map the MUD publishes has rooms with
-- no name, and rooms named with what was said or done in them as they were
-- mapped: "Moony te dice: ...", "Abres la puerta sur.", "Estás siendo
-- atacado por Lobo.". On coming to one, it is given the name the game
-- writes, if the exits the game writes are the room's, so that no other
-- room is named should the map have lost the player.

-- What a room's name is not: nothing, a sentence, or something said.
local NOT_NAMES = { " dice", "Dices ", " grita", " susurra", " exclama", "Abres ", "Estás ", "Empiezas " }

local function wrong_name(name)
    if name == "" or name:find("[%.!%?]%s*$") then
        return true
    end
    for _, said in NOT_NAMES do
        if name:find(said, 1, true) then
            return true
        end
    end
    return false
end

-- Whether every exit the game writes is one the room has.
local function has_exits(room, exits)
    if #exits == 0 then
        return false
    end
    for _, exit in exits do
        if not map.exit(room.id, exit) then
            return false
        end
    end
    return true
end

-- The rooms put right lately, told together: a few seconds after the first.
local renamed = {}

local function tell_renamed()
    if #renamed == 1 then
        local room = renamed[1]
        if room.was == "" then
            notify(`Mapa corregido: una sala sin nombre se llama ahora «{room.now}».`)
        else
            notify(`Mapa corregido: «{room.was}» se llama ahora «{room.now}».`)
        end
    elseif #renamed > 1 then
        notify(`Mapa corregido: {#renamed} salas tienen ahora su nombre.`)
    end
    renamed = {}
end

-- The room the game wrote is the one the move leads to, or, with no move,
-- the one the player is in: if the map has it named wrong, it is given
-- the name written.
local function put_right(seen)
    if not seen.name or wrong_name(seen.name) then
        return
    end
    local here = map.room()
    local there = here
    if here and seen.came then
        local exit = map.exit(here.id, seen.came)
        there = exit and map.room(exit.to)
    end
    if not there or not wrong_name(there.name) or not has_exits(there, seen.exits or {}) then
        return
    end
    if map.rename(there.id, seen.name) then
        table.insert(renamed, { was = there.name, now = seen.name })
        if #renamed == 1 then
            after(3, tell_renamed)
        end
    end
end

local function step(seen)
    -- What the game wrote in its text, rather than told over GMCP: it
    -- may be a room the map has named wrong. If it is not a room the map
    -- has, the line was not a room, and the move alone is followed.
    if seen.written then
        put_right(seen)
        if #map.named(seen.name) == 0 then
            seen.name, seen.exits = nil, nil
        end
    end
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

-- The room as the game writes it. The prompt ends in no new line, so with
-- lag the room comes after it on its line, as "] Anduar: Arco Este
-- [s,e,n,o]": what is before the name is let go, up to a name the map has
-- a room of, or else up to the last prompt.
trigger([[^(.*?) \[([A-Za-z|,\-]+)\]\s*$]], function(m)
    local name = m[1]
    while #map.named(name) == 0 do
        local rest = name:match("^.-[%]>]%s+(.+)$")
        if not rest then
            break
        end
        name = rest
    end
    local this = { name = name, exits = exits_of(m[2]), written = true }
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

-- Once per connection: the map, and whether GMCP comes.
on_event("connect", function()
    heard = false
    shown = nil
    if map.count() == 0 then
        map.import(MAP)
    else
        map.newer(MAP, function()
            notify(`Hay una versión nueva del mapa oficial. Para traerla: «{SYNC}», en el menú del mapa o en la paleta. Lo que hayas corregido en el mapa se mantiene.`)
        end)
    end
    after(90, function()
        if not heard then
            notify("Reinos de Leyenda no manda la vida, los canales ni dónde estás hasta que escribas en el juego: consentir gmcp on")
        end
    end)
end)
