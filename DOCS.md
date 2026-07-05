# Panorama — Documentation

Panorama is a self-hosted data layer platform for building personal apps. It provides
a flexible node-based data store, a plugin system for third-party apps, object storage,
and a modern web frontend.

## Table of Contents

1. [System Design](#system-design)
2. [User Guide](#user-guide)
3. [Developer Guide](#developer-guide)
4. [Panorama Query Language (PQL)](#panorama-query-language-pql)
5. [Plugin API Reference](#plugin-api-reference)
6. [.panoapp Format Specification](#panoapp-format-specification)
7. [Example Apps](#example-apps)
8. [API Reference](#api-reference)
9. [Operations Guide](#operations-guide)

---

## System Design

### Core Concepts

**Nodes** are the fundamental data unit. Every piece of data in Panorama is a node with:
- A unique UUID (`id`)
- Arbitrary fields (keyed by `namespace:field_name`)
- Optional schema conformance (preferred or required)
- System timestamps (`created_at`, `updated_at`)
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

## Panorama Query Language (PQL)

Panorama features a specialized Cypher-flavored query language (PQL v0) designed to query nodes, enforce space isolation boundaries, check field presence, and traverse reference graphs.

### Surface Syntax & Examples

#### 1. Basic Match & Space Isolation
Every `MATCH` clause **requires** an explicit `IN space(...)` specifier for mandatory tenant data isolation:
```cypher
MATCH (n) IN space("default")
RETURN n
```

#### 2. Schema Conformance Filtering
Filter nodes that conform to a specific schema (with optional version constraints):
```cypher
MATCH (n) IN space("default")
WHERE n CONFORMS TO schema("journal/JournalEntry")
RETURN n
```

#### 3. Namespaced Field Access & Predicates
Fields are accessed via `n."namespace".field_name` or `n.system.field_name`:
```cypher
MATCH (n) IN space("default")
WHERE n CONFORMS TO schema("wakatime/Heartbeat")
  AND n."wakatime".project = "panorama"
  AND n.system.node_time >= "2026-01-01T00:00:00Z"
RETURN n."wakatime".entity, n.system.node_time
```

#### 4. Field Presence Inspection
Check whether a specific field exists on a node using `HAS_FIELD`:
```cypher
MATCH (n) IN space("default")
WHERE HAS_FIELD(n, "journal", "content")
RETURN n
```

#### 5. Reference Graph Traversal
Traverse relationships between nodes using bounded reference hops:
```cypher
MATCH (a)-[:REF("attendee")]->(b) IN space("default")
WHERE a CONFORMS TO schema("trips/Event")
RETURN a.system.node_title, b.system.node_title
LIMIT 50
```

#### 6. Ordering, Limits, & Unindexed SCAN Opt-in
```cypher
MATCH (n) IN space("default")
WHERE SCAN(n."journal".content LIKE "%important%")
ORDER BY n.system.created_at DESC
LIMIT 20
SKIP 0
```
*Note: Predicates on non-indexed fields require an explicit `SCAN(...)` wrapper to prevent accidental unindexed database scans.*

### Query Execution Engine & Architecture

PQL queries execute via a **Two-Phase Compilation Engine**:
1. **Phase 1 (Meta Lookup)**:
   - Resolves space names to `space_id` UUIDs.
   - Looks up meta tables (`field_presence`, `node_schema_conformance`, `namespaces`, `managed_indexes`).
2. **Phase 2 (SQL Generation & Prepared Statement Cache)**:
   - Compiles AST into a single optimized SQLite query with Common Table Expressions (CTEs).
   - Joins directly against `field_presence` and `node_schema_conformance` tables instead of parsing raw JSON strings at query time.
   - Prepared statements are cached in an LRU cache (256 entries) for maximum performance.

---

## Plugin API Reference

The `PluginContext` trait is the primary interface provided by `panorama-core` to plugins.

### Context Method Reference

| Method | Signature | Description |
|--------|-----------|-------------|
| `create_nodes` | `async fn create_nodes(&self, nodes: Vec<Node>) -> Result<Vec<Node>, PluginError>` | Create multiple nodes in a single atomic storage operation |
| `create_node` | `async fn create_node(&self, node: Node) -> Result<Node, PluginError>` | Create a single node (convenience wrapper over `create_nodes`) |
| `get_node` | `async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError>` | Fetch a node by UUID |
| `update_node` | `async fn update_node(&self, id: Uuid, fields: HashMap<String, FieldValue>) -> Result<Node, PluginError>` | Update namespaced fields on an existing node |
| `delete_node` | `async fn delete_node(&self, id: Uuid) -> Result<(), PluginError>` | Permanently delete a node |
| `query` | `async fn query(&self, query_string: &str) -> Result<Vec<serde_json::Value>, PluginError>` | Execute a PQL query string and return matching rows/nodes |
| `register_schema` | `async fn register_schema(&self, schema: Schema) -> Result<Schema, PluginError>` | Register a custom schema in the platform registry |
| `get_schema` | `async fn get_schema(&self, id: Uuid) -> Result<Option<Schema>, PluginError>` | Retrieve a schema definition by ID |
| `put_object` | `async fn put_object(&self, bucket: &str, key: &str, data: Bytes, mime_type: &str) -> Result<ObjectRef, PluginError>` | Store a binary file blob in object storage |
| `get_object` | `async fn get_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectData>, PluginError>` | Retrieve a binary file blob from object storage |
| `delete_object` | `async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), PluginError>` | Delete a binary object from storage |
| `list_objects` | `async fn list_objects(&self, bucket: &str, prefix: Option<&str>) -> Result<Vec<ObjectRef>, PluginError>` | List object references in a bucket |
| `plugin_id` | `fn plugin_id(&self) -> &str` | Return the unique identifier string of the calling plugin |

### Capability System

Plugins declare capabilities in `manifest.json`. The platform runtime checks these permissions before performing host operations:

| Capability Field | Description |
|-----------------|-------------|
| `network_hosts` | Whitelisted remote domain names or hostnames for HTTP requests |
| `field_read` | Namespaced fields readable by plugin (e.g. `["journal:*", "system:*"]` or `["*"]`) |
| `field_write` | Namespaced fields writeable by plugin (e.g. `["wakatime:*", "system:node_time"]`) |
| `write_own_nodes` | Boolean granting permission to modify nodes created by the plugin |
| `app_managed_nodes` | Boolean granting permission for app-managed immutable nodes |
| `object_storage_read` | Boolean permission to read from object storage buckets |
| `object_storage_write` | Boolean permission to write to object storage buckets |
| `file_read` / `file_write` | Boolean permissions for local filesystem operations |
| `execute` | Boolean permission to execute external system processes |

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

### Node Management REST API

#### `POST /api/nodes` — Create Node
Create a new node in storage. Enforces required schema validations.

**Request Body**:
```json
{
  "space_id": "00000000-0000-0000-0000-000000000000",
  "fields": {
    "system:node_title": { "type": "String", "value": "My Entry" },
    "journal:content": { "type": "String", "value": "# Hello World" }
  },
  "schemas": [
    {
      "schema_node_id": "11111111-1111-1111-1111-111111111111",
      "version": { "major": 1, "minor": 0 }
    }
  ]
}
```

**Response (200 OK)**:
```json
{
  "id": "e43b1234-5678-4abc-8def-901234567890",
  "space_id": "00000000-0000-0000-0000-000000000000",
  "fields": {
    "system:node_title": { "type": "String", "value": "My Entry" },
    "journal:content": { "type": "String", "value": "# Hello World" }
  },
  "preferred_schemas": [
    {
      "schema_node_id": "11111111-1111-1111-1111-111111111111",
      "version": { "major": 1, "minor": 0 }
    }
  ],
  "created_at": "2026-07-05T01:00:00Z",
  "updated_at": "2026-07-05T01:00:00Z"
}
```

#### `GET /api/nodes` — Query / List Nodes
**Query Parameters**:
- `limit` (number, default: 100)
- `sort_by` (string, e.g. `created_at` or `updated_at`)
- `filter.<namespace>:<field_name>` (string, filter nodes matching field value)

**Response (200 OK)**: JSON array of `Node` objects.

#### `GET /api/nodes/{id}` — Get Node
**Response (200 OK)**: `Node` JSON object. If not found, returns `404 NOT_FOUND`.

#### `PUT /api/nodes/{id}` — Update Node
Update fields or attach additional schemas to an existing node.

**Request Body**:
```json
{
  "fields": {
    "system:node_title": { "type": "String", "value": "Updated Title" }
  }
}
```

#### `DELETE /api/nodes/{id}` — Delete Node
Delete node by UUID. Returns `200 OK`.

---

### PQL Query REST API

#### `POST /api/query` — Execute PQL Query
Executes a surface PQL query against the database.

**Request Body**:
```json
{
  "query": "MATCH (n) IN space(\"default\") WHERE n CONFORMS TO schema(\"journal/JournalEntry\") RETURN n LIMIT 10"
}
```

**Response (200 OK)**: JSON array of query result rows or node objects.

---

### Schemas REST API

#### `GET /api/schemas` — List Schemas
Returns all registered system and plugin schema definitions.

#### `GET /api/schemas/{id}` — Get Schema
Returns details for a specific schema UUID.

---

### Plugins REST API

#### `GET /api/plugins` — List Plugins
Lists all installed plugins along with their endpoints, schemas, and UI component definitions.

#### `GET /api/plugins/{id}` — Get Plugin Details
Returns details for a specific plugin ID.

#### `GET /api/plugins/{id}/static` — List Plugin UI Assets
Returns array of static UI asset file paths inside the `.panoapp` bundle.

#### `GET /plugin/{plugin_id}/ui/{*path}` — Serve Plugin UI Asset
Serves raw frontend static asset files (JavaScript, CSS, HTML, images).

#### `* /plugin/{plugin_id}/{*path}` — Plugin Endpoint Dispatch
Forwards HTTP request directly to the specified plugin handler (supports `GET`, `POST`, `PUT`, `DELETE`, `PATCH`).

---

### Object Storage REST API

#### `PUT /api/objects/{bucket}/{key}` — Upload Object
Uploads raw binary payload into specified bucket and key.

#### `GET /api/objects/{bucket}/{key}` — Download Object
Downloads binary blob content with `Content-Type` header.

#### `DELETE /api/objects/{bucket}/{key}` — Delete Object
Removes binary file from object storage.

#### `GET /api/objects/{bucket}` — List Objects
Lists objects in a bucket. Query param `?prefix=` filters by key prefix.

#### Resumable Chunked Uploads

1. **Initiate Session**: `POST /api/uploads`
   ```json
   {
     "bucket": "media",
     "key": "audio.mp3",
     "mime_type": "audio/mpeg",
     "total_size": 10485760
   }
   ```
   *Response*: `{ "upload_id": "session-uuid" }`

2. **Upload Chunks**: `POST /api/uploads/{upload_id}/chunks`
   ```json
   {
     "chunk_index": 0,
     "data": "<base64_encoded_chunk_data>"
   }
   ```

3. **Complete Upload**: `POST /api/uploads/{upload_id}/complete`
   Assembles all uploaded chunks into the final object storage file.

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
