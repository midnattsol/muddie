-- Reinos de Leyenda (rlmud.org), as it is played: what the MUD is asked to
-- send, and whether it sends it. Each part of the plugin is in a script of
-- its own, which can be switched off alone in the editor:
--
--   states.luau      the character's states, in the gauge Efectos
--   channels.luau    each channel in its tab
--   rooms.luau       the map, and the player followed on it
--   prompting.luau   the player's prompts, learned and not read
--   aloud.luau       what changes of the states, said
--   modes.luau       ways of playing, each reading more or less
--   history.luau     what is said and what happens, kept to read again
--   movement.luau    who comes and goes, and the target
--   life.luau        life lost and gained, said
--   keys.luau        keys as in VipMud
--   sounds.luau      the sounds of ScriptsRL
--
-- And what they share, which makes nothing by itself:
--
--   phrases.luau     what the game writes that the plugin knows by its words
--   prompt.luau      what a prompt is, and what is after it on its line
--   common.luau      the mode, whether a line is someone's words, lists
--   corrector.luau   what players write, as it is better read
--
-- The gauges of life, energy and faith are among those of the editor.
--
-- Most comes by GMCP, in packages of the MUD's own; this is what it sends,
-- as a session shows:
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
-- Tells and what is said in the room come only as text (phrases.luau).
--
-- What it shows is in the language of the interface, English or Spanish,
-- each text given in both.

gmcp.support("Char 1")
gmcp.support("Comm 1")
gmcp.support("Room 1")

-- Char.Status gives life, energy and faith each as {min = now, max = most},
-- at first all of them and then only what changed: it is merged into what
-- came before, for the gauges to have it whole.
gmcp.merge("Char.Status")

---------------------------------------------------------------------------
-- The MUD sends nothing over GMCP until asked from inside the game:
--     consentir gmcp on
-- If nothing has come a while after entering the game, this says so.

local heard = false
for _, package in { "Char", "Comm", "Room" } do
    on("gmcp", package, function()
        heard = true
    end)
end

local asking = nil

-- In the game, as rooms.luau tells once a room is written.
on("emit", "reinos_in_game", function()
    if asking then
        return
    end
    asking = after(90, function()
        if not heard then
            notify(tr({
                en = "Reinos de Leyenda does not send your life, the channels or where you are until you write in the game: consentir gmcp on",
                es = "Reinos de Leyenda no manda la vida, los canales ni dónde estás hasta que escribas en el juego: consentir gmcp on",
            }))
        end
    end)
end)

on("connect", function()
    heard = false
    if asking then
        asking:remove()
        asking = nil
    end
end)
