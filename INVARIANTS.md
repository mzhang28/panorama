# Core Invariants in Panorama

This document outlines the **5 top critical invariants** governing the design, correctness, security, and performance of Panorama. Each invariant specifies strict rules that must hold at all times, along with links to responsible code components and suggestions for automated harness testing.

---

## 1. INV-1: Meta-Table Write Atomicity (`field_presence` & `node_schema_conformance`)

- **Scope**: Node Storage & Meta-Store Engine
- **Specification**: Every operation that creates, updates, or deletes a node (or modifies its fields or attached schemas) MUST update `field_presence` and `node_schema_conformance` synchronously within the exact same database transaction.
- **Why It Matters**: Two-phase query compilation ([compiler.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/query/compiler.rs)) emits SQL CTEs that join against `field_presence` and `node_schema_conformance` instead of parsing raw JSON at query time. If meta tables become out of sync with actual node data, queries will produce phantom matches or drop valid nodes.
- **Code Locations**:
  - [meta.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/meta.rs#L11-L16) (Write invariants specification)
  - [meta.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/meta.rs#L699-L775) (`MetaStore::sync_field_presence`, `MetaStore::sync_node_conformance`, `MetaStore::remove_field_presence`)
  - [api.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/api.rs) (Node mutation pipeline)
- **Suggested Test Harness**:
  - **Fuzz / Differential Harness**: Execute random CRUD operations on nodes, then assert via SQL invariant check:
    ```sql
    -- Invariant check query: any field in nodes' fields JSON MUST exist in field_presence, and vice versa
    SELECT n.id FROM nodes n WHERE EXISTS (
      -- compare json_each(n.fields) against field_presence table
    );
    ```
  - Test transaction aborts/rollback scenarios to verify `field_presence` and `node_schema_conformance` roll back cleanly with `nodes`.

---

## 2. INV-2: Query Optimizer Index Gating (`managed_indexes.status == 'ready'`)

- **Scope**: Query Compiler & Index Lifecycle Manager
- **Specification**: The query optimizer/compiler MUST ONLY select and emit execution plans targeting managed indexes whose status in `managed_indexes` is strictly `'ready'`. Indexes with status `'building'`, `'stale'`, or `'dropped'` MUST NEVER be used during query planning.
- **Why It Matters**: Managed indexes undergo asynchronous background construction. If the compiler generates an index scan against an index in `'building'` state, the query will return incomplete data or fail.
- **Code Locations**:
  - [meta.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/meta.rs#L83-L100) (`IndexStatus` enum: `Building`, `Ready`, `Stale`, `Dropped`)
  - [compiler.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/query/compiler.rs) (`compile` phase 1 index resolution)
- **Suggested Test Harness**:
  - **State Machine Integration Harness**: Insert an index entry with `status = 'building'`, populate sample nodes, run a query that matches the index pattern, and verify the compiled query uses CTE/JSONB scan rather than the unready index. Then update status to `'ready'` and assert the plan now utilizes the index.

---

## 3. INV-3: Required Schema Write Gate (Transaction Rollback on Schema Violation)

- **Scope**: Schema Validation Engine & API Handlers
- **Specification**: When a node write or field mutation occurs under a schema with `SchemaMode::Required`, schema validation MUST be evaluated prior to transaction commit. If any `SchemaError` is produced (e.g., missing required fields, field type mismatch), the storage transaction MUST abort and roll back, leaving storage completely unchanged.
- **Why It Matters**: Nodes marked with `SchemaMode::Required` promise strict type and structural compliance. Partial writes or unvalidated invalid data corrupt data integrity and downstream plugin expectations.
- **Code Locations**:
  - [schema.rs](file:///home/michael/Projects/panorama/crates/panorama-core/src/schema.rs#L82-L89) (`SchemaMode::Required` vs `SchemaMode::Preferred`)
  - [schema.rs](file:///home/michael/Projects/panorama/crates/panorama-core/src/schema.rs#L103-L120) (`SchemaValidationResult`, `SchemaError`)
  - [api.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/api.rs) (Validation gate on write endpoints)
- **Suggested Test Harness**:
  - **Negative Test Harness**: Register a `Required` schema requiring `user:email: String`. Issue API requests missing `user:email` or passing an Integer. Assert response returns HTTP 400 / schema error AND verify database state has 0 newly inserted/modified rows.

---

## 4. INV-4: WASM Capability Authorization Sandbox (`CapabilityGrants` Verification)

- **Scope**: WASM Plugin Host Runtime & Security Subsystem
- **Specification**: Every operation performed by a WASM plugin (reading/writing namespaced fields, contacting network hosts, accessing object storage, reading/writing files) MUST be matched against the plugin's granted `CapabilityGrants`. If an operation is not authorized by a matching grant or wildcard pattern (e.g., `journal:*`), it MUST be rejected immediately with an authorization error.
- **Why It Matters**: Panorama executes untrusted third-party apps as WASM modules (`.panoapp`). Capabilities prevent compromised or rogue plugins from exfiltrating data, accessing fields outside their namespace, or performing unauthorized I/O.
- **Code Locations**:
  - [capabilities.rs](file:///home/michael/Projects/panorama/crates/panorama-core/src/capabilities.rs#L54-L81) (`can_read_field`, `can_write_field`, `can_contact_host`)
  - [wasm_runtime.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/wasm_runtime.rs) (Host function interceptors and capability checks)
- **Suggested Test Harness**:
  - **Sandboxed Plugin Policy Harness**: Instantiate a mock WASM plugin granted `journal:*` read capability. Attempt to execute host calls requesting `wakatime:hours` read or host network calls to `unauthorized.com`. Verify host returns capability denied errors for all non-granted operations.

---

## 5. INV-5: Space Boundary Data Isolation (`space_id` Partitioning)

- **Scope**: Multi-Tenant Authorization & Query AST Compiler
- **Specification**: Every node in storage belongs to a `space_id`. All node queries, reads, updates, deletes, and index scans MUST include explicit `space_id` scoping filters (e.g. `n.space_id = ?`). Nodes MUST NOT be leaked across space boundaries unless an explicit cross-space sharing/replication operation is requested.
- **Why It Matters**: Spaces represent the security and permission boundary for user data in Panorama. Missing `space_id` checks in queries or REST endpoints would leak private user data across tenant/space boundaries.
- **Code Locations**:
  - [spaces.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/spaces.rs) (Space permissions and definitions)
  - [compiler.rs](file:///home/michael/Projects/panorama/crates/panorama-server/src/query/compiler.rs#L73-L81) (`n.space_id = ?` filter in query CTE generation)
- **Suggested Test Harness**:
  - **Cross-Tenant Isolation Harness**: Create two distinct spaces (`space_A` and `space_B`). Seed identical node keys into both spaces. Query nodes in `space_A` and verify zero returned nodes belong to `space_B`. Attempt direct API access to `space_B` nodes using a context scoped to `space_A` and verify rejection.
