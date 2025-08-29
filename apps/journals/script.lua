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

-- Return the URL for today's daily journal page (YYYY-MM-DD)
function M.daily_url()
  local now = os.time()
  local date = os.date("%Y-%m-%d", now)
  return string.format("/journal/%s", date)
end

return M
