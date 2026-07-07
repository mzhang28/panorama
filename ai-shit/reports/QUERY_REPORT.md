# Query Subsystem Evaluation — Round 2 Independent Re-Audit

## 0. Method

Per instruction, this is a from-scratch audit: `design/QUERY_DESIGN.md` against the current
`crates/panorama-core/src/query/*` and `crates/panorama-server/src/query/*`, ignoring both
`PROGRESS.md` and the `QUERY_REPORT.md` already in the tree (which was this same auditor's
own prior-round report — not trusted as a starting point; every claim below was re-derived
from current source, and several were verified by actually compiling and executing SQL, not
just reading code). No git commands were run. There was concurrent, unrelated development
happening on parts of the codebase during this audit (confirmed by the user) — a transient
compile error appeared and resolved itself in `query/compiler.rs` at one point; per
instruction it was ignored rather than investigated, and all live-test evidence below was
captured once the crate compiled cleanly.

**This round looks substantially different from round 1 on paper.** Every file in
`crates/panorama-core/src/query/` and `crates/panorama-server/src/query/` grew significantly
(e.g. `compiler.rs` 734→960 lines, `ir.rs` 122→377 lines, `eval.rs` 552→611 lines), and several
of round 1's specific findings are now genuinely fixed with real logic behind them — this
section gives them credit precisely. But the design's flagship relational feature —
reference traversal — is not just still broken, it is now *verifiably* broken: I compiled and
executed the design document's own simplest canonical traversal example against real SQLite
and it fails outright. That result, and why zero tests caught it despite a substantial
rewrite of the traversal code, is the headline of this round.

## 1. What's genuinely fixed since round 1 (independently re-verified, not taken on the old report's word)

### 1.1 CRDT view selectors now correctly error at compile time (was: silently ignored)
Round 1 found `@ops`/`@at(...)` parsed into the AST and then had zero effect on compiled SQL —
directly contradicting §3.6's "downstream operators that don't understand op streams will
error at compile time." Current `compiler.rs::compile_field_access` (`:432-444`) and
`field_sql_expr` (`:677-685`) now explicitly match `CrdtView::Ops`/`CrdtView::At` and return
`Err(...)` before falling through to normal field compilation. Verified with a real test
(`test_crdt_ops_rejected`, `compiler.rs:938-948`) that compiles `n.event.attendees@ops` and
asserts the compile fails mentioning `@ops`. I re-ran it; it passes. `@merged` (the default)
is unaffected and still compiles normally. This is a real, narrow, correct fix.

