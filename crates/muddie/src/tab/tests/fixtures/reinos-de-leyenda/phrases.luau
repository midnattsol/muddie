-- What Reinos de Leyenda writes that the plugin knows by its words, all in
-- one place: what the game does not send as data (GMCP) can only be known
-- by how it is written, and if the game changes how, it is changed here and
-- nowhere else. Each is a pattern of a trigger, matched against a whole line,
-- with what comes after a prompt on its line taken as a line of its own
-- (see aloud.luau). Where the shape of a line is enough, the words are not
-- listed: a tell is any "Someone te <word>: text".
--
--   local phrases = require("phrases")
--   trigger(phrases.tell, function(m, line) ... end)
--
-- A module: it makes nothing, it only returns what it has.

local phrases = {}

-- A tell, a whisper or anything said to you alone, however it is said, and
-- those of while you were away: "Ana te dice: hola", "Ana te susurra: ven",
-- "... - Ana te dijo: hola". The one who says it, then what is said.
phrases.tell = [[^(?:.* - )?(\S+) te \p{L}+(?: [^:]*)?: (.+)$]]

-- What makes a line a tell, to tell it from what is said in the room.
phrases.to_you = " te [^%s:]+[^:]*: "

-- What is said in the room: "Ana dice en común: hola", "Ana pregunta: ¿sí?".
-- Its words are listed, as lines of the game of the same shape are not
-- said by anyone ("Página web: ..."); a word more is added here.
phrases.said_in_the_room =
    [[^(\S+(?: \S+)?) (?:dice|pregunta|exclama|susurra|responde|balbucea|ordena|grita|murmura)\b[^:]*: (.+)$]]

-- Someone comes or goes: "Orco llega desde el sur.", "Orco se va hacia el
-- norte seguido de Ana.": who, then the way.
phrases.arrives = [[^(\S+(?: \S+){0,3}) llegan? (?:\S+ )?(?:desde|de) (.+?)(?: seguid[oa]s? de .+)?\.$]]
phrases.leaves =
    [[^(\S+(?: \S+){0,3}) se van? (?:\S+ )?(?:hacia|en direcci[oó]n(?: a)?) (.+?)(?: seguid[oa]s? de .+)?\.$]]

-- Who is here: "Orco está aquí.", "Ana, Bob y un orco están aquí."
phrases.here = [[^(.+?) est[aá]n? aqu[ií]\.$]]

-- What happens in a fight.
phrases.attacked = [[^Est[aá]s siendo atacad[oa] por (.+?)\.?$]]
phrases.experience = [=[^\[Obtienes .+ puntos de experiencia\]]=]
phrases.you_kill = [[^Propinas el golpe mortal a .+$]]
phrases.someone_kills = [[^.+ propina el golpe mortal a .+$]]
phrases.dies = [[^.+ cae al suelo sin vida\.$]]
phrases.you_find = [[^¡?Descubres a .+ intentando moverse en silencio!?$]]
phrases.someone_finds = [[^.+ descubre a .+ intentando moverse en silencio!?$]]

-- The group.
phrases.group_joins = [[^\[Grupo\] .+ (?:se une al|abandona el) grupo\.$]]
phrases.group_out = [[^\[Grupo\] .+ es expulsad[oa] del grupo\.$]]
phrases.group_you = [[^\[Grupo\] (?:Te unes al|Abandonas el) grupo liderado por .+$]]

-- The prompt made in the game, the usual one or that of a fight: whether
-- it is of a fight, then how it is made.
phrases.prompt_made = [[^Tu prompt (de combate )?ha sido definido como '(.*)$]]

return phrases
