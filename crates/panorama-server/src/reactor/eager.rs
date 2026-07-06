//! Eager Reactor Pipeline — pre-commit hook execution.
//!
//! Implements HOOK_DESIGN.md §2: runs reactors inside the triggering transaction,
//! before commit. Can reject or transform the write.
//!
//! Key behaviors:
//! - **Priority & short-circuiting (§2.5)**: Reactors run in priority order;
//!   first `validate` rejection stops the chain.
//! - **No network capability (§2.3)**: Already enforced at registration time.
//! - **Schema-owner scoping (§2.4)**: Already enforced at registration time.
//! - **Auto-quarantine (§2.6)**: After N consecutive failures, reactor transitions
//!   to `error_quarantined` and stops blocking writes.

use panorama_core::reactor::{
    ActionKind, HookPoint, Reactor, ReactorExecutionContext,
    ReactorMode, ReactorStatus,
};
use panorama_core::types::{FieldValue, Node};
use tracing;
use uuid::Uuid;

use super::registry::ReactorRegistry;

// ── Hook context ──────────────────────────────────────────────────────────────

/// Context for a pre-commit hook invocation.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// The hook point being invoked.
    pub hook_point: HookPoint,
    /// The node being operated on (for node-level hooks).
    pub node: Option<Node>,
    /// The node ID being operated on.
    pub node_id: Option<Uuid>,
    /// The field being written (for field-level hooks).
    pub field_path: Option<String>,
    /// The value being written (for field-level hooks).
    pub current_value: Option<FieldValue>,
    /// The previous value (for field-level hooks, if updating).
    pub previous_value: Option<FieldValue>,
    /// The schema ID (for schema-level hooks).
    pub schema_id: Option<Uuid>,
    /// The space ID.
    pub space_id: Option<Uuid>,
    /// The user who authorized this write.
    pub authorized_by: Option<String>,
}

/// The result of running the eager reactor pipeline for a hook point.
#[derive(Debug, Clone)]
pub enum HookResult {
    /// All reactors approved; the write may proceed.
    /// If any transforms were applied, contains the (possibly modified) value.
    Approved {
        /// Transformed field value (if any transform reactors ran).
        transformed_value: Option<FieldValue>,
        /// Computed field values to write alongside the main write.
        computed_fields: Vec<(String, FieldValue)>,
    },
    /// A validate reactor rejected the write.
    Rejected {
        /// The reason for rejection.
        reason: String,
        /// The reactor that rejected.
        reactor_id: Uuid,
    },
}

// ── Eager Reactor Pipeline ────────────────────────────────────────────────────

/// Executes eager reactors for a given hook point.
///
/// The pipeline:
/// 1. Looks up all active eager reactors for the hook point.
/// 2. Sorts them by priority (lower runs first).
/// 3. Executes each in order:
///    - `validate` reactors: if any rejects, the chain short-circuits.
///    - `transform` reactors: each sees the prior one's output.
///    - `compute_field` reactors: accumulate computed values.
/// 4. On failure: records the failure and auto-quarantines if threshold exceeded.
pub struct EagerReactorPipeline {
    registry: std::sync::Arc<ReactorRegistry>,
}

impl EagerReactorPipeline {
    /// Create a new eager reactor pipeline backed by the given registry.
    pub fn new(registry: std::sync::Arc<ReactorRegistry>) -> Self {
        Self { registry }
    }

