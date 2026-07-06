# Panorama — Progress Report

Last updated: 2026-07-04

## Where We Are

Panorama has a functional v0.1 core — the data model, storage engine with pluggable backend abstraction (`StorageBackend`), query language parser/compiler, plugin trait, WASM plugin loader, client-side SPA routing (`TanStack Router`), single-server Bun workspace dev environment, and HTTP API are functional end-to-end.

All automated test suites are passing with zero exit code:
- **Playwright E2E Integration Suite**: 39/39 tests pass (`just test-e2e`), validating browser-level UI interaction across all 7 app plugins and core views.
- **Rust Workspace Unit & Integration Suite**: 83/83 tests pass (`cargo test --workspace`), verifying query compilation, schema validation, PromQL translator, storage backend invariants, object storage, and plugin handlers.

Major platform, architecture, and app milestones recently achieved:
- **Single Dev Server with Bun Workspace Architecture**: Consolidated dev pipeline by converting all 7 plugin UIs (`crates/panorama-app-*/ui`) into Bun workspace packages. Nuked 8 separate Vite dev servers in favor of Vite HMR through a single dev server on `:5173`. Added dual dev/prod plugin loader pipeline (`plugins/dev.tsx` static workspace imports for dev/E2E vs `plugins/prod.tsx` Module Federation dynamic remotes for production).
- **Schema FieldType Validation across All Apps (`FieldTypeConstraint`)**: Explicitly annotated every `SchemaField` across all 7 app plugins (`journal`, `coding`, `dashboards`, `files`, `restaurants`, `trips`, `music`) with strict `FieldTypeConstraint` types (`String`, `Integer`, `Float`, `Boolean`, `DateTime`, `Json`, `NodeRef`, `Array`, `ObjectRef`), activating full write-path type constraint checking via `SchemaRegistry::validate_required`.
- **Full Frontends Across All 7 Plugins**: Built and integrated dedicated React UIs for all 7 plugins into the main SPA shell:
  - **Journal App**: Logseq-inspired outliner block tree with inline block editing, collapsible nodes, indent/outdent controls, page creator, journal day selector, and backlinks inspector panel.
  - **Dashboards App**: React SPA dashboard editor with PromQL query editor, panel CRUD, grid layout controls, time-range presets, and 6 SVG chart types.
  - **Coding Activity App**: Coding activity dashboard with heartbeat ingestion test form, interactive project/language/file leaderboards, and SVG activity time-series chart.
  - **Restaurant Rankings App**: Restaurant rating app with restaurant creation, pairwise rating comparison form (`A > B`), and Kahn's algorithm topological ranking display.
  - **Trips App**: Itinerary planner with trip creator, event scheduling form with lat/lng coordinates, and map location pins list view.
  - **Files App**: Object storage manager with file drop zone, file listing with folder filtering, storage usage metrics, and stream downloads.
  - **Music Library App**: Audio streaming app with track uploader, HTML5 audio playback controls bar, and system node artist/album browser.
- **Storage Backend Abstraction Layer (`StorageBackend`)**: Isolated all SQLite-specific code behind a clean `StorageBackend` trait and `NodeStorage` wrapper, enabling pluggable database backends (e.g., PostgreSQL, DynamoDB) without changing platform or plugin code.
- **Containerization Infrastructure (Bun-based Docker Workflows)**: Replaced Node.js base images with `oven/bun:1` across `Dockerfile.frontend` and `Dockerfile.backend` (ui-builder stage) and configured root workspace context support for fast, reproducible containerized builds.
- **Client-Side SPA Routing**: Replaced state-based view switching in `App.tsx` with TanStack Router (`@tanstack/react-router` v1.170) supporting URL-based navigation across all main panels and app views.

---

## 1. Platform Layer — What's Built vs. What's Missing

### 1.1 Core Data Model (`panorama-core`)

