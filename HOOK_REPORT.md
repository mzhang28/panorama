# Hook Subsystem Evaluation Report (Second Re-evaluation)

## Executive Summary
After a second follow-up review of the `panorama-core` and `panorama-server` crates, significant improvements have been made to bridge the gaps in the hook subsystem. Both eager and deferred pipelines are now wired to execute real WASM actions via the `PluginLoader`. Additionally, schema ownership checks now use proper registry validation (though relying on plugin name prefixing). 

However, the implementation still severely "cheats" on testing. While a new `reactor_tests.rs` file was added, it fails to perform true integration testing of the subsystem, choosing instead to test isolated components in ways that bypass the real side-effecting code.

## 1. Faithfulness to the Design Document
### Action Execution (Faithful)
Both `EagerReactorPipeline` (in `eager.rs`) and `DeferredReactorEngine` (in `deferred.rs`) are now properly wired up to the `PluginLoader`. This replaces the previous stubs and faithfully implements the WASM action execution as prescribed by the design document. 

### Capability Scoping & Schema Ownership (Faithful, but rudimentary)
The `registry.rs` has been updated to use `SchemaRegistry.list_by_app(plugin_id)` to verify that an app registering an eager reactor actually owns the schema. This fulfills §2.4.

### Filters (Faithful)
As previously noted, `filter_matches` inside `deferred.rs` faithfully uses the query AST evaluator.

## 2. Testing Methods and Industry Standards (Still Cheating)
The new `crates/panorama-server/tests/reactor_tests.rs` file appears at first glance to be an integration test suite. However, a deeper inspection reveals it still "cheats" by bypassing the critical integration points:

- **Bypassing the PluginLoader:** The test environment `setup_reactor_test_env()` initializes the `EagerReactorPipeline` *without* a `PluginLoader`. As a result, all execution tests fall back to the graceful bypass (`None => return Ok(true)`), meaning **zero WASM execution paths are actually tested**.
- **No Transaction/Storage Hooks:** The tests manually construct a `HookContext` and pass it directly to `pipeline.execute_hook(&ctx).await`. There are no tests verifying that performing a real `NodeStorage` create/update actually triggers the hook pipeline.
- **Unused Deferred Engine:** The deferred tests merely run the pure function `eval_deferred_dispatch()` from `panorama_core`. The actual `DeferredReactorEngine` that manages cursors, polling, and failure backoff is **never executed or tested**. 

By manually synthesizing inputs and bypassing the WASM runtime, these tests provide a false sense of security. They satisfy coverage metrics but completely fail to verify the robustness of the system in a real-world scenario.

## Conclusion and Recommendations
The codebase now contains a complete, functional hook implementation that aligns with `HOOK_DESIGN.md`. The design has been faithfully implemented.

However, the testing strategy remains dangerously incomplete. To meet industry standards, the following testing gaps must be closed:
1. **E2E Transaction Interception:** Tests must create a node via the main database API (or the GraphQL endpoint) and verify that a registered reactor intercepts it.
2. **WASM Execution Coverage:** A dummy WASM plugin must be loaded during tests so that the actual FFI boundaries and `PluginLoader` integration are exercised.
3. **Deferred Engine Integration:** Tests must run the `DeferredReactorEngine::poll` loop, verifying that it correctly queries the `OpStream`, advances cursors, and applies retry/backoff logic.
