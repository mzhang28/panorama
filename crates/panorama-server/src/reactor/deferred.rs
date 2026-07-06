//! Deferred Reactor Engine — post-commit op stream subscriber pipeline.
//!
//! Implements HOOK_DESIGN.md §3: runs reactors after the triggering transaction
//! commits. Cannot abort or affect the write — it already happened.
//!
//! Key behaviors:
//! - **Causally-converged state by default (§3.3)**: Reactors fire once the
//!   op stream entry is visible, not on every intermediate CRDT op.
//! - **At-least-once delivery (§3.4)**: Durable per-reactor cursor tracks progress.
//! - **Dead-lettering (§3.4)**: Exhausting the retry budget dead-letters the event
//!   rather than blocking the reactor.
//! - **Retry with backoff (§3.4)**: Configurable exponential backoff between retries.

use panorama_core::reactor::{
  ActionKind, DeadLetter, DeferredReactorState, LifecycleEvent, OpStreamEntry, OpType, Reactor,
  ReactorExecutionContext, ReactorStatus, WatchScope, WatchTrigger,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing;
use uuid::Uuid;

use super::op_stream::OpStream;
use super::registry::ReactorRegistry;

// ── Deferred Reactor Engine ───────────────────────────────────────────────────

/// The deferred reactor engine polls the op stream and dispatches matching
/// events to deferred reactors.
///
/// In the current implementation, reactors are executed inline during polling.
/// A future version may offload execution to a background task queue.
pub struct DeferredReactorEngine {
  /// Reference to the reactor registry.
  registry: Arc<ReactorRegistry>,

  /// Reference to the op stream.
  op_stream: Arc<OpStream>,

  /// Optional reference to the plugin loader for WASM execution.
  plugin_loader: Option<Arc<crate::plugin_loader::PluginLoader>>,

  /// Per-reactor delivery state, keyed by reactor ID.
  states: RwLock<HashMap<Uuid, DeferredReactorState>>,

  /// Polling interval in milliseconds.
  poll_interval_ms: u64,
}

impl DeferredReactorEngine {
  /// Create a new deferred reactor engine.
  pub fn new(registry: Arc<ReactorRegistry>, op_stream: Arc<OpStream>) -> Self {
    Self {
      registry,
      op_stream,
      plugin_loader: None,
      states: RwLock::new(HashMap::new()),
      poll_interval_ms: 1000, // 1 second default
    }
  }

  /// Set the polling interval.
  pub fn set_poll_interval(&mut self, interval_ms: u64) {
    self.poll_interval_ms = interval_ms;
  }

  /// Set the plugin loader for WASM execution.
  pub fn with_plugin_loader(mut self, loader: Arc<crate::plugin_loader::PluginLoader>) -> Self {
    self.plugin_loader = Some(loader);
    self
  }

  /// Initialize per-reactor state from storage.
  /// Should be called once at startup.
  pub async fn initialize(&self) -> Result<(), String> {
    // Load ReactorState nodes from storage
    let query =
      "MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"system\", \"rs_reactor_id\") RETURN n";
    let rows = self
      .registry
      .storage()
      .query_lang(query)
      .map_err(|e| format!("Failed to load reactor states: {}", e))?;

    let mut states = self.states.write().await;
    for row in &rows {
      if let Some(state) = self.state_from_row(row) {
        states.insert(state.reactor_id, state);
      }
    }

    tracing::info!(
      state_count = states.len(),
      "Deferred reactor engine initialized"
    );

    Ok(())
  }

  /// Poll the op stream and dispatch matching events to deferred reactors.
  ///
  /// Each call:
  /// 1. Queries new op stream entries since each reactor's last processed sequence.
  /// 2. Matches entries against reactor triggers.
  /// 3. Executes matching reactor actions.
  /// 4. Updates per-reactor cursors and handles failures.
  pub async fn poll(&self) -> Result<usize, String> {
    let mut dispatched = 0usize;

    // Get all active deferred reactors
    let all_reactors: Vec<Reactor> = self
      .registry
      .list_all()
      .into_iter()
      .filter(|r| {
        r.mode == panorama_core::reactor::ReactorMode::Deferred && r.status == ReactorStatus::Active
      })
      .collect();

    if all_reactors.is_empty() {
      return Ok(0);
    }

    let current_seq = self.op_stream.current_sequence();

    for reactor in &all_reactors {
      let last_seq = {
        let states = self.states.read().await;
        states
          .get(&reactor.id)
          .map(|s| s.last_processed_sequence)
          .unwrap_or(0)
      };

      // Skip if no new entries
      if last_seq >= current_seq {
        continue;
      }

      // Query new entries
      let entries = match self.op_stream.query_since(last_seq, 100) {
        Ok(entries) => entries,
        Err(e) => {
          tracing::warn!(
              reactor_id = %reactor.id,
              error = %e,
              "Failed to query op stream for deferred reactor"
          );
          continue;
        }
      };

      for entry in &entries {
        // Check if this entry matches the reactor's trigger
        if !self.trigger_matches(reactor, entry) {
          continue;
        }

        // Check filter predicate
        if !self.filter_matches(reactor, entry) {
          continue;
        }

        // Execute the reactor
        match self.execute_action(reactor, entry).await {
          Ok(_) => {
            dispatched += 1;
            self.update_cursor(reactor.id, entry.sequence).await;
            self.registry.record_success(reactor.id);
          }
          Err(e) => {
            tracing::warn!(
                reactor_id = %reactor.id,
                sequence = entry.sequence,
                error = %e,
                "Deferred reactor action failed"
            );
            let should_dead_letter = self.handle_failure(reactor, entry, &e).await;
            if should_dead_letter {
              // Dead-letter this event and move past it
              self.dead_letter_event(reactor.id, entry.sequence, &e).await;
              self.update_cursor(reactor.id, entry.sequence).await;
            }
            // Otherwise, stop processing for this reactor until
            // the current event succeeds or is dead-lettered
            break;
          }
        }
      }
    }

    Ok(dispatched)
  }

  /// Start the polling loop. Runs until the provided cancellation token fires.
  pub async fn run_polling_loop(&self, mut cancel: tokio::sync::watch::Receiver<bool>) {
    tracing::info!(
      interval_ms = self.poll_interval_ms,
      "Starting deferred reactor polling loop"
    );

    loop {
      // Check for cancellation
      if *cancel.borrow() {
        tracing::info!("Deferred reactor polling loop cancelled");
        break;
      }

      match self.poll().await {
        Ok(count) => {
          if count > 0 {
            tracing::debug!(dispatched = count, "Poll cycle complete");
          }
        }
        Err(e) => {
          tracing::error!(error = %e, "Poll cycle error");
        }
      }

      // Wait for the next poll interval, checking cancellation periodically
      let _ = tokio::time::timeout(
        Duration::from_millis(self.poll_interval_ms),
        cancel.changed(),
      )
      .await;
    }
  }

  // ── Internal helpers ──────────────────────────────────────────────────────

  /// Check if an op stream entry matches a reactor's trigger.
  fn trigger_matches(&self, reactor: &Reactor, entry: &OpStreamEntry) -> bool {
    match &reactor.trigger {
      panorama_core::reactor::ReactorTrigger::Watch(watch) => match watch {
        WatchTrigger::FieldWatch { field_path, scope } => {
          // Must be a field-written event
          if entry.op_type != OpType::FieldWritten
            && entry.op_type != OpType::NodeCreated
            && entry.op_type != OpType::NodeUpdated
          {
            return false;
          }
          // Check field path matches
          if let Some(entry_fp) = &entry.field_path {
            if entry_fp != field_path {
              return false;
            }
          } else if entry.op_type == OpType::FieldWritten {
            // FieldWritten should have a field_path
            return false;
          }
          // Check scope
          self.scope_matches(scope, entry)
        }
        WatchTrigger::LifecycleWatch { event, scope } => {
          // Check event type matches
          let event_matches = match event {
            LifecycleEvent::NodeCreated => entry.op_type == OpType::NodeCreated,
            LifecycleEvent::NodeDeleted => entry.op_type == OpType::NodeDeleted,
            LifecycleEvent::SchemaInstalled => entry.op_type == OpType::SchemaInstalled,
            LifecycleEvent::SchemaMigrated => entry.op_type == OpType::SchemaMigrated,
            LifecycleEvent::AppInstalled => entry.op_type == OpType::AppInstalled,
            LifecycleEvent::AppUninstalled => entry.op_type == OpType::AppUninstalled,
          };
          if !event_matches {
            return false;
          }
          self.scope_matches(scope, entry)
        }
      },
      // Non-watch triggers shouldn't be here (they're eager)
      _ => false,
    }
  }

  /// Check if an entry is within a given watch scope.
  fn scope_matches(&self, scope: &WatchScope, entry: &OpStreamEntry) -> bool {
    match scope {
      WatchScope::SchemaId(schema_id) => entry.schema_id == Some(*schema_id),
      WatchScope::SpaceId(space_id) => entry.space_id == *space_id,
      WatchScope::Global => true,
    }
  }

  /// Check if an entry matches a reactor's filter predicate.
  ///
  /// When a filter is present, it is evaluated against the triggering node's
  /// fields using the same `eval_predicate` function used by the query engine
  /// (HOOK_DESIGN §3.1: "filter predicates reuse the query language's WHERE
  /// grammar exactly"). When no filter is present, everything matches.
  fn filter_matches(&self, reactor: &Reactor, entry: &OpStreamEntry) -> bool {
    let filter = match &reactor.filter {
      Some(f) => f,
      None => return true, // No filter = match everything
    };

    // Fetch the triggering node to evaluate the filter against
    let node = match entry.node_id {
      Some(node_id) => match self.registry.storage.get(node_id) {
        Ok(Some(n)) => n,
        _ => return false, // Node not found → filter can't match
      },
      None => return false, // No node in entry → can't evaluate field predicates
    };

    // Evaluate the predicate against the node's fields using
    // the shared query-language WHERE evaluator (HOOK_DESIGN §3.1).
    panorama_core::query::eval_predicate(filter, &node)
  }

  /// Execute a reactor's action for a given op stream entry.
  async fn execute_action(&self, reactor: &Reactor, entry: &OpStreamEntry) -> Result<(), String> {
    let exec_ctx = ReactorExecutionContext {
      reactor_id: reactor.id,
      hook_point: None,
      watch_trigger: match &reactor.trigger {
        panorama_core::reactor::ReactorTrigger::Watch(w) => Some(w.clone()),
        _ => None,
      },
      triggering_node: None,
      triggering_op: Some(entry.clone()),
      authorized_by: entry.authorized_by.clone(),
      reactor_authorized_by: reactor.authorized_by.clone(),
    };

    let input = panorama_core::reactor::ReactorActionInput {
      context: exec_ctx,
      current_value: entry.new_value.clone(),
      node: None, // Deferred reactors don't have the node in context
    };

    let loader = match &self.plugin_loader {
      Some(l) => l,
      None => {
        tracing::debug!(reactor_id = %reactor.id, "No plugin loader — skipping deferred action");
        return Ok(());
      }
    };

    match loader
      .execute_reactor_action(
        &reactor.action_ref.plugin_id,
        &reactor.action_ref.function_name,
        &input,
      )
      .await
    {
      Ok(Some(output)) => {
        tracing::debug!(
            reactor_id = %reactor.id,
            sequence = entry.sequence,
            action = ?reactor.action_kind,
            "Deferred reactor action executed via WASM"
        );

        match reactor.action_kind {
          ActionKind::ComputeField => {
            if let panorama_core::reactor::EagerReactorResult::Computed { field_key, value } =
              &output.result
            {
              let fv: panorama_core::types::FieldValue = serde_json::from_value(value.clone())
                .map_err(|e| {
                  format!("Invalid computed value from reactor {}: {}", reactor.id, e)
                })?;
              if let Some(node_id) = entry.node_id {
                let mut patch = std::collections::HashMap::new();
                patch.insert(field_key.clone(), fv);
                self
                  .registry
                  .storage()
                  .update(node_id, patch)
                  .map_err(|e| {
                    format!(
                      "Failed to write computed field '{}' from reactor {}: {}",
                      field_key, reactor.id, e
                    )
                  })?;
                tracing::info!(
                  reactor_id = %reactor.id,
                  node_id = %node_id,
                  field_key = %field_key,
                  "Deferred compute_field reactor wrote result"
                );
              }
            }
          }
          ActionKind::SideEffect => {
            // The WASM module performed the side effect via host functions
            // during execution. Log the completion.
            tracing::info!(
              reactor_id = %reactor.id,
              "Side effect completed for sequence {}",
              entry.sequence
            );
          }
          ActionKind::InternalWrite => {
            // The WASM module already wrote via host_ctx_create_node /
            // host_ctx_update_node during execution.
            tracing::info!(
              reactor_id = %reactor.id,
              "Internal write completed for sequence {}",
              entry.sequence
            );
          }
          _ => {
            // Validate/Transform on a deferred reactor — unusual but not an
            // error. The WASM ran; whatever it returned is informational.
          }
        }
        Ok(())
      }
      Ok(None) => {
        // No WASM module available — skip gracefully
        Ok(())
      }
      Err(e) => Err(e),
    }
  }

  /// Handle a failure during deferred reactor execution.
  /// Returns true if the event should be dead-lettered.
  async fn handle_failure(&self, reactor: &Reactor, entry: &OpStreamEntry, _error: &str) -> bool {
    let retry_policy = reactor.retry_policy.as_ref().cloned().unwrap_or_default();

    let mut states = self.states.write().await;
    let state = states
      .entry(reactor.id)
      .or_insert_with(|| DeferredReactorState {
        reactor_id: reactor.id,
        last_processed_sequence: 0,
        consecutive_failures: 0,
        dead_lettered: Vec::new(),
      });

    state.consecutive_failures += 1;

    if state.consecutive_failures > retry_policy.max_retries {
      tracing::warn!(
          reactor_id = %reactor.id,
          sequence = entry.sequence,
          failures = state.consecutive_failures,
          max_retries = retry_policy.max_retries,
          "Dead-lettering event after exhausting retry budget"
      );
      state.consecutive_failures = 0;
      true
    } else {
      let backoff = (retry_policy.base_backoff_ms as f64
        * retry_policy
          .backoff_multiplier
          .powi(state.consecutive_failures as i32 - 1))
      .min(retry_policy.max_backoff_ms as f64) as u64;

      tracing::debug!(
          reactor_id = %reactor.id,
          sequence = entry.sequence,
          attempt = state.consecutive_failures,
          backoff_ms = backoff,
          "Retrying deferred reactor after backoff"
      );

      // Sleep for the backoff duration before the next poll cycle retries
      tokio::time::sleep(Duration::from_millis(backoff)).await;
      false
    }
  }

  /// Update the cursor for a reactor after successfully processing an entry.
  async fn update_cursor(&self, reactor_id: Uuid, sequence: u64) {
    let mut states = self.states.write().await;
    if let Some(state) = states.get_mut(&reactor_id) {
      state.last_processed_sequence = state.last_processed_sequence.max(sequence);
      state.consecutive_failures = 0;
    } else {
      states.insert(
        reactor_id,
        DeferredReactorState {
          reactor_id,
          last_processed_sequence: sequence,
          consecutive_failures: 0,
          dead_lettered: Vec::new(),
        },
      );
    }

    // Persist to storage so cursor survives restart (§3.4).
    if let Err(e) =
      crate::reactor::registry::persist_reactor_state(&self.registry, reactor_id, sequence)
    {
      tracing::error!(reactor_id = %reactor_id, error = %e, "Failed to persist deferred reactor cursor");
    }
  }

  /// Dead-letter an event for a reactor.
  async fn dead_letter_event(&self, reactor_id: Uuid, sequence: u64, reason: &str) {
    let mut states = self.states.write().await;
    if let Some(state) = states.get_mut(&reactor_id) {
      state.dead_lettered.push(DeadLetter {
        sequence,
        reason: reason.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
      });
    }
  }

  /// Parse a DeferredReactorState from a query result row.
  fn state_from_row(&self, row: &serde_json::Value) -> Option<DeferredReactorState> {
    let node: panorama_core::types::Node = serde_json::from_value(row.clone()).ok()?;

    let reactor_id = match node.get_field("system:rs_reactor_id") {
      Some(panorama_core::types::FieldValue::NodeRef(u)) => *u,
      _ => return None,
    };
    let last_processed_sequence = match node.get_field("system:rs_last_sequence") {
      Some(panorama_core::types::FieldValue::Integer(i)) => *i as u64,
      _ => 0,
    };
    let consecutive_failures = match node.get_field("system:rs_consecutive_failures") {
      Some(panorama_core::types::FieldValue::Integer(i)) => *i as u32,
      _ => 0,
    };
    let dead_lettered = node
      .get_field("system:rs_dead_letters")
      .and_then(|v| {
        if let panorama_core::types::FieldValue::Json(j) = v {
          serde_json::from_value(j.clone()).ok()
        } else {
          None
        }
      })
      .unwrap_or_default();

    Some(DeferredReactorState {
      reactor_id,
      last_processed_sequence,
      consecutive_failures,
      dead_lettered,
    })
  }
}

// ── Accessor for ReactorRegistry ──────────────────────────────────────────────

impl ReactorRegistry {
  /// Get a reference to the storage backend.
  /// Used by DeferredReactorEngine to query ReactorState nodes.
  pub fn storage(&self) -> &crate::storage::NodeStorage {
    &self.storage
  }
}