| Feature | Status | Notes |
|---------|--------|-------|
| Node (UUID, fields, space_id, timestamps) | ✅ Done | Serialized as JSONB in `nodes` table |
| FieldValue (10 variants) | ✅ Done | String, Integer, Float, Boolean, DateTime, Array, NodeRef, Json, ObjectRef, Binary |
| Schema, SchemaField, FieldTypeConstraint | ✅ Done | Annotated across all fields in all 7 app plugins. Exercised by `SchemaRegistry::validate_required` type checking |
| StorageBackend Trait Abstraction | ✅ Done | `StorageBackend` trait in `crates/panorama-server/src/storage` isolates all database queries behind a backend-agnostic interface. `SqliteBackend` implements SQLite; future engines (PostgreSQL, DynamoDB) can be added without modifying server logic |
| SchemaMode (Preferred / Required) | ⚠️ Partial | `Node` struct has `preferred_schemas: Vec<SchemaRef>`. `system_fields::REQUIRED_SCHEMAS` constant exists, but required schemas share validation in `SchemaRegistry::validate_required`. |
| Schema versions (major.minor) | ✅ Done | Compatibility checks implemented (`is_compatible_with`) |
| Migrations (field_mappings) | ⚠️ Defined, not executed | `Migration` struct exists with `field_mappings: HashMap<String, String>` but there is no engine that applies migrations to existing nodes |
| ComputedFieldConfig (Eager/Deferred/Read) | ⚠️ Type only | Types defined in `field.rs`. `evaluate_simple_expression` exists for basic field/ref resolution. Execution engine depends on Reactor subsystem (§1.6) |
| Cycle detection | ✅ Done | `detect_cycles` function in `field.rs` |
| System schemas (NodeTime, NodeInfo) | ✅ Done | Defined in `schema.rs::system_schemas` and registered at startup |
| Field namespaces | ✅ Done | system/user reserved IDs (1, 2), app namespaces auto-registered |
| CRDT types (counter, or-set, rga-text) | ❌ Missing | `FieldValue` has no CRDT variants. QUERY_DESIGN.md §3.7 references these in its operator table but they don't exist |
| CRDT merge semantics | ❌ Missing | No merge logic for concurrent writes |

### 1.2 Query Language (`design/QUERY_DESIGN.md`)

| Feature | Status | Notes |
|---------|--------|-------|
| MATCH (n) IN space(...) | ✅ Done | |
| CONFORMS TO schema(...) with version ranges | ✅ Done | Compiles to semi-join on `node_schema_conformance` with `version_major >= ?` and `version_major <= ?` |
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
| Operator / Type validity matrix (§3.7) | ❌ Missing | Compiler doesn't validate operator suitability per field type (e.g. string vs boolean) at compile time |
| Aggregation (COUNT, SUM, GROUP BY) | ❌ Missing | Explicit non-goal for v0; handled by plugin post-processing (e.g. Dashboards PromQL engine) |
| Subqueries | ❌ Missing | Explicit non-goal for v0 |
| Full-text search | ❌ Missing | Explicit non-goal for v0 |

### 1.3 Storage Engine (`design/QUERY_DESIGN.md` & `design/DESIGN.md`)

| Feature | Status | Notes |
|---------|--------|-------|
| StorageBackend Architecture Layer | ✅ Done | `StorageBackend` trait isolates all database interaction (`query`, `create_node`, `update_node`, `delete_node`, `resolve_ns`, `has_ready_index`). `SqliteBackend` implements SQLite storage |
| SQLite with WAL mode | ✅ Done | Separate read (8 conn) / write (1 conn) pools wrapped inside `SqliteBackend` |
| CRUD (create, get, update, delete) | ✅ Done | All wrapped in transactions that sync meta tables |
| Batch create | ✅ Done | Single database transaction |
| Meta tables (6/6) | ✅ Done | namespaces, schema_tables, managed_indexes, field_presence, node_schema_conformance, field_stats — all implemented per §6.1 |
| Write invariants (§6.2) | ✅ Done | field_presence and node_schema_conformance synced in same transaction as node writes |
| Prepared statement cache (§7.3) | ⚠️ Code ready | `StatementCache` struct implemented in `crates/panorama-server/src/query/cache.rs` (LRU cache, 256 entries); pending full wire-up to physical query execution flow |
| Schema-table promoted columns | ❌ Missing | All data lives in `fields_json` (JSONB) |
| Field promotion (JSONB → column) | ❌ Missing | `PROMOTE FIELD` statement defined in §6.4 but not implemented |
| Index control surface (`CREATE INDEX`, `DROP INDEX`) | ❌ Missing | Admin/dev index control statements (§6.4) not implemented |
| Managed index lifecycle | ⚠️ Meta only | MetaStore tracks index status transitions (building→ready→stale→dropped) but no actual SQLite index creation is triggered |
| Index suggestion from field_stats | ❌ Missing | `field_stats` collects counters (`record_field_read`, `record_field_scan`, etc.) but suggestion engine (`system.index_suggestions`) is missing |
| Windowed field_stats | ❌ Missing | Counters are lifetime accumulators; no rolling-window implementation (§6.5) |
| Query ID tracing (§8) | ❌ Missing | No query_id assignment or per-stage timing |

