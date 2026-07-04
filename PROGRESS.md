# Panorama — Progress Report

Last updated: 2026-07-04

## Where We Are

Panorama has a functional v0.1 core — the data model, storage engine with pluggable backend abstraction (`StorageBackend`), query language parser/compiler, plugin trait, WASM plugin loader, client-side SPA routing (`TanStack Router`), and HTTP API are functional end-to-end.

Recently, major platform and app milestones were achieved:
- **Storage Backend Abstraction Layer (`StorageBackend`)**: Isolated all SQLite-specific code behind a clean `StorageBackend` trait and `NodeStorage` wrapper, enabling pluggable database backends (e.g., PostgreSQL, DynamoDB) without changing platform or plugin code.
- **Journal App Logseq Block Model Rewrite (`panorama-app-journal`)**: Replaced the old dual-schema model with a unified Logseq-inspired block schema (`journal:block`). Added full block CRUD, tree hierarchy fetching (`/blocks/{id}/tree`), recursive deletion, wiki-link backlink indexer (`/backlinks`), search (`/search`), and a full React outliner frontend (`JournalApp.tsx`) with collapsible block trees, inline editing, indent/outdent, and backlink panel.
- **WakaTime API Complete Implementation (`panorama-app-wakatime`)**: Complete rewrite matching the official WakaTime v1 API specification with a 25+ field schema, heartbeat single/bulk ingestion, durations, summaries, overall stats, project listing, and API key authentication.
- **Grafana PromQL Engine & Dashboard Builder (`panorama-app-grafana`)**: Implemented full PromQL recursive-descent parser, AST, and translator into Panorama Query Language (PQL), supported by a metric registry, PromQL query validation endpoint, dataset query execution, and a React dashboard builder UI rendering 6 chart types with inline SVG.
- **Client-Side SPA Routing**: Replaced state-based view switching in `App.tsx` with TanStack Router (`@tanstack/react-router` v1.170) supporting URL-based navigation across all main panels and app views.

All 9 end-to-end integration test scenarios (`just test-e2e`) pass automatically against a real server instance.

---

## 1. Platform Layer — What's Built vs. What's Missing

### 1.1 Core Data Model (`panorama-core`)

| Feature | Status | Notes |
|---------|--------|-------|
| Node (UUID, fields, space_id, timestamps) | ✅ Done | Serialized as JSONB in `nodes` table |
| FieldValue (10 variants) | ✅ Done | String, Integer, Float, Boolean, DateTime, Array, NodeRef, Json, ObjectRef, Binary |
| Schema, SchemaField, FieldTypeConstraint | ✅ Done | |
| StorageBackend Trait Abstraction | ✅ Done | `StorageBackend` trait in `crates/panorama-server/src/storage` isolates all database queries behind a backend-agnostic interface. `SqliteBackend` implements SQLite; future engines (PostgreSQL, DynamoDB) can be added without modifying server logic |
| SchemaMode (Preferred / Required) | ⚠️ Partial | `Node` struct has `preferred_schemas: Vec<SchemaRef>`. `system_fields::REQUIRED_SCHEMAS` constant exists, but required schemas share validation in `SchemaRegistry::validate_required`. |
| Schema versions (major.minor) | ✅ Done | Compatibility checks implemented (`is_compatible_with`) |
| Migrations (field_mappings) | ⚠️ Defined, not executed | `Migration` struct exists with `field_mappings: HashMap<String, String>` but there is no engine that applies migrations to existing nodes |
| ComputedFieldConfig (Eager/Deferred/Read) | ⚠️ Type only | Types defined. `evaluate_simple_expression` exists for basic field/ref resolution. No execution engine runs computed fields on write or read |
| Cycle detection | ✅ Done | `detect_cycles` function in `field.rs` |
| System schemas (NodeTime, NodeInfo) | ✅ Done | Defined in `schema.rs::system_schemas` and registered at startup |
| Field namespaces | ✅ Done | system/user reserved IDs (1, 2), app namespaces auto-registered |
| CRDT types (counter, or-set, rga-text) | ❌ Missing | `FieldValue` has no CRDT variants. QUERY_DESIGN.md §3.7 references these in its operator table but they don't exist |
| CRDT merge semantics | ❌ Missing | No merge logic for concurrent writes |

