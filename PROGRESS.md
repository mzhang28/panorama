# Panorama — Progress Report

Last updated: 2026-07-04

## Where We Are

Panorama has a working v0.0 core — the data model, SQLite storage, query language parser/compiler, plugin trait, and HTTP API are functional end-to-end. However, the apps are thin stubs (each implements 1-2 integration test scenarios), the platform has several incomplete features that block real-world use, and the frontend is skeletal. This document gives an honest, itemized breakdown of what exists, what's missing, and what to do next.

---

## 1. Platform Layer — What's Built vs. What's Missing

### 1.1 Core Data Model (`panorama-core`)

| Feature | Status | Notes |
|---------|--------|-------|
| Node (UUID, fields, space_id, timestamps) | ✅ Done | Serialized as JSONB in `nodes` table |
| FieldValue (10 variants) | ✅ Done | String, Integer, Float, Boolean, DateTime, Array, NodeRef, Json, ObjectRef, Binary |
| Schema, SchemaField, FieldTypeConstraint | ✅ Done | |
| SchemaMode (Preferred / Required) | ⚠️ Partial | `Node` struct only has `preferred_schemas: Vec<SchemaRef>`. Required schemas have no separate storage — they live in the same field, and `SchemaRegistry::validate_required` checks the `SchemaMode` on the schema object. This works but conflates two concepts that DESIGN.md says are distinct. |
| Schema versions (major.minor) | ✅ Done | Compatibility checks implemented |
| Migrations (field_mappings) | ⚠️ Defined, not executed | `Migration` struct exists with `field_mappings: HashMap<String, String>` but there is no engine that applies migrations to existing nodes |
| ComputedFieldConfig (Eager/Deferred/Read) | ⚠️ Type only | Types defined. `evaluate_simple_expression` exists for basic field/ref resolution. No execution engine — nothing actually runs computed field expressions on write or read |
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
| SCAN(...) wrapper | ⚠️ Parser only | Parser accepts `SCAN(...)` but compiler does NOT enforce it — unindexed predicates without SCAN are not rejected. The "expensive things should look expensive" rule is not enforced |
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
| Type-compatibility validation at compile time | ❌ Missing | §3.7 — compiler doesn't check that `n.title < 5` where title is a string. Always falls back to runtime json_extract |
| Aggregation (COUNT, SUM, GROUP BY) | ❌ Missing | §9 — explicit non-goal for v0 |
| Subqueries | ❌ Missing | §9 — explicit non-goal for v0 |
| Full-text search | ❌ Missing | §9 — explicit non-goal for v0 |

### 1.3 Storage Engine

| Feature | Status | Notes |
|---------|--------|-------|
| SQLite with WAL mode | ✅ Done | Separate read (8 conn) / write (1 conn) pools |
| CRUD (create, get, update, delete) | ✅ Done | All wrapped in transactions that sync meta tables |
| Batch create | ✅ Done | Single SQLite transaction |
| meta tables (6/6) | ✅ Done | namespaces, schema_tables, managed_indexes, field_presence, node_schema_conformance, field_stats — all implemented per §6.1 |
| Write invariants (§6.2) | ✅ Done | field_presence and node_schema_conformance synced in same txn as node writes |
| Prepared statement cache (§7.3) | ❌ Missing | Every query re-prepares the SQL |
| Schema-table promoted columns | ❌ Missing | All data lives in `fields_json` (JSONB). The `storage_mode` enum exists in meta tables but no promotion path exists |
| Field promotion (JSONB → column) | ❌ Missing | `PROMOTE FIELD` statement defined in §6.4 but not implemented |
| Managed index lifecycle | ⚠️ Meta only | MetaStore tracks index status transitions (building→ready→stale→dropped) but no actual SQLite index creation is triggered |
| Index suggestion from field_stats | ❌ Missing | `field_stats` collects counters but nothing reads them to suggest indexes |
| Windowed field_stats | ❌ Missing | Counters are lifetime accumulators; no rolling-window implementation (§6.5) |
| Query ID tracing (§8) | ❌ Missing | No query_id assignment or per-stage timing |