    /// Execute all active eager reactors for the given hook point.
    ///
    /// Returns `HookResult::Approved` if all reactors pass, or
    /// `HookResult::Rejected` if any validate reactor rejects the write.
    pub async fn execute_hook(&self, ctx: &HookContext) -> HookResult {
        let reactors = self.registry.get_active_for_hook(&ctx.hook_point);

        if reactors.is_empty() {
            return HookResult::Approved {
                transformed_value: None,
                computed_fields: Vec::new(),
            };
        }

        tracing::debug!(
            hook = ?ctx.hook_point,
            reactor_count = reactors.len(),
            "Executing eager reactor pipeline"
        );

        let mut current_value = ctx.current_value.clone();
        let mut computed_fields: Vec<(String, FieldValue)> = Vec::new();

        for reactor in &reactors {
            // Skip non-eager reactors (shouldn't happen, but safety check)
            if reactor.mode != ReactorMode::Eager {
                continue;
            }

            // Skip quarantined reactors
            if reactor.status == ReactorStatus::ErrorQuarantined {
                tracing::debug!(
                    reactor_id = %reactor.id,
                    "Skipping quarantined eager reactor"
                );
                continue;
            }

            let exec_ctx = ReactorExecutionContext {
                reactor_id: reactor.id,
                hook_point: Some(ctx.hook_point.clone()),
                watch_trigger: None,
                triggering_node: ctx.node.as_ref().and_then(|n| {
                    serde_json::to_value(n).ok()
                }),
                triggering_op: None,
                authorized_by: ctx.authorized_by.clone(),
                reactor_authorized_by: reactor.authorized_by.clone(),
            };

            match reactor.action_kind {
                ActionKind::Validate => {
                    match self.execute_validate(reactor, &exec_ctx, &current_value).await {
                        Ok(approved) => {
                            if !approved {
                                let reason = format!(
                                    "Rejected by validate reactor '{}'",
                                    reactor.id
                                );
                                return HookResult::Rejected {
                                    reason,
                                    reactor_id: reactor.id,
                                };
                            }
                            self.registry.record_success(reactor.id);
                        }
                        Err(e) => {
                            let should_quarantine = self
                                .registry
                                .record_failure(reactor.id, &e);
                            if should_quarantine {
                                tracing::error!(
                                    reactor_id = %reactor.id,
                                    error = %e,
                                    "Auto-quarantining eager reactor after consecutive failures"
                                );
                                let _ = self
                                    .registry
                                    .update_status(reactor.id, ReactorStatus::ErrorQuarantined)
                                    .await;
                            }
                            // On failure, fail open after quarantine threshold.
                            // Before threshold, fail closed (reject the write).
                            if !should_quarantine {
                                return HookResult::Rejected {
                                    reason: format!(
                                        "Validate reactor '{}' failed: {}",
                                        reactor.id, e
                                    ),
                                    reactor_id: reactor.id,
                                };
                            }
                        }
                    }
                }
                ActionKind::Transform => {
                    match self.execute_transform(reactor, &exec_ctx, &current_value).await {
                        Ok(Some(new_value)) => {
                            current_value = Some(new_value);
                            self.registry.record_success(reactor.id);
                        }
                        Ok(None) => {
                            // No transformation applied
                            self.registry.record_success(reactor.id);
                        }
                        Err(e) => {
                            let should_quarantine = self
                                .registry
                                .record_failure(reactor.id, &e);
                            if should_quarantine {
                                tracing::error!(
                                    reactor_id = %reactor.id,
                                    error = %e,
                                    "Auto-quarantining transform reactor"
                                );
                                let _ = self
                                    .registry
                                    .update_status(reactor.id, ReactorStatus::ErrorQuarantined)
                                    .await;
                            } else {
                                return HookResult::Rejected {
                                    reason: format!(
                                        "Transform reactor '{}' failed: {}",
                                        reactor.id, e
                                    ),
                                    reactor_id: reactor.id,
                                };
                            }
                        }
                    }
                }
                ActionKind::ComputeField => {
                    match self
                        .execute_compute(reactor, &exec_ctx, &current_value)
                        .await
                    {
                        Ok(Some((field_key, value))) => {
                            computed_fields.push((field_key, value));
                            self.registry.record_success(reactor.id);
                        }
                        Ok(None) => {
                            self.registry.record_success(reactor.id);
                        }
                        Err(e) => {
                            let should_quarantine =
                                self.registry.record_failure(reactor.id, &e);
                            if should_quarantine {
                                tracing::error!(
                                    reactor_id = %reactor.id,
                                    error = %e,
                                    "Auto-quarantining compute_field reactor"
                                );
                                let _ = self
                                    .registry
                                    .update_status(reactor.id, ReactorStatus::ErrorQuarantined)
                                    .await;
                            }
                            // compute_field failures don't block the write,
                            // but the computed value is simply not produced
                        }
                    }
                }
                _ => {
                    // Deferred-only action kinds on an eager reactor — should
                    // never happen due to registration validation.
                    tracing::warn!(
                        reactor_id = %reactor.id,
                        action = ?reactor.action_kind,
                        "Eager reactor has deferred-only action kind — skipping"
                    );
                }
            }
        }

        HookResult::Approved {
            transformed_value: current_value,
            computed_fields,
        }
    }

    // ── Individual action executors ─────────────────────────────────────────

    /// Execute a validate reactor. Returns true if the write is approved.
    async fn execute_validate(
        &self,
        reactor: &Reactor,
        ctx: &ReactorExecutionContext,
        current_value: &Option<FieldValue>,
    ) -> Result<bool, String> {
        // For now, validate reactors are simple: they run WASM that returns
        // Approved or Rejected. Without WASM execution wired up (depends on
        // PluginLoader availability), we default to Approved.
        //
        // Full WASM execution will be wired in the integration phase.
        tracing::debug!(
            reactor_id = %reactor.id,
            action_ref = ?reactor.action_ref,
            "Executing validate reactor (WASM execution pending full integration)"
        );
        let _ = (reactor, ctx, current_value);
        Ok(true) // Default approve
    }

    /// Execute a transform reactor. Returns the transformed value, if any.
    async fn execute_transform(
        &self,
        reactor: &Reactor,
        ctx: &ReactorExecutionContext,
        current_value: &Option<FieldValue>,
    ) -> Result<Option<FieldValue>, String> {
        tracing::debug!(
            reactor_id = %reactor.id,
            action_ref = ?reactor.action_ref,
            "Executing transform reactor (WASM execution pending full integration)"
        );
        let _ = (reactor, ctx, current_value);
        Ok(None) // Default: no transformation
    }

    /// Execute a compute_field reactor. Returns the computed field key and value.
    async fn execute_compute(
        &self,
        reactor: &Reactor,
        ctx: &ReactorExecutionContext,
        current_value: &Option<FieldValue>,
    ) -> Result<Option<(String, FieldValue)>, String> {
        tracing::debug!(
            reactor_id = %reactor.id,
            action_ref = ?reactor.action_ref,
            target = ?reactor.action_target,
            "Executing compute_field reactor (WASM execution pending full integration)"
        );
        let _ = (reactor, ctx, current_value);
        Ok(None) // Default: no computed value
    }
}