### 1.2 Query Language

| Feature | Status | Notes |
|---------|--------|-------|
| MATCH (n) IN space(...) | ✅ Done | |
| CONFORMS TO schema(...) with version ranges | ✅ Done | Compiles to semi-join on `node_schema_conformance` |
| Field comparisons (= != < <= > >=) | ✅ Done | Via `json_extract` on `fields_json` |
| HAS_FIELD with namespace | ✅ Done | Compiles to semi-join on `field_presence` |
| HAS_FIELD with wildcard (*) namespace | ✅ Done | |
| SCAN(...) wrapper | ⚠️ Parser only | Parser accepts `SCAN(...)` but compiler does NOT enforce it — unindexed predicates without SCAN are not rejected |
| IS NULL / IS NOT NULL | ✅ Done | |
| IN [...] | ✅ Done | |
| LIKE | ✅ Done | |
| Boolean AND / OR / NOT | ✅ Done | |
| RefTraverse (-[:REF("field")]->) single-hop | ✅ Done | Compiled to CTE JOIN |
| RefTraverse multi-hop (*1..3) | ⚠️ Parser only | Parser accepts depth syntax but compiler only handles single-hop |
| Reverse traversal (<-[:REF]-) | ❌ Missing | Not in parser or compiler |
| RETURN with field projection and aliases | ✅ Done | |
| ORDER BY ASC/DESC | ✅ Done | Uses `json_extract` — no index pushdown yet |
| LIMIT / SKIP | ✅ Done | |
| CRDT view selectors (@merged, @ops, @at) | ❌ Missing | §3.6 — not in parser or compiler |
| Type-compatibility validation at compile time | ❌ Missing | §3.7 — compiler doesn't check type mismatches at compile time |
| Aggregation (COUNT, SUM, GROUP BY) | ❌ Missing | Explicit non-goal for v0; handled by plugin post-processing (e.g. Grafana PromQL engine) |
| Subqueries | ❌ Missing | Explicit non-goal for v0 |
| Full-text search | ❌ Missing | Explicit non-goal for v0 |

### 1.3 Storage Engine

| Feature | Status | Notes |
|---------|--------|-------|
| StorageBackend Architecture Layer | ✅ Done | `StorageBackend` trait isolates all database interaction (`query`, `create_node`, `update_node`, `delete_node`, `resolve_ns`, `has_ready_index`). `SqliteBackend` implements SQLite storage |
| SQLite with WAL mode | ✅ Done | Separate read (8 conn) / write (1 conn) pools wrapped inside `SqliteBackend` |
| CRUD (create, get, update, delete) | ✅ Done | All wrapped in transactions that sync meta tables |
| Batch create | ✅ Done | Single database transaction |
| meta tables (6/6) | ✅ Done | namespaces, schema_tables, managed_indexes, field_presence, node_schema_conformance, field_stats — all implemented per §6.1 |
| Write invariants (§6.2) | ✅ Done | field_presence and node_schema_conformance synced in same transaction as node writes |
| Prepared statement cache (§7.3) | ❌ Missing | Every query re-prepares the SQL |
| Schema-table promoted columns | ❌ Missing | All data lives in `fields_json` (JSONB) |
| Field promotion (JSONB → column) | ❌ Missing | `PROMOTE FIELD` statement defined in §6.4 but not implemented |
| Managed index lifecycle | ⚠️ Meta only | MetaStore tracks index status transitions (building→ready→stale→dropped) but no actual SQLite index creation is triggered |
| Index suggestion from field_stats | ❌ Missing | `field_stats` collects counters but nothing reads them to suggest indexes |
| Windowed field_stats | ❌ Missing | Counters are lifetime accumulators; no rolling-window implementation (§6.5) |
| Query ID tracing (§8) | ❌ Missing | No query_id assignment or per-stage timing |

### 1.4 HTTP API & Navigation

