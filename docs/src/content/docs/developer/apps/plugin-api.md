---
title: Plugin API Reference
description: API documentation for the PluginContext interface and CapabilityGrants.
---

Plugins interact with the Panorama host environment through a set of secure APIs. The primary interface is the `PluginContext` trait defined in `panorama-core`.

---

## 1. PluginContext API Reference

The `PluginContext` provides methods to read and write database nodes, run queries, read/write binary objects, and trigger operations.

### Storage Operations

#### `create_nodes`
Creates multiple nodes in a single, atomic database transaction. Required schema validations are evaluated during this operation.
```rust
async fn create_nodes(&self, nodes: Vec<Node>) -> Result<Vec<Node>, PluginError>;
```

#### `create_node`
Convenience wrapper to create a single node.
```rust
async fn create_node(&self, node: Node) -> Result<Node, PluginError>;
```

#### `get_node`
Fetches a single node by its UUID.
```rust
async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError>;
```

#### `update_node`
Updates fields on an existing node. Validates changes against schemas.
```rust
async fn update_node(
    &self,
    id: Uuid,
    fields: HashMap<String, FieldValue>,
) -> Result<Node, PluginError>;
```

#### `delete_node`
Deletes a node from storage by UUID.
```rust
async fn delete_node(&self, id: Uuid) -> Result<(), PluginError>;
```

### Querying Data

#### `query`
Executes a PQL query string against the database and returns matching rows as JSON values.
```rust
async fn query(&self, query_string: &str) -> Result<Vec<serde_json::Value>, PluginError>;
```

### Schema Operations

#### `register_schema`
Registers a new schema node in the platform.
```rust
async fn register_schema(&self, schema: Schema) -> Result<Schema, PluginError>;
```

#### `get_schema`
Fetches a schema definition by ID.
```rust
async fn get_schema(&self, id: Uuid) -> Result<Option<Schema>, PluginError>;
```

### Object Storage Operations

#### `put_object`
Stores a binary file blob in object storage. Returns an `ObjectRef` which can be attached to nodes.
```rust
async fn put_object(
    &self,
    bucket: &str,
    key: &str,
    data: Bytes,
    mime_type: &str,
) -> Result<ObjectRef, PluginError>;
```

#### `get_object`
Fetches binary blob bytes from storage.
```rust
async fn get_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectData>, PluginError>;
```

#### `delete_object`
Removes an object from the binary store.
```rust
async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), PluginError>;
```

#### `list_objects`
Lists objects in a bucket matching an optional key prefix.
```rust
async fn list_objects(&self, bucket: &str, prefix: Option<&str>) -> Result<Vec<ObjectRef>, PluginError>;
```

### Utility Methods

#### `plugin_id`
Returns the plugin's own ID as assigned by the platform.
```rust
fn plugin_id(&self) -> &str;
```

#### `base_path`
Returns the base URL path prefix for this plugin's HTTP endpoints (`/plugin/<app-id>`).
```rust
fn base_path(&self) -> String;
```

#### `log`
Emits a log message through the platform's logging system.
```rust
async fn log(&self, level: LogLevel, message: &str);
```

`LogLevel` variants: `Debug`, `Info`, `Warn`, `Error`.

---

## 2. Plugin Lifecycle Methods

In addition to `handle_http_request`, the `Plugin` trait provides lifecycle hooks:

#### `initialize`
Called once when the plugin is loaded. Use this to perform setup (register schemas, seed data, etc.).
```rust
async fn initialize(&self, _ctx: &dyn PluginContext) -> Result<(), PluginError> {
    Ok(())
}
```

#### `background_tasks`
Declares background tasks the platform should run for this plugin.
```rust
fn background_tasks(&self) -> Vec<BackgroundTask> {
    vec![BackgroundTask {
        name: "cleanup".into(),
        interval_seconds: Some(3600),
        description: "Hourly cleanup job".into(),
    }]
}
```

#### `run_background_task`
Called by the platform to execute a named background task.
```rust
async fn run_background_task(
    &self,
    task_name: &str,
    ctx: &dyn PluginContext,
) -> Result<(), PluginError>;
```

---

## 3. Capabilities Sandbox Model

Every context operation is subject to verification against the app's `CapabilityGrants` declared in `manifest.json`.

