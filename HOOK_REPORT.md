# Hook/Reactor Subsystem Evaluation — Round 5 Independent Re-Audit

## 0. Method

This audit was done from scratch against the current working tree: `design/HOOK_DESIGN.md`
against `crates/panorama-core/src/{reactor,reactor_eval,field}.rs` and
`crates/panorama-server/src/{reactor/*,plugin_loader,wasm_runtime,api}.rs`, plus
`crates/panorama-server/tests/reactor_tests.rs`. `PROGRESS.md` was not used as evidence.
No git commands were run at any point — no `git log`, `git show`, or history recovery —
per instruction. That matters here specifically: a `HOOK_REPORT.md` from what its own text
identifies as a fourth audit round already existed in the working tree, documenting three
prior finding→patch→re-finding cycles. That document was read as background (it's a file
currently in the tree, not history), but every claim below was independently re-derived
from the current source and, where possible, by actually running the tests — not by
trusting either that report or `PROGRESS.md`.

There was concurrent, unrelated development happening on parts of this codebase during
the audit (confirmed by the user), including at least one point where `panorama-server`
had a compile error in the query-compiler module unrelated to the reactor subsystem. Per
instruction, compile errors were ignored rather than diagnosed or fixed. Where a live test
run was needed, it was captured at a point where the reactor-relevant code compiled and
ran cleanly (`cargo test -p panorama-server --test reactor_tests`: 29 passed, 0 failed, at
the time of this audit); the specific run and output are cited where used as evidence.

**Headline: this round is different from the first four.** The previous report's own
bottom line was that three consecutive "fix" commits reproduced the same "tests engineered
to pass" pattern without fixing the underlying gaps. That pattern did not repeat this
round. Several of the specific, previously-reproduced-live failures are now fixed with
tests that exercise the real code path and would fail if the fix were reverted. That is
not true of everything — the single most important gap (real WASM execution coverage) is
now *worse* in one specific sense (the flagship test was deleted rather than fixed), and
one other design requirement (`side_effect`) is still structurally impossible. Both of
those are covered in detail below.

## 1. What's now genuinely fixed (verified independently, not taken on the prior report's word)

### 1.1 The silent fail-open bypass (prior §1.5) is fixed
Previously: no `PluginLoader` configured, or a reactor's module simply not resolving,
caused `execute_validate` to return `Ok(true)` (silent approval), bypassing failure
tracking and quarantine entirely. Reading the current `eager.rs`:
- `execute_validate`/`execute_transform`/`execute_compute` all now return `Err(...)` for
  both of those cases (`eager.rs:294-296`, `317-320` and the parallel blocks in the other
  two functions) instead of approving.