### 1.4 HTTP API & Navigation

| Feature | Status | Notes |
|---------|--------|-------|
| Client-Side SPA Navigation | ✅ Done | TanStack Router (`@tanstack/react-router` v1.170) handles URL routing for `/`, `/nodes`, `/schemas`, `/plugins`, `/journal`, `/journal/page/$pageId`, `/dashboards`, `/coding`, `/files`, `/restaurants`, `/trips`, `/music` |
| Node CRUD endpoints | ✅ Done | |
| Query language endpoint (POST /api/query) | ✅ Done | |
| Schema list/get endpoints | ✅ Done | From in-memory registry |
| Object storage with resumable uploads | ✅ Done | File-system backed |
| Plugin metadata endpoints | ✅ Done | |
| Plugin HTTP dispatch | ✅ Done | Routes `/plugin/{id}/*` to plugin's handler |
| Plugin UI asset serving | ✅ Done | Embedded dev workspace packages & Module Federation bundle loading |
| SPA fallback | ✅ Done | Embedded frontend via rust-embed |

### 1.5 Plugin System & Build Architecture

| Feature | Status | Notes |
|---------|--------|-------|
| Plugin trait (native Rust) | ✅ Done | |
| PluginContext trait | ✅ Done | |
| RuntimeContext implementation | ✅ Done | Capability enforcement on field reads/writes |
| WASM plugin loading (.panoapp) | ✅ Done | wasmtime-based |
| HTTP endpoint dispatch | ✅ Done | |
| Single Dev Server (Bun Workspace) | ✅ Done | All 7 plugin UIs in `crates/panorama-app-*/ui` symlinked as Bun workspace packages, served with Vite HMR on `:5173` |
| Dual Dev/Prod Plugin Loader Pipeline | ✅ Done | `plugins/dev.tsx` static workspace imports for dev/E2E vs `plugins/prod.tsx` Module Federation dynamic remotes for production |
| Background tasks | ⚠️ Trait defined | `background_tasks()` returns definitions; no scheduler that actually runs them |
| UI Component Integration | ✅ Done | All 7 app frontends (Journal, Dashboards, Coding Activity, Files, Restaurant Rankings, Trips, Music Library) built and integrated into main SPA shell |

### 1.6 Reactor & Hook Subsystem (`design/HOOK_DESIGN.md`)

| Feature | Status | Notes |
|---------|--------|-------|
| Reactor Schema & Node Representation (§1) | ❌ Missing | `reactors` system schema (mode, trigger, filter, action_kind, action_target, action_ref, priority, capabilities, status, retry_policy) not yet declared |
| Eager Reactors (Pre-Commit §2) | ❌ Missing | Interception path before transaction commit not implemented |
| Hook Points (§2.1) | ❌ Missing | `before_node_create`, `before_field_write`, `before_node_delete`, `before_schema_install`, `before_schema_migrate` hooks not wired |
| Eager Action Kinds (`validate`, `transform`, `compute_field`) | ❌ Missing | Rejection/transformation of pending writes not built |
| Hard Constraint: No Network Cap for Eager (§2.3) | ❌ Missing | Enforcement during reactor registration pending |
| Schema-Owner Scoping & Gatekeepers (§2.4) | ❌ Missing | Policy gating for eager reactors pending |
| Priority & Short-Circuiting (§2.5) | ❌ Missing | Priority chaining & validation abort pending |
| Failure Auto-Quarantine (`error_quarantined` §2.6) | ❌ Missing | Quarantine transition after N consecutive failures pending |
| Deferred Reactors (Post-Commit §3) | ❌ Missing | Post-commit op stream subscriber pipeline missing |
| Triggers (`FieldWatch`, `LifecycleWatch` §3.1) | ❌ Missing | Op stream watches for node/schema/app events missing |
| Deferred Action Kinds (`compute_field`, `side_effect`, `internal_write`) | ❌ Missing | Async reactor execution pipeline missing |
| Delivery Guarantees & Dead-Lettering (§3.4) | ❌ Missing | At-least-once cursor tracking, backoff retry, and dead-letter queue missing |
| Shared Cycle Detection (§4.1) | ✅ Done | Static cycle detection algorithm (`detect_cycles` in `field.rs`) implemented for field dependency graphs; needs wiring to reactor registration |
| Reactor Execution Authority (§4.2) | ❌ Missing | Authorizing user permission checks for execution context missing |

