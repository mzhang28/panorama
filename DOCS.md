# Panorama — Documentation

Panorama is a self-hosted data layer platform for building personal apps. It provides
a flexible node-based data store, a plugin system for third-party apps, object storage,
and a modern web frontend.

## Table of Contents

1. [System Design](#system-design)
2. [User Guide](#user-guide)
3. [Developer Guide](#developer-guide)
4. [Plugin API Reference](#plugin-api-reference)
5. [.panoapp Format Specification](#panoapp-format-specification)
6. [Example Apps](#example-apps)
7. [API Reference](#api-reference)
8. [Operations Guide](#operations-guide)

---

## System Design

### Core Concepts

**Nodes** are the fundamental data unit. Every piece of data in Panorama is a node with:
- A unique UUID
- Arbitrary fields (keyed by `namespace:field_name`)
- Optional schema conformance (preferred or required)
- System timestamps (created_at, updated_at)
- Space membership

**Schemas** define groups of fields with requirements. Schemas are themselves nodes,
making the type system self-hosted. Schemas can be:
- **Preferred**: Nodes can fall out of schema (warnings raised)
- **Required**: Nodes must always conform (verified on write)

**Fields** are namespaced key-value pairs attached to nodes. The namespace system:
- `system:` — Built-in fields (node_title, node_time, created_at, etc.)
- `user:` — User-created fields
- `<app-namespace>:` — App-specific fields (e.g., `journal:`, `wakatime:`)

**Spaces** handle multi-user permissions at the space level (similar to Anytype's model).
All nodes in a space share the same permissions. Cross-space sharing requires replication.

**Object Storage** provides S3-like blob storage for large files. Nodes reference blobs
via ObjectRef fields rather than storing large data inline.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Panorama Frontend                     │
│              (Vite + React + Tanstack)                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Node     │  │ Schema   │  │ Plugin UI            │  │
│  │ Viewer   │  │ Viewer   │  │ (dynamic components) │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP (REST + plugin subpaths)
┌──────────────────────┴──────────────────────────────────┐
│                   Panorama Server (Rust)                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ REST API │  │ Plugin   │  │ WASM Runtime         │  │
│  │ /api/*   │  │ Loader   │  │ (wasmtime subprocess)│  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Node     │  │ Schema   │  │ Object Storage       │  │
│  │ Storage  │  │ Registry │  │ (SQLite + files)     │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Write**: Client → REST API → Node Storage → SQLite database
2. **Read**: Client → REST API → Node Storage → SQL query → JSON response
3. **Plugin Request**: Client → `/plugin/{id}/*` → Plugin Loader → WASM Runtime → Response
4. **Object Upload**: Client → Object Storage API → File on disk

---

## User Guide

### Installation

```bash
# Prerequisites
# - Rust 1.80+
# - Node.js 20+
# - wasmtime CLI (for WASM plugin support)

# Install wasmtime
cargo install wasmtime-cli

# Clone and build
git clone <repo-url> panorama
cd panorama

# Build the server
cargo build --release -p panorama-server

# Install frontend dependencies
cd frontend && npm install && cd ..

# Start the server
PANORAMA_DATA_DIR=./data PANORAMA_LISTEN=127.0.0.1:3000 \
  cargo run --release -p panorama-server

# In another terminal, start the frontend
cd frontend && npm run dev
```

Visit `http://localhost:5173` to access the Panorama UI.

### Installing Apps (.panoapp files)

Panorama apps are distributed as `.panoapp` files — single ZIP archives containing
a manifest, WASM module, and UI assets.

1. Download or build a `.panoapp` file (e.g., `com.panorama.journal.panoapp`)
2. Place it in the plugins directory: `data/plugins/`
3. Restart the server — the app is automatically loaded

```bash
# Example: install all example apps
cp dist/panoapp/*.panoapp data/plugins/
cargo run --release -p panorama-server
```

### Using the Web UI

**Nodes View**: Browse, create, edit, and delete nodes. Each node shows its fields,
schemas, and metadata. Click a node to see its full detail and edit fields inline.

**Schemas View**: Browse registered schemas from the system and installed plugins.
Each schema shows its fields, requirements, and version.

**Plugins View**: Browse installed plugins, view their HTTP endpoints, test endpoints
interactively, and see their UI components.

**Sidebar**: Quick navigation between views and installed apps. The "Installed Apps"
section lists all loaded plugins.

### Managing Data

Nodes can be created through:
- The web UI (Nodes → New Node)
- The REST API (`POST /api/nodes`)
- Plugin endpoints (e.g., `POST /plugin/com.panorama.journal/entries`)

Fields are set as namespaced key-value pairs. Untyped fields default to String.
Use the schema system for type enforcement.

---

## Developer Guide

### Project Structure

```
panorama/
├── crates/
│   ├── panorama-core/          # Core types + Plugin API trait (public third-party API)
│   ├── panorama-server/        # Platform server (depends on core)
│   ├── panorama-app-journal/   # Journal app (depends ONLY on core)
│   ├── panorama-app-wakatime/  # Wakatime app (depends ONLY on core)
│   ├── panorama-app-grafana/   # Dashboard app (depends ONLY on core)
│   ├── panorama-app-trips/     # Trip planner app (depends ONLY on core)
│   ├── panorama-app-beli/      # Restaurant rating app (depends ONLY on core)
│   ├── panorama-app-subsonic/  # Music streaming app (depends ONLY on core)
│   └── panorama-app-files/     # File manager app (depends ONLY on core)
├── frontend/                   # Vite + React + TanStack frontend (Module Federation host)
│   ├── src/
│   │   ├── api/client.ts       # API client for backend communication
│   │   ├── api/plugin-loader.tsx  # Dynamic Module Federation remote loading
│   │   └── components/         # Core React components (NodeViewer, SchemaViewer, PluginPanel)
│   ├── e2e/                    # Playwright E2E tests
│   └── vite.config.ts          # Module Federation host config
├── scripts/
│   ├── build-panoapp.sh        # Build plugin UIs + .panoapp packages
│   ├── e2e-harness.sh          # Isolated E2E test harness (builds everything, spawns temp server)
│   └── package-panoapp.py      # .panoapp ZIP packager
├── dist/panoapp/               # Built .panoapp packages
├── QUERY_DESIGN.md             # Panorama Query Language v0 specification
├── DESIGN.md                   # Original design document
└── DOCS.md                     # This documentation
```

### Building a Third-Party Plugin

Plugins use ONLY the public API defined in `panorama-core`. They must NOT depend on
`panorama-server` or any platform internals.

#### Step 1: Create the plugin crate

```toml
# Cargo.toml
[package]
name = "my-panorama-plugin"
version = "0.1.0"
edition = "2021"

[dependencies]
panorama-core = { git = "https://..." }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
```

#### Step 2: Implement the Plugin trait

```rust
use async_trait::async_trait;
use panorama_core::plugin::*;
use panorama_core::schema::*;
use panorama_core::types::*;
use panorama_core::capabilities::*;

pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn id(&self) -> &str { "com.example.myplugin" }
    fn name(&self) -> &str { "My Plugin" }
    fn version(&self) -> &str { "0.1.0" }
    fn description(&self) -> &str { "Does something useful" }

    fn schemas(&self) -> Vec<Schema> { /* ... */ }
    fn http_endpoints(&self) -> Vec<HttpEndpoint> { /* ... */ }
    fn ui_components(&self) -> Vec<UiComponent> { /* ... */ }
    fn required_capabilities(&self) -> CapabilityGrants { /* ... */ }

    async fn handle_http_request(
        &self,
        endpoint: &str,
        request: HttpRequest,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        // Your plugin logic here
        // Use ctx.create_node(), ctx.get_node(), ctx.query_nodes(), etc.
    }
}
```

#### Step 3: Package as .panoapp

Create a `manifest.json` and compile your WASM module:

```bash
# Create manifest (use gen-manifest.py as a template)
# Compile WASM
cargo build --target wasm32-wasip1 --release
# Package
python3 -c "
import zipfile
with zipfile.ZipFile('my-plugin.panoapp', 'w') as zf:
    zf.write('manifest.json')
    zf.write('target/wasm32-wasip1/release/my-plugin.wasm', 'plugin.wasm')
"
```

#### Step 4: Install and test

Copy the `.panoapp` to the server's plugins directory and restart.

### PluginContext API

The `PluginContext` is the ONLY way plugins interact with the platform:

| Method | Description |
|--------|-------------|
| `create_node(node) -> Node` | Create a new node |
| `get_node(id) -> Option<Node>` | Get a node by ID |
| `update_node(id, fields) -> Node` | Update a node's fields |
| `delete_node(id)` | Delete a node |
| `query_nodes(query) -> Vec<Node>` | Query nodes with filters |
| `register_schema(schema) -> Schema` | Register a schema |
| `get_schema(id) -> Option<Schema>` | Get a schema |
| `put_object(bucket, key, data, mime) -> ObjectRef` | Store an object |
| `get_object(bucket, key) -> Option<ObjectData>` | Retrieve an object |
| `delete_object(bucket, key)` | Delete an object |
| `list_objects(bucket, prefix) -> Vec<ObjectRef>` | List objects |
| `plugin_id() -> &str` | Get the plugin's ID |
| `base_path() -> String` | Get the plugin's HTTP base path |
| `query(query_string) -> Vec<Value>` | Execute a query in the Panorama Query Language |
| `log(level, message)` | Log through the platform |

See `QUERY_DESIGN.md` for the full query language specification.

### Capability System

Plugins must declare required capabilities. Users grant them during installation.
Major version bumps are required for capability changes.

| Capability | Description |
|-----------|-------------|
| `network_hosts` | Hosts the plugin can contact |
| `field_read` | Fields the plugin can read (`*` for all) |
| `field_write` | Fields the plugin can write |
| `write_own_nodes` | Can write to nodes it created |
| `app_managed_nodes` | Can have user-immutable nodes |
| `object_storage_read` | Can read from object storage |
| `object_storage_write` | Can write to object storage |
| `file_read` / `file_write` | Filesystem access |
| `execute` | Can execute external programs |

### Running Tests

```bash
# Rust unit/integration tests
cargo test --workspace

# Isolated E2E tests (handles build, temp server, cleanup automatically)
bash scripts/e2e-harness.sh

# E2E with pre-built artifacts (faster iteration)
bash scripts/e2e-harness.sh --no-build

# Filter specific tests
bash scripts/e2e-harness.sh -g "Journal"
```

---

## .panoapp Format Specification

A `.panoapp` file is a ZIP archive with the `.panoapp` extension containing:

```
my-app.panoapp
├── manifest.json        # Required: plugin metadata
├── plugin.wasm          # Optional: WASM module for backend handlers
└── ui/                  # Optional: frontend UI assets
    ├── app.js
    ├── app.css
    └── index.html
```

### manifest.json Schema

```json
{
  "manifest_version": 1,
  "id": "com.example.myapp",
  "name": "My App",
  "version": "0.1.0",
  "description": "Description of the app",
  "author": "Author Name",
  "homepage": "https://example.com",
  "icon": "ui/icon.png",
  "min_platform_version": "0.1.0",
  "schemas": [
    {
      "name": "MySchema",
      "version": {"major": 1, "minor": 0},
      "schema_mode": "Preferred",
      "fields": [
        {
          "name": "field_name",
          "namespace": "myapp",
          "required": false,
          "field_type": null,
          "default": null,
          "description": "Description",
          "computed": null
        }
      ]
    }
  ],
  "http_endpoints": [
    {
      "method": "POST",
      "path": "/my-endpoint",
      "description": "What this endpoint does"
    }
  ],
  "ui_components": [
    {
      "id": "main-view",
      "name": "Main View",
      "mount_point": "MainPage",
      "bundle_path": "ui/app.js"
    }
  ],
  "capabilities": {
    "version": 1,
    "network_hosts": [],
    "field_read": ["myapp:*"],
    "field_write": ["myapp:*"],
    "write_own_nodes": true,
    "app_managed_nodes": false,
    "object_storage_read": false,
    "object_storage_write": false,
    "file_read": false,
    "file_write": false,
    "execute": false,
    "dns_requests": false
  },
  "wasm_module": "plugin.wasm",
  "background_tasks": [],
  "env_vars": {}
}
```

### WASM Module Interface

The WASM module is compiled to `wasm32-wasip1` and communicates via WASI:

1. The server creates a temp directory with `input.json`
2. Runs: `wasmtime run --dir=<workdir> plugin.wasm -- input.json output.json`
3. The WASM module reads `input.json`, processes, writes `output.json`

**input.json**:
```json
{
  "endpoint": "my-endpoint",
  "request": {
    "method": "POST",
    "path": "/my-endpoint",
    "query_params": {},
    "headers": {},
    "body": "..."
  },
  "nodes": [
    {
      "id": "uuid",
      "fields": {"namespace:field": {"type": "String", "value": "..."}},
      "created_at": "2024-...",
      "updated_at": "2024-..."
    }
  ]
}
```

**output.json**:
```json
{
  "status": 200,
  "headers": {"Content-Type": "application/json"},
  "body": {"result": "ok"},
  "effects": [
    {"type": "create_node", "fields": {"system:node_title": {"type": "String", "value": "Hello"}}}
  ]
}
```

---

## Example Apps

Panorama ships with seven example apps demonstrating the plugin API:

### 1. Journal (`com.panorama.journal`)
Daily markdown journal with block-level references.
- **Endpoints**: `POST /entries`, `GET /entries`, `GET /entries/{id}`
- **Schema**: `journal/JournalEntry`
- **Key feature**: Entries stored as nodes with markdown content

### 2. Wakatime (`com.panorama.wakatime`)
Receives heartbeats from Wakatime-compatible clients.
- **Endpoints**: `POST /heartbeat`, `POST /heartbeats`
- **Schema**: `wakatime/Heartbeat`
- **Key feature**: Time-series data via system:node_time field

### 3. Dashboards (`com.panorama.grafana`)
Grafana-like dashboards for time-series visualization.
- **Endpoints**: `POST /query`, `POST /dashboards`, `GET /dashboards`
- **Schema**: `grafana/Dashboard`
- **Key feature**: Leaderboard queries with group-by and aggregation

### 4. Trip Planner (`com.panorama.trips`)
Plan trips with events, calendar view, and map view.
- **Endpoints**: `POST /trips`, `GET /trips`, `POST /events`, `GET /events`, `GET /events/map`
- **Schemas**: `trips/Trip`, `trips/Event`
- **Key feature**: Geolocation fields for map visualization

### 5. Beli (`com.panorama.beli`)
Restaurant ratings with PARTIAL ORDERING (pairwise comparisons).
- **Endpoints**: `POST /restaurants`, `GET /restaurants`, `POST /compare`, `GET /rankings`
- **Schemas**: `beli/Restaurant`, `beli/Comparison`
- **Key feature**: Topological sort ranking (not 5-star ratings)

### 6. Subsonic Music (`com.panorama.subsonic`)
Subsonic-compatible music streaming.
- **Endpoints**: `GET /rest/ping`, `GET /rest/getArtists`, `GET /rest/stream`, `POST /upload`
- **Schemas**: `subsonic/Artist`, `subsonic/Album`, `subsonic/Track`
- **Key feature**: Object storage for audio files, Subsonic API compatibility

### 7. File Manager (`com.panorama.files`)
File uploads with resumable transfer support.
- **Endpoints**: `POST /upload`, `GET /files`, `GET /files/{id}`, `DELETE /files/{id}`
- **Schema**: `files/File`
- **Key feature**: Resumable uploads via chunked transfer

---

## API Reference

### Node CRUD

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/nodes` | Create a node |
| `GET` | `/api/nodes/{id}` | Get a node |
| `PUT` | `/api/nodes/{id}` | Update a node |
| `DELETE` | `/api/nodes/{id}` | Delete a node |
| `GET` | `/api/nodes` | Query nodes (?limit=, ?sort_by=, ?filter.ns:field=) |
| `POST` | `/api/query` | Execute a Panorama Query Language query |

### Schemas

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/schemas` | List all schemas |
| `GET` | `/api/schemas/{id}` | Get a schema |

### Plugins

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/plugins` | List loaded plugins |
| `GET` | `/api/plugins/{id}` | Get plugin details |
| `*` | `/plugin/{id}/*` | Plugin endpoint dispatch |

### Object Storage

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/api/objects/{bucket}/{key}` | Upload an object |
| `GET` | `/api/objects/{bucket}/{key}` | Download an object |
| `DELETE` | `/api/objects/{bucket}/{key}` | Delete an object |
| `GET` | `/api/objects/{bucket}` | List objects (?prefix=) |
| `POST` | `/api/uploads` | Initiate resumable upload |
| `POST` | `/api/uploads/{id}/chunks` | Upload a chunk |
| `POST` | `/api/uploads/{id}/complete` | Complete resumable upload |

---

## Operations Guide

### Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PANORAMA_DATA_DIR` | `./data` | Data storage directory |
| `PANORAMA_LISTEN` | `127.0.0.1:3000` | Server listen address |

### Data Storage

Data is stored under `$PANORAMA_DATA_DIR/`:
- `panorama.db` — SQLite database (nodes, fields as JSON blobs, WAL mode)
- `objects/` — Object storage buckets and files
- `plugins/` — Loaded .panoapp packages

### Performance

Panorama stores nodes in SQLite with JSON field blobs and WAL mode. Queries use
`json_extract` for field-level filtering with parameterized statements. The query
language compiler emits CTE-based SQL and a prepared statement cache (LRU, 256
entries) avoids re-compilation for hot queries. This works well for personal-scale
data (tens of thousands of nodes).

### Security

- v0.x does NOT implement authentication or end-to-end encryption
- Run behind a reverse proxy (nginx, Caddy) for TLS
- Do not expose directly to the public internet
- Plugins are sandboxed via WASM (wasmtime) but trust is ultimately at the user's discretion
- Always review plugin capabilities before installing

### Backup

Back up the `$PANORAMA_DATA_DIR` directory:
```bash
tar -czf panorama-backup-$(date +%Y%m%d).tar.gz ./data/
```

### Troubleshooting

**Server won't start**: Check that the data directory is writable and port is available.
**Plugin not loading**: Verify the .panoapp file is valid ZIP with manifest.json.
**WASM execution fails**: Ensure wasmtime CLI is installed (`wasmtime --version`).
**Frontend can't connect**: Check that the Vite proxy is configured for `/api` and `/plugin`.

---

Generated with [Claude Code](https://claude.ai/code)
via [Happy](https://happy.engineering)
