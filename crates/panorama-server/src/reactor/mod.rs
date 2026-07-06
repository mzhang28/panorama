//! Reactor & Hook Subsystem — server-side implementation.
//!
//! This module implements the reactor pipeline described in `design/HOOK_DESIGN.md`:
//!
//! - **`registry`** — stores and indexes reactors, validates on registration.
//! - **`eager`** — pre-commit hook execution with priority short-circuiting.
//! - **`deferred`** — post-commit op-stream subscriber engine.
//! - **`op_stream`** — durable operation stream for deferred reactors.

pub mod deferred;
pub mod eager;
pub mod op_stream;
pub mod registry;

pub use deferred::DeferredReactorEngine;
pub use eager::EagerReactorPipeline;
pub use op_stream::OpStream;
pub use registry::ReactorRegistry;
