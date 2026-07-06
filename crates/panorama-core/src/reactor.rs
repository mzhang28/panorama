//! Reactor & Hook Subsystem — core types.
//!
//! Implements the data model described in `design/HOOK_DESIGN.md`.
//! Reactors are stored as nodes conforming to the `Reactors` system schema.
//! This module defines the Rust-level types used by the reactor registry,
//! eager pre-commit pipeline, deferred post-commit op-stream engine, and
//! the WASM action execution bridge.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::query::ast::Predicate;

// ── Reactor ───────────────────────────────────────────────────────────────────

/// A reactor declares: watch something, run WASM, optionally write a result.
/// Stored as a node conforming to the `Reactors` system schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reactor {
  /// Unique identifier for this reactor (the node id).
  pub id: Uuid,
  /// The app that defined this reactor (weak ref, survives app uninstall).
  pub defined_by_app: Option<Uuid>,
  /// The plugin ID string of the app that registered this reactor.
  /// Used for schema ownership verification (HOOK_DESIGN §2.4).
  pub registered_by_plugin: Option<String>,
  /// Schema this reactor is scoped to, if any.
  pub owner_schema_id: Option<Uuid>,
  /// Whether this reactor runs pre-commit (eager) or post-commit (deferred).
  pub mode: ReactorMode,
  /// What triggers this reactor — a hook point for eager reactors,
  /// or a FieldWatch / LifecycleWatch for deferred reactors.
  pub trigger: ReactorTrigger,
  /// Optional filter predicate using the query-language WHERE syntax.
  /// Applied to the triggering node/op before executing the action.
  pub filter: Option<Predicate>,
  /// What the reactor does when triggered.
  pub action_kind: ActionKind,
  /// Target field path (for compute_field actions).
  pub action_target: Option<String>,
  /// Reference to the WASM function to execute.
  pub action_ref: WasmRef,
  /// Execution priority for eager reactors (lower runs first).
  pub priority: i32,
  /// Capability grants for this reactor.
  /// Always empty for eager reactors (network capability forbidden).
  pub capabilities: Vec<CapRef>,
  /// Current reactor status.
  pub status: ReactorStatus,
  /// Retry policy for deferred reactors.
  pub retry_policy: Option<RetryPolicy>,
  /// The user who authorized this reactor's registration.
  /// Per HOOK_DESIGN §4.2: reactor actions execute under the authority
  /// of this user, not the app that defined the reactor and not the
  /// user whose write triggered it.
  pub authorized_by: Option<String>,
  /// When this reactor was created.
  pub created_at: String,
}

// ── Reactor Mode ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReactorMode {
  /// Runs inside the triggering transaction, before commit.
  /// Can reject or transform the write.
  Eager,
  /// Runs after the triggering transaction commits.
  /// Subscribes to the durable op stream.
  Deferred,
}

// ── Reactor Trigger ───────────────────────────────────────────────────────────

/// Unified trigger type covering both eager hook points and deferred watches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum ReactorTrigger {
  /// An eager hook point — the reactor fires before a specific write operation.
  Hook(HookPoint),
  /// A deferred watch — the reactor subscribes to op stream events.
  Watch(WatchTrigger),
}

/// Eager hook points — where in the write path a reactor fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
  /// Before a node is created. Scope: schema_id.
  BeforeNodeCreate { scope_schema_id: Option<Uuid> },
  /// Before a field is written. Scope: field_path, schema_id.
  BeforeFieldWrite {
    field_path: String,
    scope_schema_id: Option<Uuid>,
  },
  /// Before a node is deleted. Scope: schema_id.
  BeforeNodeDelete { scope_schema_id: Option<Uuid> },
  /// Before a schema is installed.
  BeforeSchemaInstall { scope_schema_id: Uuid },
  /// Before a schema is migrated.
  BeforeSchemaMigrate { scope_schema_id: Uuid },
}

/// Deferred watch triggers — what the reactor subscribes to on the op stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "watch_type")]
pub enum WatchTrigger {
  /// Watch for changes to a specific field.
  FieldWatch {
    field_path: String,
    scope: WatchScope,
  },
  /// Watch for lifecycle events (node created/deleted, schema installed/migrated, app installed/uninstalled).
  LifecycleWatch {
    event: LifecycleEvent,
    scope: WatchScope,
  },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchScope {
  /// Only events within a specific schema.
  SchemaId(Uuid),
  /// Only events within a specific space.
  SpaceId(Uuid),
  /// All events the reactor is authorized to see.
  Global,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleEvent {
  NodeCreated,
  NodeDeleted,
  SchemaInstalled,
  SchemaMigrated,
  AppInstalled,
  AppUninstalled,
}

// ── Action Kinds ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
  /// Approve or reject the write. No side effects.
  /// Eager only.
  Validate,
  /// Rewrite the value being written before it commits.
  /// Eager only.
  Transform,
  /// Compute and write a field value.
  /// Eager or deferred.
  ComputeField,
  /// External calls (send email, hit webhook).
  /// Deferred only.
  SideEffect,
  /// Write to other nodes, non-transactionally, eventually consistent.
  /// Deferred only.
  InternalWrite,
}

// ── WASM Reference ────────────────────────────────────────────────────────────

/// Reference to a WASM function that executes the reactor's action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRef {
  /// The plugin/app ID that provides the WASM module.
  pub plugin_id: String,
  /// The exported function name to call.
  pub function_name: String,
}

