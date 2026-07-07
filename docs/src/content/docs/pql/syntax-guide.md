---
title: PQL Syntax & Usage Guide
description: Syntax documentation and query examples for the Panorama Query Language (PQL).
---

The **Panorama Query Language (PQL)** is a Cypher-flavored query language designed to traverse the platform's node-reference graph.

---

## 1. Syntax Overview & Scoping

Every PQL query must satisfy these structural requirements:
*   A `MATCH` clause identifying nodes and edges.
*   An explicit space boundary `IN space(...)` enclosing the match.
*   An optional `WHERE` clause containing filter predicates.
*   A `RETURN` clause specifying projected fields or whole nodes.

---

## 2. Basic Query Examples

### Match Nodes in Space
Select all nodes in the `"personal"` space:
```cypher
MATCH (n) IN space("personal")
RETURN n
```

### Schema Conformance
Filter nodes that conform to a specific schema:
```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.journal/JournalEntry")
RETURN n
```

### Namespaced Fields
Read and compare fields under specific namespaces:
```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.coding/Heartbeat")
  AND n."io.mzhang.panorama.coding".project = "panorama"
  AND n.system.node_time >= "2026-01-01T00:00:00Z"
RETURN n."io.mzhang.panorama.coding".entity, n.system.node_time
```

### Checking Field Presence
Filter nodes that contain a specific field namespace and key using `HAS_FIELD`:
```cypher
MATCH (n) IN space("personal")
WHERE HAS_FIELD(n, "io.mzhang.panorama.journal", "content")
RETURN n
```

---

## 3. Reference Graph Traversal

Nodes link to other nodes using reference attributes (`Ref` type). PQL supports Cypher-style path matching.

### Single-Hop Relationship
Match an event and its associated attendee:
```cypher
MATCH (a)-[:REF("attendee")]->(b) IN space("personal")
WHERE a CONFORMS TO schema("io.mzhang.panorama.trips/Event")
  AND b CONFORMS TO schema("io.mzhang.panorama.trips/Person")
RETURN a.system.node_title AS event_name, b.system.node_title AS attendee_name
```

### Bounded Multi-Hop Relationship
Find relationships up to three hops away (depth bounds are mandatory; unbounded `*` hops throw compilation errors):
```cypher
MATCH (a)-[:REF("parent")*1..3]->(b) IN space("personal")
RETURN a, b
```

---

## 4. The `SCAN` Opt-In

To maintain database performance, Panorama restricts queries from scanning unindexed JSON fields by default. 

*   Predicates on fields that are not indexed or promoted to dedicated columns **must** be wrapped inside the `SCAN(...)` function.
*   Omitting `SCAN` on unindexed columns throws a compilation error.

```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.journal/JournalEntry")
  /* notes is unindexed; SCAN is required */
  AND SCAN(n.notes LIKE "%urgent%") 
RETURN n
```

---

## 5. Operators & Types

PQL supports the following operators:
`=`, `!=`, `<`, `<=`, `>`, `>=`, `IN [...]`, `LIKE`, `CONTAINS`, `IS NULL`, `IS NOT NULL`, `AND`, `OR`, `NOT`.

| Field Type | Permitted Operators | Notes |
|---|---|---|
| **String** | `=`, `!=`, `<`, `<=`, `>`, `>=`, `LIKE`, `IN`, `IS NULL`, `IS NOT NULL` | Lexicographical comparisons are byte-wise. |
| **Number** | `=`, `!=`, `<`, `<=`, `>`, `>=`, `IN`, `IS NULL`, `IS NOT NULL` | Numeric values are compared as Floats/Integers. |
| **Boolean**| `=`, `!=`, `IS NULL`, `IS NOT NULL` | No ordering operators (`<`, `>`). |
| **Ref** | `=`, `!=`, `IS NULL`, `IS NOT NULL` | Equality compares UUID strings. |
| **Timestamp**| `=`, `!=`, `<`, `<=`, `>`, `>=`, `IS NULL`, `IS NOT NULL` | ISO 8601 string-date comparisons. |
| **Arrays / Sets**| `CONTAINS`, `IS NULL`, `IS NOT NULL` | `CONTAINS` checks single-element presence. |

---

## 6. Ordering, Limits & Pagination

Use `ORDER BY`, `LIMIT`, and `SKIP` to control result lists:

```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.journal/JournalEntry")
ORDER BY n.system.created_at DESC
LIMIT 20
SKIP 40
```