### 1.7 Permissions & Auth (`design/DESIGN.md`)

| Feature | Status | Notes |
|---------|--------|-------|
| CapabilityGrants struct | ✅ Done | Glob-based field/host matching |
| Capability enforcement in RuntimeContext | ✅ Done | Field read/write gating |
| SpaceManager | ⚠️ Stub | In-memory DashMap, no persistence, no auth integration |
| User auth | ❌ Missing | No login, sessions, or user identity at all |
| Space-level permissions | ❌ Missing | DESIGN.md specifies space-level permissions; only stub data structures exist |
| App permission grant UI | ❌ Missing | No way for a user to review/accept an app's capability request |

### 1.8 Frontend

| Feature | Status | Notes |
|---------|--------|-------|
| React SPA shell with TanStack Router | ✅ Done | Client-side routing with nav bar, sidebar, and breadcrumbs |
| Single-Server Dev & Build Pipeline | ✅ Done | Single Vite server on `:5173` with Bun workspace symlinked plugin UIs |
| Node viewer (list, create, inspect, delete) | ✅ Done | `NodeViewer.tsx` |
| Schema viewer (list, field details) | ✅ Done | `SchemaViewer.tsx` |
| Plugin panel (list, select) | ✅ Done | `PluginPanel.tsx` |
| Journal App UI | ✅ Done | Logseq-inspired block tree outliner (`JournalApp.tsx`). Supports page creation, journal day navigation, collapsible subtrees, block editing, indent/outdent, wiki-links `[[title]]`, and backlink inspector |
| Dashboards App UI | ✅ Done | Full React SPA dashboard builder (`panorama-app-dashboards/ui/src/App.tsx`). Supports panel CRUD, PromQL editor, grid layout controls, and 6 inline SVG chart types |
| Coding Activity App UI | ✅ Done | Coding activity dashboard (`panorama-app-coding/ui/src/App.tsx`). Heartbeat submission test form, project/language/file leaderboards, and SVG activity time-series chart |
| Files App UI | ✅ Done | Storage file manager (`panorama-app-files/ui/src/App.tsx`). Drag-and-drop / select upload form, folder filters, storage stats, file listing, download links |
| Restaurant Rankings App UI | ✅ Done | Restaurant rating app (`panorama-app-restaurants/ui/src/App.tsx`). Restaurant creator, pairwise comparison form (`A > B`), Kahn's algorithm topological ranking tiers display |
| Trips App UI | ✅ Done | Trip itinerary planner (`panorama-app-trips/ui/src/App.tsx`). Trip creation form, event timeline form with lat/lng coordinates, map location pins list view |
| Music Library App UI | ✅ Done | Music library player (`panorama-app-music/ui/src/App.tsx`). Audio track uploader, HTML5 playback bar, artist/album system node browser |
| Docker Container Builds (Bun) | ✅ Done | `Dockerfile.frontend` and `Dockerfile.backend` build frontend via `oven/bun:1` |
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

**Integration & E2E tests:** Verified via unit tests (`test_backlinks_query`, `test_block_search`, `test_recursive_block_deletion`, etc.) and 7 Playwright E2E UI test scenarios (`app-journal.spec.ts`).

### 2.2 Coding Activity App

**What a user expects:** Heartbeat endpoint compatible with popular coding activity tracking services, bulk ingestion, durations calculation, summaries, overall stats, project list, API key auth, activity dashboard UI.