| Field Name | Type | Valid Formats | Description |
|---|---|---|---|
| `version` | Integer | `1`, `2`, … | Bumped on major capability changes; triggers user re-approval. |
| `reason` | String or null | `"Added GitHub API access"` | Human-readable explanation for capability changes. |
| `field_read` | Array | `["com.my-app:*", "system:*"]` | Allowed namespace glob patterns the app is permitted to read. |
| `field_write` | Array | `["com.my-app:*", "system:node_title"]` | Allowed namespace glob patterns the app is permitted to write. |
| `network_hosts` | Array | `["api.github.com", "example.org"]` | Whitelisted host domains the plugin is allowed to make HTTP calls to. |
| `dns_requests` | Boolean | `true` or `false` | Permission to perform DNS lookups (per-host grants in `network_hosts`). |
| `object_storage_read` | Boolean | `true` or `false` | Access to download blobs from object storage. |
| `object_storage_write`| Boolean | `true` or `false` | Access to upload/delete blobs in object storage. |
| `file_read` / `file_write` | Boolean | `true` or `false` | Access to read/write from local host filesystem. |
| `execute` | Boolean | `true` or `false` | Access to spawn subprocesses on the host. |
| `write_own_nodes` | Boolean | `true` or `false` | Grants write access to nodes created by this plugin, regardless of field namespaces. |
| `app_managed_nodes` | Boolean | `true` or `false` | Permission to mark nodes as app-managed (immutable by direct user edits). |

Any attempt to perform an operation not covered by capability grants returns a `PluginError::permission_denied` error.

---

## 4. Error Handling & Wasm Backtraces

`PluginError` carries structured diagnostic data that survives the wasm/host boundary — nothing is flattened to a string.

### Error constructors

| Constructor | Host calls | Captures |
|---|---|---|
| `PluginError::bad_request(msg)` | 0 | File + line (via `#[track_caller]`) |
| `PluginError::not_found(msg)` | 0 | File + line |
| `PluginError::permission_denied(msg)` | 0 | File + line |
| `PluginError::internal(msg)` | 0 | File + line |
| `PluginError::with_backtrace(code, msg, status)` | 1 | File + line + **full wasm call chain** |

All constructors use `#[track_caller]`, so every error carries the exact `file:line` where it was created — free, no host round-trip.

`with_backtrace` additionally calls the `host_capture_backtrace` host import, which walks the JIT frame-pointer chain and captures every wasm frame currently on the stack, plus DWARF-resolved source locations (`file:line:column`). One call per error origin, not per `?` hop — a single call at the deepest level captures every caller above it.

### When to use `with_backtrace`

Use `PluginError::with_backtrace` at the **origin** of an error deep in a call chain:

```rust
fn level3() -> Result<HttpResponse, PluginError> {
    Err(PluginError::with_backtrace(
        "DEEP_ERROR",
        "something went wrong at the lowest level".into(),
        500,
    ))
}
fn level2() -> Result<HttpResponse, PluginError> { level3() }
fn level1() -> Result<HttpResponse, PluginError> { level2() }
```

The resulting backtrace contains `level1 → level2 → level3` — the full chain captured by one host call at `level3`.

For validation errors at the handler level (`bad_request`, `not_found`), the default constructors are sufficient — `#[track_caller]` already tells you the exact line.

### For non-Rust languages

The primitive is a raw wasm import:

```wat
(import "env" "host_capture_backtrace" (func $host_capture_backtrace (result i64)))
```

Call it at your error origin. It returns an opaque `u64` id (0 on failure). Include this id in your error response as `backtrace_id`. The host resolves it to a full frame chain with DWARF file:line info.

To preserve panic messages through traps, also import and call before aborting:

```wat
(import "env" "host_report_panic" (func $host_report_panic (param i32 i32)))
```

Pass a pointer and length to the panic message string. The host stores it and attaches it to the trap error.

**Important**: DWARF debug info is only available when the wasm binary is compiled with debug symbols (e.g. Rust debug builds, or `-g` in clang). Release builds retain function names from the wasm `name` section but not file:line. For production, use `wasm-split` to extract DWARF into a companion file uploaded to Sentry, leaving only a `build_id` section in the shipped binary.

### Trap errors

Panics (`panic!`, `unreachable`, OOB access) are automatically caught by the wasmtime runtime. The host extracts the `WasmBacktrace` from the trap and attaches all frames to the error — no guest cooperation needed. The panic hook in `wasm_adapter::run_plugin` reports the panic message + location to the host before the module aborts, so even `panic!` messages are preserved.
