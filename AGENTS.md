This is panorama, an everything app similar to Notion, Anytype, Logseq.

# Architecture

The frontend is written in Qt. The backend is written in Rust, running a web server defaults to port 4141.

The data structure presented to the frontend is essentially a graph of nodes.
A node can have multiple fields associated to it, each from different apps.
This way, a single node can have multiple meanings.

The real database is SQLite, with a single nodes table and a bunch of dynamically managed tables that is tracked via `_panorama_schema*` tables.

Fields are a distinct concept from edges, which relate nodes.

# Operating instructions

- Your modifications are not complete until both Rust and C++ sides compile. This is done by running `ninja -C build` from the root directory of this repository.

# Remaining tasking

- Add an atomic `importFile` backend mutation to create the `nodes` row and all `file/*` fields in a single DB transaction.
- Add `file/uploaded_at` (timestamp) field and populate it on import.
- Deduplicate and validate blobs before copy; surface corruption checks and return deterministic paths.
- Make blob store path configurable (avoid hardcoding `blobs/`) and centralize blob helper utilities.
- Improve UI: support multi-file drops, an import queue panel with progress/errors, and nicer import UX than the temporary placeholder.
- Add integration tests: verify `importFile` (or multi-`setField`) creates `nodes` row and app fields atomically and that blob files are written to the tiered path.
- Extend GraphQL execution: run translator SQL, map rows into GraphQL-shaped JSON, and add support for filters/arguments/fragments.

# Development log

- Important context:

  - **`nodes` vs app tables:** The system models a single `nodes` table plus many dynamically-managed app-specific tables (tracked via `_panorama_schema*`). A node must have a `nodes` row for GraphQL lookups by id to succeed; app-table writes should normally create or ensure the corresponding `nodes` row exists.
  - **Mutations must be transactional:** When a mutation (e.g. `setField`) creates or updates app-specific data it should also create the `nodes` row in the same DB transaction to avoid races where the frontend queries a node by id immediately after the mutation.
  - **Translator vs execution:** The GraphQL -> SQL translator emits SQL for requested shapes but does not execute the SQL or reconstruct GraphQL-shaped JSON; the execution layer must run the SQL and map rows back into the GraphQL response structure.
  - **Schema lookups are authoritative:** All dynamic table and column names must be resolved via `_panorama_schema*` lookups to avoid SQL injection and to keep the translator simple and safe — never interpolate app or field names directly into SQL without a validated schema mapping.
  - **Tests & CI notes:** Tests create temporary DB files and rely on idempotent app installation; CI must allow these files to be created and removed. Add an integration test to assert that `setField` creates both a `nodes` row and the app field atomically.
  - **Concurrency hazards:** Concurrent writers that attempt to create the same node or schema entries can race — prefer `INSERT OR IGNORE` / `UPSERT` patterns or explicit transactions around the create+write sequence.
  - **Runtime & integration details (not obvious from code):**
    - **Backend endpoint & format:** The frontend talks to a local backend at `http://127.0.0.1:4141/graphql` using GraphQL JSON requests with an optional `variables` object.
    - **Journal node ids:** Journal pages are keyed by ISO date strings (`YYYY-MM-DD`) and opened via URLs like `/journal/YYYY-MM-DD` (Qt uses `QDate::toString(Qt::ISODate)`).
    - **Save flow & debounce:** Journal saves are debounced (1s) in the UI; the editor sends a `setField(nodeId: ..., app: "journal", field: "title", value: ...)` mutation to persist content. The UI expects the server to persist before showing `Saved`.
    - **UI threading assumptions:** Qt widgets and signals run on the main thread; network replies are handled via `QNetworkAccessManager` callbacks — avoid heavy work on reply handlers and use the main thread for widget updates.
    - **Optimistic vs authoritative state:** The UI treats a successful mutation reply as authoritative (marks `Saved`); it does not implement optimistic merges for concurrent edits.
    - **Icon & resource fallback:** The calendar button uses `QIcon::fromTheme("calendar")` with a `QStyle` fallback; for consistent cross-platform icons consider bundling an SVG resource (`:/icons/...`).
    - **Per-document indicator behavior:** Each `Journal` has a small inline dot that signals Saved/Saving/Unsaved. Programmatic loads suppress `textChanged` handling (`m_loading`) to avoid false unsaved states.
    - **Routing & widget creation:** `MainWindow::openUrl` maps `/journal/<id>` to a new `Journal` widget inside a dock; the frontend relies on this routing contract.
    - **SQL safety:** When writing dynamic tables, always resolve table/column via schema lookup and use parameter binding for values; prefer prepared statements/`sqlx` binding for all user content.

