-- For a MUD that says which words are worth completing, here in GMCP
-- packages under "Dev.Completions" carrying a list of words. Tab then
-- offers them along with the words seen in the output.

gmcp.support("Dev 1")

-- Each package keeps its own list; a new one replaces the one before.
on("gmcp", "Dev.Completions", function(data, package)
    session.completions(package, data.words)
end)
