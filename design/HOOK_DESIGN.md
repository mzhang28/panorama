# Panorama Reactor & Hook Subsystem — Design v0

## 1. Core Concept

One primitive, two execution paths. A **reactor** declares: watch something, run WASM, optionally write a result. What differs is *when* it runs relative to the triggering write, and that difference in timing forces different rules for each path.

```
reactors                                    (stored as nodes, system schema)
  id                UUID PRIMARY KEY
  defined_by_app    UUID NULL               -- weak ref, survives app uninstall
  owner_schema_id   UUID NULL               -- schema this reactor is scoped to, if any
  mode              ENUM('eager','deferred')
  trigger           JSONB                   -- see §2 / §3
  filter            <predicate>             -- reuses query-language WHERE syntax
  action_kind       ENUM('validate','transform','compute_field','side_effect','internal_write')
  action_target     <field_path> | NULL     -- for compute_field
  action_ref        <wasm_ref>
  priority          INT DEFAULT 0           -- eager only; lower runs first
  capabilities      [<cap_ref>]             -- deferred only; always empty for eager
  status            ENUM('active','disabled','error_quarantined')
  retry_policy      JSONB                   -- deferred only
  created_at
```

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
- `compute_field` — the original eager-computed-field case. Unchanged from prior design.

### 2.3 Hard constraint: no network capability

**Eager reactors cannot hold network capabilities, full stop — regardless of what an app declares or requests.** A commit blocking on external RTT makes every write's latency and availability a function of a third-party service. This is a categorically bigger blast radius than a slow local computation. `capabilities` is always empty for `mode='eager'`; the capability grant system rejects any attempt to attach one.

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

Subscribers on the durable op stream. Cannot abort or affect the write that triggered them — it already happened.

### 3.1 Triggers

Unified type covering both data changes and lifecycle events — both are just entries in the op stream:

```
trigger :=
  | FieldWatch     { field_path, scope: schema_id | space_id | global }
  | LifecycleWatch { event: NodeCreated | NodeDeleted | SchemaInstalled
                          | SchemaMigrated | AppInstalled | AppUninstalled
                   , scope }
```

`filter` is a predicate using the same syntax as query-language `WHERE` clauses — no second predicate language to design or maintain.

### 3.2 Action kinds

- `compute_field` — original deferred-computed-field case.
- `side_effect` — external calls (the IFTTT case: send email, hit webhook).
- `internal_write` — write to other nodes, non-transactionally, eventually consistent.

`capabilities` may be non-empty here — network and other grants apply normally, same as any app capability grant.

### 3.3 Ordering

Fire on **causally-converged state by default**, not on every individual op arrival. A reactor watching a CRDT field reacts once the merge settles to a new observable value, not once per contributing op — this avoids firing on stale intermediate states from offline clients merging in old operations. Opt into `every_op` explicitly if per-op firing is genuinely needed.

### 3.4 Delivery guarantees

At-least-once. Reactor actions must be idempotent, or must supply an idempotency key the action WASM checks before executing a side effect. Durable per-reactor cursor tracks progress through the op stream; retry policy governs backoff; exhausting the retry budget dead-letters the event (logged, not silently dropped) rather than quarantining the reactor — a dead letter doesn't block anything else, unlike an eager failure.

### 3.5 Transaction interaction

A reactor watching a field written inside a transaction fires **after** the transaction commits, never as part of it. The transaction can return success before any deferred reactor has run. Apps depending on ordering between "my write succeeded" and "the reactor ran" must poll or subscribe separately — this is not implicit.

## 4. Cross-Cutting Concerns

### 4.1 Cycle detection

One checker, shared with the computed-field dependency graph (not a separate mechanism). Graph edges include:
- Eager reactor write → triggers another hook point.
- Lifecycle event → triggers a write → triggers another lifecycle event.
- Deferred reactor `internal_write` → triggers another `FieldWatch`.

Detection is static, based on declared `watches` / `action_target`, checked at registration time. Verifying the WASM's actual behavior matches its declared dependencies is out of scope for v0 — declarations are trusted.

### 4.2 Authority

Reactor actions execute under the authority of **the user who authorized the reactor's registration**, not the app that defined it and not the user whose write triggered it. State this explicitly in the capability check at execution time.

### 4.3 Query-language reuse

`filter` predicates reuse the query language's `WHERE` grammar exactly. Same parser, same semantics (three-valued logic on missing fields, type-aware comparison, etc.), applied to a single candidate node/op instead of a result set.

## 5. Left Open, Deliberately

- Hook points for capability grant/revoke events — no concrete use case yet, add when one exists.
- Real static verification that action WASM's behavior matches its declared `watches`/dependencies.
- Resolution beyond priority ordering when two eager reactors on the same hook point conflict (one transforms, another wants to reject the transformed result) — priority order handles it mechanically but the UX of debugging "which reactor did this" isn't designed.
- Whether `error_quarantined` reactors should have a max quarantine count before requiring app-level reinstall, or can be reactivated indefinitely.