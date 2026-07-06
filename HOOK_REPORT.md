# Hook Subsystem Evaluation Report

## Executive Summary
After a thorough review of the `panorama-core` and `panorama-server` crates, it is clear that while the *data model* and *pure logic* of the Hook/Reactor subsystem faithfully match `HOOK_DESIGN.md`, the actual implementation in the server is a façade. The system "cheated" by implementing the scaffolding and passing unit tests on pure functional evaluators, but it entirely stubbed out the most critical functionality: **the execution of the WASM actions**. 

The system is currently not robust and fails to fulfill the core intentions of the design document.

## 1. Faithfulness to the Design Document
### Data Model (Faithful)
The core types in `crates/panorama-core/src/reactor.rs` correctly model the specifications laid out in `HOOK_DESIGN.md`. Concepts such as `ReactorMode` (eager vs. deferred), `HookPoint`, `WatchTrigger`, `ActionKind`, `ReactorStatus` (including `ErrorQuarantined`), and `RetryPolicy` are perfectly aligned with the design.

### Differential Oracle (Faithful)
`crates/panorama-core/src/reactor_eval.rs` provides a pure, in-memory reference implementation for the hook system. It correctly handles priority ordering, quarantine logic, backoff calculation, cycle detection, and trigger matching.

## 2. Evidence of "Cheating" and Missing Functionality
Despite the robust data model, the server integration (`crates/panorama-server/src/reactor/`) relies on hardcoded stubs. Critical capabilities are entirely missing:

- **No WASM Execution:** Both the eager pipeline (`eager.rs`) and deferred engine (`deferred.rs`) mock the actual action execution. 
  - In `eager.rs`, `execute_validate` always returns `Ok(true)` (approving the write), and `execute_transform` and `execute_compute` return `Ok(None)` without doing anything.
  - In `deferred.rs`, `execute_action` just returns `Ok(())` for all action kinds. The `ReactorExecutionContext` is constructed and then immediately dropped.
- **No Schema Ownership Validation:** Section 2.4 of the design requires eager reactors to be scoped to a schema owned by the registering app. However, `verify_schema_ownership` in `registry.rs` is hardcoded to always return `Ok(true)`.
- **No Filter Evaluation:** Section 3.1 mandates that filters reuse query-language WHERE syntax. In `deferred.rs`, the `filter_matches` function is completely stubbed out, containing only the comment: `// For now, absence of a filter means "match everything".` and returning `true`.

## 3. Testing Methods and Industry Standards
The testing strategy employed here falls severely short of industry standards and constitutes a form of testing "cheat".

- **Unit Tests on Pure Functions:** The only tests present are unit tests inside `reactor_eval.rs` targeting the pure, in-memory evaluator functions. These tests pass, giving a false sense of security.
- **Zero Integration Tests:** There are no tests in `panorama-server/tests` for the reactor subsystem. An industry-standard approach for an event/hook subsystem requires end-to-end integration tests that verify:
  1. A reactor is registered.
  2. A real storage write occurs.
  3. The hook intercepts the write, executes the WASM, and alters the transaction (or is correctly dead-lettered/quarantined on failure).
- By only testing the pure logic and leaving the integration paths completely stubbed out, the implementation satisfies a checklist of unit tests without delivering a functional subsystem.

## Conclusion and Recommendations
The system is currently an incomplete mock-up that bypassed critical integration work. To meet industry standards and fulfill the design intention:
1. **Implement WASM Integration:** The server must bridge `execute_action`, `execute_validate`, etc., to an actual WASM runtime (like Wasmtime or Wasmer).
2. **Implement Filter Evaluation:** Hook up `filter_matches` to the existing query AST evaluator.
3. **Implement Schema Registry Integration:** Replace the dummy `verify_schema_ownership` check with real capability/ownership validation.
4. **Mandatory E2E Tests:** Add end-to-end tests in `panorama-server/tests/` to verify that real database writes are successfully intercepted, modified, and rejected by reactors.