| Feature | Status | Notes |
|---------|--------|-------|
| Client-Side SPA Navigation | ✅ Done | TanStack Router (`@tanstack/react-router` v1.170) handles URL routing for `/`, `/nodes`, `/schemas`, `/plugins`, `/journal`, `/journal/page/$pageId`, `/grafana` |
| Node CRUD endpoints | ✅ Done | |
| Query language endpoint (POST /api/query) | ✅ Done | |
| Schema list/get endpoints | ✅ Done | From in-memory registry |
| Object storage with resumable uploads | ✅ Done | File-system backed |
| Plugin metadata endpoints | ✅ Done | |
| Plugin HTTP dispatch | ✅ Done | Routes `/plugin/{id}/*` to plugin's handler |
| Plugin UI asset serving | ✅ Done | |
| SPA fallback | ✅ Done | Embedded frontend via rust-embed |

### 1.5 Plugin System

| Feature | Status | Notes |
|---------|--------|-------|
| Plugin trait (native Rust) | ✅ Done | |
| PluginContext trait | ✅ Done | |
| RuntimeContext implementation | ✅ Done | Capability enforcement on field reads/writes |
| WASM plugin loading (.panoapp) | ✅ Done | wasmtime-based |
| HTTP endpoint dispatch | ✅ Done | |
| Background tasks | ⚠️ Trait defined | `background_tasks()` returns definitions; no scheduler that actually runs them |
| UI Component Integration | ✅ Done | Frontends for Journal App (Logseq block tree outliner) and Grafana (PromQL dashboard builder) integrated into main SPA shell |

### 1.6 Permissions & Auth

| Feature | Status | Notes |
|---------|--------|-------|
| CapabilityGrants struct | ✅ Done | Glob-based field/host matching |
| Capability enforcement in RuntimeContext | ✅ Done | Field read/write gating |
| SpaceManager | ⚠️ Stub | In-memory DashMap, no persistence, no auth integration |
| User auth | ❌ Missing | No login, sessions, or user identity at all |
| Space-level permissions | ❌ Missing | DESIGN.md specifies space-level permissions; only stub data structures exist |
| App permission grant UI | ❌ Missing | No way for a user to review/accept an app's capability request |

### 1.7 Frontend

| Feature | Status | Notes |
|---------|--------|-------|
| React SPA shell with TanStack Router | ✅ Done | Client-side routing with nav bar, sidebar, and breadcrumbs |
| Node viewer (list, create, inspect, delete) | ✅ Done | `NodeViewer.tsx` |
| Schema viewer (list, field details) | ✅ Done | `SchemaViewer.tsx` |
| Plugin panel (list, select) | ✅ Done | `PluginPanel.tsx` |
| Journal App UI | ✅ Done | Logseq-inspired block tree outliner (`JournalApp.tsx`). Supports page creation, journal day navigation, collapsible subtrees, block editing, indent/outdent, wiki-links `[[title]]`, and backlink inspector |
| Grafana App UI | ✅ Done | Full React SPA dashboard builder (`panorama-app-grafana/ui/src/App.tsx`). Supports panel CRUD, PromQL editor, grid layout controls, and 6 inline SVG chart types |
| Other app UIs (Wakatime, Files, Beli, Trips, Subsonic) | ⚠️ Stubbed | UI component definitions exist; awaiting dedicated frontend builds or integration |
| Production build embedding | ✅ Done | Via rust-embed into `panorama-server` |

---

## 2. App-by-App Feature Breakdown

### 2.1 Journal App

**What a user expects:** Daily journal with outliner block tree, markdown block editing, calendar/day navigation, page creation, wiki links (`[[page]]`), backlinks inspector, search, recursive delete.

