-- The channels: each in its tab, made before anyone speaks on it, and taken
-- away when the character leaves it. What was said on them before logging
-- in comes too, kept and not read: it comes all at once.
--
-- The MUD sends them over GMCP: which there are on logging in
-- (Comm.Canales), each message (Comm.MensajeCanal, and
-- Comm.MensajeCanalHistorico for those of before), and each channel the
-- character joins (Comm.EnciendeCanal) and leaves (Comm.ApagaCanal).

local common = require("common")

local function said(data, before: boolean)
    if type(data) == "table" and data.canal and data.mensaje then
        channels.say(data.canal, data.mensaje, { aloud = if before then "never" else "usual" })
    end
end

on("gmcp", "Comm.MensajeCanal", function(data)
    said(data, false)
end)

on("gmcp", "Comm.MensajeCanalHistorico", function(data)
    said(data, true)
end)

on("gmcp", "Comm.Canales", function(data)
    for name in pairs(data) do
        channels.open(name)
    end
end)

on("gmcp", "Comm.EnciendeCanal", function(data)
    local name = common.text_of(data)
    if name then
        channels.open(name)
    end
end)

on("gmcp", "Comm.ApagaCanal", function(data)
    local name = common.text_of(data)
    if name then
        channels.close(name)
    end
end)
