# Panorama Query Language v0 — Design

## 1. Goals & Non-Goals

**Goals**
- Cypher-flavored surface syntax for humans; a small IR for the engine.
- Support the common workflows: fetch nodes by space + schema + field predicates, follow references, project fields.
- Enforce the data model at the language level: space filters are mandatory, capability checks happen implicitly, namespace-qualified field access.
- Compile efficiently against the meta-table storage layout (JSONB primary + promoted columns per schema + `field_presence` + `managed_indexes`).

**Non-goals for v0**
- Aggregation (COUNT, SUM, GROUP BY, window functions).
- Transitive/unbounded graph traversal.
- Full-text search (mentioned; not designed here).
- Subqueries and correlated queries.
- Mutations (this doc is read-only; writes are a separate API).

## 2. Data Model Recap (relevant slice)

- **Node** — `(id, space_id, created_at, updated_at, created_by, created_by_app)`.
- **Field** — addressed by `(node_id, ns_id, field_name)`; namespace is either `system`, `user`, or an app namespace (with an app-side stable identifier that resolves to a local `ns_id`).
- **Schema** — declared collection of expected fields with types; nodes may `conform to` zero or more schemas; conformance can be `preferred` or `required`.
- **Type** — a first-class object per field, declaring conflict resolution strategy (CRDT-merge, LWW, reject-concurrent, app-defined).

## 3. Surface Syntax

### 3.1 Basic node selection

```
MATCH (n) IN space("personal")
RETURN n
```

`IN space(...)` is **required** on every `MATCH`. There is no query that scans across spaces implicitly; cross-space queries must name each space explicitly, and each is capability-checked independently.

### 3.2 Schema conformance filter

```
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("com.example.event")
RETURN n
```

Version constraints:

```
WHERE n CONFORMS TO schema("com.example.event", >=2)
WHERE n CONFORMS TO schema("com.example.event", 2..4)
```

### 3.3 Field access

Namespaced. `n.<namespace>.<field>` where namespace can be `system`, `user`, or an app identifier (quoted if it contains dots):

```
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("com.example.event")
  AND n."com.example.event".start_time > "2026-01-01"
  AND n."com.example.event".start_time < "2026-08-01"
RETURN n."com.example.event".title,
       n."com.example.event".start_time,
       n.system.created_at
```

Shorthand: if a `CONFORMS TO schema(X)` clause is present, field paths on `n` may omit the namespace and be resolved against `X`:

```
WHERE n CONFORMS TO schema("com.example.event")
  AND n.start_time > "2026-01-01"
RETURN n.title, n.start_time
```

### 3.4 Field presence

```
WHERE HAS_FIELD(n, "com.example.event", "attendees")
WHERE HAS_FIELD(n, "*", "title")     -- any namespace
```

### 3.5 Reference traversal

Bounded depth only. Single-hop:

```
MATCH (a)-[:REF("attendee")]->(b) IN space("personal")
WHERE a CONFORMS TO schema("com.example.event")
  AND b CONFORMS TO schema("com.example.person")
RETURN a.title, b.name
LIMIT 100
```

Bounded multi-hop (explicit depth required):

```
MATCH (a)-[:REF("parent")*1..3]->(b) IN space("work")
RETURN a, b
LIMIT 100
```

The `*` form without a bound (`*..` or `*`) is a **syntax error** in v0.

Reverse traversal (`<-[:REF(...)]-`) is only permitted on ref-typed fields that have a reverse-ref index declared. Otherwise a compile-time error tells the user which index to add.

### 3.6 CRDT view selectors

Default read is the merged current value. Explicit selectors are available:

```
n.event.attendees@merged          -- default; can be omitted
n.event.attendees@ops             -- op log (returns a sequence of ops, not a value)
n.event.attendees@at(cursor_x)    -- value at a specific causal cut
```

`@ops` and `@at(...)` return typed op-stream values; downstream operators that don't understand op streams will error at compile time.

### 3.7 Predicates

Operators: `=`, `!=`, `<`, `<=`, `>`, `>=`, `IN [...]`, `LIKE`, `CONTAINS`, `IS NULL`, `IS NOT NULL`. Boolean composition: `AND`, `OR`, `NOT`.

Not every operator is valid on every type. Validity is checked at compile time whenever the field's type is known (i.e., it's part of a `CONFORMS TO` schema or has a declared type); for untyped ad-hoc fields, validity is checked at runtime against the value envelope's `t` tag.

