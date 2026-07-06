# Panorama Reactor & Hook Subsystem — Design v1

## 1. Core Concept

One primitive, two execution paths. A **reactor** declares: watch something, run WASM, optionally write a result or perform external work. What differs is *when* it runs relative to the triggering write, and that difference in timing forces different rules for each path. Deferred reactors additionally split into three trigger shapes, and capability-bearing execution is confined to exactly one of them.

```
reactors                                    (stored as nodes, system schema)
  id                UUID PRIMARY KEY
  defined_by_app    UUID NULL               -- weak ref, survives app uninstall
  owner_schema_id   UUID NULL               -- schema this reactor is scoped to, if any
  mode              ENUM('eager','deferred')
  trigger           JSONB                   -- see §2 / §3
  filter            <predicate>             -- reuses query-language WHERE syntax; N/A for PendingWork (match_filter/done_filter subsume it)
  action_kind       ENUM('validate','transform','compute_field','internal_write','process')
  action_target     <field_path> | NULL     -- for compute_field
  action_ref        <wasm_ref>
  priority          INT DEFAULT 0           -- eager only; lower runs first
  capabilities      [<cap_ref>]             -- only valid when action_kind='process'; empty otherwise, always
  status            ENUM('active','disabled','error_quarantined')
  retry_policy      JSONB                   -- deferred, FieldWatch/LifecycleWatch only (delivery retry, not job retry)
  rescan_interval   DURATION NULL           -- PendingWork only; overrides global default
  max_attempts      INT NULL                -- PendingWork only; overrides global default
  created_at
```

`action_kind='validate'` and `'transform'` are eager-only. `'compute_field'` may be either eager or deferred. `'internal_write'` and `'process'` are deferred-only; `'process'` additionally requires `trigger` to be `PendingWork`. `capabilities` is non-empty only for `action_kind='process'` — this is a hard constraint enforced at registration, not a convention.

## 2. Eager Reactors (Pre-Commit)

Run **inside** the triggering transaction, before commit. Can reject or transform the write. Because they sit on the commit path of every write to their scope, they carry the tightest constraints in the system.

### 2.1 Hook points

```
before_node_create      (scope: schema_id)
before_field_write      (scope: field_path, schema_id)
before_node_delete      (scope: schema_id)
before_schema_install   (scope: schema_id)
before_schema_migrate   (scope: schema_id)
```

### 2.2 Action kinds

- `validate` — approve or reject. No side effect. This is the general write-interception case (authorization/validation logic), not a special mechanism.
- `transform` — rewrite the value being written before it commits.
- `compute_field` — derive a field's value synchronously as part of the write.

### 2.3 Hard constraint: no network capability

**Eager reactors cannot hold network capabilities, full stop — regardless of what an app declares or requests.** A commit blocking on external RTT makes every write's latency and availability a function of a third-party service. This is a categorically bigger blast radius than a slow local computation. `capabilities` is always empty for `mode='eager'`; the capability grant system rejects any attempt to attach one. No exceptions.

