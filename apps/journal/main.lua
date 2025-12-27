-- Mock Database
mockDb = {}

-- Helper to generate IDs
local function uuid()
    local template ='xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'
    return string.gsub(template, '[xy]', function (c)
        local v = (c == 'x') and math.random(0, 0xf) or math.random(8, 0xb)
        return string.format('%x', v)
    end)
end

-- We will receive 'req' as a table: { method = "...", body = { ... } }

function load_page(req)
    print("[Lua] load_page called")
    local body = req.body or {}
    local pageId = body.pageId
    
    if pageId and mockDb[pageId] then
        return mockDb[pageId]
    end
    return nil
end

function save_page(req)
    print("[Lua] save_page called")
    local body = req.body or {}
    local pageId = body.pageId
    local title = body.title
    local content = body.content
    
    if pageId then
        if not mockDb[pageId] then
             -- If not exists, maybe create it if we have all fields? 
             -- But usually we load first. 
             -- For the mock, we can just update if it exists or create new if we want.
             -- But wait, loadPreviousDaily creates entries. 
             -- If we save a page that doesn't exist (e.g. today created on init), we should create it.
             mockDb[pageId] = { id = pageId }
        end
        mockDb[pageId].title = title
        mockDb[pageId].content = content
        return true
    end
    return false
end

function load_previous_daily(req)
    print("[Lua] load_previous_daily called")
    local body = req.body or {}
    local currentDayStr = body.currentDayStr
    
    -- Find all pages with journal_day < currentDayStr
    local pages = {}
    for _, p in pairs(mockDb) do
        if p.journal_day and p.journal_day < currentDayStr then
            table.insert(pages, p)
        end
    end
    
    -- Sort descending
    table.sort(pages, function(a, b) return a.journal_day > b.journal_day end)
    
    if #pages > 0 then
        return pages[1]
    else
        -- Create one for previous day
        -- Lua date parsing is minimal. We assume YYYY-MM-DD.
        -- We can use os.time and os.date
        
        local y, m, d = currentDayStr:match("(%d+)-(%d+)-(%d+)")
        local currentTime = os.time({year=y, month=m, day=d})
        local prevTime = currentTime - 24 * 60 * 60
        local prevDayStr = os.date("%Y-%m-%d", prevTime)
        
        local newId = uuid()
        local newPage = {
            id = newId,
            title = prevDayStr,
            content = "",
            journal_day = prevDayStr
        }
        mockDb[newId] = newPage
        return newPage
    end
end

-- Initialize Today if needed (Logic from init() in JS, but maybe we expose a get_today function?)
-- Or we just let the frontend handle "today" creation logic by calling check?
-- The frontend JS: "Check if today exists in mockDb, if not create it".
-- We can expose `get_today` which ensures it exists.

function get_today(req)
    local today = os.date("%Y-%m-%d")
    
    -- Find today
    for _, p in pairs(mockDb) do
        if p.journal_day == today then
            return p
        end
    end
    
    -- Create today
    local id = uuid()
    local todayPage = {
        id = id,
        title = today,
        content = "",
        journal_day = today
    }
    mockDb[id] = todayPage
    return todayPage
end

-- Also let's seed yesterday as per JS stub
local today = os.date("%Y-%m-%d")
local yesterdayTime = os.time() - 24 * 60 * 60
local yesterdayStr = os.date("%Y-%m-%d", yesterdayTime)
local yId = uuid()
mockDb[yId] = {
    id = yId,
    title = yesterdayStr,
    content = "Yesterday was a good day. I started building the journal app.",
    journal_day = yesterdayStr
}

print("Journal App Loaded (Lua)")
