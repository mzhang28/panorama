---
title: PQL Data Model
description: Understanding the underlying graph and node representation in the Panorama Query Language.
---

PQL is designed specifically to query the Panorama node storage layer. This document describes how the platform's data models map to the query language.

---

## 1. Node Scoping & Spaces

All nodes in storage belong to exactly one partition called a **Space**. 

In PQL, data isolation is enforced at the language layer:
*   There is no query that implicitly scans across multiple spaces.
*   Every `MATCH` clause **requires** an explicit `IN space(...)` specifier.
*   Cross-space queries are unsupported within a single `MATCH` block; each space boundary must be declared and is evaluated independently for security.

```cypher
/* Querying the "personal" space */
MATCH (n) IN space("personal")
RETURN n
```

---

## 2. Namespaced Field Addressing

Unlike typical graph databases where fields are simple keys (e.g. `n.name`), Panorama requires fields to be addressed through their namespace to ensure distinct apps do not overwrite or collide on keys.

A field path is written as:
`node.<namespace>.<field_name>`

*   **System Namespace**: `n.system.created_at` or `n.system.node_title`.
*   **User Namespace**: `n.user.my_custom_field`.
*   **App Namespace**: Namespaces containing special characters (like dots) must be double-quoted, e.g. `n."io.mzhang.panorama.journal".content`.

```cypher
MATCH (n) IN space("personal")
WHERE n.system.node_title = "My Entry"
  AND n."io.mzhang.panorama.journal".content CONTAINS "learning"
RETURN n
```

---

## 3. Schema Conformance

Nodes can declare compliance with zero or more schemas. PQL allows filtering nodes based on their schema conformance using the `CONFORMS TO` operator:

```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.journal/JournalEntry")
RETURN n
```

### Version Constraints
You can restrict results to specific version ranges of a schema:

```cypher
/* Matches schema major version 2 or newer */
WHERE n CONFORMS TO schema("com.example.event", >=2)

/* Matches schema major versions between 2 and 4 inclusive */
WHERE n CONFORMS TO schema("com.example.event", 2..4)
```

### Namespace Shorthand Resolution
When a `CONFORMS TO schema("namespace")` clause is evaluated, you can omit the namespace prefix for field names belonging to that schema. The compiler automatically resolves them:

```cypher
MATCH (n) IN space("personal")
WHERE n CONFORMS TO schema("io.mzhang.panorama.journal")
  /* Automatically resolves to n."io.mzhang.panorama.journal".content */
  AND n.content LIKE "%work%" 
RETURN n
```
