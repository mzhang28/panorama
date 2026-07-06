# Panorama Performance & Profiling Report

This report outlines concrete performance improvements based on a deep architectural review of the entire codebase, including the WASM runtime, reactor pipeline, meta tables, and the database query compiler. 

## 1. Actual Performance Bottlenecks & Improvements

### 1.1 WASM Runtime Instantiation & Linker Churn (`wasm_runtime.rs`)
- **The Bug:** In `execute_wasm_handler`, on *every single request* to a plugin, the system creates a brand new `wasmtime::Linker`, re-registers WASI, re-wraps and registers all 6 host functions (`host_ctx_query`, etc.), and then calls `instantiate_async` from scratch.
- **The Impact:** This adds milliseconds of CPU overhead to every plugin HTTP request and reactor action. Linking and instantiation are heavy operations meant to be done once per module.
- **The Fix:** Move the `Linker` creation and host function `.func_wrap` registrations into `plugin_loader.rs` during plugin initialization. Cache a `wasmtime::InstancePre` for each plugin, allowing nearly instant instantiation for each request.

### 1.2 Eager Reactor Pipeline N+1 Sequential Execution (`api.rs`)
- **The Bug:** In `create_node` and `update_node`, the API iterates sequentially over every field in a node (`for field_path in &field_keys`). For each field, it awaits `state.eager_pipeline.execute_hook(...)`.
- **The Impact:** If a node has 50 fields, creating it blocks on 50 sequential reactor pipeline executions. Since the pipeline involves checking triggers and potentially running WASM actions, write throughput will plummet as field counts grow.
- **The Fix:** The `BeforeFieldWrite` hook point should be modified to accept a batch of fields, or the `api.rs` loop should execute these hooks concurrently using `futures::future::join_all`. 

### 1.3 `sync_field_presence` N+1 Database Lookups (`meta.rs`)
- **The Bug:** During node creation or updates, `sync_field_presence` iterates through every field on the node and calls `resolve_ns_id`. This method executes a `SELECT` against the `namespaces` table.
- **The Impact:** A single node update with 30 fields executes 30 synchronous `SELECT` queries inside the write transaction, causing massive SQLite WAL contention and write latency.
- **The Fix:** Push the N+1 loop down into SQLite. Instead of iterating in Rust, serialize the fields into a JSON array and use SQLite's `json_each` to execute an `INSERT OR IGNORE` for new namespaces followed by an `INSERT INTO ... SELECT` for `field_presence`. 
  - **SQL Pattern:**
    ```sql
    -- 1. Ensure all namespaces exist
    INSERT OR IGNORE INTO namespaces (kind, stable_identifier)
    SELECT 'app', json_extract(value, '$.ns') FROM json_each(?);
    
    -- 2. Bulk insert into field_presence
    INSERT INTO field_presence (ns_id, field_name, node_id, value_type)
    SELECT ns.ns_id, json_extract(je.value, '$.name'), ?, json_extract(je.value, '$.type')
    FROM json_each(?) je
    JOIN namespaces ns ON ns.stable_identifier = json_extract(je.value, '$.ns');
    ```
  - **Implementation Notes:** The `fields` JSON array is bound twice (once per statement), which is computationally cheap. We can `JOIN` solely on `stable_identifier` since our schema enforces it as globally `UNIQUE`. Finally, because `sync_field_presence` runs within `NodeStorage::create_node`, this executes within the existing overarching database transaction. This completely eliminates the Rust-to-SQLite boundary overhead per field.

### 1.4 The Statement Cache Hit Path is Defeated (`sqlite.rs`)
- **The Bug:** In `StorageBackend::query`, when there is a cache hit on the IR shape (`self.statement_cache.get_sql(cache_key)`), the code immediately calls `compile(&ast, &conn)` anyway just to extract the new parameters.
- **The Impact:** This completely defeats the purpose of the query cache. A cache hit still performs Phase 1 (database lookups for meta tables) and Phase 2 (full AST traversal and SQL string construction) of compilation.
- **The Fix:** Extract parameters during the IR lowering phase, or cache the `CompileCtx` so that `compile_phase2` can be used to skip the meta-table lookups.

### 1.5 Inefficient Row Deserialization (`sqlite.rs`)
- **The Bug:** In `execute_compiled`, every single column returned by SQLite is fetched as a `String` (`row.get::<_, String>(i)`), and then passed to `serde_json::from_str(&s)`. 
- **The Impact:** This means `id` (UUID), `space_id`, `created_at`, `updated_at`, and all normal text fields are needlessly subjected to a full JSON parser state machine. In a query returning hundreds of nodes, this causes massive CPU overhead and memory allocation churn.
- **The Fix:** Use `row.get_ref(i)` to inspect the underlying SQLite data type. Only attempt `serde_json::from_str` if the column is explicitly known to be JSON (e.g., `fields_json`).

### 1.6 Unwindowed `field_stats` (`meta.rs`)
- **The Gap:** `compiler.rs` correctly records scans via `record_scan_stat`, but as noted in earlier audits, these are lifetime counters.
- **The Impact:** The system cannot distinguish between a query that was scanned 10,000 times a year ago vs. one being scanned 100 times a second right now.
- **The Fix:** Implement windowing (e.g., hourly/daily buckets) for `field_stats` to make the data actionable for auto-indexing algorithms.

---

## 2. Industry-Standard Benchmarking & Profiling Methods

To systematically measure and improve Panorama's performance (and prevent regressions like the cache bug), the following industry-standard tools and strategies should be adopted:

### 2.1 Micro-benchmarking (Rust/Core)
- **`criterion.rs` / `divan`**: Write micro-benchmarks targeting `panorama_core::query::parse_query` and `compile()`. This would immediately flag that the cache hit path in `sqlite.rs` is taking exactly as long as a cache miss.

### 2.2 System Load Testing (API/Server)
- **`k6` or `Vegeta`**: Create load-testing scripts simulating real-world concurrent user loads on `POST /api/query`.
- **Data Scaling Tests**: Generate synthetic datasets (e.g., 100,000 nodes) and benchmark to measure the explicit penalty of `json_extract` scans versus hypothetical promoted indexed columns.

### 2.3 CPU and Memory Profiling
- **Linux `perf` & Flamegraphs**: Run `cargo flamegraph` during a load test. Analyzing the SVG will likely reveal `wasmtime::Linker` setup and `serde_json::from_str` dominating the CPU time during WASM execution and database querying, respectively.
- **Heap Profiling (`DHAT`)**: Use DHAT to track peak memory usage and allocation rates. The current `execute_compiled` row mapping and WASM JSON marshalling will show a massive number of short-lived string allocations.

### 2.4 Query Execution Profiling (SQLite)
- **`EXPLAIN QUERY PLAN`**: Automatically prepend `EXPLAIN QUERY PLAN` to generated SQL in test suites to assert that expected indices are actually being used by SQLite.
- **Distributed Tracing (`tracing` + Jaeger)**: Instrument `panorama-server` with `tracing` spans to break down latency across HTTP overhead, WASM initialization, PQL Parsing, SQL Execution, and JSON Serialization.
