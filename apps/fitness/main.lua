-- Helper to generate IDs
local function uuid()
    local template = 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'
    return string.gsub(template, '[xy]', function(c)
        local v = (c == 'x') and math.random(0, 0xf) or math.random(8, 0xb)
        return string.format('%x', v)
    end)
end

function log_activity(req)
    local body = req.body or {}
    local activityType = body.type
    local timestamp = body.timestamp
    local activityData = body.data

    if not activityType or not timestamp then 
        return { success = false, error = "Missing type or timestamp" } 
    end

    local id = uuid()
    local recordId = "nodes:`" .. id .. "`"
    local sql = "CREATE " .. recordId .. " CONTENT $data"

    local dbData = {
        ["fitness/type"] = activityType,
        ["fitness/timestamp"] = timestamp,
        ["fitness/data"] = activityData
    }

    db.query(sql, { data = dbData })

    return { success = true, id = id }
end

function get_activities(req)
    local sql = "SELECT * FROM nodes WHERE `fitness/type` IS NOT NONE AND `fitness/timestamp` IS NOT NONE ORDER BY `fitness/timestamp` DESC LIMIT 50"
    local res = db.query(sql, {})

    local activities = {}
    if res then
        for _, row in ipairs(res) do
            if row["fitness/type"] and row["fitness/timestamp"] then
                local act = {
                    id = row.id,
                    type = row["fitness/type"],
                    timestamp = row["fitness/timestamp"],
                    data = row["fitness/data"] or {}
                }
                -- Clean up ID
                if act.id then
                    act.id = string.gsub(act.id, "nodes:", "")
                    act.id = string.gsub(act.id, "[`⟨⟩]", "")
                end
                table.insert(activities, act)
            end
        end
    end

    return activities
end
