-- Reinos de Leyenda (rlmud.org): what is said on its channels, which it
-- sends with GMCP in packages of its own, each channel in its tab.
--
-- The MUD sends nothing over GMCP until asked from inside the game:
--     consentir gmcp on

supports("Comm 1")

on_gmcp("Comm.MensajeCanal", function(data)
    if data.canal and data.mensaje then
        channel(data.canal, data.mensaje)
    end
end)

-- The channels the character is on: each has its tab before anyone
-- speaks on it.
on_gmcp("Comm.Canales", function(data)
    for name in pairs(data) do
        channel(name)
    end
end)
