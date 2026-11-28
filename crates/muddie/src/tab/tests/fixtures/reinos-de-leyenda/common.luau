-- What several scripts of the plugin ask, in one place: the mode there is,
-- whether a line is someone's words, keeping a list, and the text inside
-- what the MUD sends.
--
--   local common = require("common")
--   common.mode()                       -- "combat", "xp", "idle"...
--   common.words("Ana dice: hola")      -- true: someone said it
--   common.keep(list, "text", 50)       -- first in the list, 50 at most
--   common.text_of({ nombre = "Ana" })  -- "Ana"
--
-- A module: it makes nothing, it only returns what it has.

local common = {}

-- The way of playing there is, which modes.luau keeps and changes: combat
-- until another is chosen.
function common.mode(): string
    return if type(vars.reinos_mode) == "string" then vars.reinos_mode else "combat"
end

-- Whether a line is said on a channel: those begin with the channel between
-- brackets, as "[chat] Ana: hola".
function common.of_a_channel(text: string): boolean
    return text:match("^%[") ~= nil
end

-- Whether a line is someone's words, on a channel or said, and so not what
-- the game tells: what anyone says may look like anything.
function common.words(text: string): boolean
    return common.of_a_channel(text) or text:match(": ") ~= nil
end

-- Puts `text` first in `list`, which keeps `most` at most.
function common.keep(list: { string }, text: string, most: number)
    table.insert(list, 1, text)
    if #list > most then
        table.remove(list)
    end
end

-- Text, as it comes or inside a table of the MUD's, as its name.
function common.text_of(value: any): string?
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

return common
