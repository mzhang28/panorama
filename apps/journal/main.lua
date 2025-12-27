-- Helper to generate IDs
local function uuid()
    local template = 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'
    return string.gsub(template, '[xy]', function(c)
        local v = (c == 'x') and math.random(0, 0xf) or math.random(8, 0xb)
        return string.format('%x', v)
    end)
end

function load_page(req)
    print("[Lua] load_page called")
    local body = req.body or {}
    local pageId = body.pageId
    if not pageId then return nil end

    local recordId = "nodes:`" .. pageId .. "`"
    local sql = "SELECT * FROM " .. recordId

    local res = db.query(sql, {})
    if res and #res > 0 then
        local page = res[1]

        -- Map DB fields to Frontend fields
        page.title = page["journal/title"]
        page.content = page["journal/content"]

        if page.id and string.find(page.id, "nodes:") == 1 then
            page.id = string.gsub(page.id, "nodes:", "")
            page.id = string.gsub(page.id, "[`⟨⟩]", "")
        end
        return page
    end
    return nil
end

function save_page(req)
    print("[Lua] save_page called")
    local body = req.body or {}
    local pageId = body.pageId
    local title = body.title
    local content = body.content

    if not pageId then return false end

    local recordId = "nodes:`" .. pageId .. "`"
    local sql = "UPDATE " .. recordId .. " MERGE $data"

    local data = {
        ["journal/title"] = title,
        ["journal/content"] = content
    }

    db.query(sql, { data = data })
    return true
end

function load_previous_daily(req)
    print("[Lua] load_previous_daily called")
    local body = req.body or {}
    local currentDayStr = body.currentDayStr

    local sql = "SELECT * FROM nodes WHERE journal_day < $date ORDER BY journal_day DESC LIMIT 1"
    local res = db.query(sql, { date = currentDayStr })

    if res and #res > 0 then
        local page = res[1]

        page.title = page["journal/title"]
        page.content = page["journal/content"]

        if page.id then
            page.id = string.gsub(page.id, "nodes:", "")
            page.id = string.gsub(page.id, "[`⟨⟩]", "")
        end
        return page
    else
        -- Create previous day
        local y, m, d = currentDayStr:match("(%d+)-(%d+)-(%d+)")
        local currentTime = os.time({ year = y, month = m, day = d })
        local prevTime = currentTime - 24 * 60 * 60
        local prevDayStr = os.date("%Y-%m-%d", prevTime)

        local newId = uuid()
        local recordId = "nodes:`" .. newId .. "`"
        local sqlCreate = "CREATE " .. recordId .. " CONTENT $data"

        -- DB Data
        local data = {
            ["journal/title"] = prevDayStr,
            ["journal/content"] = "",
            journal_day = prevDayStr
        }
        db.query(sqlCreate, { data = data })

        -- Frontend Data
        return {
            id = newId,
            title = prevDayStr,
            content = "",
            journal_day = prevDayStr
        }
    end
end

function get_today(req)
    print("[Lua] get_today called")
    local today = os.date("%Y-%m-%d")

    local sql = "SELECT * FROM nodes WHERE journal_day = $date LIMIT 1"
    local res = db.query(sql, { date = today })

    if res and #res > 0 then
        local page = res[1]

        page.title = page["journal/title"]
        page.content = page["journal/content"]

        if page.id then
            page.id = string.gsub(page.id, "nodes:", "")
            page.id = string.gsub(page.id, "[`⟨⟩]", "")
        end
        return page
    end

    -- Create today
    local id = uuid()
    local recordId = "nodes:`" .. id .. "`"
    local sqlCreate = "CREATE " .. recordId .. " CONTENT $data"

    local data = {
        ["journal/title"] = today,
        ["journal/content"] = "",
        journal_day = today
    }
    db.query(sqlCreate, { data = data })

    return {
        id = id,
        title = today,
        content = "",
        journal_day = today
    }
end
