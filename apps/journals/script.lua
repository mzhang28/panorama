-- Simple Journal app Lua logic (POC)
-- Expose a single function `summary` that returns a short summary

local M = {}

function M.summary(content)
  if not content then return "" end
  local s = tostring(content)
  if #s > 80 then
    return string.sub(s,1,77) .. "..."
  end
  return s
end

return M

