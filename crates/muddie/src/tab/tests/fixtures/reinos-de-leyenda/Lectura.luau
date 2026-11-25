-- What is read aloud of Reinos de Leyenda to who listens to the game: here,
-- and only here, apart from what the gauges say by themselves, which each
-- says in the editor. It says nothing when nothing is read aloud, and it can
-- be switched off alone, among the scripts: what is shown stays.
--
-- What it says is in the language of the interface, English or Spanish.

---------------------------------------------------------------------------
-- States: of those the character is in, only what changes: the name of one
-- that comes, and that one is gone. The gauge Efectos has them all, whether
-- it is shown or not; those there are on logging in are not news.

on("gauge", "Efectos", function(_, change)
    for _, state in change.came do
        speak(`{state.name}.`)
    end
    for _, state in change.gone do
        speak({ en = `No longer: {state.name}.`, es = `Ya no: {state.name}.` })
    end
end)