| Feature | Status | Notes |
|---------|--------|-------|
| Unified Logseq Block Schema (`journal:block`) | ✅ Done | Single block schema with parent_id, order (fractional), page_id, journal_day, properties, tags, refs |
| Create page / journal day block | ✅ Done | `POST /blocks` creates root blocks or journal day blocks |
| Get single block | ✅ Done | `GET /blocks/{id}` |
| Update block content, properties, order | ✅ Done | `PUT /blocks/{id}` — supports content, fractional order, and properties |
| Recursive block deletion | ✅ Done | `DELETE /blocks/{id}` — deletes block and all sub-tree descendants recursively |
| List pages | ✅ Done | `GET /pages` returns all root page blocks |
| Get journal day block | ✅ Done | `GET /journal/{day}` auto-creates/fetches day block |
| Get block tree | ✅ Done | `GET /blocks/{id}/tree` returns complete hierarchical block subtree |
| Backlinks query | ✅ Done | `GET /backlinks?page=...` finds blocks referencing a page title via `[[wiki links]]` |
| Block search | ✅ Done | `GET /search?q=...` searches block contents |
| Logseq-inspired Outliner React UI | ✅ Done | Outliner tree with collapsible nodes, inline block editor, bullet point hierarchy, indent/outdent block controls, page creation sidebar, journal day selector, and backlinks inspector panel (`JournalApp.tsx`) |
| Tagging / categories | ⚠️ Schema ready | `tags` field in `journal:block` schema |
| Export (PDF, Markdown zip) | ❌ Missing | |

**Integration tests:** Exercised via `just test-e2e` Playwright suite (Journal block hierarchy, page creation, outliner navigation).

### 2.2 Wakatime App

**What a user expects:** Wakatime-compatible heartbeat endpoint, bulk ingestion, durations calculation, summaries, overall stats, project list, API key auth.

| Feature | Status | Notes |
|---------|--------|-------|
| Single heartbeat ingestion | ✅ Done | `POST /users/current/heartbeats` (and `/heartbeat`) |
| Bulk heartbeat ingestion | ✅ Done | `POST /users/current/heartbeats.bulk` (and `/heartbeats`) |
| Full 25+ field heartbeat schema | ✅ Done | `wakatime:entity`, `wakatime:type`, `wakatime:category`, `wakatime:project`, `wakatime:branch`, `wakatime:language`, `wakatime:lines`, `wakatime:lineno`, `wakatime:cursorpos`, `wakatime:is_write`, etc. |
| Durations API | ✅ Done | `GET /users/current/durations` returns time-bucketed duration breakdowns |
| Summaries API | ✅ Done | `GET /users/current/summaries` daily stats breakdowns over date range |
| Overall Stats API | ✅ Done | `GET /users/current/stats` top projects, top languages, daily average |
| Projects API | ✅ Done | `GET /users/current/projects` list of active coding projects |
| API Key Authorization | ✅ Done | Validates `Authorization: Bearer <key>` and `X-Api-Key` headers |
| Dashboard UI | ⚠️ Via Grafana | Grafana plugin provides PromQL dashboard charts over Wakatime metrics |

**Integration tests:** E2E and unit tests cover single/bulk heartbeat ingestion, durations calculation, and stats reporting.

### 2.3 Grafana App

**What a user expects:** Dashboard builder with panels, time-series queries, multiple chart types, PromQL query language, PromQL metric registry, dashboard CRUD, validation, import/export.

| Feature | Status | Notes |
|---------|--------|-------|
| Save dashboard config | ✅ Done | POST /api/dashboards stores JSON config as a node |
| List dashboards | ✅ Done | GET /api/dashboards |
| Get dashboard by UID | ✅ Done | GET /api/dashboards/{uid} |
| Update dashboard | ✅ Done | PUT /api/dashboards/{uid} with version bumping |
| Delete dashboard | ✅ Done | DELETE /api/dashboards/{uid} |
| Export/import dashboard JSON | ✅ Done | GET /api/dashboards/export/{uid}, POST /api/dashboards/import |
| Duplicate dashboard | ✅ Done | POST /api/dashboards/{uid}/duplicate |
| Home dashboard auto-creation | ✅ Done | GET /api/dashboards/home auto-creates a default on first access |
| Folder support | ✅ Done | GET/POST /api/folders for organizing dashboards |
| PromQL query engine | ✅ Done | Full recursive-descent parser + AST → PQL translator. Supports instant/range vectors, label matchers (= != =~ !~), all binary operators, aggregations (sum/avg/min/max/count by/without), functions (rate/irate/increase/delta/topk/bottomk/histogram_quantile/sort/absent), subqueries, offset, @ modifier |
| Metric registry | ✅ Done | Configurable metric name → namespace/field mapping with WakaTime defaults |
| PromQL validation endpoint | ✅ Done | POST /api/promql/validate returns PQL translation preview |
| Query execution | ✅ Done | POST /api/ds/query accepts batch of PromQL panel queries, translates to PQL, executes via ctx.query(), applies post-processing (rate/increase/group-aggregation/sort/filter) |
| Multiple panel types | ✅ Done | leaderboard, timeseries, stat, piechart, table, heatmap — all render in the React frontend |
| Time range presets | ✅ Done | 15 presets from "Last 1 hour" to "This month", plus relative time parser (now-Nd/Nh/Nm/Ns) |
| Dashboard editor UI | ✅ Done | Full React SPA via Module Federation with panel CRUD, PromQL textarea, grid position editor, panel type selector |
| Inline SVG chart rendering | ✅ Done | All chart types render with inline SVG — no external chart library |
| Template variables | ❌ Missing | Schema exists but not wired to query interpolation |
| Alerting | ❌ Missing | |
| Drag-and-drop layout | ❌ Missing | Grid positions are editable as numbers, no drag handles |