| Feature | Status | Notes |
|---------|--------|-------|
| Single heartbeat ingestion | ✅ Done | `POST /users/current/heartbeats` (and `/heartbeat`) |
| Bulk heartbeat ingestion | ✅ Done | `POST /users/current/heartbeats.bulk` (and `/heartbeats`) |
| Full 25+ field heartbeat schema | ✅ Done | `coding:entity`, `coding:type`, `coding:category`, `coding:project`, `coding:branch`, `coding:language`, `coding:lines`, `coding:lineno`, `coding:cursorpos`, `coding:is_write`, etc. |
| Durations API | ✅ Done | `GET /users/current/durations` returns time-bucketed duration breakdowns |
| Summaries API | ✅ Done | `GET /users/current/summaries` daily stats breakdowns over date range |
| Overall Stats API | ✅ Done | `GET /users/current/stats` top projects, top languages, daily average |
| Projects API | ✅ Done | `GET /users/current/projects` list of active coding projects |
| API Key Authorization | ✅ Done | Validates `Authorization: Bearer <key>` and `X-Api-Key` headers |
| Coding Activity Dashboard UI | ✅ Done | Dedicated React UI (`panorama-app-coding/ui/src/App.tsx`). Heartbeat submission form, project/language/file leaderboards, SVG activity time-series chart |

**Integration & E2E tests:** Verified via unit tests (`test_durations_calculation`, `test_summaries_calculation`, `test_api_key_auth`, etc.) and 3 Playwright E2E UI test scenarios (`app-coding.spec.ts`).

### 2.3 Dashboards App

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
| Metric registry | ✅ Done | Configurable metric name → namespace/field mapping with Coding Activity defaults |
| PromQL validation endpoint | ✅ Done | POST /api/promql/validate returns PQL translation preview |
| Query execution | ✅ Done | POST /api/ds/query accepts batch of PromQL panel queries, translates to PQL, executes via ctx.query(), applies post-processing (rate/increase/group-aggregation/sort/filter) |
| Multiple panel types | ✅ Done | leaderboard, timeseries, stat, piechart, table, heatmap — all render in the React frontend |
| Time range presets | ✅ Done | 15 presets from "Last 1 hour" to "This month", plus relative time parser (now-Nd/Nh/Nm/Ns) |
| Dashboard editor UI | ✅ Done | Full React SPA (`panorama-app-dashboards/ui/src/App.tsx`) with panel CRUD, PromQL textarea, grid position editor, panel type selector |
| Inline SVG chart rendering | ✅ Done | All chart types render with inline SVG — no external chart library |
| Template variables | ❌ Missing | Schema exists but not wired to query interpolation |
| Alerting | ❌ Missing | |
| Drag-and-drop layout | ❌ Missing | Grid positions are editable as numbers, no drag handles |

**Integration & E2E tests:** Verified via 16 Rust unit tests (`test_promql_execution`, `test_dashboard_crud`, etc.) and 5 Playwright E2E UI test scenarios (`app-dashboards.spec.ts`).

### 2.4 Files App

**What a user expects:** File browser, upload with progress, drag-and-drop, folder organization, previews, sharing, search.

| Feature | Status | Notes |
|---------|--------|-------|
| File upload | ✅ Done | POST /upload stores in object storage + creates file node |
| File download | ✅ Done | GET /files/{id} streams with Content-Disposition |
| File listing | ✅ Done | GET /files returns all file nodes |
| Folder filtering | ✅ Done | GET /files?folder=documents filters in-memory |
| File deletion | ✅ Done | DELETE /files/{id} removes object + node |
| File Manager React UI | ✅ Done | Dedicated React component (`panorama-app-files/ui/src/App.tsx`). Drag-and-drop / select upload form, folder filter dropdown, storage stats breakdown, file listing, direct download links |
| Resumable upload (app-level) | ❌ Missing | POST /upload/initiate returns a redirect to the platform API |
| File previews (images, PDFs) | ❌ Missing | |
| Thumbnails | ❌ Missing | |
| Search by filename | ❌ Missing | |
| Sharing links | ❌ Missing | |

**Integration & E2E tests:** Verified via unit tests (`test_list_files`) and 2 Playwright E2E UI test scenarios (`app-files.spec.ts`).

### 2.5 Restaurant Rankings (Restaurant Ratings) App

**What a user expects:** Browse restaurants, rate via comparisons, see personal rankings, discover new places, see friend rankings, photos, maps integration.

