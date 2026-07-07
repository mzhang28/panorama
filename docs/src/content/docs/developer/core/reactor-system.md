---
title: Reactor & Hook Subsystem
description: Deep dive into the eager pre-commit and deferred post-commit reactor triggers and execution flows.
---

The **Reactor Subsystem** handles event-driven computation in Panorama. Reactors enable applications to run custom logic in response to node changes. 

Reactors are stored as first-class nodes in the system database and fall into two execution paths: **Eager** (pre-commit) and **Deferred** (post-commit).

---

## 1. System Design

All reactors define a trigger pattern, an action kind, and a reference to a WASM module:

```
Reactors (stored as system nodes)
├── Mode (Eager | Deferred)
├── Action Kind (validate | transform | compute_field | internal_write | process)
├── Trigger (FieldWatch | LifecycleWatch | PendingWork)
└── WASM Reference (Wasm module executable)
```

---

## 2. Eager Reactors (Pre-Commit Hooks)

Eager reactors run **inside** the storage write transaction before data is committed to SQLite. They can reject, intercept, or modify a write payload.

### Hook Points
*   `before_node_create` (scoped to a specific `schema_id`)
*   `before_field_write` (scoped to a `field_path` or `schema_id`)
*   `before_node_delete` (scoped to a `schema_id`)
*   `before_schema_install`
*   `before_schema_migrate`

### Action Kinds
*   `validate`: Approves or rejects a write without side effects. If a validation reactor fails, the transaction aborts.
*   `transform`: Rewrites field values before they are written.
*   `compute_field`: Derives and populates a computed field synchronously.

### Hard Constraint: No Network Access
Eager reactors sit directly on the database commit path. Blocking a commit on network round trips makes database availability dependent on third-party services. Consequently, **Eager reactors cannot request or receive network capability grants.** If an eager reactor attempts to use network features, the platform rejects its execution.

### Failure Policy: Error Quarantine
To prevent a buggy eager hook from blocking all writes to a schema forever:
1.  If an eager reactor fails (crashes or times out) $N$ consecutive times, the platform transitions its status to `error_quarantined`.
2.  Once quarantined, the reactor is bypassed, writes succeed, and a warning is logged.
3.  Developers must manually resolve the bug and reactivate the reactor.

---

## 3. Deferred Reactors (Post-Commit Workers)

Deferred reactors run asynchronously **after** a transaction has committed. They cannot abort the write that triggered them.

### Triggers
1.  **FieldWatch**: Edge-triggered on changes to a namespaced field. Runs on *causally converged* state by default (after CRDT synchronizations settle).
2.  **LifecycleWatch**: Edge-triggered on system events (e.g. `NodeCreated`, `SchemaInstalled`).
3.  **PendingWork**: Level-triggered on a predicate condition (`match_filter ∧ ¬done_filter`).
    *   Fires immediately when a node begins matching `match_filter` (or stops matching `done_filter`).
    *   Re-evaluated periodically during background directory scans at a configured `rescan_interval`. This serves as the self-healing and retry mechanism.

### Action Kinds
*   `compute_field`: Derives field values asynchronously (no capability grants).
*   `internal_write`: Writes to other nodes non-transactionally.
*   `process`: The only deferred action permitted to hold external capabilities (network, command execution, filesystem). Run exclusively under a `PendingWork` trigger.

### Failure Policy: Retries and Dead-Lettering
*   **FieldWatch / LifecycleWatch**: At-least-once delivery. If the reactor execution fails, it retries according to a `retry_policy` backoff. If the budget is exhausted, the event is moved to a dead-letter queue.
*   **PendingWork**: Retries are handled implicitly by periodic background rescans. If the `process` action returns a non-zero exit code, the node remains in a pending state and is re-run during the next rescan.
*   If a node fails `max_attempts` times, it transitions to `dead_lettered` and is skipped by subsequent scans until manually re-queued.