| Type | Valid operators | Notes |
|---|---|---|
| number | `= != < <= > >= IN IS NULL IS NOT NULL` | Standard total order. |
| string | `= != < <= > >= IN LIKE IS NULL IS NOT NULL` | `<`/`>` are byte-wise lexicographic, not locale-aware, in v0. |
| timestamp | `= != < <= > >= IN IS NULL IS NOT NULL` | Same as number. |
| boolean | `= != IS NULL IS NOT NULL` | No ordering. |
| ref | `= != IS NULL IS NOT NULL` | Equality is node-id identity. No ordering. |
| or-set / array (any element type) | `CONTAINS IS NULL IS NOT NULL` | No `<`/`>`/`=` against the whole set in v0 — a set has no natural order and set-equality is a scan-worthy operation, not an index-friendly one. `CONTAINS` checks single-element membership and can be indexed (see §6). |
| counter | `= != < <= > >=` (against merged numeric value) | `IN`/`IS NULL` generally not meaningful; omitted rather than banned. |
| rga-text | `= != LIKE IS NULL IS NOT NULL` | `<`/`>` deliberately excluded — lexicographic ordering on long collaboratively-edited text is rarely what anyone wants and invites accidental full scans. Use `LIKE` or (future) full-text search. |

Using an operator against an incompatible type is a compile-time error when the type is statically known, and a runtime error otherwise (fails the row, does not silently coerce).

Per §4.3, any operator applied to a missing field evaluates to `NULL` and is excluded by `WHERE`'s three-valued logic — this applies uniformly regardless of which operator table row is in play.

### 3.8 Ordering, limits

```
ORDER BY n.start_time ASC
LIMIT 50
SKIP 100
```

`ORDER BY` on an unindexed field emits a compile-time warning; the query still runs but the planner logs it in `field_stats` so it can be considered for promotion later.

### 3.9 The "SCAN" opt-in

Predicates against non-indexed, non-promoted fields require an explicit `SCAN` marker:

```
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("com.example.event")
  AND SCAN(n.notes LIKE "%important%")
RETURN n
```

Without `SCAN`, a compile error names the field and suggests either adding an index or wrapping in `SCAN`. This is deliberate friction — expensive queries should look expensive.

### 3.10 Result shape

`RETURN` produces rows. Each row's columns are named by their expression (or a `AS` alias):

```
RETURN n.title AS title, n.start_time AS start
```

Whole-node returns (`RETURN n`) yield a materialized node object with all fields the caller has capability to read.

## 4. Semantics

### 4.1 Evaluation order (logical)

For each `MATCH` clause, in order:
1. **Space + capability filter.** Restrict candidate nodes to those in the named spaces where the caller has read capability. Non-negotiable; happens before anything else.
2. **Schema conformance filter.** Restrict to nodes conforming to the named schemas at required version constraints. Uses the `node_schema_conformance` table.
3. **Indexed field predicates.** Push down predicates on promoted / indexed fields.
4. **Presence predicates.** `HAS_FIELD` filters via `field_presence`.
5. **Unindexed / `SCAN` predicates.** Applied last, over the smallest possible candidate set.
6. **Reference traversal.** Follow `-[:REF(...)]->` edges, re-applying capability + schema filters on the target.
7. **Projection.** Materialize the requested fields, re-checking read capability per field-namespace pair.