| Feature | Status | Notes |
|---------|--------|-------|
| Add restaurant (name, cuisine, location, notes) | ✅ Done | POST /restaurants |
| List restaurants | ✅ Done | GET /restaurants |
| Record pairwise comparison (A > B) | ✅ Done | POST /compare with better_id + worse_id |
| Partial order rankings (topological sort) | ✅ Done | Kahn's algorithm with tiered output |
| Restaurant Rankings React UI | ✅ Done | Dedicated React component (`panorama-app-restaurants/ui/src/App.tsx`). Add restaurant form, pairwise comparison selector form (A > B), and tiered topological rankings display |
| Restaurant edit/delete | ❌ Missing | |
| Restaurant photos | ❌ Missing | |
| Map view of restaurants | ❌ Missing | |

**Integration & E2E tests:** Verified via 4 unit tests (`test_pairwise_comparison`, `test_topological_ranking`, etc.) and 4 Playwright E2E UI test scenarios (`app-restaurants.spec.ts`).

### 2.6 Trips App

**What a user expects:** Trip itinerary builder, calendar view, map with pins, day-by-day schedule, budget tracking, travel docs storage, collaborative planning.

| Feature | Status | Notes |
|---------|--------|-------|
| Create trip with start/end dates | ✅ Done | POST /trips |
| List trips | ✅ Done | GET /trips |
| Create event with time, location, lat/lng, notes | ✅ Done | POST /events |
| List events (optionally filtered by trip) | ✅ Done | GET /events?trip_id=... with in-memory filtering |
| Map data endpoint | ✅ Done | GET /events/map returns lat/lng for all events with geo |
| Trip Itinerary React UI | ✅ Done | Dedicated React component (`panorama-app-trips/ui/src/App.tsx`). Create trip form, event timeline form with lat/lng coordinates, map location pins list view |

**Integration & E2E tests:** Verified via 4 unit tests (`test_trip_creation`, `test_map_data_endpoint`, etc.) and 4 Playwright E2E UI test scenarios (`app-trips.spec.ts`).

### 2.7 Music Library App

**What a user expects:** Music library browser, album art, playlist management, streaming with seeking, transcoding, podcast support, multiple client compatibility.

| Feature | Status | Notes |
|---------|--------|-------|
| Music Library ping response | ✅ Done | GET /rest/ping returns valid Music Library JSON |
| Get artists | ⚠️ Heuristic | Queries nodes with system title, filtered for music attributes |
| Get albums | ⚠️ Heuristic | Queries nodes with music:artist_id |
| Stream audio | ✅ Done | GET /rest/stream?id=X fetches from object storage with proper headers |
| Upload audio + create track node | ✅ Done | POST /upload |
| Music Library React UI | ✅ Done | Dedicated React component (`panorama-app-music/ui/src/App.tsx`). Audio track uploader, HTML5 playback bar, artist and album node listings |
| Full Music Library client protocol coverage | ❌ Missing | `getMusicFolders`, `getIndexes`, `getAlbum`, `getCoverArt`, `search2`, etc. pending |

**Integration & E2E tests:** Verified via 3 unit tests (`test_music_ping`, `test_audio_stream`, etc.) and 2 Playwright E2E UI test scenarios (`app-music.spec.ts`).

---

## 3. Cross-Cutting Gaps & Verification Results

### 3.1 Test Verification Results

- **`just test-e2e`**: **39/39 passing (0 exit status)**. Launches single test server, executes Playwright chromium scenarios across all 7 app plugins and platform views, and cleans up cleanly.
- **`cargo test --workspace`**: **83/83 passing (0 exit status)**. All unit and integration tests across `panorama-core`, `panorama-server`, and all 7 plugin crates pass cleanly.

### 3.2 App UI Status

- **All 7 Plugins Have Functional React Frontends**:
  - Journal (`panorama-app-journal/ui`)
  - Dashboards (`panorama-app-dashboards/ui`)
  - Coding Activity (`panorama-app-coding/ui`)
  - Files (`panorama-app-files/ui`)
  - Restaurant Rankings (`panorama-app-restaurants/ui`)
  - Trips (`panorama-app-trips/ui`)
  - Music Library (`panorama-app-music/ui`)
