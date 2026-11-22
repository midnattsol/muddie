-- Gauges for a MUD whose Char.Vitals uses field names muddie does not know.
-- Adjust the field names to what your MUD sends; see them with
--   lua on_gmcp("Char", function(data, package) echo(package) end)

on_gmcp("Char.Vitals", function(vitals)
    gauge("HP", vitals.health, vitals.health_max)
    gauge("Mana", vitals.mana, vitals.mana_max)
    -- A warning by voice and sound when health is low.
    if vitals.health < vitals.health_max * 0.25 then
        speak("Health low", {interrupt = true})
        play("alarm.ogg", {volume = 0.6})
    end
end)