- Implemented idempotent app installation (journal) in Rust:

  - `install_default_apps` now checks `_panorama_schema_columns` before creating new dynamic tables.
  - Added `Dal::new`, `Dal::has_schema_key`, `Dal::schema_count`, and `Dal::schema_entry` helpers to centralize schema lookups.

- Implemented a basic GraphQL -> SQL translator (minimal, safe subset):

  - Supports a single top-level `nodes` selection; node scalars (e.g. `id`, `type`, timestamps); nested app fields like `journal { title }`.
  - App fields are translated to keys of the form `app/field` and looked up in `_panorama_schema_columns` to discover the real sqlite table and column names.
  - Builds a `SELECT` that pulls `nodes.*` scalars and LEFT JOINs the dynamic app tables, returning a `sqlx::QueryBuilder` with the SQL.
  - The translator currently returns SQL only — it does not execute the query nor reconstruct the GraphQL-shaped JSON response.

- Server wiring and safety notes:

  - The axum handler now calls the translator with an owned `Dal` clone and an owned `String` query to avoid lifetime and `Send` issues when registering the route.
  - This design trades some cloning for simpler async/axum compatibility; we can revisit lifetimes if performance matters.

- Tests added:

  - `panorama-core/tests/install_default_apps.rs` — verifies journal fields are created exactly once (idempotent install).
  - `panorama-core/tests/graphql_to_sql.rs` — creates a small DB, installs the journal app, runs the translator on `{ nodes { id type journal { title } } }`, and asserts the generated SQL references the dynamic table/column.
  - Temporary DB files used by tests are removed after the test (`test_install_default_apps.db`, `test_graphql_to_sql.db`).

- Limitations and next steps:

  - The GraphQL subset is intentionally small: no arguments/filters, no fragments, no edges, no mutations, no execution of SQL and no mapping back to GraphQL JSON.
  - Next steps: execute the built SQL, map results back into GraphQL response shape, add support for filters/arguments and fragments, and extend test coverage for multiple apps/fields.

- Build & test commands used during development:

  - Build both sides: `ninja -C build` (required for a complete change)
  - Run Rust crate tests: `cd panorama-core && cargo test`

- Misc notes:

  - Build produced several unrelated warnings (unused imports/unused vars) and some macOS linker warnings about SDK versions; these are orthogonal to the implemented features and can be cleaned up separately.

- Recent JournalStore learnings:

  - **Central store:** Added a singleton `JournalStore` (`QObject`) to cache per-node content, debounce saves, and broadcast updates to all open `Journal` widgets via Qt signals (`contentChanged`, `statusChanged`).
  - **API surface:** `ensureLoaded(nodeId)` loads initial content, `setContent(nodeId, content)` updates cache + schedules save, `setNetworkManager(...)` wires `QNetworkAccessManager` for GraphQL requests.
  - **Save flow:** Per-node `QTimer` debounces (1s) then issues a `setField` GraphQL mutation; `statusChanged` reports `unsaved` → `saving` → `saved` (or `error`).
  - **Cross-window updates:** Editors subscribe to `contentChanged` to receive authoritative content; when one window types it calls `setContent` which immediately broadcasts the new text to others.
  - **Cursor-jump bug & fix:** Emitting `contentChanged` back to the origin caused the origin editor to call `setPlainText(...)` and reset the cursor to the start. Fix: `Journal` now ignores incoming `contentChanged` when the editor already holds the identical text (skip `setPlainText`).
- **Alternatives & future improvements:** Could embed an origin token to avoid echoing to the sender, normalize whitespace before comparison, or preserve/restore cursor/selection on programmatic updates; consider changing `statusChanged` to a typed enum (`Q_ENUM`) and removing the singleton in favor of DI.
  - **Build notes:** Added `ui/stores/JournalStore.cpp` to `CMakeLists.txt` so `Q_OBJECT` is moc'ed. Local sandbox build showed unrelated Qt/uic and macOS SDK warnings (toolchain environment issues) but the source changes are correct for a normal dev environment.

