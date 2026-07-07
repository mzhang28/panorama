# Hook/Reactor Subsystem Evaluation — Round 6 Independent Re-Audit

## 0. Method

Per instruction, this is a from-scratch audit against the current working tree:
`design/HOOK_DESIGN.md` vs. `crates/panorama-core/src/{reactor,reactor_eval,field}.rs`,
`crates/panorama-server/src/{reactor/*,plugin_loader,wasm_runtime,api}.rs`, and
`crates/panorama-server/tests/reactor_tests.rs`. `PROGRESS.md` was not used as evidence, and
the round-5 `HOOK_REPORT.md` already in the tree was treated the same way — read for
context, not trusted; every claim below was independently re-derived from current source,
several by actually running tests. No git commands were run.

There was concurrent, unrelated development happening elsewhere in the codebase during this
audit (confirmed by the user) — specifically a compile error appeared in
`query/compiler.rs` partway through (an `E0382` "use of moved value" error, entirely inside
the query subsystem, unrelated to anything in this report). Per instruction it was ignored.
The live test evidence below (§1.1) was captured in a run that completed cleanly before that
error appeared; later attempts to re-run the full suite hit the unrelated compile failure,
which is noted rather than worked around.

**Headline: this round closed the single highest-priority gap named across five prior
audits.** Real, checked-in, deterministic WASM execution tests for reactors now exist and
pass. That is a materially different outcome than any of the previous five rounds produced
for this specific gap, and it's covered first and in detail below. A second real fix
(field-scoped hooks firing at node-creation time) also landed. Both are credited precisely.
What's still open — `side_effect`, HTTP-level black-box coverage, and a few smaller items —
is unchanged from round 5, confirmed by re-reading rather than assumed.

## 1. What's now genuinely fixed (verified independently — for the headline item, by running it)

### 1.1 Real WASM execution coverage for reactors now exists — the gap named in all five prior rounds
Every prior audit (rounds 1 through 5) named the same top-priority gap in some form: nothing
in the test suite ran real, third-party-style WASM logic through the reactor execution path
and checked what it did. Round 4's attempt was a broken, always-skipped placeholder; round 5
found that placeholder had been deleted rather than fixed, leaving zero coverage.

This round, a new crate, `crates/panorama-app-test-reactor/`, was added: a minimal,
test-only WASM plugin (`TestReactorPlugin`) that implements the real `Plugin` trait and
dispatches on `__reactor__/{function_name}` — the exact same dispatch convention
`PluginLoader::execute_reactor_action` uses for any real plugin (`lib.rs:39-53`). Its handler
(`lib.rs:56-102`) genuinely deserializes the real `ReactorActionInput` type (not a stub
shape) and returns one of four fixed, deterministic outputs by function name:
`test_validate_approve` → `Approved`, `test_validate_reject` → `Rejected { reason: "test
rejection" }`, `test_transform` → `Transformed { new_value: "transformed-by-wasm" }`,
`test_compute` → `Computed { field_key: "test:computed_value", value: 42 }`. It compiles to
`wasm32-wasip1` via a `[[bin]]` target and a `wasm_main.rs` that calls
`panorama_core::wasm_adapter::run_plugin` — the same guest-side entry point real plugins use.

Four new tests in `reactor_tests.rs` (`test_wasm_validate_approve`, `test_wasm_validate_reject`,
`test_wasm_transform`, `test_wasm_compute`, `:1288-1418`) build a `PluginLoader`, load this
plugin from an in-memory `PanoAppPackage` (`setup_loader_with_test_wasm`, `:1246-1285`), and
call the real `execute_reactor_action` against each function name, asserting on the specific
deterministic output — not "did it not crash," an exact value match
(`assert_eq!(reason, "test rejection")`, `assert_eq!(value, serde_json::json!(42))`, etc.).
This exercises the actual wasmtime compilation, instantiation, WASI stdin/stdout protocol, and
`execute_wasm_handler`/`execute_reactor_action` code paths for real — not a mocked substitute.

The build-time fragility that broke round 4's version is also fixed: `test_reactor_wasm_bytes`
(`:1199-1243`) builds the fixture on demand via `cargo build -p panorama-app-test-reactor
--target wasm32-wasip1` from the workspace root (derived from `CARGO_MANIFEST_DIR`, not a
hardcoded relative path assuming a specific CWD), rather than depending on `just test-e2e`
having already produced a `.panoapp` at a path relative to the test binary's CWD. I ran these
four tests myself: `cargo test -p panorama-server test_wasm_ -- --nocapture` → **4 passed, 0
failed**, with the crate compiling cleanly for that run (before the unrelated query-module
compile error described in §0 appeared).

