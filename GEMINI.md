# Panorama App Development Guide

## Overview

Panorama apps are extensions that run within the Panorama ecosystem. They consist of a frontend (web technology) and a backend (Lua environment with SurrealDB access).

## App Structure

Apps are located in the `apps/` directory. Each app requires:

1.  `manifest.yml`: Configuration and metadata.
2.  `main.lua`: The backend entrypoint.
3.  Frontend assets (or a dev server).

## Manifest (`manifest.yml`)

The manifest defines the app's identity, permissions, database schema, and exposed API functions.

```yaml
name: "journal" # Unique identifier
version: "0.1.0"
lua_entrypoint: "main.lua" # Path to Lua backend
permissions: "privileged" # (Optional)

# Database Schema Extensions
# Apps share a single `nodes` table in SurrealDB.
# Define your specific fields here to ensure they exist.
# USE NAMESPACING (e.g., `appname/fieldname`) to avoid collisions.
fields:
  journal/title:
    type: string
  journal/content:
    type: string
  journal_day:
    type: string

# API Exposure
# List functions defined in main.lua that can be called from the frontend.
functions:
  - load_page
  - save_page

# Development Config
dev:
  frontend_server: "http://localhost:7999" # For HMR/Dev servers
```

## Lua Backend (`main.lua`)

The Lua environment runs in a separate thread/runtime per app.

### The `db` Object

A global `db` object is provided to interact with SurrealDB.

- **`db.query(sql_string, params_table)`**: Executes a query.
  - Returns: `List<Object>` (Rows).

### API Functions

Functions listed in `manifest.functions` must be global functions in Lua.

- **Signature**: `function my_func(req)`
- **`req`**: Table containing request details.
  - `req.body`: The JSON body sent from the frontend.
- **Return**: A table (serialized to JSON) or `nil`.

### Working with SurrealDB IDs

SurrealDB Record IDs (`nodes:uuid`) can contain hyphens, which confuse the parser if not escaped.

1.  **Querying**: Always wrap the ID part in backticks.
    ```lua
    local recordId = "nodes:`" .. uuid_string .. "`"
    local sql = "SELECT * FROM " .. recordId
    ```
2.  **Reading**: IDs returned from DB will include the `nodes:` prefix and potentially brackets. Strip them if sending raw UUIDs to the frontend.
    ```lua
    if page.id then
        page.id = string.gsub(page.id, "nodes:", "")
        page.id = string.gsub(page.id, "[`⟨⟩]", "")
    end
    ```

## Frontend Integration

The frontend can call Lua functions defined in the manifest, using the custom `panorama-api` protocol.

### Protocol

`panorama-api://<app-name>/<function_name>`

### Example (JavaScript)

```javascript
const API_BASE = "panorama-api://journal";

async function savePage(id, content) {
  const response = await fetch(`${API_BASE}/save_page`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ pageId: id, content: content }),
  });
  return await response.json();
}
```