If the intent is "call a webhook before allowing the write," that requirement doesn't map to eager semantics — either it's deferred (runs after the write, can't gate it), or it isn't supported. No eager path exists for it.

### 2.4 Capability scoping: schema owner only, by default

An app may only register an eager reactor on a hook point scoped to a schema **it owns** (`owner_schema_id` must match a schema the registering app defined). Without this restriction, any app could silently gatekeep writes to any other app's schema by registering a competing `before_node_create` hook.

Cross-app gatekeeping (a "policy" app validating writes across schemas it doesn't own) requires an explicit `gatekeeper` capability, granted per-schema by the schema's owning app or the user. Not the default.

### 2.5 Ordering and short-circuiting

Multiple eager reactors on the same hook point run in `priority` order (ties broken by registration order). The first `validate` rejection short-circuits — remaining reactors on that hook point do not run. `transform` reactors chain in priority order, each seeing the prior one's output.

### 2.6 Failure handling: quarantine, not fail-open or fail-closed-forever

A broken eager reactor (throws, times out) is a potential denial-of-service on every write to its scope. Neither acceptable default:
- Fail open (skip it silently) — defeats the point of validation, security hole.
- Fail closed forever — bricks writes to the schema until a human intervenes.

Instead: after N consecutive failures, the reactor auto-transitions to `error_quarantined`. Quarantined eager reactors stop blocking writes, the event is logged loudly, and the schema owner is surfaced the failure. Manual re-activation required.

## 3. Deferred Reactors (Post-Commit)

Subscribers on the durable op stream, or on a periodically-recomputed predicate. Cannot abort or affect the write that triggered them — it already happened.

### 3.1 Triggers

Three trigger shapes: two edge-triggered (fire on new ops in the stream), one level-triggered (fires on a standing predicate over current state).

```
trigger :=
  | FieldWatch     { field_path, scope: schema_id | space_id | global }
  | LifecycleWatch { event: NodeCreated | NodeDeleted | SchemaInstalled
                          | SchemaMigrated | AppInstalled | AppUninstalled
                   , scope }
  | PendingWork    { match_filter: <predicate>   -- e.g. mimetype == 'image/heic'
                    , done_filter: <predicate>   -- e.g. processed_by = 'app' AND processed_by_version >= '2.0'
                    , scope
                    }
```

For `FieldWatch` and `LifecycleWatch`, `filter` is a predicate using the same syntax as query-language `WHERE` clauses — no second predicate language to design or maintain.

`PendingWork` has no separate `filter`; the pending set at any moment is every node matching `match_filter AND NOT done_filter` within scope. It fires two ways:

1. **On write**: when a node transitions into `match_filter ∧ ¬done_filter` (either it just started matching `match_filter`, or it just stopped matching `done_filter`).
2. **On rescan**: a periodic full scan over `scope` for any node still matching `match_filter ∧ ¬done_filter`, at `rescan_interval` (falls back to a global default if unset on the reactor). This is what catches nodes that predate the reactor's registration, missed ops, or reactor downtime — and it's also the retry mechanism (§3.4).

### 3.2 Action kinds

- `compute_field` — derive a field's value asynchronously after the triggering write commits. No capabilities.
- `internal_write` — write to other nodes, non-transactionally, eventually consistent. No capabilities.
- `process` — the only capability-bearing deferred action. Valid only when `trigger` is `PendingWork`. Runs with whatever is in `capabilities` (network, external programs, etc.), attempts to make `done_filter` true, and may itself write further ops — those ops are ordinary writes, visible to any other reactor's `FieldWatch`/`LifecycleWatch`/`PendingWork` normally. There is no separate event-emission mechanism; emitting an event is just making a write that something else watches.

External calls gated by a condition (the IFTTT case: send email, hit a webhook once some state holds) are handled by `PendingWork` + `process`: the condition is the match/done filter pair, not inline logic in the action, and the action is expected to actually resolve the condition rather than just fire-and-forget.

### 3.3 Ordering

`FieldWatch` fires on **causally-converged state by default**, not on every individual op arrival. A reactor watching a CRDT field reacts once the merge settles to a new observable value, not once per contributing op — this avoids firing on stale intermediate states from offline clients merging in old operations. Opt into `every_op` explicitly if per-op firing is genuinely needed.

Not applicable to `PendingWork` — there's no per-op firing concept, only "still pending as of this write or this rescan."

### 3.4 Delivery / retry guarantees

Two distinct models depending on trigger type — do not conflate them.

**FieldWatch / LifecycleWatch**: at-least-once delivery of the op itself. Reactor actions must be idempotent, or must supply an idempotency key the action WASM checks before taking a non-idempotent action. Durable per-reactor cursor tracks progress through the op stream; `retry_policy` governs redelivery backoff; exhausting the retry budget dead-letters the event (logged, not silently dropped) rather than quarantining the reactor — a dead letter doesn't block anything else, unlike an eager failure.

**PendingWork**: retry is implicit in rescanning, not a delivery mechanism. A `process` action returning nonzero (§3.4.1) leaves the node matching `match_filter ∧ ¬done_filter`, so it's picked up again next rescan. No separate idempotency-key machinery is needed for this layer — rescan-as-retry only works because reconciling toward `done_filter` is idempotent by construction (running it again on an already-done node is a no-op, since `done_filter` already holds). Anything the `process` action does that isn't safe to repeat (e.g. side effects with the external system) is the app's problem to make idempotent, same as any external integration.

#### 3.4.1 Failure definition (PendingWork)

A `process` invocation failed if and only if it returned nonzero. Returning zero counts as success even if `done_filter` didn't end up true — that outcome is indistinguishable from "still processing, try later" and is handled the same way: still pending, picked up next rescan. Everything else about what "trying" means is up to the program.

#### 3.4.2 Per-node attempt tracking and dead-letter

```
reactor_pending_state                        (per-item tracking, PendingWork reactors only)
  reactor_id        UUID
  node_id           UUID
  attempt_count     INT
  last_error        TEXT NULL      -- nonzero exit info
  last_attempt_at   timestamp
  status            ENUM('pending', 'dead_lettered')
  PRIMARY KEY (reactor_id, node_id)
```

Each failed attempt increments `attempt_count`. Once `attempt_count` reaches `max_attempts` (reactor-level override, else global default), the node transitions to `dead_lettered`: excluded from future rescans for that reactor, not retried again automatically. Dead-lettered items are surfaced in a queryable view (per reactor, listing node, attempt count, last error) so they can be inspected and manually re-queued — not a silent log line, and not `error_quarantined` (§2.6), which is an eager, whole-reactor, write-blocking state. A stuck `PendingWork` item never blocks a write; it only stops being retried.

### 3.5 Transaction interaction

A reactor watching a field written inside a transaction fires **after** the transaction commits, never as part of it. The transaction can return success before any deferred reactor has run. Apps depending on ordering between "my write succeeded" and "the reactor ran" must poll or subscribe separately — this is not implicit.

## 4. Cross-Cutting Concerns

### 4.1 Cycle detection

One checker, shared with the computed-field dependency graph (not a separate mechanism). Graph edges include:
- Eager reactor write → triggers another hook point.
- Lifecycle event → triggers a write → triggers another lifecycle event.
- Deferred reactor `internal_write` → triggers another `FieldWatch`.

Detection is static, based on declared `watches` / `action_target` / `match_filter` / `done_filter`, checked at registration time. Verifying the WASM's actual behavior matches its declared dependencies is out of scope for v1 — declarations are trusted.

**Open**: whether ops written by a `process` action are tracked in this graph, or treated as opaque app writes like any other app-initiated write — not yet decided.

### 4.2 Authority

Reactor actions execute under the authority of **the user who authorized the reactor's registration**, not the app that defined it and not the user whose write triggered it. State this explicitly in the capability check at execution time.

### 4.3 Query-language reuse

`filter` predicates (`FieldWatch`/`LifecycleWatch`) and `match_filter`/`done_filter` predicates (`PendingWork`) all reuse the query language's `WHERE` grammar exactly. Same parser, same semantics (three-valued logic on missing fields, type-aware comparison, etc.), applied to a single candidate node/op instead of a result set.

## 5. Left Open, Deliberately

- Whether `process`-authored writes participate in reactor cycle detection (§4.1).
- Global default values for `rescan_interval` and `max_attempts`.
- Hook points for capability grant/revoke events — no concrete use case yet, add when one exists.
- Real static verification that action WASM's behavior matches its declared dependencies.
- Resolution beyond priority ordering when two eager reactors on the same hook point conflict (one transforms, another wants to reject the transformed result) — priority order handles it mechanically but the UX of debugging "which reactor did this" isn't designed.
