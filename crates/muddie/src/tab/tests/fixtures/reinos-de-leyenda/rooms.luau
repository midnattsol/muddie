-- The map of Reinos de Leyenda, which the MUD publishes made with Mudlet,
-- and the player followed on it, room by room.

local common = require("common")
local prompt = require("prompt")

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

-- The month a newer map was last told of, kept in the profile: told at each
-- connection, it would be noise.
local NEWER_TOLD = "reinos_newer_map_told"

map.action(SYNC, function()
    vars[NEWER_TOLD] = nil
    map.import(MAP)
end)

-- How its rooms are named: the region first, as in "Anduar: Plaza Mayor",
-- and some with their exits after, as in "Desierto de Sharframna [n,s]".
map.names = { region = ":", exits = "[]" }

-- It is played in Spanish: its words for directions are those of the
-- language, long and short, "norte" and "n" to "fuera" and "fu". One of its
-- own would go after it, with map.directions.
map.language = "es"

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

-- The room written last, waiting for the move that follows it.
local written_last = nil

local function exits_of(list)
    local exits = {}
    -- Doors are marked on their exits with | and -.
    for exit in (list:gsub("[|%-]", "")):gmatch("[^%[%],%s]+") do
        table.insert(exits, exit)
    end
    return exits
end

---------------------------------------------------------------------------
-- Keeping the map up to date as the player goes. The map the MUD publishes
-- names some rooms with what was said or done in them as they were mapped
-- ("Moony te dice: ..."), with their names cut short ("Erial de los
-- Condenados: Ca"), or with nothing; and the MUD renames rooms, opens
-- exits and builds rooms the map has not. The map learns from the room
-- the game writes on each move: it puts right what it has wrong once it
-- is borne out, and adds the rooms it has not, never from a room it does
-- not know for certain the player was in. It says what it changed, and it
-- can be undone. On unless switched off, from the map's menu or the
-- palette, and remembered in the profile.

local LEARNING = "reinos_learning"
local LEARNING_ACTION = { en = "Keep the map up to date as you go", es = "Mantener el mapa al día sobre la marcha" }
local learning = vars[LEARNING] ~= false
map.learn = learning
map.action(LEARNING_ACTION, function(on)
    learning = on
    vars[LEARNING] = on
    map.learn = on
end, { checked = learning })

-- Whether every exit written is a direction: else the line was something
-- said with brackets, not a room.
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

-- The map did not follow a move: told once, until it follows again.
local stray_told = false

local function step(seen)
    -- A line the game wrote whose brackets hold anything but directions
    -- was not a room: the move alone is followed.
    if seen.written and seen.exits and not all_directions(seen.exits) then
        seen.name, seen.exits = nil, nil
    end
    -- A move with no room written to check it by, by an exit the map has
    -- not or that leads to no room it has: it is said, and the map stays
    -- where it was.
    local here = map.room()
    local exit = here and seen.came and map.exit(here.id, seen.came)
    local stray = seen.came and not seen.name and here and not (exit and map.room(exit.to))
    if stray and not stray_told then
        notify(if exit
            then tr({
                en = `The map does not know where {seen.came} from {here.name} leads: it has not followed you.`,
                es = `El mapa no sabe adónde lleva {seen.came} desde {here.name}: no te ha seguido.`,
            })
            else tr({
                en = `The map has no exit {seen.came} from {here.name}: it has not followed you.`,
                es = `El mapa no tiene la salida {seen.came} desde {here.name}: no te ha seguido.`,
            }))
    end
    if seen.came then
        stray_told = stray
    end
    local now = map.arrive({ name = seen.name, exits = seen.exits, came = seen.came })
    -- Shown when Muddie is started with --debug.
    log(`Reinos: name={seen.name} exits={table.concat(seen.exits or {}, ",")} came={seen.came} -> {now}`)
end

-- The room as the game writes it. Each player sets what the prompt says and
-- how a room is shown: the prompt ends in no new line, and with lag the room
-- comes after it on its line, as "[PV 90] Anduar: Arco Este [s,e,n,o]". Known
-- the prompt, the room is the line, or what is after the prompt. Not known, the
-- game ends every one in ">", "]" or "}", exits between brackets too.

-- What a line may be, whole, with no prompt before it: the line, and what is
-- left of it after each sign that may end a prompt. The room is one of these
-- or it is not there. Never the last words of a line: anyone may say the name
-- of a room.
local function without_prompt(line)
    -- What comes after a prompt is a line of its own, and known as one
    -- (see aloud.luau): not here as well.
    if prompt.rest(line) then
        return {}
    end
    local parts = { line }
    if prompt.is_known() then
        return parts
    end
    local from = 1
    while true do
        local _, last = line:find("[>%]}]%s*", from)
        if not last or last >= #line then
            return parts
        end
        from = last + 1
        table.insert(parts, line:sub(from))
    end
end

-- The room a line writes: its name, if it is one the map has; else what
-- would be its name, if it can be one.
local function room_written(line)
    local parts = without_prompt(line)
    for _, part in parts do
        if #map.find(part, { exact = true }) > 0 then
            return part, nil
        end
    end
    -- What ends in a colon names what follows it, as "SL:" the exits.
    local last = parts[#parts] or ""
    return nil, if last:match("%S") and not last:match(":%s*$") then last else nil
end

-- Whether a name has a region, or a part, as the rooms of most places do:
-- "Anduar: Plaza Mayor", "Catedral de la Guerra - Tesorería".
local function like_a_room(name)
    return name:match("%S: %S") ~= nil or name:match("%S %- %S") ~= nil
end

-- What was written before exits with no move after it, and is no room the
-- map has, with the room the player was in: part of a prompt of several
-- lines, as "Salidas [n,s]", which comes in room after room, or a room only
-- looked at. What is known to be of the prompt, true, is not taken for the
-- room moved to.
local no_room = {}

local function looked_at(this)
    local name = this.unknown
    local here = map.room()
    if not like_a_room(name) then
        no_room[name] = true
    elseif here then
        local before = no_room[name] or here.id
        no_room[name] = if before == here.id then before else true
    else
        -- Lost: it may be a room the map has by another name.
        this.name = name
        step(this)
    end
end

local function written(this)
    -- A room the map has is not put aside for what comes after it.
    if written_last and written_last.name and not this.name then
        return
    end
    written_last = this
    after(0.2, function()
        if written_last ~= this then
            return
        end
        written_last = nil
        if this.name then
            step(this)
        elseif this.unknown then
            looked_at(this)
        end
    end)
end

-- The player is in the game, as a room is written: told once a connection,
-- for reinos.luau to look whether GMCP comes.
local entered = false

local function enter()
    if not entered then
        entered = true
        emit("reinos_in_game")
    end
end

-- With its exits after it, as most show it.
trigger([[^(.*?) \[([A-Za-z|,\-]+)\]\s*$]], function(m)
    local name, unknown = room_written(m[1])
    local exits = exits_of(m[2])
    if all_directions(exits) then
        enter()
    end
    written({ name = name, unknown = unknown, exits = exits, written = true })
end, { place = "Reinos de Leyenda/Room written" })

-- Or by its name alone, as some show it: only a name the map has.
trigger([[^(.*\S: \S.*?)\s*$]], function(m)
    if m[1]:match("%[[^%]]*%]%s*$") then
        return
    end
    local name = room_written(m[1])
    if name then
        enter()
        written({ name = name, written = true })
    end
end, { place = "Reinos de Leyenda/Room written without exits" })

on("gmcp", "Room.Movimiento", function(way)
    local seen = written_last or {}
    written_last = nil
    if not seen.name then
        if seen.unknown and no_room[seen.unknown] ~= true then
            seen.name = seen.unknown
        else
            seen.exits = nil
        end
    end
    seen.came = common.text_of(way)
    step(seen)
end)

-- A move that could not be made: it did not happen, and a walk stops.
on("gmcp", "Room.MovimientoErroneo", function()
    written_last = nil
    map.failed()
end)

on("gmcp", "Room.Actual", function(room)
    room = common.text_of(room)
    if not room then
        return
    end
    local name, list = room:match("^(.-)%s*(%b[])%s*$")
    step({ name = name or room, exits = list and exits_of(list) })
end)

-- Once per connection: what was seen is forgotten, and the map brought, or
-- looked whether the MUD's is newer.
on("connect", function()
    stray_told = false
    no_room = {}
    written_last = nil
    entered = false
    if map.count() == 0 then
        map.import(MAP)
    else
        map.newer(MAP, function()
            local month = os.date("%Y-%m")
            if vars[NEWER_TOLD] == month then
                return
            end
            vars[NEWER_TOLD] = month
            notify(tr({
                en = `There is a new version of the official map. To bring it: “{SYNC.en}”, in the map's menu or the palette. What you have put right on the map is kept.`,
                es = `Hay una versión nueva del mapa oficial. Para traerla: «{SYNC.es}», en el menú del mapa o en la paleta. Lo que hayas corregido en el mapa se mantiene.`,
            }))
        end)
    end
end)
