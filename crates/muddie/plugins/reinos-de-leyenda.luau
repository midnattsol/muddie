-- Reinos de Leyenda (rlmud.org): what is said on its channels, which it
-- sends with GMCP in packages of its own, each channel in its tab.
--
-- The MUD sends nothing over GMCP until asked from inside the game:
--     consentir gmcp on
-- If nothing has come a while after connecting, this says so.

supports("Comm 1")

local heard = false

on_gmcp("Comm.MensajeCanal", function(data)
    heard = true
    if data.canal and data.mensaje then
        channel(data.canal, data.mensaje)
    end
end)

-- The channels the character is on: each has its tab before anyone
-- speaks on it.
on_gmcp("Comm.Canales", function(data)
    heard = true
    for name in pairs(data) do
        channel(name)
    end
end)

-- Long enough to have logged in; once per connection.
on_event("connect", function()
    heard = false
    after(90, function()
        if not heard then
            echo("Reinos de Leyenda no manda los canales hasta que escribas en el juego: consentir gmcp on")
        end
    end)
end)
