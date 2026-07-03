# Panorama — App Evaluation, E2E Test Roadmap & Backend Performance Analysis

This document evaluates how closely the current application suite adheres to the design goals in [DESIGN.md](file:///home/michael/Projects/panorama/DESIGN.md), outlines actionable next features and condensed E2E test specifications for **each** app, and provides a thorough performance analysis of the backend architecture with concrete optimization recommendations.

---

## 1. Overall System & Architecture Alignment

### High-Level Alignment Summary

Panorama's core data engine and plugin architecture align exceptionally well with the foundational requirements defined in [DESIGN.md](file:///home/michael/Projects/panorama/DESIGN.md):

| Design Goal in `DESIGN.md` | Implementation Status | Evaluation & Notes |
| :--- | :--- | :--- |
| **Node Data Structure** | ✅ **Fully Aligned** | Nodes use UUIDs, arbitrary namespaced fields (`system:`, `user:`, `app:`), non-relational storage in SQLite with JSON extract helpers. |
| **Self-Hosted Schemas** | ✅ **Fully Aligned** | Schemas are nodes, versioned, supporting Preferred vs Required modes. System fields like `node_time`, `node_title`, `created_at` are standardized. |
| **Plugin Capabilities System** | ✅ **Fully Aligned** | Fine-grained capability manifest (`field_read`, `field_write`, `object_storage_read`, `write_own_nodes`). Sandbox enforcement via WASM (wasmtime). |
| **Object Storage API** | ✅ **Fully Aligned** | S3-like blob storage (`put_object`, `get_object`, `delete_object`) with resumable upload API hooks. |
| **Spaces & Permissions** | ⚠️ **Partial (v0.0)** | Default space (`default`) is supported across queries and nodes; multi-tenant space ACLs planned for v0.x. |
| **App Ecosystem** | 🔄 **Functional Prototypes** | All 7 target v0.0 applications are built as standalone crates relying *only* on `panorama-core`. |

---

## 2. Detailed Per-App Evaluation, Next Features & E2E Test Specifications

---

### 2.1 Journal App (`panorama-app-journal`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Daily journal where entries are stacked vertically with most recent on top, notes stored as markdown, and block-level breakdown into paragraph nodes for cross-referencing.
- **Current Status**:
  - Implements `journal/JournalEntry` schema with `system:node_title`, `system:node_time`, `journal:content`, `journal:mood`, `journal:paragraph_refs`.
  - Exposes `POST /entries` and `GET /entries` (queries ordered by `system:node_time DESC`).
  - Web UI renders simple entry submission form and raw text list.
- **Gaps**: Paragraph breakdown into child nodes is un-implemented (only a field schema placeholder exists). No rich markdown rendering (renders raw text in `<pre>`). No entry editing or search/filter by mood or date.

#### Actionable Next Features
1. **Block-Level Paragraph Decomposition & Linking**: Automatically split markdown content into individual paragraph child nodes upon entry creation, record node UUID links in `journal:paragraph_refs`, and allow embedding/referencing individual paragraphs in other notes.
2. **Rich Markdown Rendering & WYSIWYG Editor**: Replace plain text areas with a rich markdown editor and live viewer (headings, lists, bold/italic, blockquotes, code blocks).
3. **Journal Entry Editing & Soft Deletion**: Support inline editing of saved journal entries and updating node fields via `PUT /entries/{id}`.
4. **Timeline & Mood Filter Bar**: Filter journal entries dynamically by date range (calendar picker) or mood tags (`happy`, `thoughtful`, etc.).

#### E2E Test Specifications
- **Block-Level Paragraph Decomposition**: Create a multi-paragraph journal entry via the UI form $\rightarrow$ Verify paragraph reference links (`.paragraph-ref-link`) are auto-generated $\rightarrow$ Confirm child paragraph nodes exist and can be navigated to individually.
- **Rich Markdown Rendering**: Input raw markdown formatting (headings, bullet points, code blocks) $\rightarrow$ Save entry $\rightarrow$ Verify the entry detail view renders semantic HTML tags (`<h1>`, `<ul>`, `<code>`) instead of raw unformatted preformatted text.
- **Journal Entry Editing**: Click the "Edit" button on an existing entry card $\rightarrow$ Modify title and content in the editor modal $\rightarrow$ Save changes $\rightarrow$ Verify updated text is reflected in both entry list and node storage.
- **Timeline & Mood Filtering**: Select a specific mood tag (e.g. `excited`) from the mood filter dropdown $\rightarrow$ Verify only entries containing the matching mood badge remain visible in the entry timeline.

---

### 2.2 Wakatime App (`panorama-app-wakatime`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Expose an endpoint that real Wakatime clients/IDE plugins can submit heartbeats to, translating them into time-series nodes for project/language tracking.
- **Current Status**:
  - Implements `wakatime/Heartbeat` schema (`system:node_time`, `wakatime:entity`, `wakatime:project`, `wakatime:language`, `wakatime:duration`, `wakatime:category`).
  - Exposes `POST /heartbeat` and `POST /heartbeats` endpoints supporting standard Wakatime JSON payloads.
  - Basic manual JSON heartbeat submission UI.
- **Gaps**: Lacks Wakatime API key authentication header support, real-time activity status widget ("Currently coding..."), and session idle duration aggregation logic.

#### Actionable Next Features
1. **Automatic Heartbeat Duration & Idle Calculation**: Compute actual coding session durations based on timestamp gaps between consecutive heartbeats (using a configurable 2-minute idle cutoff).
2. **Live Activity Pulse Widget**: Real-time status indicator on the dashboard showing active file, project, and programming language based on heartbeats received within the last 5 minutes.
3. **Wakatime CLI API Key Authentication**: Validate incoming Wakatime CLI HTTP `Authorization: Basic <api_key>` headers against user security tokens.
4. **Project & Language Summary Statistics API**: Endpoint `GET /stats/summary?range=7d` returning pre-aggregated coding time totals per project and language.

#### E2E Test Specifications
- **Session Duration Calculation**: Submit a series of consecutive heartbeats 1 minute apart $\rightarrow$ Navigate to Wakatime dashboard $\rightarrow$ Verify calculated active coding session duration aggregates correctly to 2 minutes.
- **Live Activity Pulse Widget**: Trigger a heartbeat with active project "panorama" and language "Rust" $\rightarrow$ Verify a live status badge ("Currently Coding: panorama (Rust)") appears immediately in the UI header.
- **API Key Authentication**: Issue an HTTP POST request to `/heartbeat` with an invalid `Authorization` header $\rightarrow$ Verify the server responds with a `401 Unauthorized` HTTP status code.
- **Summary Statistics View**: Click the "Weekly Summary" tab in the Wakatime plugin UI $\rightarrow$ Verify total coding hours per project are populated in the summary table.

---

### 2.3 Dashboards / Grafana App (`panorama-app-grafana`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Grafana-like dashboards for querying and visualizing time-series data (e.g. Wakatime coding hours, top projects leaderboard) over arbitrary time ranges.
- **Current Status**:
  - Implements `grafana/Dashboard` schema (`system:node_title`, `grafana:config`).
  - Endpoints `POST /query`, `POST /dashboards`, `GET /dashboards`.
  - Supports backend aggregation algorithms (`count`, `sum_duration`, `leaderboard`).
  - Web UI renders plain HTML table with dropdowns.
- **Gaps**: Lacks visual chart rendering (line/bar/pie graphs), preset time range selectors (`24h`, `7d`, `30d`), multi-panel custom dashboard layout grid, and a PromQL/Panorama query expression builder.

#### Actionable Next Features
1. **Visual Charting Engine**: Embed SVG/Canvas bar and line charts (e.g. using Recharts or Chart.js) to render time-series trend lines and leaderboard breakdowns.
2. **Quick Time Range Selectors UI**: Interactive time range buttons (`Last 24 Hours`, `Last 7 Days`, `Last 30 Days`, `Custom Range`) that automatically bind to query payloads.
3. **Multi-Panel Drag & Drop Dashboard Builder**: Interactive grid UI allowing users to create, rearrange, resize, and save dashboards containing multiple graph panels.
4. **PromQL / Expression Query Builder**: UI visual query builder for constructing aggregation functions (`sum_over_time`, `rate`, `topk`) over node fields.

#### E2E Test Specifications
- **Visual Charting Engine**: Select "Leaderboard" aggregation from the query panel $\rightarrow$ Verify a Canvas/SVG chart element renders with bar heights proportional to project duration.
- **Quick Time Range Selectors**: Switch dashboard filter between "Last 24 Hours" and "Last 7 Days" $\rightarrow$ Verify displayed metric totals update dynamically to reflect the expanded time window.
- **Multi-Panel Dashboard Builder**: Click "+ Add Panel", configure panel title and query expression, and save dashboard $\rightarrow$ Reload browser page $\rightarrow$ Confirm the saved panel layout persists and renders correctly.
- **Query Expression Builder**: Build a query using the `topk(3, ...)` function $\rightarrow$ Execute query $\rightarrow$ Verify only the top 3 items are returned in the result table/chart.

---

### 2.4 Trip Planner App (`panorama-app-trips`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Plan trips with scheduled events, incorporating calendar grid views and interactive map views for geographical event coordinates.
- **Current Status**:
  - Implements `trips/Trip` and `trips/Event` schemas (`system:node_title`, `system:node_start_time`, `system:node_end_time`, `trips:latitude`, `trips:longitude`, `trips:location_name`, `trips:notes`).
  - Endpoints `POST /trips`, `GET /trips`, `POST /events`, `GET /events`, `GET /events/map`.
  - UI allows creating trips, adding events with lat/lng inputs, and viewing location lists.
- **Gaps**: Map view is a plain text list rather than an interactive Leaflet/OpenStreetMap container. Calendar view lacks a visual month/week grid. Lacks travel route sequencing and event attachment support.

#### Actionable Next Features
1. **Interactive Leaflet / OpenStreetMap Container**: Integrate Leaflet.js to render interactive map tiles, pin event markers, display popups, and draw travel route polylines.
2. **Visual Calendar Grid View**: Render an interactive monthly/weekly calendar grid sorting events by `system:node_start_time` and `system:node_end_time`.
3. **Itinerary Drag-and-Drop Sequencer**: Interface to re-order daily trip events and auto-calculate distance between successive coordinates.
4. **Trip Event Attachments**: Support linking object storage files (flight tickets, hotel reservations, photos) directly to event nodes via `ObjectRef`.

#### E2E Test Specifications
- **Leaflet Map Integration**: Navigate to "Map View" tab for a trip containing geocoded events $\rightarrow$ Verify `.leaflet-container` is initialized and event marker icons (`.leaflet-marker-icon`) are visible on the map.
- **Calendar Grid View**: Switch to "Calendar View" tab $\rightarrow$ Verify scheduled trip events appear as interactive pills inside the correct day grid cells of the month view.
- **Itinerary Drag-and-Drop Sequencer**: Re-order two event items in the daily itinerary via drag-and-drop $\rightarrow$ Click "Save Itinerary" $\rightarrow$ Confirm event sequence order is updated in storage and reloaded UI.
- **Event Media Attachments**: Upload a PDF ticket to an event node $\rightarrow$ Verify an attachment badge appears on the event card and clicking it opens the PDF object stream.

---

### 2.5 Beli Alternative App (`panorama-app-beli`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Rate restaurants using a PARTIAL ORDERING model (pairwise comparisons $A > B$) computed via topological sorting into tiers, rather than arbitrary 5-star ratings. Pull info from OSM.
- **Current Status**:
  - Implements `beli/Restaurant` and `beli/Comparison` schemas.
  - Implements Kahn's algorithm in `compute_rankings()` to generate topological tiers.
  - Endpoints `POST /restaurants`, `GET /restaurants`, `POST /compare`, `GET /rankings`.
  - Basic UI for adding restaurants, submitting comparisons, and viewing tier lists.
- **Gaps**: Lacks OpenStreetMap / Overpass API integration for restaurant auto-complete, conflict/cycle resolution UI for contradictory comparisons ($A > B > C > A$), context-based rankings (e.g. "Best Pizza" vs "Best Sushi"), and an interactive pairwise matchmaker wizard.

#### Actionable Next Features
1. **OpenStreetMap / Overpass API Integration**: Search input connected to OSM to auto-fill restaurant name, cuisine type, address, and GPS location.
2. **Cycle & Conflict Resolution Modal**: Detect directed graph cycles when users record conflicting comparisons and offer an interactive UI modal to resolve the conflict.
3. **Category-Specific Partial Orders**: Filter comparisons and calculate distinct ranking DAGs by context/cuisine (`beli:context`, e.g., "Ramen", "Cocktail Bars").
4. **Interactive Pairwise Matchmaker Wizard**: "Matchmaker" UI mode that presents uncompared restaurant pairs ("Which was better?") to guide the user in completing the partial order graph efficiently.

#### E2E Test Specifications
- **OSM Search Integration**: Search for a restaurant name in the OSM search input $\rightarrow$ Select a result from auto-complete $\rightarrow$ Verify restaurant name, cuisine, and address fields populate automatically.
- **Cycle Resolution UI**: Submit pairwise comparisons forming a loop ($A > B$, $B > C$, $C > A$) $\rightarrow$ Verify a conflict resolution modal alerts the user of a directed graph cycle.
- **Category-Specific Rankings**: Filter rankings by context category "Pizza" $\rightarrow$ Verify topological tiers recalculate and display only restaurants categorized under Pizza.
- **Matchmaker Wizard**: Launch Matchmaker mode $\rightarrow$ View recommended unranked restaurant pair $\rightarrow$ Select preferred option $\rightarrow$ Verify ranking tiers update immediately.

---

### 2.6 Subsonic Music Streaming App (`panorama-app-subsonic`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Subsonic-compatible music server interface for streaming audio files stored in object storage.
- **Current Status**:
  - Implements `subsonic/Artist`, `subsonic/Album`, `subsonic/Track` schemas.
  - Stores audio blobs in object storage via `put_object("subsonic-audio", ...)` with `subsonic:audio_ref`.
  - Endpoints `GET /rest/ping`, `GET /rest/getArtists`, `GET /rest/getAlbumList2`, `GET /rest/stream`, `POST /upload`.
  - Web UI lists artists and album metadata.
- **Gaps**: Web UI lacks an embedded HTML5 audio player widget (play/pause/seek controls). Missing key Subsonic client endpoints (`getMusicFolders`, `getSong`, `search3`, `getCoverArt`, `scrobble`). Upload endpoint relies on query string parameters instead of automatic ID3 audio tag parsing.

#### Actionable Next Features
1. **Embedded Web Audio Player Widget**: Persistent HTML5 audio player bar with play/pause, volume control, scrub bar, current track info, and playback queue management.
2. **Automatic ID3 Tag & Cover Art Extractor**: Server-side parsing of uploaded MP3/FLAC audio files to extract ID3 tags (artist, album, track title, track number, duration) and store embedded cover art images in object storage.
3. **Subsonic Client API Expansion**: Implement missing API endpoints (`/rest/getSong`, `/rest/search3`, `/rest/getCoverArt`, `/rest/scrobble`) to support third-party mobile clients (e.g. Navidrome, Dsub, Ultrasonic).
4. **Playlists & Favorite Tracks Management**: Allow creating, editing, and streaming custom user playlist nodes.

#### E2E Test Specifications
- **Web Audio Player Widget**: Click "Play" next to a track in the music library $\rightarrow$ Verify persistent bottom audio player bar displays track title, HTML5 `<audio>` element plays, and pause button is active.
- **Automatic ID3 Tag Extraction**: Upload an MP3 file with ID3 tags $\rightarrow$ Confirm Artist, Album, and Track nodes are automatically created and linked in node storage.
- **Subsonic API Compatibility**: Request `/rest/getCoverArt?id=<album_id>` $\rightarrow$ Verify server responds with `200 OK` and binary `image/jpeg` payload.
- **Playlist Management**: Create a new playlist "Chill Beats" and add 3 tracks $\rightarrow$ Verify playlist card displays correct track count and plays sequentially.

---

### 2.7 File Manager App (`panorama-app-files`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Support uploading files, resumable transfers, and managing large files in object storage.
- **Current Status**:
  - Implements `files/File` schema (`system:node_title`, `files:object_ref`, `files:file_size`, `files:mime_type`, `files:folder`).
  - Endpoints `POST /upload`, `POST /upload/initiate`, `GET /files`, `GET /files/{id}`, `DELETE /files/{id}`.
  - Downloads serve correct `Content-Disposition: attachment` headers.
  - Web UI features drag-and-drop zone and file list.
- **Gaps**: Resumable upload UI and endpoint logic (`POST /upload/initiate`) is currently a stub pointing to `/api/uploads` without chunked client upload handling. Lacks inline file preview modals, folder tree navigation, and bulk actions (multi-delete, zip download).

#### Actionable Next Features
1. **Resumable Chunked Upload Manager**: Client-side chunked uploader utilizing `/api/uploads` with progress tracking, pause/resume controls, and retry handling for large files.
2. **In-Browser File Previewer Modal**: Inline preview modal supporting image display (PNG/JPG/WEBP), PDF document viewing, text/code inspection, and audio/video playback.
3. **Folder Tree Navigation & Drag-and-Drop Organization**: Interactive folder sidebar allowing directory creation, renaming, and drag-and-drop file relocation.
4. **Bulk Selection & ZIP Download**: Multi-select toolbar for batch deleting files or downloading selected files as a ZIP archive.

#### E2E Test Specifications
- **Resumable Chunked Upload Manager**: Initiate a large file upload $\rightarrow$ Click "Pause" on progress bar $\rightarrow$ Confirm status changes to Paused $\rightarrow$ Click "Resume" $\rightarrow$ Verify upload completes successfully.
- **In-Browser File Previewer**: Click an image file row in the file browser $\rightarrow$ Confirm a modal dialog opens displaying the image preview via object stream URL.
- **Folder Tree Navigation**: Create a folder "Documents" and drag a file into it $\rightarrow$ Click "Documents" folder $\rightarrow$ Verify file is located inside the directory view.
- **Bulk Selection & Deletion**: Select multiple file checkboxes $\rightarrow$ Click "Delete Selected" and accept confirmation prompt $\rightarrow$ Confirm all selected files are removed from storage and UI.

---

## 3. Backend Performance Analysis & Optimization Opportunities

A thorough code audit of `crates/panorama-server` and `crates/panorama-core` revealed 7 critical performance bottlenecks. Below is an in-depth analysis of unoptimal code areas along with targeted remediation plans.

### 3.1 WASM Module Re-Compilation on Every Request
- **Location**: `crates/panorama-server/src/wasm_runtime.rs` ([execute_wasm_handler](file:///home/michael/Projects/panorama/crates/panorama-server/src/wasm_runtime.rs#L13-L27))
- **Issue**:
  On every single HTTP request routed to a WASM plugin (`/plugin/{id}/*`), the server initializes a new `wasmtime::Engine` and re-compiles raw WASM bytecode (`wasmtime::Module::from_binary(&engine, wasm_bytes)`).
  ```rust
  let mut config = wasmtime::Config::new();
  let engine = wasmtime::Engine::new(&config)?;
  let module = wasmtime::Module::from_binary(&engine, wasm_bytes)?; // High CPU cost per request
  ```
- **Performance Impact**: Adds **10ms – 100ms** latency overhead per HTTP request due to JIT compilation.
- **Optimization Strategy**:
  1. Initialize `wasmtime::Engine` once globally in `AppState`.
  2. Cache compiled `wasmtime::Module` objects in an in-memory `Arc<DashMap<String, Module>>` registry when `.panoapp` packages are loaded.
  3. Pre-compile WASM modules to `.cwasm` Ahead-Of-Time (AOT) artifacts during `.panoapp` packaging.
- **Expected Win**: Reduces WASM plugin dispatch latency from **~50ms to <1ms**.

---

### 3.2 Single Global Mutex Lock on SQLite Database
- **Location**: `crates/panorama-server/src/storage.rs` ([NodeStorage struct](file:///home/michael/Projects/panorama/crates/panorama-server/src/storage.rs#L19-L22))
- **Issue**:
  `NodeStorage` wraps the underlying SQLite connection in `Arc<std::sync::Mutex<Connection>>`. Even though SQLite is configured with `PRAGMA journal_mode=WAL;`, wrapping the single connection in a `Mutex` forces **all read queries** (`get`, `query_lang`, `get_ready_indexes`) to wait for any active read or write operation to finish.
  ```rust
  pub struct NodeStorage {
      conn: Arc<std::sync::Mutex<Connection>>, // Serializes all readers & writers
  }
  ```
- **Performance Impact**: Completely negates SQLite WAL mode concurrent reader benefits, causing query bottlenecking under concurrent requests.
- **Optimization Strategy**:
  Replace `Arc<Mutex<Connection>>` with a connection pool (e.g. `r2d2_sqlite` or `deadpool-sqlite`) maintaining:
  - 1 dedicated write connection pool.
  - $N$ read-only connections (`SQLITE_OPEN_READ_ONLY`) allowing non-blocking concurrent reads.
- **Expected Win**: Scales query throughput by **4x – 10x** under concurrent API load.

---

### 3.3 Statement Cache Bypass in Query Language Compiler
- **Location**: `crates/panorama-server/src/storage.rs` ([query_lang method](file:///home/michael/Projects/panorama/crates/panorama-server/src/storage.rs#L34-L41)) & `crates/panorama-server/src/query/cache.rs`
- **Issue**:
  Although `StatementCache` (an LRU cache for compiled SQL) is defined in `query/cache.rs`, `NodeStorage::query_lang()` completely ignores it. Every query execution re-parses the AST, re-runs Phase 1 meta-table lookups against SQLite, and re-compiles SQL strings.
- **Performance Impact**: Increases CPU cycles spent parsing strings and querying metadata for identical repetitive queries (e.g. dashboard queries).
- **Optimization Strategy**:
  Integrate `StatementCache` into `query_lang()`:
  ```rust
  if let Some(cached) = self.cache.get(query_string) {
      // Execute cached SQL directly with parameters
  }
  ```
- **Expected Win**: Eliminates compiler overhead for hot queries (**30% – 50% CPU reduction** on repeated queries).

---

### 3.4 In-Memory Chunk Assembly for Resumable Uploads
- **Location**: `crates/panorama-server/src/object_store.rs` ([complete_upload method](file:///home/michael/Projects/panorama/crates/panorama-server/src/object_store.rs#L242-L260))
- **Issue**:
  Pending upload chunks are buffered in memory inside `HashMap<u32, Vec<u8>>`. Upon completing an upload, `complete_upload` concatenates all chunk byte vectors into a single contiguous `Vec<u8>` in RAM before writing to disk:
  ```rust
  let mut all_data = Vec::new();
  for idx in indices {
      all_data.extend_from_slice(chunk); // Full file buffered in RAM!
  }
  self.put(&upload.bucket, &upload.key, &all_data, &upload.mime_type)
  ```
- **Performance Impact**: High RAM usage and out-of-memory (OOM) crash risk when handling multi-gigabyte video or music file uploads.
- **Optimization Strategy**:
  Stream uploaded chunks directly into a temporary file on disk (`$DATA_DIR/tmp/upload_id.part`) using append mode (`std::fs::OpenOptions::new().append(true)`). On completion, execute an atomic file rename (`std::fs::rename`).
- **Expected Win**: Reduces memory footprint of multi-GB uploads from **O(File Size)** to **O(Chunk Size)** (~MBs).

---

### 3.5 Heavy Transaction Overhead in Meta-Table Synchronisation
- **Location**: `crates/panorama-server/src/meta.rs` ([sync_field_presence](file:///home/michael/Projects/panorama/crates/panorama-server/src/meta.rs#L699-L723))
- **Issue**:
  On every single node creation or update, `sync_field_presence` executes `DELETE FROM field_presence WHERE node_id = ?1`, followed by `resolve_ns_id` lookups and individual `INSERT` queries for every field on the node:
  ```rust
  conn.execute("DELETE FROM field_presence WHERE node_id = ?1", params![nid])?;
  for (key, value) in fields {
      let ns_id = Self::resolve_ns_id(conn, ns_str)?; // Triggers SELECT for every field!
      conn.execute("INSERT INTO field_presence ...", ...)?;
  }
  ```
- **Performance Impact**: A node with 20 fields executes 22 separate SQL statements on every update inside a transaction.
- **Optimization Strategy**:
  1. Cache `namespace_str -> ns_id` in an in-memory `Arc<DashMap<String, i64>>` to avoid `resolve_ns_id` SELECT queries.
  2. Implement differential field diffing (only INSERT/DELETE changed fields) or use SQLite multi-row `INSERT INTO field_presence VALUES (...), (...)`.
- **Expected Win**: Reduces write transaction execution time by **60% – 80%**.

---

### 3.6 Full Table Scans for Field Filter Queries
- **Location**: `crates/panorama-app-grafana`, `crates/panorama-app-trips`, `crates/panorama-app-journal`
- **Issue**:
  Apps query nodes using `MATCH (n) IN space("default") WHERE HAS_FIELD(n, "domain", "field") RETURN n`. The compiler generates CTE queries filtering against `field_presence`, but field value filtering relies on runtime `json_extract(fields_json, '$.domain:field.value')` without utilizing SQLite expression indexes.
- **Performance Impact**: Queries perform full table scans over all nodes as dataset sizes grow.
- **Optimization Strategy**:
  Utilize `managed_indexes` to generate SQLite expression indexes on frequently queried fields:
  ```sql
  CREATE INDEX idx_node_time ON nodes(json_extract(fields_json, '$.system:node_time.value'));
  CREATE INDEX idx_wakatime_project ON nodes(json_extract(fields_json, '$.wakatime:project.value'));
  ```
- **Expected Win**: Transforms **$O(N)$ full table scans into $O(\log N)$ B-Tree index lookups**.

---

### 3.7 Synchronous Startup Metadata Scanning in Object Storage
- **Location**: `crates/panorama-server/src/object_store.rs` ([load_metadata method](file:///home/michael/Projects/panorama/crates/panorama-server/src/object_store.rs#L58-L81))
- **Issue**:
  During server initialization, `load_metadata()` synchronously iterates through all directories under `$DATA_DIR/objects/` and parses every `.meta.json` file on disk to populate the in-memory `DashMap`.
- **Performance Impact**: Server startup time degrades linearly with the number of stored objects (blocking server launch for seconds when thousands of files exist).
- **Optimization Strategy**:
  Persist object metadata inside a dedicated SQLite table (`object_metadata`) instead of scanning individual JSON files on disk during boot.
- **Expected Win**: Instant server startup (**$O(1)$ database initialization** regardless of object count).

---

## 4. Summary Roadmap & Recommended Implementation Order

To achieve full design fidelity and optimal performance, execution should follow three structured phases:

1. **Phase 1: High Impact UI/UX Enhancements**
   - Interactive Leaflet maps in **Trip Planner**
   - Canvas/SVG charting in **Dashboards**
   - HTML5 Audio Player in **Subsonic Music**
2. **Phase 2: Data & Core Workflow Features**
   - Block-level paragraph child node breakdown in **Journal**
   - OpenStreetMap live search in **Beli**
   - Automatic ID3 tag extraction in **Subsonic Music**
3. **Phase 3: Backend Performance & Optimization Wins**
   - WASM module caching & AOT compilation in `wasm_runtime.rs`
   - SQLite connection pooling (replacing single Mutex) in `storage.rs`
   - Streaming disk-buffered uploads in `object_store.rs`
   - In-memory namespace cache & batch inserts in `meta.rs`
