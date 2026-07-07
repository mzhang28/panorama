# Query Language Correctness Properties

Properties of the Panorama Query Language that can be checked with Rust property-based tests (`proptest`). These are drawn from techniques that have found real bugs in production databases: SQLancer's PQS/TLP/NoREC oracles (Rigger & Su, 2019–2020), Limbo's deterministic simulator, FoundationDB's simulation framework, and SQLite's differential test harness.

**Architecture:** PQL text → `parse_query` → AST → `compile` → SQL + params → SQLite execution. The compiler is where bugs live — manual `pi` tracking across recursive predicate compilation, json_extract key formatting, CTE wiring.

---

## 1. Reference Evaluator (Differential Oracle)

**Inspiration:** Pivoted Query Synthesis (Rigger & Su, OSDI 2020), SQLite's SLT (SQL Logic Test), Limbo simulator

**The property:** Evaluating a predicate tree directly in Rust against in-memory `Node` data must produce the same row set as compiling it to SQL and executing against the same data in SQLite.

**Why it finds bugs:** This catches every class of compiler error — wrong `json_extract` key format, CmpOp swapped in SQL emission, `IsNull` using wrong `json_type` check, `ConformsTo` semi-join missing rows, `HasField` ns_id resolution errors, `In` generating wrong placeholder count, `Like` pattern not escaped, `And`/`Or` parenthesization wrong. If Rust says "row 3 matches" and SQLite says "row 3 doesn't match," you have a bug.

**Proptest strategy:**
1. Generate random `Vec<Node>` with varied fields (String, Integer, Float, Boolean, Null across multiple namespaces)
2. Generate random `Predicate` trees that reference the actual field names/values present in the generated nodes
3. Evaluate predicate in Rust: `fn eval(pred: &Predicate, node: &Node) -> bool`
4. Insert nodes into SQLite, compile `SELECT * FROM nodes WHERE <pred>` via the compiler, execute
5. Assert: `rust_matches.sort() == sqlite_matches.sort()`

**Bootstrap path:** Start with `FieldCompare` + `And` only (simplest). Add `Or`, `Not`, `IsNull`, `In`, `Like`, `ConformsTo`, `HasField` one at a time. Every new predicate variant will find at least one bug in its first run.

**Key design decision — pivot-aware generation:** Instead of purely random predicates (which mostly evaluate to FALSE and test nothing), bias generation toward predicates that match at least one row. For each generated `FieldCompare`, pick the `value` from an actual field on an existing node with 50% probability. This is the core insight from PQS — the pivot row guarantees the query is non-empty, so you're testing correctness of matching, not just correct emptiness.

---

## 2. Ternary Logic Partitioning (TLP)

**Inspiration:** TLP oracle from SQLancer (Rigger & Su, OOPSLA 2020). Found 175+ bugs across 8 DBMSs.

**The property:** For any predicate `p` and any dataset, the row sets from `WHERE p`, `WHERE NOT p`, and `WHERE p IS NULL` must partition the full table — every row falls into exactly one bucket, and the union equals `SELECT *` with no WHERE.

```
SELECT * FROM t WHERE p
UNION ALL
SELECT * FROM t WHERE NOT p
UNION ALL
SELECT * FROM t WHERE (p) IS NULL
```
must have the same cardinality and content as `SELECT * FROM t`.

**Why it finds bugs:** Catches NULL-handling bugs in the compiler, incorrect `IS NULL`/`IS NOT NULL` emission, `Not` wrapping of compound predicates, and edge cases where the SQL NULL semantics diverge from the AST's intent. SQL's 3VL (NULL comparisons) interacting with `json_extract` (which returns SQL NULL for missing keys) is a rich source of bugs.

**Proptest strategy:**
1. For each predicate variant, generate the three partition queries
2. Execute all four queries (base + three partitions)
3. Assert: `base_rows.sorted() == (true_rows ++ false_rows ++ null_rows).sorted()`
4. Assert: the three partition row sets are mutually disjoint

**Panorama-specific edge:** `json_extract(n.fields_json, '$."key".value')` returns SQL NULL when the key doesn't exist. So `n.missing_field = "x"` is NULL, not FALSE. The compiler's `IsNull` uses `json_type(...) IS NULL` which treats missing keys and explicit nulls the same. TLP catches inconsistencies here.

---

## 3. CRUD Roundtrip Invariants

**Inspiration:** Limbo simulator property checks, FoundationDB's deterministic simulation

**The property:** Basic data operations are invertible — what you write is what you read.

| Operation | Property |
|-----------|----------|
| INSERT → SELECT | After inserting node with known fields, `WHERE n.ns.field = <value>` returns it |
| INSERT → RETURN * | `RETURN n` on a newly inserted node returns all its fields |
| UPDATE → SELECT | After updating field to new value, querying for old value returns empty, new value returns the node |
| DELETE → SELECT | After deleting node, same query returns empty |
| INSERT → COUNT | `RETURN n` row count increments by 1 per insert |