### 1.4 HTTP API

| Feature | Status | Notes |
|---------|--------|-------|
| Node CRUD endpoints | ✅ Done | |
| Query language endpoint (POST /api/query) | ✅ Done | |
| Schema list/get endpoints | ✅ Done | From in-memory registry |
| Object storage with resumable uploads | ✅ Done | File-system backed |
| Plugin metadata endpoints | ✅ Done | |
| Plugin HTTP dispatch | ✅ Done | Routes /plugin/{id}/* to plugin's handler |
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
| UI component loading | ⚠️ Module Federation stubs | Frontend has the plugin-loader but no actual remote components |

### 1.6 Permissions & Auth

| Feature | Status | Notes |
|---------|--------|-------|
| CapabilityGrants struct | ✅ Done | Glob-based field/host matching |
| Capability enforcement in RuntimeContext | ✅ Done | Field read/write gating |
| SpaceManager | ⚠️ Stub | In-memory DashMap, no persistence, no auth integration |
| User auth | ❌ Missing | No login, sessions, or user identity at all |
| Space-level permissions | ❌ Missing | DESIGN.md specifies space-level permissions; only the stub data structures exist |
| App permission grant UI | ❌ Missing | No way for a user to review/accept an app's capability request |

### 1.7 Frontend

| Feature | Status | Notes |
|---------|--------|-------|
| React SPA shell with sidebar nav | ✅ Done | |
| Node viewer (list, create, inspect, delete) | ✅ Done | `NodeViewer.tsx` |
| Schema viewer (list, field details) | ✅ Done | `SchemaViewer.tsx` |
| Plugin panel (list, select) | ✅ Done | `PluginPanel.tsx` |
| Journal app UI | ⚠️ Partial | Implemented as a hardcoded frontend component (not loaded via plugin UI). Has entry creation, mood display, markdown rendering. No editing, no paragraph view, no date navigation |
| Other app UIs (Wakatime, Files, Grafana, Beli, Trips, Subsonic) | ❌ Missing | All declare `ui_components` pointing to `.js` bundles that don't exist. The frontend dynamically attempts Module Federation imports that will 404 |
| Production build embedding | ✅ Done | Via rust-embed |

---

## 2. App-by-App Feature Breakdown

Each app below is assessed against what a real user would expect from such an app. The integration tests are the ground truth for what actually works.

### 2.1 Journal App

**What a user expects:** Daily journal with markdown entries, calendar date navigation, rich text editing, mood tracking, paragraph-level referencing, search, import/export.

| Feature | Status | Notes |
|---------|--------|-------|
| Create entry with title, content, mood, time | ✅ Done | POST /entries creates entry + paragraph child nodes |
| Paragraph decomposition | ✅ Done | Splits markdown on `\n\n` into child Paragraph nodes with backrefs |
| List entries (timeline) | ✅ Done | GET /entries returns entries ordered by time DESC |
| Mood filtering | ✅ Done | GET /entries?mood=happy — filters in query language |
| Date range filtering | ✅ Done | GET /entries?from=...&to=... — filters in query language |
| Get single entry | ✅ Done | GET /entries/{id} |
| Get paragraph children | ✅ Done | GET /entries/{id}/paragraphs — follows paragraph_refs NodeRefs |
| Update entry (title, content, mood) | ✅ Done | PUT /entries/{id} — rebuilds paragraphs on content change |
| Soft delete / undelete | ⚠️ Partial | DELETE sets `journal:deleted=true`. PUT blocks updates to deleted entries. No undelete endpoint. No filtering of deleted entries from list by default |
| Rich markdown rendering | ✅ Done | Frontend renders markdown via `marked` library (headings, lists, code blocks) |
| Entry editing UI | ❌ Missing | Frontend only has create form, no edit form for existing entries |
| Paragraph view in UI | ❌ Missing | Frontend doesn't show paragraph children or link to them |
| Calendar date navigation | ❌ Missing | No date picker, no "jump to date", no monthly/weekly view |
| Search | ❌ Missing | No full-text search across entries |
| Tagging / categories | ❌ Missing | Only mood field exists; no custom tags |
| Image upload in entries | ❌ Missing | |
| Export (PDF, plain text) | ❌ Missing | |

**Integration tests:** 2 tests (create+list, field verification after create). The create test exercises the full paragraph decomposition path.

### 2.2 Wakatime App

**What a user expects:** Wakatime-compatible endpoint, dashboard with coding stats, project breakdowns, language stats, time-range queries, leaderboards.

| Feature | Status | Notes |
|---------|--------|-------|
| Single heartbeat ingestion | ✅ Done | POST /heartbeat |
| Bulk heartbeat ingestion | ✅ Done | POST /heartbeats (same handler, detects array) |
| Heartbeat schema (entity, project, language, duration, category) | ✅ Done | |
| Time-based queries | ❌ Missing | No endpoint to query heartbeats by date range (Grafana app handles this separately) |
| Project breakdown | ❌ Missing | No dedicated stats endpoint (Grafana app queries raw nodes) |
| Language stats | ❌ Missing | |
| Leaderboard | ❌ Missing | (Grafana app implements this in-memory over raw nodes) |
| Wakatime API compatibility beyond heartbeat | ❌ Missing | No /users/current, /summaries, /stats, etc. Wakatime clients expect more than just heartbeat ingestion |
| API key auth | ❌ Missing | Real Wakatime clients send API keys; no auth |
| Dashboard UI | ❌ Missing | Declares a UI component that doesn't exist |

**Integration tests:** 2 tests (single heartbeat, bulk heartbeat). Tests verify the response `{"status": "ok", "created": N}`.

### 2.3 Grafana App

**What a user expects:** Dashboard builder with panels, time-series queries, multiple chart types, drag-and-drop layout, PromQL expressions, template variables, alerting.

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

**Integration tests:** 3 tests (PromQL count aggregation, PromQL leaderboard, save+list dashboards). The query tests create wakatime nodes first, then query them through the Grafana plugin via PromQL expressions (`count by (project) (wakatime_duration)`, `sum by (project) (wakatime_duration)`).

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
| Sorting (by size, date, name) | ❌ Missing | |
| Sharing links | ❌ Missing | |
| Folder tree navigation | ❌ Missing | Only flat `folder` string field |

**Integration tests:** 1 test (upload + download + list + delete). This is the most complete single test — it exercises the full lifecycle.

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
| Cuisine/location filtering | ❌ Missing | The "list restaurants" query doesn't support filtering |
| Friend rankings / social | ❌ Missing | |
| OSM integration | ❌ Missing | DESIGN.md mentions pulling public info from OSM |
| Rating history / trends | ❌ Missing | |
| Context-filtered rankings (e.g., "best ramen") | ❌ Missing | Context is stored on comparisons but not filterable |

**Integration tests:** 1 test (add 3 restaurants, 2 pairwise comparisons, verify 3-tier ranking). The partial order implementation is genuinely interesting and working.

### 2.6 Trips App

**What a user expects:** Trip itinerary builder, calendar view, map with pins, day-by-day schedule, budget tracking, travel docs storage, collaborative planning.

| Feature | Status | Notes |
|---------|--------|-------|
| Create trip with start/end dates | ✅ Done | POST /trips |
| List trips | ✅ Done | GET /trips |
| Create event with time, location, lat/lng, notes | ✅ Done | POST /events |
| List events (optionally filtered by trip) | ✅ Done | GET /events?trip_id=... with in-memory filtering |
| Map data endpoint | ✅ Done | GET /events/map returns lat/lng for all events with geo |
| Calendar view | ❌ Missing | Declares a UI component that doesn't exist |
| Map view with pins | ❌ Missing | Declares a UI component that doesn't exist; the data endpoint works though |
| Trip edit/delete | ❌ Missing | |
| Event edit/delete | ❌ Missing | |
| Day-by-day itinerary | ❌ Missing | Events aren't grouped by day |
| Budget tracking | ❌ Missing | |
| Travel docs (flight confirmations, hotel bookings) | ❌ Missing | |

**Integration tests:** 2 tests (create trip + event + list, map view with 2 events). Good basic CRUD coverage.

### 2.7 Subsonic App

**What a user expects:** Music library browser, album art, playlist management, streaming with seeking, transcoding, podcast support, multiple client compatibility.

| Feature | Status | Notes |
|---------|--------|-------|
| Subsonic ping response | ✅ Done | GET /rest/ping returns valid Subsonic JSON |
| Get artists | ⚠️ Broken | Fetches ALL nodes, then filters to nodes without artist_id/album_id/audio_ref fields. This is a heuristic that would misidentify non-artist nodes as artists |
| Get albums | ⚠️ Broken | Fetches nodes with `artist_id` field. Same heuristic problem — any node from any app with that field would show up |
| Stream audio | ✅ Done | GET /rest/stream?id=X fetches from object storage with proper headers |
| Upload audio + create track node | ✅ Done | POST /upload |
| Album art | ❌ Missing | Schema has `cover_art_ref` but no endpoint to serve it |
| Track listing (getAlbum) | ❌ Missing | No endpoint to list tracks in an album |
| Playlists | ❌ Missing | |
| Search (search2/search3) | ❌ Missing | |
| Transcoding | ❌ Missing | |
| Scrobbling | ❌ Missing | |
| Real Subsonic client compatibility | ❌ Missing | Only ping, getArtists, getAlbumList2, stream are stubbed. A real Subsonic client also needs getMusicFolders, getIndexes, getMusicDirectory, getAlbum, getCoverArt, search2, getPlaylists, getStarred, etc. Even the response format for getArtists is wrong — the `index` structure in Subsonic uses alphabetical groups |

**Integration tests:** 2 tests (ping, upload + stream). The upload + stream test is useful — it exercises the full object storage path through a plugin.

---

## 3. Cross-Cutting Gaps

These are issues that affect every app or the platform as a whole.

### 3.1 App UI status

- **Journal** is the only app besides Grafana with a frontend component — a single `JournalApp.tsx` with create form and timeline. No editing, no paragraph view, no calendar.
- **Grafana** has a full React SPA dashboard editor with PromQL query editor, panel CRUD, grid position controls, and inline SVG chart rendering (leaderboard, timeseries, stat, piechart, table, heatmap). Loaded via Module Federation.
- **All other apps** declare `ui_components` pointing to `.js` bundle files that don't exist on disk. The frontend attempts Module Federation imports (`registerPluginRemote` / `loadPluginComponent`) that will 404.
- The UI component model (Module Federation remotes) requires separate build tooling per app. The build script `build-panoapp.sh` packages WASM but not UI bundles.

### 3.2 All apps are hardcoded to `space("default")`

Every app's query hardcodes `IN space("default")`. There's no mechanism for a user to select which space to operate in. The SpaceManager stub exists but isn't wired to the API layer.

### 3.3 No app has update or delete for its own entities (except Journal and Files)

| App | Create | Read | Update | Delete |
|-----|--------|------|--------|--------|
| Journal | ✅ | ✅ | ✅ | ✅ (soft) |
| Wakatime | ✅ | — | — | — |
| Grafana (dashboards) | ✅ | ✅ | ✅ | ✅ |
| Files | ✅ | ✅ | — | ✅ |
| Beli | ✅ | ✅ | — | — |
| Trips | ✅ | ✅ | — | — |
| Subsonic | ✅ | ✅ | — | — |

### 3.4 No pagination in app list endpoints

Every app's list endpoint queries all nodes and returns everything at once. The Journal app hardcodes `LIMIT 100`. No cursor or offset-based pagination.

### 3.5 Background tasks defined but never run

Several apps define `background_tasks()` (e.g., for syncing with external services) but the plugin loader has no scheduler to actually invoke `run_background_task`. The trait method has a default implementation returning `NOT_FOUND`.

### 3.6 No error recovery or retry logic

If an app's HTTP handler fails mid-operation (e.g., after creating a node but before creating its children), there's no rollback. The Journal app creates the entry node first, then creates paragraphs, then updates the entry with paragraph refs — if paragraph creation fails, the entry is left without refs. There's no cleanup.

### 3.7 Cross-app data queries are still convention-based

The Grafana app now uses a PromQL metric registry that maps metric names (e.g., `wakatime_duration`) to namespace/field pairs (e.g., `wakatime:duration`). This is cleaner than the old hardcoded field names, but still convention-based — any node with the right fields matches. There's no schema-based dispatch ("query all nodes conforming to schema X").

---

## 4. Concrete Next Steps (Prioritized)

### 4.1 Immediate (unblocks real usage)

1. **Separate required vs. preferred schema tracking on Node.** Add `required_schemas: Vec<SchemaRef>` field, update the nodes table, thread through all CRUD ops and meta sync. Currently only `preferred_schemas` exists; required schemas are conflated with preferred ones.

2. **Enforce SCAN at compile time.** The compiler must check whether each field predicate has a `ready` index or promoted column. Unindexed predicates without `SCAN` must be a compile error. This is the primary query safety mechanism from QUERY_DESIGN.md §3.9 and it currently does nothing.

3. **Make the Journal app a real app.** It's the flagship example. Fill in the missing features from §2.1: edit UI, paragraph view, calendar date navigation, undelete, and filtering deleted entries from the default list.

4. **Fix the Subsonic artist/album queries.** The current heuristic (nodes without certain fields = artist) will conflate nodes from any app. Add a proper schema conformance filter (`WHERE n CONFORMS TO schema("subsonic/Artist")`) that requires artists to explicitly claim the schema. Same for albums and tracks.

### 4.2 Next (deepens the platform)

5. **Prepared statement cache (§7.3).** Every query re-prepares SQL. Cache keyed on IR shape + physical table names, invalidated on schema migration/index change/field promotion.

6. **Computed field execution engine.** Wire the existing `ComputeMode` types into the write path: on node create/update, find computed fields that depend on dirty fields, evaluate expressions, and store results. Eager mode runs in the write transaction; Deferred spawns a task; Read annotates for the query layer.

7. **Build at least one more real app UI (Files or Trips).** The Journal app proves the pattern. Pick Files (simpler) or Trips (more interesting UI with map) and build the frontend component so it ships with the binary.

8. **Pagination for all list endpoints.** Add `limit` and `cursor` parameters to every app's list handler. The query language supports `LIMIT`/`SKIP` — it just needs to be used.

### 4.3 Later (polish and deferred features)

9. `field_stats` → index suggestion feedback loop (§6.5)
10. Windowed field_stats (hourly buckets)
11. Type-compatibility validation at compile time (§3.7)
12. CRDT FieldValue variants + merge + `@ops`/`@at` view selectors
13. Field promotion path (JSONB → typed column)
14. Background task scheduler in plugin loader
15. User auth + space-level permissions
16. Query ID tracing and per-stage profiling

### 4.4 Explicitly Deferred (v0.x+)

- Aggregation, unbounded traversal, full-text search, subqueries (QUERY_DESIGN.md §9)
- End-to-end encryption, sync (DESIGN.md)
- Export/import, garbage collection (DESIGN.md)
