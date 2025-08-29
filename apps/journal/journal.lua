-- Lua entrypoint for journal plugin

local journal = {}

function journal.openUrl(url)
end

function journal.on_self_loaded()
    print("Journal plugin loaded!")
end

function journal.on_plugins_loaded()
end

return journal