**Why it finds bugs:** The simplest possible end-to-end test that exercises the full pipeline: parse → compile → execute → deserialize. If these fail, nothing else matters. Also catches storage-layer bugs (field_presence sync, JSON serialization roundtrip).

**Proptest strategy:**
1. Generate random `Node` with random fields
2. Insert into storage
3. Query with `MATCH (n) IN space(...) WHERE n.ns.field = <value> RETURN n`
4. Assert result contains exactly the inserted node
5. Update field, query again, assert new value
6. Delete node, query again, assert empty

---

## 4. Compiler Parameter Integrity

**Inspiration:** SQLite's prepared statement verification, FoundationDB's deterministic replay

**The property:** The compiler manually tracks a `param_idx` counter (`pi`) across recursive `compile_predicate` calls. `And`/`Or` advance `pi` between left and right children. `HasField` conditionally consumes 1 or 2 params depending on wildcard namespace. `In` consumes `values.len()` params. The generated SQL's `?N` placeholders must exactly match `params` in count, indices, and order.

**Specific checks:**
- Every `?N` in SQL appears exactly once (`N` is unique)
- All indices `1..n` are present (no gaps)
- `params.len() == n`
- The SQL is preparable: `conn.prepare(&sql).is_ok()`
- `conn.query_row("SELECT count(*) FROM (" + &sql + ")", params).is_ok()` — executes without type error

**Why it finds bugs:** The param index tracking is the most fragile part of the compiler. A single off-by-one in `pi` tracking silently binds the wrong value to the wrong placeholder. The query executes, returns wrong results, no error. This is the kind of bug that fuzzing won't find (no crash) but property testing will (wrong answer).

**Proptest strategy:**
1. Generate random AST with varied predicate trees (deep nesting, all predicate types)
2. Compile
3. Extract all `?N` from SQL with regex
4. Verify indices are exactly `{1, 2, ..., params.len()}`
5. Prepare the SQL, bind params, verify no rusqlite error

---

## 5. AST Algebraic Simplifications

**Inspiration:** NoREC oracle (Rigger & Su, ESEC/FSE 2020), SQL optimizer verification

**The property:** Algebraically equivalent ASTs produce identical results.

| Simplification | Equivalence |
|----------------|-------------|
| Double negation | `Not(Not(p))` ≡ `p` |
| De Morgan | `Not(And(a, b))` ≡ `Or(Not(a), Not(b))` |
| De Morgan | `Not(Or(a, b))` ≡ `And(Not(a), Not(b))` |
| Idempotence | `And(p, p)` ≡ `p` |
| CmpOp negation | `Not(FieldCompare { op: Eq, .. })` ≡ `FieldCompare { op: Neq, .. }` |
| IsNull duality | `IsNull { not: true }` ≡ `Not(IsNull { not: false })` |

**Why it finds bugs:** These transformations exercise the compiler's `Not`, `And`, `Or` code paths in combination. A bug in how `Not` wraps a `FieldCompare` (should swap the CmpOp, not wrap in `NOT (...)`) or how `Not` distributes over `And` shows up as different results for equivalent queries.

**Proptest strategy:**
1. Generate random predicate tree
2. Apply simplification rules to produce an equivalent predicate
3. Compile both, execute against same data
4. Assert identical row sets

**Important:** Test this against non-empty data. Equivalent predicates that both match zero rows trivially pass (both return empty). Use the pivot-aware generation from Strategy 1 to ensure at least one row is in play.

---

## How These Compare

| Strategy | Finds compiler bugs | Finds NULL bugs | Finds optimizer bugs | Finds param bugs | Effort to implement |
|----------|---------------------|-----------------|----------------------|------------------|---------------------|
| 1. Reference eval | ✓ all | ✓ | — | — | High (build Rust evaluator) |
| 2. TLP | ✓ | ✓✓✓ | — | — | Medium |
| 3. CRUD roundtrip | ✓ (end-to-end) | — | — | ✓ | Low |
| 4. Param integrity | — | — | — | ✓✓✓ | Low |
| 5. Algebraic simplification | ✓ (Not/And/Or) | ✓ | — | — | Low-Medium |

## Implementation Order

1. **Param integrity** (1 hour) — easiest, finds the scariest bugs (silent param corruption)
2. **CRUD roundtrip** (2 hours) — exercises full pipeline, prerequisite for everything else
3. **TLP** (3 hours) — highest bug-finding ROI, especially for NULL edge cases
4. **Algebraic simplification** (2 hours) — leverages TLP infrastructure, tests compiler's boolean logic
5. **Reference evaluator** (1 day) — the gold standard, catches everything the others miss

## Parser Fuzzing (Already Exists)

`fuzz/src/bin/fuzz_pql.rs` already runs AFL against `parse_query`. This catches panics and hangs but cannot detect logic bugs (wrong answers). Keep it, but it's complementary — the properties above catch correctness, not crashes.
