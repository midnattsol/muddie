-- The player's prompts: learned as the game says them, not read, and what
-- the game writes after one on its line taken as a line of its own.
--
-- Reinos ends the prompt with no new line, and with lag what comes next
-- joins it: "Pv:90\90 Jgd:> Ana te dice: hola". That is shown, read and known
-- by every trigger of the plugin as the line it is, "Ana te dice: hola",
-- as if the game had ended the prompt before it; the prompt alone is shown
-- and not read: what is in it is said by the gauges as it runs low, and
-- read with Alt+0. How a prompt is told apart is in prompt.luau.

local prompt = require("prompt")
local phrases = require("phrases")

---------------------------------------------------------------------------
-- Learning them: the game says each whole, on lines as long as it writes
-- them, as "Tu prompt ha sido definido como 'Pv:$v\$V Jgd:$j>'.", and that
-- of a fight with "de combate".

trigger(phrases.prompt_made, function(m)
    local template = m[2]
    while not template:match("'%.%s*$") do
        local _, line = wait("line", "", { timeout = 1, prompt = false })
        if not line then
            return
        end
        template ..= " " .. line.text
    end
    prompt.learn(m[1] ~= nil and m[1] ~= "", (template:gsub("'%.%s*$", "")))
end, { place = "Reinos de Leyenda/Prompt" })

---------------------------------------------------------------------------
-- Not read, and what is after one a line of its own.

trigger("^(.*)$", function(_, line)
    local rest = prompt.rest(line.text)
    if rest == "" then
        line:mute()
    elseif rest then
        line:replace(line.text:sub(1, #line.text - #rest))
        line:mute()
        session.feed(rest .. "\n")
    end
end, { place = "Reinos de Leyenda/Prompt aloud" })

---------------------------------------------------------------------------
-- One not known is read each time: once in the game, that is told, once,
-- with how to make it known.

local playing = false
on("gmcp", "Char", function()
    playing = true
end)
on("disconnect", function()
    playing = false
end)

trigger("\\S", function(_, line)
    if playing and not vars.reinos_prompt_told and not prompt.is_known() and not prompt.rest(line.text) then
        vars.reinos_prompt_told = true
        notify({
            en = "Your prompt is read each time. For it not to be, make it again in the game: prompt, and promptcombate for that of a fight.",
            es = "Se te lee el prompt cada vez. Para que no, defínelo otra vez en el juego: prompt, y promptcombate para el de combate.",
        })
    end
end, { place = "Reinos de Leyenda/Prompt not known", prompt = true })
