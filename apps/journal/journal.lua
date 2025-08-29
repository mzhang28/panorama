-- Lua entrypoint for journal plugin

local journal = {}

function journal.openUrl(url)
end

function journal.on_self_loaded()
    print("Journal plugin loaded!")
    print(("panorama v%s"):format(panorama.version))
    panorama.query()
end

function journal.on_plugins_loaded()
    panorama.openUrl(("/journal/%s"):format(panorama.date()))
end

return journal
