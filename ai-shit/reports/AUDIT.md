# Panorama Codebase Quality & Security Audit

This document summarizes the findings from a code quality and security audit conducted on the Panorama codebase. The audit focuses on identifying security vulnerabilities, swallowed/suppressed errors, stubbed/mocked implementations, and notable performance or design inefficiencies.

---

## 1. Security Vulnerabilities

### 🚨 WASM Host-Guest Buffer Overflow / Heap Corruption
* **Location:** 
  * Host side: [`crates/panorama-server/src/wasm_runtime.rs`](file:///home/michael/Projects/panorama/crates/panorama-server/src/wasm_runtime.rs#L166-L173) (functions: `host_ctx_create_nodes`, `host_ctx_create_node`, `host_ctx_get_node`, `host_ctx_update_node`, and `host_ctx_query`)
  * Guest side: [`crates/panorama-core/src/wasm_adapter.rs`](file:///home/michael/Projects/panorama/crates/panorama-core/src/wasm_adapter.rs#L171-L182)
* **Description:** 
  When the WASM guest invokes context functions to retrieve or modify data, the guest allocates a fixed buffer of size `HOST_BUF_SIZE` (512 KiB) in its linear memory and passes a raw pointer (`r_ptr`) to the host. 
  However, the host functions do not receive the size of the guest-allocated buffer. Instead, the host copies serialized JSON data up to the limit of the remaining WASM linear memory:
  ```rust
  let wl = out_bytes.len().min(data_mut.len() - r_start);
  data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
  ```
  If the host returns a dataset (such as query results or node listings) larger than 512 KiB, it will copy more than 512 KiB of data starting at `r_ptr`. This overwrites active heap structures in the guest WASM module, causing heap corruption, buffer overflows, and crashes.
* **Remediation:** 
  Modify the host FFI signatures to accept a maximum buffer size parameter (`r_len`) and enforce `wl = out_bytes.len().min(r_len as usize)`.

---

### 🚨 SQL Injection in PQL Relationship Edge Queries
* **Location:** [`crates/panorama-server/src/query/compiler.rs`](file:///home/michael/Projects/panorama/crates/panorama-server/src/query/compiler.rs#L273-L278)
* **Description:** 
  The PQL relationship edge type is parsed as a string literal (using double quotes) and allows arbitrary characters, including single quotes. When compiling relationship hops, the query compiler constructs SQL by direct interpolation:
  ```rust
  let edge_json_path = format!("\"{}\"", edge_type);
  let join_condition = format!(
    "t.id = json_extract(s.fields_json, '$.{}.value')",
    edge_json_path
  );
  ```
  The resulting `join_condition` is later interpolated directly into CTE SELECT definitions. If an attacker inputs a relationship query such as `MATCH (a)-[:REF("attendee' OR 1=1 --")]->(b)`, the single quote escapes SQLite's `'$.{}.value'` JSON path literal, injecting arbitrary logic and commenting out the rest of the query.
* **Remediation:** 
  Sanitize the `edge_type` identifier by checking it against valid schemas, escaping single quotes, or using SQLite parameters for the json-path query parameter.

---

## 2. Swallowed & Suppressed Errors

### ⚠️ Silent Fallbacks on File Compression/Decompression Failures
* **Location:** [`crates/panorama-server/src/panoapp.rs`](file:///home/michael/Projects/panorama/crates/panorama-server/src/panoapp.rs#L100-L110)
* **Description:** 
  When importing `.panoapp` ZIP packages, if the WASM module fails to load or decompress, the error is swallowed and `wasm_bytes` is silently set to `None`:
  ```rust
  let wasm_bytes = {
    let wasm_path = manifest.wasm_module.as_deref().unwrap_or("plugin.wasm");
    match archive.by_name(wasm_path) {
      Ok(mut wf) => { ... }
      Err(_) => None, // Swallows zip format errors and missing files
    }
  };
  ```
  Additionally, when loading static files, any failed read results in silent omission from the app's static file manifest rather than raising a package corruption error:
  ```rust
  if entry.read_to_end(&mut buf).is_ok() {
    ui_files.insert(rel, buf);
  }
  ```

---

### ⚠️ Blind SQLite Value Mapping Fallbacks
* **Location:** [`crates/panorama-server/src/storage/sqlite.rs`](file:///home/michael/Projects/panorama/crates/panorama-server/src/storage/sqlite.rs#L170-L185)
* **Description:** 
  When parsing values from SQLite rows, multiple data conversion errors (such as invalid UTF-8 strings or malformed JSON payloads) are swallowed and replaced with fallback values (`serde_json::Value::Null` or empty strings `""`):
  ```rust
  Ok(ValueRef::Real(f)) => serde_json::Number::from_f64(f)
    .map(serde_json::Value::Number)
    .unwrap_or(serde_json::Value::Null),
  Ok(ValueRef::Text(bytes)) => {
    let s = std::str::from_utf8(bytes).unwrap_or("");
    if col.ends_with("_json") {
      serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_string()))
    } else {
      serde_json::Value::String(s.to_string())
    }
  }
  Err(_) => serde_json::Value::Null,
  ```
  This conceals database corruption and schema representation inconsistencies from the application layer.

---

### ⚠️ Missing Error Handling in Frontend JSON Parsing
* **Location:** [`frontend/src/api/client.ts`](file:///home/michael/Projects/panorama/frontend/src/api/client.ts#L94-L108)
* **Description:** 
  The frontend client safely wraps `fields_json` parsing in a `try-catch` block, but leaves `row.preferred_schemas_json` unhandled:
  ```typescript
  preferred_schemas: row.preferred_schemas_json
    ? typeof row.preferred_schemas_json === "string"
      ? JSON.parse(row.preferred_schemas_json) // throws on invalid JSON
      : row.preferred_schemas_json
    : [],
  ```
  If `preferred_schemas_json` contains malformed or unexpected data, it will crash the client interface.

---

## 3. Stubbed Implementations

### 🚧 Memory-Only Authorization & Hooks Bypasses
* **Location:** [`crates/panorama-server/src/spaces.rs`](file:///home/michael/Projects/panorama/crates/panorama-server/src/spaces.rs#L28-L33) & [`crates/panorama-server/src/api.rs`](file:///home/michael/Projects/panorama/crates/panorama-server/src/api.rs#L192)
* **Description:** 
  * `SpaceManager` permissions and multi-user authentication are stubbed for v0.0 (`/// For v0.0, this is mostly a stub - real auth will come later.`). They use an in-memory `DashMap` and lack database persistence.
  * In the API endpoints (e.g., node creation, updates), the `authorized_by` parameter passed to eager pipeline hooks is hardcoded to `None`, bypassing authentication checks.

---

### 🚧 Unimplemented PromQL Post-Processing Operators
* **Location:** [`crates/panorama-app-dashboards/src/promql/translator.rs`](file:///home/michael/Projects/panorama/crates/panorama-app-dashboards/src/promql/translator.rs#L1002-L1006)
* **Description:** 
  In the PromQL translation post-processing block, advanced steps like standard deviation (`GroupStddev`) and variance (`GroupStdvar`) are matched by the catch-all `_` fallback. The translator silently passes through raw data points without returning an error or notifying the user that the requested mathematical operation is unimplemented.

---

## 4. Notable Design & Performance Concerns

### 📉 In-Memory Database Filtering
* **Location:** 
  * [`crates/panorama-app-music/src/lib.rs`](file:///home/michael/Projects/panorama/crates/panorama-app-music/src/lib.rs#L289-L308)
  * [`crates/panorama-app-trips/src/lib.rs`](file:///home/michael/Projects/panorama/crates/panorama-app-trips/src/lib.rs#L358-L376)
* **Description:** 
  Several third-party applications query the database for all nodes in a space (e.g. using `MATCH (n)`) and filter them in memory using Rust iterators:
  * The Music app fetches all nodes with a title to filter for artists.
  * The Trips app queries all events in a space and filters them by `trip_id` in Rust memory.
  As the size of the database grows, this pattern results in high latency, large memory allocations, and CPU overhead. Filtering should instead be offloaded to PQL query criteria.

---

### 📉 Panic Vulnerability in WASM Async Bridge
* **Location:** [`crates/panorama-core/src/wasm_adapter.rs`](file:///home/michael/Projects/panorama/crates/panorama-core/src/wasm_adapter.rs#L95-L98)
* **Description:** 
  Because WASM guest functions must run synchronously under the current architecture, any async task invoked within a plugin must be executed via `block_on`. If a future returns `Poll::Pending` (even if it's just yielding control to a microtask/tick), the execution fails immediately:
  ```rust
  Poll::Pending => {
    panic!("async future yielded Pending in WASM — all host functions must be synchronous")
  }
  ```
  This makes writing complex asynchronous logic in plugins highly brittle, as standard Rust async functions and libraries often yield `Pending` once before completing.