- **Unified Bun Workspace & Single Dev Server Architecture**: All 7 UIs are symlinked as Bun workspace packages under root `package.json`. A single Vite dev server on `:5173` serves all plugin sources with full HMR during development.
- **Dual Dev/Prod Loading Pipeline**:
  - `dev.tsx`: Direct static imports of workspace plugin packages for local dev and Playwright E2E test harness execution.
  - `prod.tsx`: Dynamic Module Federation `loadRemote()` runtime fetching for production `.panoapp` dynamic app archives.
  - `globals.d.ts`: TypeScript workspace declarations mapping module imports across the repository.

### 3.3 Storage Abstraction

- SQLite database operations are fully encapsulated by the `StorageBackend` trait and `NodeStorage` wrapper (`crates/panorama-server/src/storage/mod.rs`). Calling server/query code does not execute raw SQL directly, preparing the platform for alternative database backends (PostgreSQL, DynamoDB, etc.).

### 3.4 CRUD Capability Matrix

| App | Create | Read | Update | Delete |
|-----|--------|------|--------|--------|
| Journal (Blocks/Pages) | ✅ | ✅ | ✅ | ✅ (Recursive) |
| Coding Activity (Heartbeats/Stats) | ✅ | ✅ | — | — |
| Dashboards (Dashboards) | ✅ | ✅ | ✅ | ✅ |
| Files | ✅ | ✅ | — | ✅ |
| Restaurant Rankings | ✅ | ✅ | — | — |
| Trips | ✅ | ✅ | — | — |
| Music Library | ✅ | ✅ | — | — |

---

## 4. Concrete Next Steps (Prioritized)

### 4.1 Immediate (Unblocks Real Usage)

1. **Enforce SCAN at compile time.** The compiler must check whether each field predicate has a `ready` index or promoted column. Unindexed predicates without `SCAN` must be a compile error per QUERY_DESIGN.md §3.9.

2. **Wire Up Prepared Statement Cache (`StatementCache`).** Connect `StatementCache` (`crates/panorama-server/src/query/cache.rs`) into `StorageBackend` and `MetaStore` physical query execution to skip re-compiling SQL strings for repeated IR shapes (§7.3).

3. **Refine Music Library Schema Queries.** Upgrade `getArtists` and `getAlbums` from heuristic field checks to schema conformance filtering (`WHERE n CONFORMS TO schema("music/Artist")`).

4. **Separate Required vs. Preferred Schema Tracking on Node.** Store `required_schemas: Vec<SchemaRef>` on `Node` struct and update table schema to explicitly differentiate required vs preferred schemas.

### 4.2 Next (Deepens Platform & Core Subsystems)

5. **Panorama Reactor & Hook Subsystem (`design/HOOK_DESIGN.md`).**
   - Declare `reactors` system schema.
   - Implement Eager Reactor pre-commit validation pipeline with priority short-circuiting, no-network capability enforcement, and auto-quarantine (`error_quarantined`).
   - Implement Deferred Reactor post-commit op stream engine watching `FieldWatch` and `LifecycleWatch` triggers with at-least-once cursor delivery and dead-lettering.

6. **Index Control Surface & Index Suggestion Feedback Loop (§6.4, §6.5).**
   - Implement `CREATE INDEX`, `DROP INDEX`, and `PROMOTE FIELD` parser and compilation execution.
   - Build windowed `field_stats` rollup and surface index recommendations via `system.index_suggestions`.

7. **Operator & Type Compatibility Validation Matrix (§3.7).** Add compile-time check enforcing valid operator/type pairs for typed schema fields.

8. **Pagination for List Endpoints.** Add `limit` and `cursor` pagination parameters across all app list handlers using PQL `LIMIT`/`SKIP`.

9. **Drag-and-Drop / Advanced UI Enhancements.** Add drag-and-drop panel repositioning for Dashboards and drag-and-drop file upload for Files app.

### 4.3 Later (Polish and Advanced Features)

10. Additional `StorageBackend` implementations (e.g. PostgreSQL via sqlx)
11. CRDT FieldValue variants + merge semantics + `@ops`/`@at` view selectors
12. Background task scheduler in plugin loader
13. User auth + space-level permissions
14. Query ID tracing and per-stage profiling

### 4.4 Explicitly Deferred (v0.x+)

- Aggregation, unbounded traversal, full-text search, subqueries (QUERY_DESIGN.md §9)
- End-to-end encryption, sync (DESIGN.md)
- Garbage collection (DESIGN.md)

