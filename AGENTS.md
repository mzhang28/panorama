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