**Integration tests:** E2E tests cover PromQL panel query execution, dashboard CRUD, export/import, and metric registry mapping.

### 2.4 Files App

**What a user expects:** File browser, upload with progress, drag-and-drop, folder organization, previews, sharing, search.

| Feature | Status | Notes |
|---------|--------|-------|
| File upload | ✅ Done | POST /upload stores in object storage + creates file node |
| File download | ✅ Done | GET /files/{id} streams with Content-Disposition |
| File listing | ✅ Done | GET /files returns all file nodes |
| Folder filtering | ✅ Done | GET /files?folder=documents filters in-memory |
| File deletion | ✅ Done | DELETE /files/{id} removes object + node |
| Resumable upload (app-level) | ❌ Missing | POST /upload/initiate returns a redirect to the platform API |
| Drag-and-drop upload UI | ❌ Missing | |
| File previews (images, PDFs) | ❌ Missing | |
| Thumbnails | ❌ Missing | |
| Search by filename | ❌ Missing | |
| Sharing links | ❌ Missing | |

**Integration tests:** Upload + download + list + delete lifecycle test passing.

### 2.5 Beli (Restaurant Ratings) App

**What a user expects:** Browse restaurants, rate via comparisons, see personal rankings, discover new places, see friend rankings, photos, maps integration.

| Feature | Status | Notes |
|---------|--------|-------|
| Add restaurant (name, cuisine, location, notes) | ✅ Done | POST /restaurants |
| List restaurants | ✅ Done | GET /restaurants |
| Record pairwise comparison (A > B) | ✅ Done | POST /compare with better_id + worse_id |
| Partial order rankings (topological sort) | ✅ Done | Kahn's algorithm with tiered output |
| Restaurant edit/delete | ❌ Missing | |
| Restaurant photos | ❌ Missing | |
| Map view of restaurants | ❌ Missing | |

**Integration tests:** Pairwise comparison + 3-tier topological sort ranking test passing.

### 2.6 Trips App

**What a user expects:** Trip itinerary builder, calendar view, map with pins, day-by-day schedule, budget tracking, travel docs storage, collaborative planning.

| Feature | Status | Notes |
|---------|--------|-------|
| Create trip with start/end dates | ✅ Done | POST /trips |
| List trips | ✅ Done | GET /trips |
| Create event with time, location, lat/lng, notes | ✅ Done | POST /events |
| List events (optionally filtered by trip) | ✅ Done | GET /events?trip_id=... with in-memory filtering |
| Map data endpoint | ✅ Done | GET /events/map returns lat/lng for all events with geo |
| Calendar / Map UI view | ❌ Missing | Data endpoints work; UI pending |

**Integration tests:** Trip creation, event assignment, and geo-map endpoint test passing.

### 2.7 Subsonic App

**What a user expects:** Music library browser, album art, playlist management, streaming with seeking, transcoding, podcast support, multiple client compatibility.