// ── Capability Reference ──────────────────────────────────────────────────────

/// Reference to a named capability grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapRef {
  pub name: String,
}

// ── Reactor Status ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReactorStatus {
  /// Reactor is active and will fire when triggered.
  Active,
  /// Reactor is manually disabled.
  Disabled,
  /// Reactor has been automatically quarantined after N consecutive failures.
  /// Quarantined eager reactors stop blocking writes.
  ErrorQuarantined,
}

// ── Retry Policy ──────────────────────────────────────────────────────────────

/// Retry policy for deferred reactors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
  /// Maximum number of retry attempts before dead-lettering.
  pub max_retries: u32,
  /// Base backoff duration in milliseconds.
  pub base_backoff_ms: u64,
  /// Maximum backoff duration in milliseconds.
  pub max_backoff_ms: u64,
  /// Backoff multiplier (e.g., 2.0 for exponential backoff).
  pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
  fn default() -> Self {
    Self {
      max_retries: 5,
      base_backoff_ms: 1000,
      max_backoff_ms: 60_000,
      backoff_multiplier: 2.0,
    }
  }
}

// ── Eager Reactor Result ──────────────────────────────────────────────────────

/// The result of running an eager reactor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EagerReactorResult {
  /// Validation passed; the write may proceed.
  Approved,
  /// Validation rejected the write with a reason.
  Rejected { reason: String },
  /// The value was transformed; use the new value.
  Transformed { new_value: serde_json::Value },
  /// A computed field value was produced.
  Computed {
    field_key: String,
    value: serde_json::Value,
  },
}

// ── Op Stream Entry ───────────────────────────────────────────────────────────

/// A single entry in the durable operation stream.
/// Written atomically with the transaction that produces it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpStreamEntry {
  /// Monotonically increasing sequence number.
  pub sequence: u64,
  /// The type of operation that occurred.
  pub op_type: OpType,
  /// The node ID affected (if applicable).
  pub node_id: Option<Uuid>,
  /// The schema ID affected (if applicable).
  pub schema_id: Option<Uuid>,
  /// The space ID where the operation occurred.
  pub space_id: Uuid,
  /// The field path that was written (for field writes).
  pub field_path: Option<String>,
  /// The new value that was written (for field writes).
  pub new_value: Option<serde_json::Value>,
  /// The previous value (for field writes).
  pub previous_value: Option<serde_json::Value>,
  /// The user who authorized the operation.
  pub authorized_by: Option<String>,
  /// ISO 8601 timestamp of when the operation was committed.
  pub committed_at: String,
  /// The app ID that triggered this operation (if any).
  pub source_app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OpType {
  NodeCreated,
  NodeUpdated,
  NodeDeleted,
  FieldWritten,
  SchemaInstalled,
  SchemaMigrated,
  AppInstalled,
  AppUninstalled,
}

// ── Deferred Reactor State ────────────────────────────────────────────────────

/// Tracks the delivery state of a deferred reactor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeferredReactorState {
  /// The reactor this state belongs to.
  pub reactor_id: Uuid,
  /// The last sequence number this reactor has processed.
  pub last_processed_sequence: u64,
  /// Number of consecutive failures for the current event.
  pub consecutive_failures: u32,
  /// Events that have been dead-lettered (sequence -> failure reason).
  pub dead_lettered: Vec<DeadLetter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetter {
  pub sequence: u64,
  pub reason: String,
  pub timestamp: String,
}

// ── Reactor Execution Context ─────────────────────────────────────────────────

/// Context passed to a reactor's WASM action when it executes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorExecutionContext {
  /// The reactor that is executing.
  pub reactor_id: Uuid,
  /// The hook point that triggered this execution (eager reactors).
  pub hook_point: Option<HookPoint>,
  /// The trigger that fired (deferred reactors).
  pub watch_trigger: Option<WatchTrigger>,
  /// The node that triggered the reactor (if applicable).
  pub triggering_node: Option<serde_json::Value>,
  /// The op stream entry that triggered the reactor (deferred reactors).
  pub triggering_op: Option<OpStreamEntry>,
  /// The user who authorized the original write.
  pub authorized_by: Option<String>,
  /// The user who authorized the reactor's registration.
  pub reactor_authorized_by: Option<String>,
}

// ── Reactor Input/Output for WASM ─────────────────────────────────────────────

/// Input passed to a reactor's WASM action function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorActionInput {
  /// The execution context.
  pub context: ReactorExecutionContext,
  /// The current value being written (for transform/validate hooks).
  pub current_value: Option<serde_json::Value>,
  /// The node being operated on (for node-level hooks).
  pub node: Option<serde_json::Value>,
}

/// Output returned from a reactor's WASM action function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorActionOutput {
  /// The result of the action.
  pub result: EagerReactorResult,
  /// Optional log message.
  pub log: Option<String>,
}