## Operational Notes (not obvious from code)

- **Plugin model & manifests:** Apps live under `./apps/<appname>` and declare a `manifest.yaml`. Manifests include `meta`, a `fields` list (`key` + `type`), a `lua` object (`entrypoint` + `functions`), `ui_plugins` list (each has `target` + `path`), and `permissions` (booleans). The server scans `./apps` (and any paths in `PANORAMA_APPS_PATH`) to discover apps.

- **Lua apps run in the backend:** Each app with a `lua.entrypoint` is launched in its own long-running worker thread inside the backend. The worker hosts an mlua VM and registers a global `openUrl(url)` function. This keeps app logic centralized and avoids embedding app scripts in the UI process.

- **openUrl flow & source context:** When app Lua calls `openUrl(url)` the backend enqueues a UI event { source: <app_path>, url: <string> }. The frontend polls `GET /ui/events` and opens the URL. The `source` field is available so the UI can make layout decisions (for example: open journal links in a right-hand split if the source contains `journal`). This mechanism preserves the UI's control over windowing while allowing apps to request navigation.

- **RPC for app functions:** The frontend can call `POST /call_app_function` to invoke a function in the app's Lua VM (this is used by the startup runner). Calls are routed to the app worker via an in-process channel and blocked with a short timeout for a synchronous return value. Current POC passes simple string args and expects a string result; consider switching to JSON for structured arguments/results.

- **Startup tasks:** `panorama-system/config.yaml` contains a `startup:` list. Entries can be `type: call_app` with `app` and `function`. On UI startup the Qt app queries `GET /startup`, posts the call to `/call_app_function`, and if the result is a URL calls `MainWindow::openUrl(...)`. This is how the "open today's daily note" use-case is implemented (the journal Lua app exposes `daily_url()` which returns `/journal/YYYY-MM-DD`).

- **Native UI plugins:** The server reports `ui_plugins` for the current runtime target triple; the Qt app requests `GET /plugins` and attempts to `QPluginLoader::load()` the returned paths. The plugin interface (`ui/plugins/PluginInterface.h`) requires `availableWidgetTypes()` and `createWidget(type, parent)`. Plugins must be compiled for the running archive (the manifest contains `target` entries), and the main CMake optionally includes `apps/*/ui_plugin` if present.

- **Security & permissions:** The manifest `permissions` table is authoritative and should be enforced: `allow_native` gates loading native plugins, `allow_os_access` gates exposing dangerous Lua globals. The POC removes the `os` global unless `allow_os_access=true`, but this is not a replacement for OS-level sandboxing — treat Lua scripts as untrusted unless explicitly granted rights.

- **Sandbox limitations & safety:** The current Lua sandbox is lightweight (masking `os`); do not assume it prevents all misuse. Long-running workers run untrusted code on backend threads — consider timeouts, memory limits, and OS sandboxing in production.

- **Building plugin artifacts:** The top-level CMake will include `apps/<app>/ui_plugin` when the CMakeLists is present. If your environment's `uic`/Qt tools are incompatible, build the plugin separately under `apps/<app>/ui_plugin` to avoid reconfiguring the full UI project.

- **Environment variables:** `PANORAMA_APPS_PATH` can be set to a colon-separated list of extra app search paths (e.g. user-local install locations). The server will scan these in addition to `./apps`.

- **Testing & evolution:** The current POC uses string args and polling for UI events. For a more robust architecture consider:
  - Structured JSON RPC for function calls and returns.
  - WebSocket or server→UI push notifications for UI events (avoid polling).
  - Per-call timeouts and resource accounting for Lua worker calls.
  - Manifest validation (duplicate fields, unsupported types, missing functions).

These notes codify the runtime contract and security assumptions that aren't obvious from the code alone. If you want I can add CI tests that validate startup behavior (e.g. that `daily_url` resolves and the UI receives an event), add JSON-RPC encoding, or replace polling with a WebSocket bridge.