This is a genuine, verified close of the gap for the three eager action kinds
(`Validate`/`Transform`/`ComputeField`). It does not extend to deferred `compute_field` (still
no test drives a real WASM-computed value through `DeferredReactorEngine::execute_action` and
checks it landed in storage — the code path for that was fixed in round 5 per prior audits,
but remains untested) or to `side_effect` (see §3.1 — structurally still impossible, so no
test could cover it regardless).

### 1.2 Field-scoped hooks now fire at node-creation time (was: only from the first update onward)
Round 5 found `api.rs::create_node` fired `BeforeNodeCreate` once for the whole node but never
looped over the node's initial fields to fire per-field `BeforeFieldWrite` hooks, unlike
`update_node`, which did. Current `api.rs::create_node` (`:208-249`) now has a real loop:
```rust
let field_keys: Vec<String> = node.fields.keys().cloned().collect();
for field_path in &field_keys {
  ...
  match state.eager_pipeline.execute_hook(&field_ctx).await {
    HookResult::Rejected { reason, .. } => { return Err(...); }
    HookResult::Approved { transformed_value, computed_fields, .. } => {
      if let Some(tv) = transformed_value { node.set_field(field_path, tv); }
      for (key, value) in &computed_fields { node.set_field(key, value.clone()); }
    }
  }
}
```
This correctly rejects the whole create if any field-scoped reactor rejects, and correctly
applies transforms/computed fields per field before the node is persisted — the same pattern
`update_node` already used. This is a real production-code fix, confirmed by reading the
current handler, not inferred from a test.

