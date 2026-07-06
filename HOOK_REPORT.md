# Hook Subsystem Evaluation Report (Re-evaluation)

## Executive Summary
After a follow-up review of the `panorama-core` and `panorama-server` crates, there have been updates bridging some parts of the system, but the implementation is still partially a façade and continues to "cheat" on testing requirements. 

While the **eager reactors** have been wired up to the `PluginLoader` to execute real WASM, **deferred reactors** remain completely stubbed out. Testing remains exclusively limited to pure unit tests, with zero integration tests for the real side-effecting code.

## 1. Faithfulness to the Design Document
### Data Model & Differential Oracle (Faithful)
The core types and the pure, in-memory evaluator (`reactor_eval.rs`) remain faithfully aligned with `HOOK_DESIGN.md`.

### Eager Reactors (Partially Faithful / Improved)
The recent commits connected the `EagerReactorPipeline` to the `PluginLoader`. Eager hooks for validation, transformation, and computed fields now execute the WASM action specified by the reactor. However, if the `PluginLoader` is unavailable, it defaults to approving the write—a questionable fallback for a system whose purpose is to securely enforce invariants on the pre-commit path.

### Deferred Reactors (Incomplete / Stubbed)
- **Filters (Faithful):** The recent updates successfully wired up `filter_matches` to the query AST evaluator (`panorama_core::query::eval_predicate`), fulfilling §3.1 of the design.
- **Action Execution (Cheating):** The `execute_action` method in `deferred.rs` remains entirely stubbed. `ComputeField`, `SideEffect`, and `InternalWrite` actions just log a debug message and return `Ok(())`. WASM execution is completely missing here.

### Capability Scoping & Schema Ownership (Incomplete)
Section 2.4 requires eager reactors to be scoped to a schema owned by the registering app. The recent updates query the `SchemaRegistry` to see if the schema exists, but explicitly skip verifying if the app actually owns the schema due to missing app-ID to plugin-ID resolution (logging `Schema ownership verified — schema exists in registry`). This fails the security requirement of the design.

## 2. Testing Methods and Industry Standards
The testing strategy continues to fall severely short of industry standards.

- **No Integration Tests:** There are still absolutely zero integration tests in `panorama-server/tests` for the reactor subsystem. An industry-standard approach for an event/hook subsystem requires end-to-end integration tests that verify:
  1. A reactor is registered.
  2. A real storage write occurs.
  3. The hook intercepts the write, executes the WASM via the `PluginLoader`, and alters the transaction.
- **Testing is "Cheating":** The developer passed "all tests" by only testing the pure functional scaffolding (`reactor_eval.rs`). The messy, side-effecting parts (WASM bridge in `eager.rs`, completely stubbed execution in `deferred.rs`) are shielded from test coverage. 

## Conclusion and Recommendations
The system is making progress but remains incomplete and continues to bypass critical integration work and testing. 

1. **Implement Deferred WASM Execution:** `execute_action` in `deferred.rs` must be wired to the `PluginLoader`, matching what was done for `eager.rs`.
2. **Properly Enforce Schema Ownership:** The system must resolve the plugin ID and strictly enforce that the app registering the eager hook actually owns the schema (via naming conventions as intended).
3. **Mandatory E2E Tests:** End-to-end integration tests in `panorama-server/tests/` remain the most critical missing piece. Without these tests, the subsystem cannot be considered complete or robust by any standard.
