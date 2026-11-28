-- The character's states, as envenenado or aturdido: each a label of the
-- gauge Efectos, with what it is, the worst kinds first, in their colour.
-- What changes of them is said by aloud.luau, which can be switched off
-- alone; what is shown stays.
--
-- The MUD sends them over GMCP: all on logging in (Char.Status.Estados.set),
-- then each that comes (.add) and goes (.remove). A state is {nombre, corto,
-- tipo, ayuda}, or its name alone.

local common = require("common")

-- The colour of each kind of state, the worst first.
local KINDS = {
    enfermedades = { colour = "#e05050", order = 1 },
    incapacidades = { colour = "#e0c040", order = 2 },
}

-- A state as the gauge shows it, with what it is known by.
local function label(state)
    if type(state) ~= "table" then
        state = { nombre = state }
    end
    local kind = KINDS[state.tipo or ""]
    local id = common.text_of(state) or ""
    return {
        id = id,
        name = state.corto or id,
        about = state.ayuda or "",
        colour = kind and kind.colour or "",
        order = kind and kind.order or 99,
    }
end

-- Those the character is in.
local states = {}

local function show()
    table.sort(states, function(a, b)
        if a.order ~= b.order then
            return a.order < b.order
        end
        return a.name < b.name
    end)
    local items = {}
    for _, state in states do
        table.insert(items, { name = state.name, about = state.about, colour = state.colour })
    end
    gauges.set("Efectos", {
        kind = "effects",
        items = items,
        colour = "#8a9bb8",
        aloud = "never",
        panel = "Efectos",
        place = "Reinos de Leyenda/Efectos",
    })
end

-- Takes out the state known by `id`.
local function leave(id)
    for index, known in states do
        if known.id == id then
            table.remove(states, index)
            return
        end
    end
end

on("gmcp", "Char.Status.Estados.set", function(data)
    states = {}
    if type(data) == "table" then
        for key, state in pairs(data) do
            table.insert(states, label(type(state) == "table" and state or key))
        end
    end
    show()
end)

on("gmcp", "Char.Status.Estados.add", function(state)
    local new = label(state)
    leave(new.id)
    table.insert(states, new)
    show()
end)

on("gmcp", "Char.Status.Estados.remove", function(state)
    leave(common.text_of(state))
    show()
end)

on("disconnect", function()
    states = {}
end)