## 2. A nuance worth flagging precisely: the test added alongside §1.2 doesn't verify §1.2
A new test, `test_before_field_write_fires_on_create` (`reactor_tests.rs:1153-1192`), was
added in the same round as the §1.2 fix, and its name directly answers round 5's exact
criticism. But reading its body: it never calls `state.storage.create(...)` or anything at the
`api.rs` layer at all. It manually constructs a `HookContext` with
`hook_point: HookPoint::BeforeFieldWrite { field_path: "secret:value", ... }` and calls
`pipeline.execute_hook(&ctx)` directly, then asserts the result is `Rejected`. That only
re-confirms that the pipeline correctly processes a `BeforeFieldWrite` hook when handed one —
which was never in doubt and was already covered by other tests (e.g.
`test_eager_validate_approved`). It does not exercise the actual behavior in question (whether
`create_node` loops over a node's initial fields and fires the hook per field), because it
never goes through `create_node`.

To be precise about why this matters and why it's different from the round-4/5 test-theater
pattern: in those cases, the underlying capability was *also* broken, and the fake-looking
test was covering for that. Here, independently confirmed by reading `api.rs` directly (§1.2),
the underlying fix is real and correct. So this isn't "a fake fix disguised by a fake test" —
it's "a real fix, paired with a test that doesn't actually prove it via the code path that
matters," which is a smaller problem but the same category of problem: if `create_node`'s new
loop were reverted or broken in a refactor, this test would keep passing and give no signal.
The gap round 5 asked to close (a test that drives this through the real handler) is still
open, even though the capability itself now works.

## 3. What's still broken or unfixed — re-verified this round, not carried over from round 5 on faith

I re-read `crates/panorama-server/src/reactor/{registry,eager,deferred,op_stream}.rs` and
`plugin_loader.rs`/`wasm_runtime.rs` in full this round; all are byte-identical in line count
to round 5's versions and, on reading, unchanged in substance. So the following are
confirmed-unchanged, not assumed-unchanged:

### 3.1 `side_effect` is still structurally impossible
`wasm_runtime.rs` still links exactly the same seven host functions
(`host_ctx_create_nodes`, `host_ctx_create_node`, `host_ctx_get_node`, `host_ctx_update_node`,
`host_ctx_delete_node`, `host_ctx_query`, `host_ctx_log`) and no network/fetch/socket host
function of any kind — the file is unchanged from round 5. `deferred.rs`'s `SideEffect` branch
(`:389-397`) is still a bare log line asserting the WASM module "performed the side effect via
host functions" when no such host function exists to call. The design's headline deferred use
case ("the IFTTT case: send email, hit webhook", §3.2) remains unimplementable, unchanged
across six rounds now.

### 3.2 No HTTP-level black-box test exists yet
Grepped `reactor_tests.rs` fresh this round for `axum::`, `oneshot`, `build_router`,
`reqwest`, `TestServer` — zero matches, same as round 5. Also checked
`integration_test.rs` for reactor-related HTTP coverage — the only reactor-adjacent lines
there are schema registrations in test setup, nothing that exercises a reactor through
`POST /api/reactors` or checks a node write's HTTP response for a rejection. Round 5's
recommendation #1 is still open.

### 3.3 The transaction-interception test still bypasses the real hook pipeline
`test_op_stream_append_through_storage_and_query` (`:813-863`, confirmed present at the same
relative position and unchanged in body) still calls `state.storage.create(node)` directly and
hand-fabricates the op-stream entry it then asserts exists, never touching
`state.eager_pipeline.execute_hook` or the `api.rs` handler. Unchanged from round 5.

### 3.4 Priority ordering is still not actually tested
`test_priority_ordering` (`:277-334`) is byte-identical to round 5's version: it registers
three reactors at priorities 10/0/5 and asserts only the aggregate `Rejected` outcome (true
because there's no plugin loader, fails closed before quarantine). It still cannot and does
not distinguish "priority 0 ran before priority 10" from the reverse. Unchanged.

### 3.5 Schema ownership is still a naming convention
`verify_schema_ownership` (`registry.rs`, unchanged) still infers ownership purely from
whether a schema's name is prefixed `"{plugin_id}/"`. Unchanged from round 5; same low-severity
caveat applies (nothing else enforces prefix uniqueness either).

### 3.6 No unit tests exist inside the reactor server modules themselves
Re-confirmed by grep: `registry.rs`, `eager.rs`, `deferred.rs`, `op_stream.rs` still contain no
`#[cfg(test)] mod tests` block. All coverage for these files is still exclusively through the
external `reactor_tests.rs` integration file. Unchanged from round 5; still a defensible
choice for wiring-heavy code, still means no fast isolated coverage of e.g. the backoff-math
or edge-construction helpers in isolation.

## 4. Testing methodology vs. industry standards

Scoring against the same five-item checklist round 5 used (drawn from round 4's original
recommendations):

1. HTTP-level black-box test — still missing (§3.2).
2. A real, checked-in WASM fixture for deterministic reactor testing — **done this round**
   (§1.1), and done well: it exercises the real bridge end-to-end with exact-value assertions,
   not a "didn't crash" check.
3. A genuine quarantine test through the real pipeline — done in round 5, still present and
   unchanged.
4. A restart/recovery test for cursor durability — done in round 5, still present and
   unchanged.
5. A cross-reactor cycle test — done in round 5, still present and unchanged.

Four of five are now genuinely done, each verified by reading the test body and, for the two
most consequential ones (quarantine and WASM execution), by actually running them. That's a
better hit rate than any round before it. The one remaining item (#1) is the last structural
gap in coverage terms — nothing in the suite currently proves that a rejected write actually
produces the right HTTP-level outcome for a real client, only that the internal pipeline
components individually behave correctly when driven directly.

Separately, §2's finding is worth naming as its own methodology point, distinct from the
checklist: a test can be added in good faith alongside a real fix and still not verify that
fix, simply by exercising a lower layer than the one that changed. That's a narrower failure
than "the test is fake because the feature is fake" (the pattern in rounds 1-4), but it's
worth watching for specifically now that the more obvious version of the pattern has mostly
stopped recurring — the risk shifts from "fake fixes with fake tests" to "real fixes with
tests that quietly test something adjacent instead."

## 5. Bottom line

This is the sixth audit of this subsystem and the first to find the single most-repeated
criticism — no real WASM execution coverage — genuinely and thoroughly resolved, not patched
around. The fixture is real, checked into the repo, exercises the actual wasmtime/WASI bridge,
and asserts exact deterministic outputs; I verified it by running it. A second real fix
(field-scoped hooks firing at creation time, not just updates) also landed and was verified by
reading the current handler.

What remains open is narrower than it was five rounds ago: `side_effect`'s impossibility is a
structural gap (no network host function exists to call, not a missing test — this needs an
actual capability-gated host function added to `wasm_runtime.rs`, and no amount of testing
would fix it without that), and the HTTP-level black-box test is the one item from round 4's
original recommendations that still hasn't landed after two chances. Both are well-scoped,
concrete next steps rather than open-ended gaps. The subsystem's trajectory across six rounds
now shows more fixing than gaming — worth stating plainly, since that wasn't true for most of
this subsystem's history.
