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
--
-- What it shows is in the language of the interface, English or Spanish,
-- each text given in both with tr.

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
-- notes, marks, rooms to avoid and exits of their own, and the rooms the
-- map added where the player went, if the user switched that on (below).

local MAP = "https://mudlet.reinosdeleyenda.es/Mapa_Mudlet.zip"
-- Its name, in the language the interface is in when shown.
local SYNC = { en = "Sync with the official map", es = "Sincronizar con el mapa oficial" }

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
-- Putting the map right as the player goes. The map the MUD publishes names
-- some rooms with what was said or done in them as they were mapped
-- ("Moony te dice: ..."), with their names cut short ("Erial de los
-- Condenados: Ca"), or with nothing; the MUD renames rooms and opens exits
-- the map has not. The map follows the player by the way they go and the
-- exits written, and puts right what the game shows it to have wrong once
-- it is borne out, and says so. On unless switched off, from the map's
-- menu or the palette, and remembered in the profile.

local FIXING = "Reinos de Leyenda: corregir el mapa"
local FIXING_ACTION = { en = "Put right the map as you go", es = "Corregir el mapa sobre la marcha" }
local fixing = recall(FIXING) ~= false
map.put_right(fixing)
map.action(FIXING_ACTION, function(on)
    fixing = on
    remember(FIXING, on)
    map.put_right(on)
end, { checked = fixing })

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

---------------------------------------------------------------------------
-- Adding the rooms the map has not. Off unless the user switches it on,
-- from the map's menu or the palette, and remembered in the profile. A
-- room is added only by a move of the compass made, as Room.Movimiento
-- says, by an exit the map has leading nowhere, with the room written
-- just before it: with a name that is a room's, and exits that are all
-- directions, so that a line said with brackets is not taken for one.
-- The map adds it only if it knows for certain where the player was,
-- and links to the room if it has it already that way. Other moves, as
-- by a command or a portal, are not added.

local ADDING = "Reinos de Leyenda: añadir salas"
local ADDING_ACTION = { en = "Add the rooms the map has not", es = "Añadir al mapa las salas que no tiene" }
local adding = recall(ADDING) == true

-- Whether the room the map has the player in is the one the game wrote
-- last, by its name and exits: a room is only added after one that is,
-- so that none hangs from a room the map has the player in wrongly.
local confirmed = false

-- Whether every exit written is a direction.
local function all_directions(exits)
    if #exits == 0 then
        return false
    end
    for _, exit in exits do
        if not map.direction(exit) then
            return false
        end
    end
    return true
end

-- Adds the room written, if the move leads nowhere the map has; returns
-- whether the map now has the player there.
local function add_room(seen)
    if not (adding and confirmed and seen.written and seen.came and seen.name) then
        return false
    end
    if not map.direction(seen.came) or not all_directions(seen.exits or {}) then
        return false
    end
    local here = map.room()
    local exit = here and map.exit(here.id, seen.came)
    if not here or (exit and map.room(exit.to)) then
        return false
    end
    local id, how = map.add(seen)
    log(`Reinos: añadir {seen.name} por {seen.came} -> {id} {how}`)
    return id ~= nil
end

-- A switch: the map's menu shows it ticked while on, and the palette says
-- whether it is.
map.action(ADDING_ACTION, function(on)
    adding = on
    remember(ADDING, on)
    notify(if on
        then tr({
            en = "Rooms the map has not will be added where you go. They are drawn with a dashed edge; /map added lists them and /map undo undoes the last.",
            es = "Las salas que no estén en el mapa se añadirán donde vayas. Se ven con el borde discontinuo; /map added las muestra y /map undo deshace la última.",
        })
        else tr({ en = "Rooms are no longer added to the map.", es = "Ya no se añaden salas al mapa." }))
end, { checked = adding })

local function step(seen)
    -- What the game wrote in its text, rather than told over GMCP: it
    -- may be a room the map has not. A line whose brackets hold anything
    -- but directions was said, not a room. A name the map has no room of
    -- is kept only where the way taken leads to a room, which may be the
    -- one, named otherwise; elsewhere, the move alone is followed.
    if seen.written then
        if add_room(seen) then
            confirmed = true
            return
        end
        local here = map.room()
        local exit = here and seen.came and map.exit(here.id, seen.came)
        local leads = exit ~= nil and map.room(exit.to) ~= nil
        if not all_directions(seen.exits or {}) or (#map.named(seen.name) == 0 and not leads) then
            seen.name, seen.exits = nil, nil
        end
    end
    -- A move by an exit the map does not have, or that leads nowhere it
    -- knows, with no room written to check it by: it is said, and the map
    -- stays where it was.
    local here = map.room()
    local exit = here and seen.came and map.exit(here.id, seen.came)
    if seen.came and not seen.name and here and not (exit and map.room(exit.to)) then
        notify(tr({
            en = `The map has no exit {seen.came} from {here.name}: it has not followed you.`,
            es = `El mapa no tiene la salida {seen.came} desde {here.name}: no te ha seguido.`,
        }))
    end
    local now = map.arrive(seen)
    local there = now and map.room(now)
    confirmed = there ~= nil
        and seen.name ~= nil
        and table.find(map.named(seen.name), now) ~= nil
        and has_exits(there, seen.exits or {})
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
            notify(tr({
                en = `There is a new version of the official map. To bring it: “{SYNC.en}”, in the map's menu or the palette. What you have put right on the map is kept.`,
                es = `Hay una versión nueva del mapa oficial. Para traerla: «{SYNC.es}», en el menú del mapa o en la paleta. Lo que hayas corregido en el mapa se mantiene.`,
            }))
        end)
    end
    after(90, function()
        if not heard then
            notify(tr({
                en = "Reinos de Leyenda does not send your life, the channels or where you are until you write in the game: consentir gmcp on",
                es = "Reinos de Leyenda no manda la vida, los canales ni dónde estás hasta que escribas en el juego: consentir gmcp on",
            }))
        end
    end)
end)
