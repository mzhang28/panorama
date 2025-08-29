-- Lua entrypoint for journal plugin

local ipc = require("ipc")

local journal = {}

function journal.openUrl(url)
    -- Send openUrl request to Rust backend via IPC
    ipc.send_to_rust("openUrl:" .. url)
end

return journal