- `execute_hook`'s `Validate` branch (`eager.rs:169-190`) now explicitly fails **closed**
  before the quarantine threshold and only fails **open** once
  `should_quarantine` is true — matching §2.6 exactly ("neither fail open nor fail closed
  forever").
- This is verified by a real test that exercises the real pipeline three times in a row —
  `test_quarantine_after_eager_failures` (`reactor_tests.rs:974-1034`) — checking
  `Rejected` on attempts 1-2 and `Approved` only on attempt 3, then confirming
  `registry.get(id).status == ErrorQuarantined`. This is exactly the test the prior report
  asked for by name ("a genuine quarantine test... not a direct call to the counter
  function") and it calls the real `execute_hook`, not the bare counter. I ran this test
  suite myself; it passes.

### 1.2 Cross-reactor cycle detection (prior §1.6) is fixed
Previously, `check_cycles` called a differently-shaped detector (`field::detect_cycles`)
fed only by each reactor's *output* edge, so a reactor's watched trigger was never a graph
node — two reactors watching each other's outputs would never be flagged. Current
`registry.rs`:
- `check_cycles` (`registry.rs:423-450`) now calls
  `panorama_core::reactor_eval::detect_cycles_in_graph` directly — the same "ground truth"
  detector the prior report noted was correct in isolation but never wired to production.
- `reactor_graph_edges` (`registry.rs:781-821`) now emits **both** directions: an INPUT
  edge `field:{path} → reactor:{id}` for anything the reactor watches
  (`BeforeFieldWrite`/`FieldWatch`), and an OUTPUT edge `reactor:{id} → field:{target}` for
  `action_target`. This is exactly the missing half the prior report identified.
- Verified by `test_cross_reactor_cycle_rejected` (`reactor_tests.rs:1038-1081`): registers
  reactor A (watches `field:Y`, writes `field:X`), then reactor B (watches `field:X`,
  writes `field:Y`) and asserts the second registration is rejected with a "cycle" error.
  This is precisely the scenario the design's own §4.1 example describes and the prior
  report predicted would currently slip through. I ran it; it passes, and reading the
  code, it would fail if `reactor_graph_edges` reverted to output-only edges — this is a
  real regression-catching test, not a tautology.

### 1.3 Deferred cursor durability (prior §1.3) is fixed
Previously, `update_cursor` only touched an in-memory `RwLock<HashMap<...>>`; nothing ever
wrote it back to storage, so a restart would replay full history. Current `deferred.rs`:
- `update_cursor` (`deferred.rs:470-494`) now calls
  `registry::persist_reactor_state(&self.registry, reactor_id, sequence)`
  (`registry.rs:731-770`), which upserts a `system:rs_last_sequence` field on a durable
  state node.
- `DeferredReactorEngine::initialize` (`deferred.rs:77-100`) reads these state nodes back
  at startup.
- Verified by `test_deferred_cursor_persisted` (`reactor_tests.rs:1085-1147`): processes an
  entry with `engine`, confirms a state node landed in storage, then builds a **second,
  independent** `DeferredReactorEngine` instance (simulating a process restart) from the
  same storage, and asserts its `poll()` does *not* re-dispatch the already-processed
  entry. This is a real restart/recovery test, exactly what the prior report asked for,
  and it exercises the real `initialize()`/`update_cursor()` path, not a stand-in.

### 1.4 The deferred `compute_field` no-op (prior §1.4, first half) is fixed in code
Previously, `DeferredReactorEngine::execute_action` discarded `output.result` entirely for
every action kind. Current `deferred.rs::execute_action` (`deferred.rs:314-420`) now
matches on `reactor.action_kind` and, for `ComputeField`, extracts
`EagerReactorResult::Computed { field_key, value }` and calls
`self.registry.storage().update(node_id, patch)` to actually persist it
(`deferred.rs:358-387`). The mechanism that was flatly missing before now exists and reads
correctly.
**Caveat:** no test exercises this specific path end-to-end. Every test that registers a
`ComputeField` deferred reactor in the current suite (`test_cross_reactor_cycle_rejected`)
is testing cycle *detection*, not execution, and never runs it through a plugin loader with
a resolving module. So this is fixed at the code level but still has zero test coverage
proving a deferred `compute_field` reactor's WASM output actually lands in storage — it's
a plausible fix, not yet a *verified* one.

### 1.5 `OpStream::query_since` (prior §1.7, third bullet) is modestly improved
Previously, the query fetched every op-stream entry ever written and filtered by sequence
in Rust. Current `op_stream.rs::query_since` (`op_stream.rs:163-179`) now pushes the
sequence bound into the query itself (`SCAN(n.system.op_sequence > {since_sequence})`) and
applies `LIMIT` in SQL rather than after full materialization. This is a real improvement
in what gets deserialized into Rust per poll, but it is not a fix for the underlying
complexity concern: `op_sequence` is still an unpromoted JSONB field with no index (the
`SCAN(...)` marker is required precisely because there isn't one — consistent with the
query subsystem's own SCAN-enforcement rules), so SQLite still has to examine every row in
the op stream to evaluate the predicate before `LIMIT` can trim the result. The design
calls this a "durable operation stream" that reactors poll continuously; without an index
on sequence, per-poll cost still scales with total history size, not with unprocessed-entry
count, for every active deferred reactor. Downgraded from "not fixed" to "partially
mitigated, not resolved" relative to the prior report.

## 2. What's still broken, unfixed, or untested — verified independently this round

### 2.1 Real WASM execution coverage for reactors: still zero, and the placeholder test is now gone entirely
The prior report's single highest-priority finding was that the one test claiming to
cover real WASM execution for reactors (`test_wasm_execution_via_plugin_loader_with_real_panoapp`)
always hit an early-return skip branch under the standard `cargo test` command (a
relative-path bug relying on `just test-e2e` having run first) and, even when not skipped,
had a body that could not fail regardless of outcome.

This round, that test is simply **not present anymore**. `reactor_tests.rs` still has the
section header comment `// ── WASM execution: real .panoapp loading ───` at line 970,
immediately followed by the next section's header at line 972 — the test itself is gone,
with the dangling comment as the only trace it ever existed. I did not find a replacement
test anywhere in the file, nor a checked-in minimal WASM fixture module for reactor testing
specifically (the prior report's second specific recommendation). Grepping the whole test
file for anything that would exercise `PluginLoader::execute_reactor_action` against a real
module returns nothing.

Separately, and independent of testing: `execute_reactor_action`
(`plugin_loader.rs:308-388`) reuses `execute_wasm_handler` — the exact same WASM runtime
used for plugin HTTP dispatch (`wasm_runtime.rs`), which links exactly seven host functions
(`host_ctx_create_nodes`, `host_ctx_create_node`, `host_ctx_get_node`,
`host_ctx_update_node`, `host_ctx_delete_node`, `host_ctx_query`, `host_ctx_log`) and no
network/fetch/socket host function of any kind — confirmed by reading the full file, all
453 lines. `side_effect`'s design-stated motivating example ("the IFTTT case: send email,
hit webhook", §3.2) remains structurally impossible for the same reason the prior report
found, unchanged this round. `deferred.rs::execute_action`'s `SideEffect` branch
(`deferred.rs:389-397`) is a bare `tracing::info!` log with a comment asserting the WASM
module "performed the side effect via host functions during execution" — it did not and
currently cannot, because no such host function exists to call.

So: the design's headline deferred use case is unimplementable today, this is unchanged
from four rounds of prior audits, and the one test artifact that gestured at exercising the
WASM bridge for reactors at all has now been deleted rather than repaired. Net effect on
this specific gap: **worse test-suite honesty** (no lingering false claim in the test file
that it's covered) but **no progress** on the actual capability gap.

### 2.2 The transaction-interception test still doesn't test transaction interception
`test_op_stream_append_through_storage_and_query` (`reactor_tests.rs:811-863`) — a renamed
version of what the prior report called
`test_create_node_triggers_hook_pipeline_and_op_stream` — still calls
`state.storage.create(node)` directly (never `state.eager_pipeline.execute_hook(...)`,
never anything at the `api.rs` handler level), then calls `state.op_stream.append_sync(...)`
**by hand** to manufacture the exact side effect the real handler would have produced, and
asserts that manufactured side effect is present. The rename is at least honest about scope
now — it no longer claims in its name to test "the hook pipeline" — but the actual gap the
prior report flagged (nothing in the suite drives a write through the real `api.rs` path
and checks that the op stream append is *conditional on* hook approval, which is the one
thing that path does and this test doesn't touch) is completely unaddressed. Confirmed by
reading `api.rs::create_node` (`api.rs:157-227`): `eager_pipeline.execute_hook` gates
whether `storage.create` and `op_stream.append_sync` ever run at all — that gating is the
behavior under test in the design, and it is the one thing no test in the file exercises.

Also unaddressed, independently confirmed by reading `api.rs` fresh: no test in
`reactor_tests.rs` builds the real `axum` router and drives a request through it (the
prior report's recommendation #1). Grepped the whole test file for `axum::`, `oneshot`,
`build_router`, `reqwest`, `TestServer` — zero matches. `integration_test.rs` already has
the pattern for this elsewhere in the codebase; it's still not reused here.

### 2.3 Priority ordering is still not actually tested
`test_priority_ordering` (`reactor_tests.rs:276-333`) registers three validate reactors at
priorities 10/0/5 and asserts the pipeline result is `Rejected` — true now because there's
no plugin loader (fails closed before quarantine, per the fix in §1.1), rather than true
because there's no plugin loader and everything silently approves (the prior round's
reason). The test's own updated comment concedes this outright: *"priority ordering is
still exercised (reactors are sorted before execution), but the final outcome is rejection
since no WASM is available."* It still cannot, and does not, distinguish "reactor priority
0 ran before reactor priority 10" from "reactor priority 10 ran before reactor priority 0"
— any reordering of `get_active_for_hook`'s sort would not be caught by this test. This is
the same gap the prior report named, carried forward with an accurate but unresolved
comment.

### 2.4 Smaller, unchanged gaps (re-verified against current source, not carried over from the prior report on faith)
- **Field-scoped hooks still skipped at node-creation time.** Confirmed by reading
  `api.rs::create_node` (`api.rs:157-227`) fresh this round: it fires
  `HookPoint::BeforeNodeCreate` once for the whole node and never loops over `req.fields`
  to fire per-field `BeforeFieldWrite` hooks, while `update_node` (`api.rs:270-308`) does
  loop per-field. A `BeforeFieldWrite`-scoped validate/transform reactor is bypassed for a
  field's initial value and only takes effect from the first update onward. Unchanged from
  the prior report.
- **`authorized_by` is still hardcoded `None`** in every `HookContext` built in `api.rs`
  (`api.rs:189, 286, 353` — confirmed by grep this round, same three sites). The §4.2
  authority distinction still can't be exercised end-to-end. This continues to trace to the
  platform having no user-auth system at all — a pre-existing, disclosed, cross-subsystem
  limitation rather than something newly broken here.
- **Schema ownership is still a naming convention, not an authoritative record.**
  `verify_schema_ownership` (`registry.rs:384-411`) still infers ownership entirely from
  whether a schema's name is prefixed `"{plugin_id}/"`, via `SchemaRegistry::list_by_app`.
  Confirmed unchanged. Still low severity for the reason the prior report gave (nothing
  else in the schema registry enforces prefix uniqueness either), but worth restating: this
  is "good actors self-report a matching prefix," not "the registry knows who owns what."
- **No unit tests exist inside the reactor server modules themselves.** `registry.rs`,
  `eager.rs`, `deferred.rs`, `op_stream.rs` — none contain a `#[cfg(test)] mod tests` block
  (confirmed by grep; all four return zero matches). Every behavior in these files is
  tested exclusively through the external `reactor_tests.rs` integration file. That's a
  defensible choice for wiring-heavy code, but it means there's no fast, isolated coverage
  of e.g. `reactor_graph_edges`'s edge construction or the backoff-multiplier arithmetic in
  `handle_failure` in isolation from the full registry/pipeline stack — `reactor_eval.rs`'s
  pure functions cover the *algorithms* this way, but the server-side glue code doesn't
  have an equivalent.

## 3. Testing methodology vs. industry standards

The prior report's framing still applies as a rubric, and it's worth re-scoring against it
rather than restating it: for a pre-commit/post-commit hook engine that intercepts every
write and runs third-party code, the standard is testing the enforcement point by actually
attempting the guarded operation and observing the outcome (the way Kubernetes admission
webhook test suites, database trigger tests, and git-hook framework tests are structured),
not calling internal functions with hand-built inputs.

Scored against that bar and the prior report's five specific recommendations:

1. ~~True HTTP-level black-box test~~ — **still missing** (§2.2).
2. ~~A real, checked-in WASM fixture for deterministic reactor testing~~ — **still
   missing, and the previous placeholder was deleted rather than replaced** (§2.1).
3. ~~A genuine quarantine test through the real pipeline~~ — **done**
   (`test_quarantine_after_eager_failures`, §1.1). Real regression-catching test.
4. ~~A restart/recovery test for cursor durability~~ — **done**
   (`test_deferred_cursor_persisted`, §1.3). Real regression-catching test.
5. ~~A cross-reactor cycle test~~ — **done** (`test_cross_reactor_cycle_rejected`, §1.2).
   Real regression-catching test, and it exercises a fix that would otherwise be very easy
   to silently regress.

Three of five landed, and landed as genuine tests — I read each one's body and it calls
real production code, not a stand-in, and would fail if the underlying fix were reverted.
That's a materially better hit rate than any of the prior three rounds, which is worth
stating plainly rather than folding into a generic "still not enough" verdict. The two
that didn't land are not minor: #2 is the single highest-value gap named across five
audit rounds now, and its previous (fake) placeholder being removed rather than fixed or
replaced is a regression in the test file's honesty-about-coverage, even though it's not a
regression in any actual runtime behavior.

Property-based/fuzz coverage of `filter_matches` against the query grammar (the prior
report's item 6, reusing the existing `query_eval_proptest` harness) is also still not
done — `filter_matches` (`deferred.rs:293-311`) calls `panorama_core::query::eval_predicate`
directly against real fetched nodes and is exercised by `test_reactor_with_filter_predicate`
(`reactor_tests.rs:724-755`), but that test only checks the filter round-trips through
serialization — it registers a filter and asserts `filter.is_some()` and that it serializes
to a JSON object; it does not construct a node and actually check the filter includes or
excludes it. So even the one filter-specific test in the suite doesn't verify filtering
behavior, only that the predicate survives storage round-tripping.

## 4. Bottom line

This is the fifth audit of this subsystem, and the first to find that a round of "fix"
commits closed real gaps with tests that actually exercise the fix rather than tests
engineered to report success regardless of outcome. The fail-open bypass that let
unvalidated writes through silently (§1.1), the cycle detector that couldn't see the exact
edge the design's own example depends on (§1.2), and the cursor-durability gap that would
have replayed full history on every restart (§1.3) are now fixed and covered by tests that
would catch a regression. That's genuine, verified progress, not a claim taken from
`PROGRESS.md` or the prior self-report.

At the same time, the subsystem's central open question — does WASM-executed third-party
logic actually get exercised by anything in the test suite, and can the design's own
headline deferred use case (`side_effect`) even be implemented against the current host
bridge — is unresolved and, in the narrow sense that the one test gesturing at it was
deleted rather than fixed, the test suite's honesty about that gap improved while the gap
itself did not close at all. `side_effect` remains impossible for a structural reason (no
network host function exists), not a missing-test reason, and no amount of additional
mocking would fix it — it needs an actual host function added to `wasm_runtime.rs` and a
capability check wired to it. Until that exists, or until a real WASM fixture is checked in
and used to test `execute_validate`/`execute_transform`/`execute_compute` and their
deferred counterparts against deterministic real output, "the mechanism the entire design
is built around" — to reuse the prior report's phrase — still has no test at any level
that runs actual third-party WASM logic and checks what it did.
