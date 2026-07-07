# Wasm Plugin Backtrace Capture — Design

## Goal

Server-side wasm plugin runtime needs to produce backtraces for guest errors that the host can send to Sentry, for both crashing (trap) and non-crashing (`Result::Err`) failure paths, without requiring 3rd-party plugin authors to follow an unenforceable convention correctly.

## Foundational constraint

The wasm ISA has no instruction to inspect its own call stack — this is an intentional sandbox boundary, not a missing feature. Consequently `std::backtrace::Backtrace::capture()` (and therefore `anyhow`'s backtrace support, which is a thin wrapper around it) silently degrades to `BacktraceStatus::Unsupported` on `wasm32-*` targets: no panic, no error, just an empty backtrace. This is why "just use anyhow" looks like it should work and doesn't.

## Why host-side capture works anyway

The host runtime (Wasmtime) JIT-compiles wasm to native machine code (Cranelift). Compiled wasm-to-wasm calls are ordinary native `call` instructions with a real stack and real frame-pointer chains, maintained the same way as any compiled program (`Config::native_unwind_info`). Two consequences:

- This stack is only readable by a **native observer outside the sandbox** — i.e. the host embedder — never by wasm bytecode itself.
- The host only regains control at explicit boundary crossings: a host import call, or a trap. Plain guest-internal calls run invisibly; the host isn't notified per call and doesn't need to be — it can inspect whatever stack already exists, on demand, at the moment it's given control.

`wasmtime::WasmBacktrace::force_capture(&mut impl AsContextMut)` does exactly this: walks the live frame-pointer chain and returns every wasm frame present, all the way to the top-level export, in one call — regardless of how many invisible guest-internal calls led there.

## Two failure modes, two different treatments

### Traps (panic, `unreachable`, OOB access, stack overflow) — free

Wasmtime automatically attaches a `WasmBacktrace` to the error/trap returned from a call, via the same frame-pointer walk, with zero guest cooperation and zero design work needed.

Gap: the panic *message* isn't preserved by default on a wasm32 abort. Fix: a one-time panic hook installed at plugin init (not per call site) that reports message + `Location` via a fire-and-forget host import.

### Recoverable errors (`Result::Err` returned normally) — hard case

`?`-propagated returns genuinely pop each wasm frame as they happen. Nothing can be reconstructed after the fact — if nothing captured while a frame was live, that information is gone permanently by the time the error reaches the export boundary.

Mechanism: the guest calls a host import **at error-construction time**. Because that call happens synchronously, nested inside the still-live call chain, `force_capture` inside the host import returns the complete chain back to the top-level export — not just the current frame.

Correctness rule: capture once, at the origin (first construction), never on every `?` hop. One call already contains every caller. Re-capturing per hop is redundant, and if an intermediate layer converts to a different error type, risks silently dropping the original capture.

## Enforced vs. recommended

Considered and **rejected**: making the error type a Component Model resource constructible only by a host function, so the host round trip is structurally unavoidable. Rejected because it forces a cost (one host call, minimum, per error object) on every plugin unconditionally, regardless of whether deep diagnostics are actually needed — too rigid for a "warn, don't force" default, and requires migrating to the component model as a hard dependency of the design.

Adopted instead — tiered, opt-in:

1. **Default (zero host round trip):** `PluginError::new()` uses `#[track_caller]` to record `(file, line)` locally in the guest. Always correct for the origin line, no cooperation-dependent depth. Combined with what the host already knows for free — which export was called, with what input, plugin id/version — this is often sufficient for triage on its own.
2. **Opt-in upgrade:** the same constructor optionally calls a host import (`capture_backtrace() -> id`) that runs `force_capture` host-side, giving the full call chain for plugin authors who want deeper diagnostics. Cost is paid once per error object, not per function call.

## Rejected approaches, with reasons

- **`anyhow`/`std::backtrace` inside the guest** — silently `Unsupported` on wasm32; no substrate to walk from inside the sandbox.
- **Host-maintained shadow stack via a host import on every guest-to-guest call (push/pop)** — 2×N host trampoline calls per invocation, still requires guest instrumentation to know when to push/pop, needs overflow/desync handling on trap, and reconstructs something `force_capture` already provides for free in O(depth), on demand.
- **Guest-local software shadow stack** (`#[traced]` attribute + thread-local + `Drop` guard, no host calls) — viable, but superseded by tier 2 above: a single host call at error origin captures every real frame in the chain, instrumented or not, whereas the shadow stack only records frames for functions explicitly annotated.
- **ABI-enforced host-only-constructible error resource** — real enforcement (a forged handle fails the host's resource-table validation), but imposes an unconditional per-error host round trip and a hard component-model dependency; rejected as disproportionate to the goal.

## Symbolication (separate axis from capture)

Frame identity — module + function offset — is available from `force_capture` with no debug info at all. Human-readable function/file/line needs DWARF, which should *not* live in the binary a 3rd-party plugin ships (binary size, and you can't guarantee they'd retain it anyway).

Use Sentry's native WASM debug-file support instead of parsing DWARF host-side:

- A `build_id` custom section identifies the binary (LLVM ≥17 emits this automatically; otherwise inject via `wasm-split`).
- `wasm-split` splits DWARF into a separate debug companion, uploaded to Sentry once at publish time; only the stripped binary is what actually runs.
- Symbolication happens server-side in Sentry against the uploaded companion — the host never needs `addr2line` or DWARF parsing.

This is enforceable where guest cooperation isn't: gate it at the plugin package pipeline (publish/signing step) — reject packages lacking a valid `build_id`, require the debug companion upload as part of that step. It's a property of shipped bytes checked at a boundary you control, not something dependent on plugin author diligence.

## Sentry wire details

- `WasmBacktrace::frames()` is innermost-first. Sentry's `Stacktrace.frames` requires oldest-to-newest (caller-to-callee), with the last frame being the error site — reverse before sending.
- `debug_meta.images`: `{ type: "wasm", debug_id: <build_id>, code_file: <plugin-id> }`; each frame carries `instruction_addr`.
- Trap path and recoverable-error path are mutually exclusive per event.

## Open items

- `force_capture` cost on the opt-in path is a frame-pointer walk (cheap) but non-zero; if a plugin uses errors as control flow in a hot loop, consider a rate limit or capability/quota gate rather than a hard block.
- No component-model dependency is required by the adopted design — panic-hook and trap-path capture work regardless of core-wasm vs. component model, and the opt-in host-import capture works as either a raw import or a WIT function.
- Guest build should strip everything except the `build_id` section, so the split-debug-companion pipeline gets a clean stripped artifact to ship.