| Feature | Status | Notes |
|---------|--------|-------|
| Subsonic ping response | ✅ Done | GET /rest/ping returns valid Subsonic JSON |
| Get artists | ⚠️ Heuristic | Queries nodes with system title, filtered for subsonic attributes |
| Get albums | ⚠️ Heuristic | Queries nodes with subsonic:artist_id |
| Stream audio | ✅ Done | GET /rest/stream?id=X fetches from object storage with proper headers |
| Upload audio + create track node | ✅ Done | POST /upload |
| Full Subsonic client protocol coverage | ❌ Missing | `getMusicFolders`, `getIndexes`, `getAlbum`, `getCoverArt`, `search2`, etc. pending |

**Integration tests:** Ping, upload audio, and stream response tests passing.

---

## 3. Cross-Cutting Gaps

### 3.1 App UI Status

- **Journal App** has a full Logseq-inspired outliner React SPA component with block tree editing, page creation, journal day navigation, and backlink panel (`JournalApp.tsx`).
- **Grafana App** has a full React SPA dashboard editor with PromQL query editor, panel CRUD, grid controls, and inline SVG chart rendering (`App.tsx`).
- **TanStack Router** manages SPA shell navigation across all built-in panels and app views.
- **Other apps** declare `ui_components` in WASM manifests; dedicated frontend builds can be registered via module federation.

### 3.2 Storage Abstraction

- SQLite database operations are now fully encapsulated by the `StorageBackend` trait and `NodeStorage` wrapper (`crates/panorama-server/src/storage/mod.rs`). Calling server/query code does not execute raw SQL directly, preparing the platform for alternative database backends (PostgreSQL, DynamoDB, etc.).

### 3.3 CRUD Capability Matrix

| App | Create | Read | Update | Delete |
|-----|--------|------|--------|--------|
| Journal (Blocks/Pages) | ✅ | ✅ | ✅ | ✅ (Recursive) |
| Wakatime (Heartbeats/Stats) | ✅ | ✅ | — | — |
| Grafana (Dashboards) | ✅ | ✅ | ✅ | ✅ |
| Files | ✅ | ✅ | — | ✅ |
| Beli | ✅ | ✅ | — | — |
| Trips | ✅ | ✅ | — | — |
| Subsonic | ✅ | ✅ | — | — |

---

## 4. Concrete Next Steps (Prioritized)

### 4.1 Immediate (Unblocks Real Usage)

1. **Enforce SCAN at compile time.** The compiler must check whether each field predicate has a `ready` index or promoted column. Unindexed predicates without `SCAN` must be a compile error per QUERY_DESIGN.md §3.9.

2. **Refine Subsonic Schema Queries.** Upgrade `getArtists` and `getAlbums` from heuristic field checks to schema conformance filtering (`WHERE n CONFORMS TO schema("subsonic/Artist")`).

3. **Separate Required vs. Preferred Schema Tracking on Node.** Store `required_schemas: Vec<SchemaRef>` on `Node` struct and update table schema to explicitly differentiate required vs preferred schemas.

### 4.2 Next (Deepens the Platform)

4. **Prepared Statement Cache (§7.3).** Every query currently re-prepares SQL. Add cache keyed on IR shape + physical table names, invalidated on schema migration or index changes.

5. **Computed Field Execution Engine.** Wire `ComputeMode` types into write path: on node write/update, evaluate computed field expressions and store results.

6. **Build Frontend UIs for Files or Trips.** Create dedicated React components for Files (file browser with upload/download) or Trips (interactive map + itinerary timeline).

7. **Pagination for List Endpoints.** Add `limit` and `cursor` pagination parameters across all app list handlers using PQL `LIMIT`/`SKIP`.

### 4.3 Later (Polish and Advanced Features)

8. `field_stats` → index suggestion feedback loop (§6.5)
9. Windowed field_stats (hourly buckets)
10. Type-compatibility validation at compile time (§3.7)
11. Additional `StorageBackend` implementations (e.g. PostgreSQL via sqlx)
12. CRDT FieldValue variants + merge semantics + `@ops`/`@at` view selectors
13. Background task scheduler in plugin loader
14. User auth + space-level permissions
15. Query ID tracing and per-stage profiling

### 4.4 Explicitly Deferred (v0.x+)

- Aggregation, unbounded traversal, full-text search, subqueries (QUERY_DESIGN.md §9)
- End-to-end encryption, sync (DESIGN.md)
- Garbage collection (DESIGN.md)
