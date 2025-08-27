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