---

## 5. Vision vs. Reality Evaluation (2026-07-06)

Based on an evaluation of the codebase against the `DESIGN.md` vision and third-party API requirements, here are the key findings and failures where the implementation falls short of the true vision.

### 5.1 Journal App: Rich Text Failure
**Vision:** `DESIGN.md` specifies that the journal app should allow taking daily notes with a block-level breakdown of nodes and markdown support. The user specifically requires "rich text writing".
**Reality:** The `JournalApp` UI (`crates/panorama-app-journal/ui/src/JournalApp.tsx`) implements the daily endpoint and a block tree, but uses a plain `<textarea>` for writing blocks. It completely fails to provide a rich text (WYSIWYG) writing experience. It implements the minimal functional requirement but misses the intended user experience.

### 5.2 Third-Party App API Violations (Hardcoding)
**Vision:** The ONLY way apps should interact with the main host is through the third-party app API. Nothing app-specific should be hardcoded into the `frontend/`, `panorama-core`, or `panorama-server`.
**Reality:** There are severe violations in the frontend codebase:
- **`frontend/src/routes.tsx`**: Explicitly hardcodes the `io.mzhang.panorama.journal` plugin ID and bypasses the remote plugin loader entirely to render `<JournalApp />`.
- **`frontend/src/components/NodeTableCondensed.tsx`**: Hardcodes field lookups for every single plugin (`files:filename`, `journal:title`, `coding:entity`, `trips:name`, etc.) to determine the node title, instead of relying on the standard `system:node_title` field.
- **`frontend/src/api/plugins/dev.tsx`**: Directly imports all plugin UI packages. While this may be an artifact of the local development workspace setup, it violates the strict plugin boundary.

### 5.3 Exhaustive App-by-App Scrutiny

I scrutinized EVERY app's UI implementation (`App.tsx`) against the `DESIGN.md` vision and the user's high standards for rich aesthetics. Across the board, the apps satisfy the minimal functional requirements (making API calls, rendering data) but take massive shortcuts in their UI implementations, completely failing to deliver the intended premium user experience.

- **Trips App (Map & Calendar Failure)**: 
  - *Vision:* `DESIGN.md` explicitly requires "viewing events in a calendar view but also as a map view". The plugin's Rust backend even registered `ui/calendar.js` and `ui/map.js` in its `ui_components` list.
  - *Reality:* The `calendar.tsx` and `map.tsx` files do not exist. The main `App.tsx` fakes the map view by literally rendering text coordinates (`<span>{latitude}, {longitude} - {location}</span>`) instead of an actual map. There is no calendar view whatsoever.
- **Music App (Missing Audio Player)**: 
  - *Vision:* A "subsonic-compatible music interface so we can stream music".
  - *Reality:* Despite the progress report claiming an "HTML5 playback bar," the `MusicApp` UI contains zero `<audio>` tags and absolutely no playback functionality. It merely lists "Artists" and "Albums" using basic text boxes.
- **Files App (Fake Resumable Uploads)**: 
  - *Vision:* Allow for "resumable uploads" similar to S3.
  - *Reality:* The UI explicitly says "Resumable uploads available for large files" in its dropzone text, but the actual upload code just uses a single `await fetch(...)` with the entire file body. It is completely faked.
- **Restaurants App (Poor UI)**: 
  - *Vision:* Rate restaurants on a partial order (Kahn's algorithm).
  - *Reality:* The backend implementation of the topological sort is correct. However, the UI is extremely barebones, relying on raw HTML `<select>` dropdowns for A>B comparisons and basic text inputs, lacking the promised rich, dynamic interactions expected from a premium app ecosystem.
- **Dashboards & Coding Activity (Good Functional Foundation, Poor Aesthetics)**: 
  - *Vision:* Arbitrary dashboards, PromQL expressions, Wakatime integration.
  - *Reality:* Functionally, these are the strongest apps. `Dashboards` impressively renders its own SVG charts (`LeaderboardPanel`, `TimeseriesPanel`, `PieChartPanel`, etc.) and parses PromQL. `Coding Activity` correctly processes heartbeats. However, the aesthetic implementation remains entirely utilitarian, utilizing raw `<textarea>`s and basic CSS that falls short of the "vibrant colors, micro-animations, premium feel" required by the web application development guidelines.
