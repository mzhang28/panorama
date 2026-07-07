---
title: PQL Execution & Compiler Engine
description: Behind the scenes of how PQL parses, plans, compiles, caches, and executes queries.
---

PQL queries do not run as interpreted graphs. Instead, they are compiled through a **Two-Phase Compilation Engine** into highly optimized SQLite SQL statements.

This document details the compilation stages, prepared statement caching, and physical database execution.

---

## 1. Compilation Phases

### Phase 1: Meta Lookup & Scoping
When a PQL string is submitted, the compiler parses it into an Abstract Syntax Tree (AST) and performs metadata validation:
1.  **Space Scoping**: Resolves the string identifier in `IN space("name")` to its internal `space_id` UUID and verifies user permissions.
2.  **Namespace Mapping**: Maps logical namespaces to internal IDs (`ns_id`).
3.  **Schema Check**: Looks up the physical table representations of target schemas in the system `schema_tables` database.
4.  **Index Resolution**: Consults `managed_indexes` to identify if any queried fields are indexed (either as dedicated columns or JSONB expression paths).

### Phase 2: SQL Generation
The compiler takes the parsed AST and resolved metadata, then generates a single, complex SQL statement featuring Common Table Expressions (CTEs) to isolate spaces and check schema conformance:

```
PQL AST ────────► [Phase 1: Meta Lookup] ────────► [Phase 2: SQL Gen] ────────► SQL Query
```

#### SQL Compilation Example
Consider this PQL query:
```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.journal/JournalEntry")
  AND n.content LIKE "%work%"
RETURN n.content
LIMIT 10
```

The compilation engine resolves the schema to the physical table `schema_data_journal` (where `content` is a promoted column) and emits the following SQLite query:

```sql
WITH space_nodes AS (
  /* Step 1: Filter nodes by space partition */
  SELECT id FROM nodes WHERE space_id = ?
),
conforming_nodes AS (
  /* Step 2: Filter by schema conformance using metadata index */
  SELECT c.node_id
  FROM node_schema_conformance c
  JOIN space_nodes sn ON sn.id = c.node_id
  WHERE c.schema_id = ?
)
/* Step 3: Fetch columns from the physical table */
SELECT sd.content
FROM schema_data_journal sd
JOIN conforming_nodes cn ON cn.node_id = sd.node_id
WHERE sd.content LIKE ?
LIMIT 10;
```

---

## 2. Prepared Statement Cache

Compiling PQL to SQL and parsing it in SQLite has overhead. To maintain high performance, the compiler maintains a thread-safe **Prepared Statement LRU Cache** (default capacity: 256 entries):

*   **Cache Keys**: Generated from the structural shape of the PQL query AST (ignoring parameter literal values like space names or filter targets).
*   **Cache Eviction**: The cache is automatically invalidated when changes to the metadata catalog occur, such as:
    *   A schema is modified or migrated (`schema_tables` updates).
    *   A field is promoted to a dedicated column.
    *   An index is registered, rebuilt, or dropped.

---

## 3. Physical Storage Execution

At the database storage layer:
1.  **WAL Mode**: SQLite is configured in Write-Ahead Log (WAL) mode to permit concurrent reads and writes.
2.  **JSON Extract Fallback**: For unindexed/unpromoted fields (queries wrapped in `SCAN(...)`), the compiler emits SQLite `json_extract()` operations against the node's JSON fields column.
3.  **Cardinality Estimation**: The planner uses indices on `node_schema_conformance(schema_id, node_id)` and `field_presence(ns_id, field_name, node_id)` to quickly filter down candidate sets before applying unindexed JSON scans.