The planner may reorder these when equivalent (e.g., start from `field_presence` if it's more selective than the space filter), but the logical semantics are as above.

### 4.2 Capability semantics

- Node-level capabilities are per-space; a node is either readable or not, based on the caller's membership.
- Field-level capabilities are per (app, field-path) grants. If the query is being run by an app, projected fields must be in the app's capability grants; if not, they are silently excluded from the result (not errored). If the query is run by a user directly, no field-level filter applies.
- Ref traversal into a space the caller can't read yields no rows for that path; not an error.

### 4.3 Missing-field semantics

Reading a field the node doesn't have returns `NULL`, not an error. Predicates on missing fields evaluate to `NULL` (SQL three-valued logic), which is filtered out by `WHERE`. This means `WHERE n.foo = 5` excludes both "foo is 3" and "foo is absent" — as expected.

### 4.4 Type awareness

- Field values are coerced according to their declared type in the schema (if the schema is present in the query via `CONFORMS TO`) or according to the value envelope's `t` tag otherwise.
- Comparisons across incompatible types (`n.start_time < 5` where `start_time` is a timestamp) are compile-time errors when the schema is known, runtime errors otherwise.
- CRDT-typed fields compare against their merged value unless a view selector says otherwise.

## 5. Query IR

The surface language compiles to a small IR. Each IR node is a stage in the evaluation order; the compiler emits them in order and the planner may rewrite for physical execution.

```
IR ::=
  | Source(space_ids, capability_ctx)
  | ConformanceFilter(schema_id, version_pred)
  | IndexedPredicate(field_path, op, value)
  | PresencePredicate(field_path)
  | ScanPredicate(field_path, op, value)
  | RefTraverse(edge_type, min_depth, max_depth, direction)
  | Project(fields)
  | OrderBy(field_path, dir)
  | Limit(n)
  | Skip(n)

op ::= EQ | NEQ | LT | LTE | GT | GTE | CONTAINS | IN_SET | LIKE | IS_NULL | IS_NOT_NULL
```

`op` is validated against the type-compatibility table in §3.7 at IR construction time — an `IndexedPredicate` or `ScanPredicate` with an invalid (type, op) pair never reaches the planner.

Each IR node carries annotations (estimated cardinality, cost, whether an index is available). This is the layer that gets traced/profiled per query-id.

## 6. Meta Table Management

The physical layout is opaque to the language; the compiler consults a set of meta tables to plan every query.

### 6.1 Meta tables

```
schema_tables
  schema_id            UUID PRIMARY KEY
  physical_table_name  TEXT              -- e.g. schema_data_abc123
  field_mappings       JSONB             -- { logical_field → {column, type, indexed?} }
  storage_mode         ENUM('promoted','jsonb','hybrid')
  migration_state      ENUM('stable','migrating','deprecated')
  created_at, updated_at

managed_indexes
  index_id             UUID PRIMARY KEY
  target_schema_id     UUID NULL
  target_field         TEXT
  index_type           TEXT              -- 'btree' | 'unique' | 'fts' | 'expr_jsonb'
  physical_index_name  TEXT
  status               ENUM('building','ready','stale','dropped')
  created_at

field_presence
  ns_id                INT
  field_name           TEXT
  node_id              UUID
  value_type           TEXT              -- optional but recommended
  PRIMARY KEY (ns_id, field_name, node_id)
  INDEX (node_id)

node_schema_conformance
  node_id              UUID
  schema_id            UUID
  version_major        INT
  version_minor        INT
  PRIMARY KEY (node_id, schema_id)
  INDEX (schema_id, version_major, node_id)

namespaces
  ns_id                INT PRIMARY KEY
  kind                 ENUM('system','user','app')
  app_id               UUID NULL         -- for kind='app'
  stable_identifier    TEXT              -- e.g. "com.example.calendar"

field_stats
  ns_id, field_name    -- key
  read_count           BIGINT
  write_count          BIGINT
  scan_count           BIGINT            -- how often SCAN'd
  order_by_count       BIGINT
  updated_at
```

### 6.2 Meta write invariants

- Every field write updates `field_presence` in the same transaction. No exceptions.
- Every schema conformance change updates `node_schema_conformance` in the same transaction.
- `field_stats` is updated periodically (batched), not per-op. Approximate counts are fine.
- `managed_indexes.status` transitions are the only way the planner learns about an index. An index in `building` state is not used for planning.

### 6.3 Meta tables are queryable

The same query language can be pointed at meta tables via a system namespace (`system.schema_tables`, `system.managed_indexes`, etc.), subject to admin capability. This is how tooling introspects the storage layout without a separate API.

### 6.4 Index control surface

Index management is a **meta-mutation**, not a data mutation, and not part of the read-only query language in §3. It has two entry points:

**Declared at schema-install time.** An app's `TableDeclaration` marks a field `indexed: true` (per the original `FieldDefinition` shape). On install, this creates a `managed_indexes` row in `building` state and kicks off async construction. This is the common case — most indexes should be declared by the app that knows its own access patterns, not discovered later.

**Ad-hoc admin/dev statements**, for cases the app didn't anticipate or for fields on ad-hoc/unschemed data:

```
CREATE INDEX ON schema("com.example.event").start_time;
DROP INDEX ON schema("com.example.event").start_time;
PROMOTE FIELD schema("com.example.event").notes;
```

`CREATE INDEX` inserts a `managed_indexes` row (`status = 'building'`) and returns immediately; the index is not used by the planner until `status = 'ready'`. `PROMOTE FIELD` is a separate, heavier operation — it moves a field out of JSONB into a typed column on the schema-shadow table (dual-write migration per the earlier storage design), which is a prerequisite for the cheapest index form but not required for `CREATE INDEX` itself (a JSONB expression index works without promotion, just at higher cost).

These statements require an admin/schema-owner capability — they are not available to arbitrary apps, since an unindexed field becoming indexed changes write-path cost for everyone writing that field.

### 6.5 Feedback loop: field_stats → index suggestions

Tracking scan/order-by frequency per field (the `field_stats` table in §6.1) is the right mechanism for deciding what to index — the alternative is guessing at install time and never revisiting it, which is how systems end up either under-indexed (slow queries) or over-indexed (write amplification for indexes nobody uses).

Two things matter for making this actually useful rather than noisy:

**Windowed, not lifetime, counters.** `field_stats` should be maintained as rolling counts over a trailing window (e.g., hourly buckets rolled up to a 7- or 30-day view), not a single running total. A one-time bulk import that hammers `SCAN` on a field shouldn't permanently bias the advisor toward indexing something that's never touched again after import day.

**Suggest, don't auto-apply.** A periodic background job reads `field_stats`, and for any (schema, field) pair where `scan_count` or `order_by_count` in the trailing window exceeds a threshold, it surfaces a suggestion — e.g., writes a system-namespaced node the admin tooling can query (`system.index_suggestions`), or fires a reactor notification. It does **not** call `CREATE INDEX` itself by default. Index creation has a real write-path cost, and that trade-off should be a decision the schema owner makes, not something that happens silently in the background. An explicit `auto_apply: true` flag per app/schema can opt into automatic creation later, once you trust the thresholds — but that shouldn't be the default.

## 7. Two-Phase Compilation

### 7.1 Phase 1: meta lookup

The compiler resolves everything that requires knowing physical layout:
- Resolve namespace identifiers to `ns_id`.
- Resolve schema identifiers to `schema_id`, look up `physical_table_name` and `field_mappings`.
- Look up `managed_indexes` matching each field predicate.
- Determine, per field, whether it lives in a promoted column, an indexed JSONB expression, or unpromoted JSONB.

All of this is a small number of indexed lookups against meta tables, and all of it is cachable per (schema_id, version) and per (ns_id, field_name).

### 7.2 Phase 2: SQL generation

The compiler emits a single SQLite statement using CTEs, one per IR stage where useful, with the physical table names and column names baked in.

Example — the query
```
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("com.example.event")
  AND n.start_time > "2026-01-01"
  AND HAS_FIELD(n, "com.example.event", "attendees")
RETURN n.title, n.start_time
LIMIT 100
```
compiles (roughly) to:

```sql
WITH space_nodes AS (
  SELECT id FROM nodes WHERE space_id = ?    -- personal space id
),
conforming AS (
  SELECT c.node_id
  FROM node_schema_conformance c
  JOIN space_nodes sn ON sn.id = c.node_id
  WHERE c.schema_id = ?                       -- event schema id
),
with_attendees AS (
  SELECT node_id FROM field_presence
  WHERE ns_id = ? AND field_name = 'attendees'
    AND node_id IN (SELECT node_id FROM conforming)
)
SELECT sd.title, sd.start_time
FROM schema_data_abc123 sd                    -- physical table for event schema
JOIN with_attendees wa ON wa.node_id = sd.node_id
WHERE sd.start_time > ?
LIMIT 100;
```

Notes:
- `title` and `start_time` are promoted columns on `schema_data_abc123`.
- The `WHERE start_time > ?` gets pushed all the way down by SQLite once the joins are visible.
- Only one round trip.

### 7.3 Prepared statement cache

The compiler caches prepared SQLite statements keyed on:
- The IR shape (query structure minus parameter values).
- The physical table names resolved from meta.

Cache invalidation triggers:
- Schema migration bumps `physical_table_name` or `field_mappings`.
- `managed_indexes` status change.
- Field promotion (JSONB → column).

An LRU with modest size (a few hundred entries) covers most workloads.

### 7.4 Fallback for unpromoted fields

For a field predicate against a field that isn't in a promoted column and doesn't have an expression index, the compiler emits `json_extract(sd.jsonb_data, '$."ns"."field"') = ?` and increments `field_stats.scan_count`. If a `SCAN` marker wasn't present in the source, this is a compile error.

## 8. Physical Execution Sketch

- Every query is assigned a `query_id` (UUID) at parse time. Propagates through IR annotations, the emitted SQL (as a comment for debugging), and any traces.
- Query enters SQLite as one prepared statement (or a small handful if meta needed batched lookup).
- Per-stage timing captured via IR node annotations, either always-on counters or sampled deep traces.
- Result rows are streamed back, capability-filtered on the way out for any late-bound checks (mostly a no-op since space/schema filters happened up front).

## 9. Deferred to Future Versions

Explicit list of things this doc does not address, so nobody thinks they're implicitly designed:

- Aggregation (`COUNT`, `SUM`, `GROUP BY`, window functions).
- Unbounded and shortest-path traversal.
- Full-text search operators (mentioned; separate design).
- Multi-CTE optimizations for very large queries.
- Materialized-view integration for deferred computed fields as query sources.
- Subqueries and `EXISTS`.
- User-defined predicate functions (would require capability grant and WASM).
- Write/mutation surface.

## 10. Open Questions

- **Namespace shorthand collisions.** If `CONFORMS TO` is used with two schemas that both define `title`, does bare `n.title` error or pick one? Suggested: error, force qualification.
- **`RETURN n` for nodes with fields the caller can't read.** Return with omitted fields, or error? Suggested: omit silently, but expose the omission via a `system.hidden_fields` sidecar column for tools that care.
- **`ORDER BY` with a `LIMIT` on unindexed fields.** Should this be blocked entirely, or allowed with a warning? Depends on how strict you want the "expensive things look expensive" rule to be.