### 1.2 `CONTAINS` now exists (was: absent from the entire pipeline)
Round 1 found no `CONTAINS` operator anywhere — not in the AST, parser, compiler, or
evaluator — despite §3.7 naming it the *only* valid non-null operator for arrays/or-sets.
`Predicate::Contains` now exists and is parsed, and `compiler.rs:610-629` compiles it to a
`LIKE` check against the JSON-serialized array text (e.g. `%"urgent"%` for a string element) —
a reasonable, explicitly-labeled v0 shortcut ("For v0, CONTAINS compiles to a LIKE check on
the JSON array representation"). `eval.rs` also now implements `Predicate::Contains`
(`:236`+) with matching semantics. Verified by `test_contains_compiles`
(`compiler.rs:950-959`); I re-ran it, it passes.

### 1.3 The operator/type validity matrix (§3.7) is now partially real (was: entirely unenforced)
Round 1 found zero type-checking anywhere, evidenced concretely by the reference evaluator
itself implementing `<`/`>` for booleans — an operator/type pairing the design explicitly
forbids. There is now a real `check_type_validity` function (`compiler.rs:720-750`), wired in
for `Predicate::FieldCompare` when the field's type is statically known via a resolved schema
(`ctx.field_type_for`, `compiler.rs:530-533`). It correctly restricts `Boolean`/`NodeRef` to
`Eq`/`Neq` only (no ordering) — the specific violation round 1 called out is fixed for typed
fields. This is genuine, verified progress, not cosmetic. See §2.6 for what's still incomplete
about it.

### 1.4 `field_stats.scan_count` now has a real producer (was: recording functions defined but never called)
Round 1 found `MetaStore::record_field_scan`/`record_field_read`/`record_field_write` existed
but had zero call sites outside their own tests. `compiler.rs` now has `record_scan_stat`
(`:754-`), called from the `Predicate::Scan` compilation arm (`:518`), which calls
`MetaStore::record_field_scan` for the field being scanned. So a real `SCAN(...)` query
compiled today does increment a real counter. This is genuine, though see §2.7 — the
counters still aren't windowed and still have no consumer, so the fix closes the producer
half of §6.5's loop, not the whole loop.

### 1.5 The prepared-statement cache is now actually instantiated and consulted (was: dead code)
Round 1 found `StatementCache` was never constructed anywhere reachable from the running
server. It now is: `storage/sqlite.rs` holds a `statement_cache: StatementCache` field
(`:50`), constructed in `SqliteBackend::new` (`:96`), and `query()` (`:189-223`) actually
checks it before compiling and inserts into it after. This means a query with byte-identical
source text repeated twice now genuinely skips re-parsing and re-compiling on the second call
— a real behavior change, verified by reading the control flow (cache hit returns early at
`:200-206` before `parse_query`/`compile` are even called). See §2.8 for the specific way
this still falls short of §7.3's actual requirement.

## 2. What's still broken, newly found broken, or unfixed

### 2.1 Reference traversal (§3.5): the design's own simplest example now fails outright — empirically verified, not inferred
This is the headline finding. I temporarily added a test to `compiler.rs`'s own test module
that compiles and *executes* the design document's canonical single-hop traversal example —
even the simplified form with no WHERE clause at all:

```
MATCH (a)-[:REF("attendee")]->(b) IN space("personal") RETURN a.title, b.name LIMIT 100
```

Compilation succeeds and produces:

```sql
WITH _traverse_b AS (SELECT t.* FROM nodes t JOIN _match_a s ON t.id = json_extract(s.fields_json, '$."attendee".value') WHERE t.space_id = ?1)
SELECT json_extract(fields_json, '$."title".value') AS title, json_extract(fields_json, '$."name".value') AS name FROM _match_a, _traverse_b LIMIT ?2
```

Executing this against a real SQLite connection fails immediately:
`SqliteFailure(..., Some("no such table: _match_a"))`. I ran this myself, saw the failure,
then reverted the temporary test (it is not left in the tree). The bug: for a `MatchClause`
whose `source` is `RefTraverse`, the compiler's `source_cte` for the traversal's *own* source
variable (`"a"`) is computed as `ctx.var_cte.get(&mc.variable).unwrap_or_else(|| format!("_match_{}", mc.variable))`
(`compiler.rs:192-196`) — a lookup that is never populated for a bare
`MATCH (a)-[...]->(b) IN space(...)` (there is no separate `MatchSource::Space` clause for
`"a"` in this syntax to populate it), so it silently falls back to a placeholder string that
is never actually defined as a CTE anywhere in the emitted SQL, and both the join and the
final `FROM` clause reference it as if it existed.

**Root cause, precisely, and why it is not a matter of missing schema/promotion setup.** It's
worth ruling this out explicitly, since it's the natural first hypothesis: is `_match_a`
missing because `"a"` wasn't `CONFORMS TO`'d to a schema, or because some field wasn't
`PROMOTE FIELD`'d (§6.4)? No. The *only* code path anywhere in `compile()` that ever pushes a
base, space-filtered CTE (`_match_{var}_space AS (SELECT n.* FROM nodes n WHERE n.space_id =
?)`) into the `ctes` list and registers it into `ctx.var_cte` is the
`MatchSource::Space(space_name) => { ... }` arm (`compiler.rs:119-183`). That arm — and with
it, all of the schema/conformance/promotion machinery in `compiler.rs:141-169` that consumes
`ctx.resolved_schemas` — only runs when a `MatchClause`'s `source` is `MatchSource::Space`.
For `MATCH (a)-[:REF("attendee")]->(b) IN space("personal")`, the parser produces exactly
**one** `MatchClause` (`variable: "a"`, `source: MatchSource::RefTraverse{...}`) — there is no
second, independent `MatchClause` for `"a"` as a plain `Space` match anywhere in this AST
shape. So the `Space` arm never executes for `"a"` at all, and nothing else in `compile()`
ever builds `"a"`'s base CTE either — the `RefTraverse` arm only ever builds hop CTEs for the
*target* variable (`"b"`'s side of the join), never a base CTE for its own source variable.
This holds regardless of whether `"a"` has a `CONFORMS TO` clause, a promoted column, a ready
index, or nothing at all — adding any of that would populate `ctx.resolved_schemas` for `"a"`,
but that data is only ever read from inside the `Space` arm's loop, which this `MatchClause`
never reaches. The fix has to be in the `RefTraverse` arm itself: it needs to construct its
own base space-filtered CTE for the source variable (mirroring what the `Space` arm does for
an ordinary match), rather than assuming — via an unconditional `.unwrap_or_else` fallback
that always fires here — that one already exists somewhere else in the plan.

This is a regression in severity, not just an unfixed gap. Round 1's finding was that
traversal queries ran but produced wrong results (WHERE clause silently dropped, RETURN
misattributed to the wrong table). Both of those specific bugs are now genuinely fixed — the
target-side WHERE clause is compiled (`compiler.rs:252-260`), and RETURN projection now
correctly tracks a separate CTE per variable (`compiler.rs:265-311`, "Each column may
reference a different variable... Determine the FROM table per-column"). That's real,
non-trivial rework, and it's evidence someone specifically tried to fix round 1's traversal
findings. But the rewrite never established a base CTE for the traversal's own source
variable, so the simplest possible traversal query — no WHERE clause, not even reachable via
the two bugs that got fixed — cannot execute at all. Every reference-traversal query in the
system is currently broken, verified by direct execution, not inference.

**Why this shipped despite substantial rework:** zero tests, in either round, compile *and
execute* a `RefTraverse` query. Confirmed by grep: `compiler.rs`'s own test module has no
`RefTraverse`/`REF(` test; `query_eval_proptest.rs` has none either — same as round 1,
unchanged. The two parser-level tests for traversal (`test_ref_traverse`,
`test_ref_traverse_multi_hop`) only assert on parsed AST shape and were already passing
before and after this rewrite, so they gave no signal either way. A person or agent doing
substantial, largely-correct-looking surgery on the WHERE-clause and RETURN-projection bugs
never ran the result against a database, and nothing in CI or the test suite would have
caught it — the same root cause named in round 1's report.

Separately, unchanged from round 1: multi-hop depth handling is now a real loop
(`compiler.rs:220-242`, one CTE per hop up to `max_depth`) rather than being silently ignored
— genuine additional progress not mentioned above — but this can't be verified to actually
produce correct multi-hop results given the base single-hop case doesn't execute. Reverse
traversal (`<-[:REF(...)]-`) is still entirely absent from the grammar — confirmed unchanged
by re-reading `parser.rs::ref_traverse`, still only recognizes `-[`.

### 2.2 The conformance-CTE naming collision from round 1 is unfixed, and is now more reachable
Round 1 found `extract_conforms_to` names each schema's conformance CTE using the *outer*
match-clause variable (`format!("_conforming_{}", variable)` where `variable` is always
`mc.variable`, e.g. `"a"`) rather than the CONFORMS TO predicate's own referenced variable
(`field`, which could be `"a"` or `"b"`). This is unchanged (`compiler.rs:401`, byte-identical
to round 1). Consequence: a WHERE clause with both `a CONFORMS TO X` and `b CONFORMS TO Y` in
one traversal clause produces two `ResolvedSchema` entries that both want the CTE name
`_conforming_a`; the second one is silently dropped by the `added_ctes.contains(...)` guard in
the main compile loop, so only one side's schema-conformance filtering actually reaches the
SQL. This was low-severity-but-latent in round 1 because the target-side WHERE clause wasn't
compiled at all yet (§2.1's predecessor bug made this moot). Now that target-side WHERE
compilation is wired in (§2.1), this bug is directly reachable by exactly the query shape the
design uses as its own canonical example. Not independently re-verified by execution this
round (traversal doesn't run at all per §2.1, so this bug can't currently manifest in
isolation) — flagged as a static-analysis finding with high confidence given the unchanged
code, worth re-checking once §2.1 is fixed.

### 2.3 The IR (§5) is now constructed — and then still thrown away
Round 1 found `ir.rs` was dead code, zero references outside itself. It's no longer
dead — `ir::lower_to_ir` is a real, reasonably complete AST→IR lowering function, and
`IrPlan::cache_key()` correctly hashes only structural shape, explicitly excluding literal
values, which is exactly §7.3's requirement ("cache keyed on the IR shape... minus parameter
values"). But: `compiler.rs::compile()` computes it and discards it immediately —
`let _ir = panorama_core::query::ir::lower_to_ir(query);` (`compiler.rs:96`), underscore-
prefixed, never read again anywhere in the function. Confirmed by grep: `.cache_key()` is
never called anywhere in `panorama-server` — the one method that would make this IR
construction actually useful for its stated purpose is never invoked. There's still no
`query_id`-attached per-stage cost/cardinality tracking anywhere either (§5, §8) — `query_id`
now exists and is threaded through to a SQL comment (`compiler.rs:368`, genuine, minor
progress toward §8), but nothing attaches per-stage annotations to it.

### 2.4 The statement cache is wired in — but still keyed exactly the way round 1 said was wrong, right next to code that would fix it
This is the specific, almost paradoxical finding: `cache.rs`'s own doc comment now correctly
states the requirement ("Keyed on the IR plan shape hash... rather than the raw source
string"), and `IrPlan::cache_key()` (§2.3) exists to do exactly that. But the actual call site
in `storage/sqlite.rs::query()` (`:194-199`) computes its cache key by hashing the raw `pql`
string directly, with a comment admitting it: *"Check statement cache using source hash...
TODO: upgrade to IR-shape key (§7.3) which would also share cache entries when only literal
values differ."* So the cache is real and does produce real hits now (§1.5) — but only for
byte-identical repeated query text, which is a real but narrower win than the design asks
for; two queries differing only in a literal (`WHERE n.foo = "bar"` vs `WHERE n.foo = "baz"`)
still miss the cache and recompile from scratch, exactly the case §7.3 is centrally about.
The fix for this is sitting unused one file over.

### 2.5 Index control surface (§6.4): still entirely absent
Re-checked by grepping the parser and compiler for `CREATE INDEX`, `DROP INDEX`,
`PROMOTE FIELD` — zero matches, unchanged from round 1. No admin/dev statement surface exists
for any of the three operations §6.4 specifies.

### 2.6 `field_stats` → index-suggestion feedback loop (§6.5): half-fixed at best
The producer side now works (§1.4). Still missing, re-verified this round: no windowing
(grepped `meta.rs` for `windowed`/`bucket`/`hour`/`trailing` — zero matches; the table is still
flat lifetime counters, contradicting §6.5's explicit "windowed, not lifetime, counters"
requirement) and no consumer (`system.index_suggestions` and any suggestion job: zero matches
anywhere in the tree, same as round 1).

### 2.7 The type-validity check (§1.3/§3.7) is real but has a specific, checkable bug and a scope gap
Two issues found reading `check_type_validity` closely (`compiler.rs:722-750`):
- It's only invoked from the `Predicate::FieldCompare` arm (`compiler.rs:530-533`). `In`,
  `Like`, `Contains`, and `IsNull` never call it. So e.g. `LIKE` against a `Boolean` field, or
  `CONTAINS` against a `String` field — both invalid per §3.7's table — compile without
  error. The design's own operator table is per-(type, operator), not per-(type,
  FieldCompare-only), so this is a real scope gap, not a design choice.
- It lumps `"String" | "DateTime" | "RgaText"` into one branch allowing
  `Lt`/`Lte`/`Gt`/`Gte` (`compiler.rs:728-731`). §3.7's own table excludes `<`/`>` for
  rga-text specifically, with an explicit rationale in the design doc ("deliberately
  excluded — lexicographic ordering on long collaboratively-edited text is rarely what
  anyone wants and invites accidental full scans"). The new code re-introduces exactly the
  kind of type/operator mismatch this feature exists to prevent, just for a different type
  than round 1's boolean example.
- Practically low-severity today: `"RgaText"`/`"Counter"`/`"OrSet"` type tags can only ever
  reach this function if some schema's field-mapping JSON declares that literal string as a
  field's `"type"`. Re-checking `panorama-core`'s `FieldTypeConstraint` enum (String, Integer,
  Float, Boolean, DateTime, Json, NodeRef, Array, ObjectRef — 9 variants, no CRDT types,
  unchanged from round 1) confirms no real schema can currently produce these tags. So this
  is a latent correctness bug in code that's currently unreachable by any real schema — the
  same "written to a spec with no real data behind it" pattern as round 1's CRDT-view
  finding, recurring in the new type-checking code specifically.
- No test exercises `check_type_validity` at all — not the boolean-ordering-rejected case
  round 1 asked for by name, not the rga-text bug just found, nothing. Confirmed by reading
  `compiler.rs`'s full test module (§1.1/§1.2's two new tests are the only additions).

### 2.8 Capability enforcement (§4.2): still no seam to attach to
Re-checked: `StorageBackend::query(&self, pql: &str) -> Result<...>` (`storage/mod.rs:30`) is
unchanged — still no caller identity or capability context parameter anywhere in the query
path. Unchanged from round 1.

## 3. Testing methodology vs. industry standards

The differential-oracle property tests (`query_eval_proptest.rs`) remain legitimate and
unchanged in what they cover — 13 tests still pass, still exercising real SQLite execution
against a real second implementation for single-MATCH, non-traversal, non-CRDT queries. That
verdict from round 1 stands.

What's new this round, scored plainly:
- Two genuinely good, narrow unit tests were added for two genuinely fixed features
  (`test_crdt_ops_rejected`, `test_contains_compiles`) — real tests, real assertions, would
  catch a regression. Credit given in §1.1/§1.2.
- Meanwhile, `compiler.rs` gained roughly 100 net lines of reference-traversal logic (the
  WHERE-clause fix, the per-variable RETURN-projection fix, the multi-hop loop) and
  `eval.rs` gained a parallel, independently-different RefTraverse implementation in the
  reference evaluator — and neither got a single test. Not "a weak test," not "a test that
  doesn't fully cover it" — zero tests, for a feature substantial enough to be worth ~100
  lines of new compiler logic. That absence is precisely what let a "no such table" SQLite
  error ship in the exact code path the design treats as its second-most-important feature
  (after basic field filtering). This is the same shape of gap round 1 found — coverage
  stops exactly at the edge of what's already known to work — recurring in the same feature
  round 1 already named as the biggest hole.
- The type-validity check (§2.7) has the same story at smaller scale: real logic, zero
  tests, and a genuine bug (rga-text ordering) found on first close reading.
- No negative/error-path property tests still exist for anything (confirmed unchanged from
  round 1) — every `check()` call in the proptest file still only asserts success-path
  agreement.

**Verdict, updated for this round:** the testing is not cheating in the mock-everything
sense — every test that exists calls real production code and would fail if reverted, which
is a meaningfully better bar than the reactor/hook subsystem's worst rounds. But test
*coverage* continues to be shaped, whether deliberately or not, to validate exactly the
surface that's already solid while staying completely silent on new, non-trivial logic added
in the same commits — and this round supplied direct proof of why that matters: the one
feature with a from-scratch rewrite and zero tests is the one feature that doesn't run.

## 4. Bottom line

Real progress happened this round, and it should be named precisely rather than folded into
a blanket "still broken": CRDT view selectors now correctly refuse to silently no-op,
`CONTAINS` exists end-to-end, boolean/noderef ordering operators are now rejected at compile
time for typed fields, `field_stats.scan_count` has a real producer, and the prepared-
statement cache is real and produces real hits for repeated queries. None of that was true
in round 1, and none of it is a cosmetic or test-satisfying change — each is verified by
reading the control flow and, for the two most important claims, by running the code.

Against that, the subsystem's central relational feature is now empirically confirmed broken
in the strongest possible sense: not "wrong results," not "an edge case," but the design
document's own simplest example failing to execute against a real database, immediately,
every time. It broke while being partially fixed — two real bugs from round 1 were repaired
in the same rewrite that left this one unaddressed — which is exactly what you'd expect from
substantial code changes landing with no test at any level for the feature being changed.
The IR and statement-cache stories tell a smaller version of the same lesson: the "correct"
mechanism (`IrPlan::cache_key()`, shape-based hashing) was actually built this round, and
sits one function call away from being wired to the exact place that needs it, still
disconnected. Fixing reference traversal — and adding even one test that executes it against
real SQLite — remains the single highest-leverage thing to do next; everything else in this
report is smaller than that gap.
