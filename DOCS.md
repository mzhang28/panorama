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
- Space membership (`space_id`)

**Schemas** define groups of fields with requirements. Schemas are themselves nodes,
making the type system self-hosted. Schemas can be:
- **Preferred**: Nodes can fall out of schema (warnings raised)
- **Required**: Nodes must always conform (verified on write transaction gate)

**Fields** are namespaced key-value pairs attached to nodes. The namespace system:
- `system:` — Built-in fields (`node_title`, `node_time`, `node_start_time`, `node_end_time`, `created_at`, etc.)
- `user:` — User-created fields
- `<app-namespace>:` — App-specific fields (e.g., `journal:`, `wakatime:`, `grafana:`, `trips:`, `beli:`, `subsonic:`, `files:`)

**Spaces** handle multi-user permissions at the space level. All nodes in a space share the same `space_id` permissions boundary.

**Object Storage** provides S3-like blob storage for large files. Nodes reference blobs via ObjectRef fields rather than storing large data inline.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Panorama Frontend                     │
│              (Vite + React + TanStack)                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Node     │  │ Schema   │  │ Plugin UI            │  │
│  │ Viewer   │  │ Viewer   │  │ (Module Federation)  │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP (REST + plugin subpaths)
┌──────────────────────┴──────────────────────────────────┐
│                   Panorama Server (Rust)                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ REST API │  │ Plugin   │  │ Embedded Wasmtime    │  │
│  │ /api/*   │  │ Loader   │  │ WASI preview1 Engine │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Node     │  │ Schema   │  │ Object Storage       │  │
│  │ Storage  │  │ Registry │  │ (SQLite + Files)     │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Write**: Client → REST API → Node Storage → SQLite database (`nodes/panorama.db`)
2. **Read**: Client → REST API → Node Storage → SQL query (CTE + JSON extracts) → JSON response
3. **Plugin Request**: Client → `/plugin/{id}/*` → Plugin Loader → Wasmtime embedded engine → WASI stdin/stdout & host functions → Response
4. **Object Upload**: Client → Object Storage API → File on disk (`data/objects/`)

---

## User Guide

### Installation

```bash
# Prerequisites
# - Rust 1.80+
# - Node.js 20+ / Bun

# Clone and build
git clone <repo-url> panorama
cd panorama

# Build everything using `just` (frontend, WASM plugins, server, and .panoapp packages)
just build

# Or start the server with pre-packaged plugins
just serve
```

Visit `http://localhost:5173` (dev) or server port `http://localhost:3000` to access the Panorama UI.

### Installing Apps (.panoapp files)

Panorama apps are distributed as `.panoapp` files — single ZIP archives containing a manifest, WASM module (`plugin.wasm`), and frontend UI assets (`ui/`).

1. Build or download a `.panoapp` file (e.g. `dist/panoapp/io.mzhang.panorama.journal.panoapp`)
2. Place it in the plugins directory: `data/plugins/`
3. Start or restart the server — the app is automatically discovered and loaded

```bash
# Package all example apps and run server
just serve
```

### Using the Web UI

- **Nodes View**: Browse, create, edit, and delete nodes. Each node displays its fields, schemas, and metadata.
- **Schemas View**: Browse registered schemas from system and installed plugins with field definitions and requirements.
- **Plugins View**: Browse loaded plugins, inspect endpoints, test endpoints interactively, and view app interfaces.
- **Sidebar**: Quick navigation between core views and installed apps.

---

## Developer Guide

### Project Structure

```
panorama/
├── crates/
│   ├── panorama-core/          # Core types, query parser/AST, Plugin trait & PluginContext
│   ├── panorama-server/        # Axum web server, SQLite storage, query compiler, Wasmtime runtime
│   ├── panorama-app-journal/   # Journal app plugin (WASM / native)
│   ├── panorama-app-wakatime/  # Wakatime activity app plugin
│   ├── panorama-app-grafana/   # Dashboard & query engine app plugin
│   ├── panorama-app-trips/     # Trip planner app plugin
│   ├── panorama-app-beli/      # Restaurant partial-ordering app plugin
│   ├── panorama-app-subsonic/  # Music streaming app plugin
│   ├── panorama-app-files/     # File manager app plugin
│   └── wasm-types/             # WASM shared type definitions
├── frontend/                   # React + TanStack + Module Federation frontend
├── design/
│   ├── DESIGN.md               # Architecture design document
│   ├── HOOK_DESIGN.md          # Hooks and event system specification
│   └── QUERY_DESIGN.md         # Panorama Query Language specification
├── scripts/
│   ├── build-wasm.sh           # Compiles plugins to wasm32-wasip1
│   ├── build-panoapp.sh        # Builds plugin UIs and packages .panoapp files
│   ├── package-panoapp.py      # Python packager for .panoapp ZIP archives
│   ├── serve.sh                # Helper script to launch server with plugins
│   └── e2e.py                  # Playwright E2E test runner
├── dist/panoapp/               # Output directory for built .panoapp packages
├── justfile                    # Task runner commands (`just build`, `just test-e2e`, etc.)
├── INVARIANTS.md               # Core system invariants & validation rules
└── DOCS.md                     # Documentation
```

### Building a Third-Party Plugin

Plugins depend ONLY on `panorama-core`. They do NOT depend on `panorama-server` or internal server modules.

#### Step 1: Implement the Plugin trait

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
        // Plugin logic using ctx methods
    }
}
```

#### Step 2: Build WASM Target & Package as .panoapp

```bash
# Build WASM module for wasm32-wasip1 target
RUSTFLAGS="-C link-arg=--allow-undefined" cargo build --release -p my-plugin --target wasm32-wasip1

# Package manifest, WASM binary, and frontend assets into .panoapp
python3 scripts/package-panoapp.py manifest.json target/wasm32-wasip1/release/my_plugin.wasm dist/panoapp --ui-dir ui/dist
```

### PluginContext API

The `PluginContext` is the official interface provided to plugins:

| Method | Description |
|--------|-------------|
| `create_nodes(nodes) -> Vec<Node>` | Create multiple nodes in batch |
| `create_node(node) -> Node` | Create a single node |
| `get_node(id) -> Option<Node>` | Get a node by UUID |
| `update_node(id, fields) -> Node` | Update a node's fields |
| `delete_node(id)` | Delete a node |
| `query(query_string) -> Vec<Value>` | Execute a query in Panorama Query Language |
| `register_schema(schema) -> Schema` | Register a schema |
| `get_schema(id) -> Option<Schema>` | Get a schema by ID |
| `put_object(bucket, key, data, mime) -> ObjectRef` | Store an object in object storage |
| `get_object(bucket, key) -> Option<ObjectData>` | Retrieve an object |
| `delete_object(bucket, key)` | Delete an object |
| `list_objects(bucket, prefix) -> Vec<ObjectRef>` | List objects in a bucket |
| `plugin_id() -> &str` | Get the plugin's registered ID |

See `design/QUERY_DESIGN.md` for full PQL specification.

### Capability System

| Capability | Description |
|-----------|-------------|
| `network_hosts` | Whitelisted hosts the plugin can contact |
| `field_read` | Namespaced fields readable by plugin (`*` for all) |
| `field_write` | Namespaced fields writeable by plugin |
| `write_own_nodes` | Permission to write nodes created by plugin |
| `app_managed_nodes` | Permission for app-managed nodes |
| `object_storage_read` | Permission to read object storage |
| `object_storage_write` | Permission to write object storage |
| `file_read` / `file_write` | Filesystem access permission |
| `execute` | External process execution permission |

### Running Tests

```bash
# Run all Playwright E2E tests against an isolated server harness
just test-e2e

# Run Rust workspace unit/integration tests
just test-rust

# Type check workspace
just check
```

---

## .panoapp Format Specification

A `.panoapp` file is a standard ZIP archive containing:

```
my-app.panoapp
├── manifest.json        # Plugin metadata, schemas, endpoints, capabilities
├── plugin.wasm          # WASM module (target: wasm32-wasip1)
└── ui/                  # Web UI static assets
    ├── app.js
    ├── app.css
    └── index.html
```

### WASM Module Interface

The WASM module is compiled for `wasm32-wasip1` and executed via embedded Wasmtime in `panorama-server`:

1. Request payload is passed via standard input (`stdin`) as JSON:
```json
{
  "endpoint": "/path",
  "request": {
    "method": "POST",
    "query_params": {},
    "headers": {},
    "body": "..."
  }
}
```

2. Storage and context calls execute synchronously via imported host functions in the `"env"` module (`host_ctx_create_nodes`, `host_ctx_get_node`, `host_ctx_update_node`, `host_ctx_delete_node`, `host_ctx_query`, `host_ctx_log`).

3. WASM handler writes response JSON to standard output (`stdout`):
```json
{
  "status": 200,
  "headers": {"Content-Type": "application/json"},
  "body": {"result": "ok"}
}
```

---

## Example Apps

Panorama includes seven example app plugins:

### 1. Journal (`io.mzhang.panorama.journal`)
Daily markdown journal with block-level references.
- **Endpoints**: `POST /entries`, `GET /entries`, `GET /entries/{id}`
- **Schema**: `JournalEntry` (`journal/JournalEntry`)

### 2. Coding Activity / WakaTime (`io.mzhang.panorama.wakatime`)
WakaTime-compatible heartbeat and activity stats tracker.
- **Endpoints**: `POST /users/current/heartbeats`, `POST /users/current/heartbeats.bulk`, `GET /users/current/durations`, `GET /stats`, `POST /heartbeat`, `POST /heartbeats`, `GET /summaries`
- **Schemas**: `Heartbeat`, `Duration`, `DailySummary`

### 3. Dashboards (`io.mzhang.panorama.grafana`)
Leaderboard and time-series visualization system.
- **Endpoints**: `GET /api/dashboards`, `POST /api/dashboards`, `POST /api/ds/query`, `GET /api/query/options`, `POST /api/dashboards/import`, `/api/folders`
- **Schemas**: `Dashboard`, `Folder`

### 4. Trip Planner (`io.mzhang.panorama.trips`)
Trip planner with calendar and geolocation map views.
- **Endpoints**: `POST /trips`, `GET /trips`, `POST /events`, `GET /events`, `GET /events/map`
- **Schemas**: `Trip`, `Event`

### 5. Restaurant Rankings / Beli (`io.mzhang.panorama.beli`)
Restaurant partial ordering rankings via pairwise comparisons.
- **Endpoints**: `POST /restaurants`, `GET /restaurants`, `POST /compare`, `GET /rankings`
- **Schemas**: `Restaurant`, `Comparison`

### 6. Music Library / Subsonic (`io.mzhang.panorama.subsonic`)
Subsonic-compatible music streaming with object storage for audio.
- **Endpoints**: `GET /rest/ping`, `GET /rest/getArtists`, `GET /rest/getAlbumList2`, `GET /rest/stream`, `POST /upload`
- **Schemas**: `Artist`, `Album`, `Track`

### 7. File Manager (`io.mzhang.panorama.files`)
File browser with resumable chunked upload support.
- **Endpoints**: `POST /upload`, `POST /upload/initiate`, `GET /files`, `GET /files/{id}`, `DELETE /files/{id}`
- **Schema**: `File`

---

## API Reference

### Node CRUD

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/nodes` | Create a node |
| `GET` | `/api/nodes` | Query nodes (`?limit=`, `?sort_by=`, filter params) |
| `GET` | `/api/nodes/{id}` | Get node by UUID |
| `PUT` | `/api/nodes/{id}` | Update node fields |
| `DELETE` | `/api/nodes/{id}` | Delete node |
| `POST` | `/api/query` | Execute PQL query |

### Schemas

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/schemas` | List all schemas |
| `GET` | `/api/schemas/{id}` | Get schema details |

### Plugins

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/plugins` | List loaded plugins |
| `GET` | `/api/plugins/{id}` | Get plugin details |
| `GET` | `/api/plugins/{id}/static` | List plugin UI static files |
| `GET` | `/plugin/{id}/ui/{*path}` | Serve plugin UI assets |
| `*` | `/plugin/{id}/{*path}` | Dispatch HTTP request to plugin |

### Object Storage

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/api/objects/{bucket}/{key}` | Upload object blob |
| `GET` | `/api/objects/{bucket}/{key}` | Download object blob |
| `DELETE` | `/api/objects/{bucket}/{key}` | Delete object blob |
| `GET` | `/api/objects/{bucket}` | List objects in bucket |
| `POST` | `/api/uploads` | Initiate resumable upload session |
| `POST` | `/api/uploads/{id}/chunks` | Upload chunk |
| `POST` | `/api/uploads/{id}/complete` | Finalize resumable upload |

---

## Operations Guide

### Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PANORAMA_DATA_DIR` | `./data` | Root directory for data storage |
| `PANORAMA_LISTEN` | `127.0.0.1:3000` | Server bind address and port |

### Data Storage

Data is stored under `$PANORAMA_DATA_DIR/`:
- `nodes/panorama.db` — SQLite database (nodes table + 6 meta tables: `namespaces`, `schema_tables`, `managed_indexes`, `field_presence`, `node_schema_conformance`, `field_stats`)
- `objects/` — Object storage files grouped by bucket
- `plugins/` — Installed `.panoapp` packages

### Performance

Nodes are stored in SQLite with JSON field blobs and WAL mode enabled. The query engine uses `json_extract` with CTEs and maintains a prepared statement cache (LRU, 256 entries).

### Security

- Sandboxed WASM execution via embedded Wasmtime with capability grants.
- Run behind a TLS reverse proxy (e.g. nginx or Caddy) for production deployments.

### Backup

Back up the `$PANORAMA_DATA_DIR` directory:
```bash
tar -czf panorama-backup-$(date +%Y%m%d).tar.gz ./data/
```
