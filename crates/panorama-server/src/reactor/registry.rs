//! Reactor Registry — stores, indexes, and validates reactors.
//!
//! Reactors are persisted as nodes (conforming to the `Reactors` system schema)
//! and indexed in-memory by hook point for fast lookup during write operations.

use std::collections::HashMap;

use dashmap::DashMap;
use panorama_core::reactor::{
  ActionKind, HookPoint, Reactor, ReactorMode, ReactorStatus, ReactorTrigger, WatchTrigger,
};
use panorama_core::types::{FieldValue, Node};
use tokio::sync::RwLock;
use tracing;
use uuid::Uuid;

use crate::schema_registry::SchemaRegistry;
use crate::storage::NodeStorage;

// ── Failure tracking ──────────────────────────────────────────────────────────

/// Tracks consecutive failures for a reactor (used for auto-quarantine).
#[derive(Debug, Clone, Default)]
struct FailureTracker {
  consecutive_failures: u32,
  last_failure_reason: Option<String>,
}

// ── Reactor Registry ──────────────────────────────────────────────────────────

/// Central registry for all reactors in the system.
///
/// Reactors are stored as nodes and indexed in-memory by:
/// - Hook point (for eager pre-commit dispatch)
/// - Watch trigger type (for deferred post-commit dispatch)
/// - Status (for filtering active/inactive)
///
/// Thread-safe: uses `DashMap` for concurrent read access and `RwLock` for writes.
pub struct ReactorRegistry {
  /// Reference to the storage backend for persisting reactor nodes.
  pub(crate) storage: NodeStorage,

  /// Reference to the schema registry for ownership validation.
  pub schema_registry: SchemaRegistry,

  /// In-memory index: hook point string -> list of reactor IDs.
  /// Hook points are serialized to strings for use as map keys.
  hook_index: DashMap<String, Vec<Uuid>>,

  /// In-memory index: reactor ID -> Reactor.
  reactors: DashMap<Uuid, Reactor>,

  /// Failure tracking for auto-quarantine.
  /// Keyed by reactor ID. Only tracked for eager reactors.
  failure_trackers: DashMap<Uuid, FailureTracker>,

  /// Maximum consecutive failures before auto-quarantine.
  /// Default: 3 (per HOOK_DESIGN.md §2.6)
  quarantine_threshold: u32,

  /// Whether the registry has been initialized (loaded from storage).
  initialized: RwLock<bool>,
}

impl ReactorRegistry {
  /// Create a new, empty reactor registry.
  pub fn new(storage: NodeStorage, schema_registry: SchemaRegistry) -> Self {
    Self {
      storage,
      schema_registry,
      hook_index: DashMap::new(),
      reactors: DashMap::new(),
      failure_trackers: DashMap::new(),
      quarantine_threshold: 3,
      initialized: RwLock::new(false),
    }
  }

  /// Set the quarantine threshold (max consecutive failures before auto-quarantine).
  pub fn set_quarantine_threshold(&mut self, threshold: u32) {
    self.quarantine_threshold = threshold;
  }

  // ── Initialization ────────────────────────────────────────────────────────

  /// Load all reactor nodes from storage and build in-memory indexes.
  /// Should be called once at startup after system schemas are registered.
  pub async fn initialize(&self) -> Result<(), String> {
    let mut initialized = self.initialized.write().await;
    if *initialized {
      return Ok(());
    }

    // Query for all nodes conforming to the Reactors schema
    let query =
      "MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"system\", \"reactor_name\") RETURN n";
    let rows = self
      .storage
      .query_lang(query)
      .map_err(|e| format!("Failed to load reactors from storage: {}", e))?;

    for row in &rows {
      if let Some(node) = self.node_from_row(row) {
        if let Some(reactor) = self.reactor_from_node(&node) {
          self.index_reactor(&reactor);
        }
      }
    }

    tracing::info!(
      reactor_count = self.reactors.len(),
      "Reactor registry initialized"
    );
    *initialized = true;
    Ok(())
  }

  // ── Registration ──────────────────────────────────────────────────────────

  /// Register a new reactor.
  ///
  /// Performs the following validations (per HOOK_DESIGN.md):
  /// 1. **Schema-owner scoping (§2.4)**: Eager reactors must be scoped to a schema
  ///    owned by the registering app.
  /// 2. **No-network constraint (§2.3)**: Eager reactors cannot hold capability grants.
  /// 3. **Cycle detection (§4.1)**: Checks that the reactor's triggers and actions
  ///    don't create cycles in the dependency graph.
  ///
  /// On success, persists the reactor as a node and indexes it.
  pub async fn register(&self, reactor: Reactor) -> Result<Reactor, String> {
    // ── Validation ──────────────────────────────────────────────────────

    // 1. Eager reactors must have empty capabilities (no network)
    if reactor.mode == ReactorMode::Eager && !reactor.capabilities.is_empty() {
      return Err(
        "Eager reactors cannot hold capability grants (HOOK_DESIGN §2.3). \
                 Network capabilities are forbidden on the pre-commit path."
          .into(),
      );
    }

    // 2. Validate action kind matches mode
    match (&reactor.mode, &reactor.action_kind) {
      (ReactorMode::Eager, ActionKind::Validate)
      | (ReactorMode::Eager, ActionKind::Transform)
      | (ReactorMode::Eager, ActionKind::ComputeField) => {
        // Valid eager action kinds
      }
      (ReactorMode::Deferred, ActionKind::ComputeField)
      | (ReactorMode::Deferred, ActionKind::SideEffect)
      | (ReactorMode::Deferred, ActionKind::InternalWrite) => {
        // Valid deferred action kinds
      }
      (ReactorMode::Eager, ActionKind::SideEffect) => {
        return Err(
          "SideEffect actions are not allowed for eager reactors. \
                     Use deferred mode instead (HOOK_DESIGN §2.2 vs §3.2)."
            .into(),
        );
      }
      (ReactorMode::Eager, ActionKind::InternalWrite) => {
        return Err(
          "InternalWrite actions are not allowed for eager reactors. \
                     Use deferred mode instead (HOOK_DESIGN §2.2 vs §3.2)."
            .into(),
        );
      }
      (ReactorMode::Deferred, ActionKind::Validate) => {
        return Err(
          "Validate actions are only valid for eager reactors (HOOK_DESIGN §2.2).".into(),
        );
      }
      (ReactorMode::Deferred, ActionKind::Transform) => {
        return Err(
          "Transform actions are only valid for eager reactors (HOOK_DESIGN §2.2).".into(),
        );
      }
    }

    // 3. Schema-owner scoping (HOOK_DESIGN §2.4):
    //    An eager reactor scoped to a schema must be registered by the
    //    app that owns that schema, unless a `gatekeeper` capability is held.
    if reactor.mode == ReactorMode::Eager {
      if let Some(schema_id) = reactor.owner_schema_id {
        if let Some(ref plugin_id) = reactor.registered_by_plugin {
          if let Some(app_id) = reactor.defined_by_app {
            let app_owns_schema = self
              .verify_schema_ownership(app_id, schema_id, plugin_id)
              .unwrap_or(false);
            if !app_owns_schema {
              return Err(format!(
                "Eager reactor scoped to schema '{}' must be registered \
                                 by the schema's owning app (plugin '{}'). \
                                 Schema ownership check failed (HOOK_DESIGN §2.4). \
                                 Cross-app gatekeeping requires the `gatekeeper` capability.",
                schema_id, plugin_id
              ));
            }
          }
        }
      }
    }

    // 4. Cycle detection — check against existing reactors
    if let Err(cycle_err) = self.check_cycles(&reactor) {
      return Err(format!("Cycle detected: {}", cycle_err));
    }

    // ── Persist ────────────────────────────────────────────────────────

    let node = self.reactor_to_node(&reactor);
    let created = self
      .storage
      .create(node)
      .map_err(|e| format!("Failed to persist reactor node: {}", e))?;

    let mut persisted_reactor = reactor;
    persisted_reactor.id = created.id;

    // ── Index ──────────────────────────────────────────────────────────

    self.index_reactor(&persisted_reactor);

    tracing::info!(
        reactor_id = %persisted_reactor.id,
        mode = ?persisted_reactor.mode,
        action = ?persisted_reactor.action_kind,
        "Reactor registered"
    );

    Ok(persisted_reactor)
  }

  /// Update the status of a reactor (e.g., disable, re-enable, quarantine).
  pub async fn update_status(&self, reactor_id: Uuid, status: ReactorStatus) -> Result<(), String> {
    let mut fields = HashMap::new();
    fields.insert(
      "system:reactor_status".to_string(),
      FieldValue::String(match &status {
        ReactorStatus::Active => "active".into(),
        ReactorStatus::Disabled => "disabled".into(),
        ReactorStatus::ErrorQuarantined => "error_quarantined".into(),
      }),
    );

    self
      .storage
      .update(reactor_id, fields)
      .map_err(|e| format!("Failed to update reactor status: {}", e))?;

    // Update in-memory
    if let Some(mut reactor) = self.reactors.get_mut(&reactor_id) {
      reactor.status = status.clone();
    }

    // Clear failure tracker if reactivated
    if status == ReactorStatus::Active {
      self.failure_trackers.remove(&reactor_id);
    }

    tracing::info!(
        reactor_id = %reactor_id,
        status = ?status,
        "Reactor status updated"
    );

    Ok(())
  }

  /// Delete a reactor by ID.
  pub async fn delete(&self, reactor_id: Uuid) -> Result<(), String> {
    self
      .storage
      .delete(reactor_id)
      .map_err(|e| format!("Failed to delete reactor: {}", e))?;

    // Remove from indexes
    if let Some((_, reactor)) = self.reactors.remove(&reactor_id) {
      let key = hook_point_key(&reactor.trigger);
      if let Some(mut entry) = self.hook_index.get_mut(&key) {
        entry.retain(|id| *id != reactor_id);
      }
    }
    self.failure_trackers.remove(&reactor_id);

    Ok(())
  }

  // ── Lookup ────────────────────────────────────────────────────────────────

  /// Get a reactor by ID.
  pub fn get(&self, reactor_id: &Uuid) -> Option<Reactor> {
    self.reactors.get(reactor_id).map(|r| r.clone())
  }

  /// List all registered reactors.
  pub fn list_all(&self) -> Vec<Reactor> {
    self.reactors.iter().map(|r| r.value().clone()).collect()
  }

  /// List all active reactors for a given hook point.
  /// Used by the eager reactor pipeline before each write.
  pub fn get_active_for_hook(&self, hook: &HookPoint) -> Vec<Reactor> {
    let key = hook_point_key(&ReactorTrigger::Hook(hook.clone()));
    let mut reactors: Vec<Reactor> = self
      .hook_index
      .get(&key)
      .map(|ids| {
        ids
          .iter()
          .filter_map(|id| self.reactors.get(id))
          .map(|r| r.value().clone())
          .filter(|r| r.status == ReactorStatus::Active)
          .collect()
      })
      .unwrap_or_default();

    // Sort by priority (lower runs first), then by creation time for ties
    reactors.sort_by_key(|r| (r.priority, r.created_at.clone()));
    reactors
  }

  /// List all active deferred reactors matching a given watch trigger.
  pub fn get_active_for_watch(&self, watch: &WatchTrigger) -> Vec<Reactor> {
    let key = watch_trigger_key(watch);
    let mut reactors: Vec<Reactor> = self
      .hook_index
      .get(&key)
      .map(|ids| {
        ids
          .iter()
          .filter_map(|id| self.reactors.get(id))
          .map(|r| r.value().clone())
          .filter(|r| r.status == ReactorStatus::Active && r.mode == ReactorMode::Deferred)
          .collect()
      })
      .unwrap_or_default();

    reactors.sort_by_key(|r| r.created_at.clone());
    reactors
  }

  // ── Failure tracking ──────────────────────────────────────────────────────

  /// Record a failure for the given reactor.
  /// Returns true if the reactor should be quarantined.
  pub fn record_failure(&self, reactor_id: Uuid, reason: &str) -> bool {
    let mut tracker = self.failure_trackers.entry(reactor_id).or_default();
    tracker.consecutive_failures += 1;
    tracker.last_failure_reason = Some(reason.to_string());

    tracing::warn!(
        reactor_id = %reactor_id,
        failures = tracker.consecutive_failures,
        threshold = self.quarantine_threshold,
        reason = %reason,
        "Reactor failure recorded"
    );

    tracker.consecutive_failures >= self.quarantine_threshold
  }

  /// Record a success for the given reactor (resets failure counter).
  pub fn record_success(&self, reactor_id: Uuid) {
    self.failure_trackers.remove(&reactor_id);
  }

  // ── Internal helpers ──────────────────────────────────────────────────────

  /// Add a reactor to all in-memory indexes.
  fn index_reactor(&self, reactor: &Reactor) {
    let key = hook_point_key(&reactor.trigger);
    self.hook_index.entry(key).or_default().push(reactor.id);
    self.reactors.insert(reactor.id, reactor.clone());
  }

  /// Check for cycles when adding a new reactor to the existing set.
  /// Verify that an app owns a schema (HOOK_DESIGN §2.4).
  ///
  /// Schema ownership is tracked via naming convention: schema names are
  /// prefixed with the owning plugin ID (e.g. "io.mzhang.panorama.journal/Block").
  /// Uses `SchemaRegistry.list_by_app()` to check whether a schema belongs
  /// to the registering plugin.
  fn verify_schema_ownership(
    &self,
    _app_id: Uuid,
    schema_id: Uuid,
    plugin_id: &str,
  ) -> Result<bool, String> {
    // Look up the schema
    let schema = match self.schema_registry.get(&schema_id) {
      Some(s) => s,
      None => return Ok(false),
    };

    // Check if this schema is owned by the registering plugin.
    // `list_by_app` filters schemas whose name starts with "{plugin_id}/"
    let owned_schemas = self.schema_registry.list_by_app(plugin_id);
    let owns = owned_schemas.iter().any(|s| s.node_id == schema_id);

    if !owns {
      tracing::warn!(
          schema_name = %schema.name,
          schema_id = %schema_id,
          plugin_id = %plugin_id,
          "Schema ownership check FAILED — schema not owned by registering plugin"
      );
    }

    Ok(owns)
  }

  /// Check for cycles when adding a new reactor.
  ///
  /// Uses the (from, to)-edge cycle detector from `panorama_core::reactor_eval`
  /// (HOOK_DESIGN §4.1). Edges are:
  /// - `reactor:{id}` → `field:{name}`  (reactor writes this field)
  /// - `field:{name}` → `reactor:{id}`  (reactor watches this field)
  ///
  /// A cycle exists when following these edges returns to the start:
  ///   A writes X, B writes Y; B watches X, A watches Y
  ///   → A → field:X → B → field:Y → A  (cycle)
  fn check_cycles(&self, reactor: &Reactor) -> Result<(), String> {
    use panorama_core::reactor_eval::detect_cycles_in_graph;

    let mut existing_edges: Vec<(String, String)> = Vec::new();

    // Collect all existing reactor edges
    for r in self.reactors.iter() {
      let r = r.value();
      let rid = format!("reactor:{}", r.id);
      if let Some(edges) = reactor_graph_edges(r) {
        for (from, to) in &edges {
          existing_edges.push((from.clone(), to.clone()));
        }
      }
    }

    // Get the new reactor's edges
    let new_edges = reactor_graph_edges(reactor).unwrap_or_default();

    if detect_cycles_in_graph(&existing_edges, &new_edges) {
      return Err(format!(
        "Adding reactor '{}' would create a dependency cycle",
        reactor.id
      ));
    }

    Ok(())
  }

  /// Convert a Reactor to a Node for storage.
  fn reactor_to_node(&self, reactor: &Reactor) -> Node {
    let mut node = Node::new(Uuid::nil());
    node.preferred_schemas = vec![panorama_core::types::SchemaRef {
      schema_node_id: Uuid::nil(), // Will be resolved
      version: panorama_core::types::SchemaVersion::new(1, 0),
    }];

    // Serialize complex fields as JSON
    let trigger_json = serde_json::to_value(&reactor.trigger).unwrap_or(serde_json::Value::Null);
    let filter_json = reactor
      .filter
      .as_ref()
      .map(|f| serde_json::to_value(f).unwrap_or(serde_json::Value::Null));
    let action_ref_json =
      serde_json::to_value(&reactor.action_ref).unwrap_or(serde_json::Value::Null);
    let capabilities_json =
      serde_json::to_value(&reactor.capabilities).unwrap_or(serde_json::Value::Null);
    let retry_policy_json = reactor
      .retry_policy
      .as_ref()
      .map(|rp| serde_json::to_value(rp).unwrap_or(serde_json::Value::Null));

    node.set_field(
      "system:reactor_name",
      FieldValue::String(format!("reactor-{}", reactor.id)),
    );
    if let Some(app_id) = &reactor.defined_by_app {
      node.set_field("system:defined_by_app", FieldValue::NodeRef(*app_id));
    }
    if let Some(schema_id) = &reactor.owner_schema_id {
      node.set_field("system:owner_schema_id", FieldValue::NodeRef(*schema_id));
    }
    node.set_field(
      "system:reactor_mode",
      FieldValue::String(match reactor.mode {
        ReactorMode::Eager => "eager".into(),
        ReactorMode::Deferred => "deferred".into(),
      }),
    );
    node.set_field("system:reactor_trigger", FieldValue::Json(trigger_json));
    if let Some(f) = filter_json {
      node.set_field("system:reactor_filter", FieldValue::Json(f));
    }
    node.set_field(
      "system:action_kind",
      FieldValue::String(match reactor.action_kind {
        ActionKind::Validate => "validate".into(),
        ActionKind::Transform => "transform".into(),
        ActionKind::ComputeField => "compute_field".into(),
        ActionKind::SideEffect => "side_effect".into(),
        ActionKind::InternalWrite => "internal_write".into(),
      }),
    );
    if let Some(target) = &reactor.action_target {
      node.set_field("system:action_target", FieldValue::String(target.clone()));
    }
    node.set_field("system:action_ref", FieldValue::Json(action_ref_json));
    node.set_field(
      "system:reactor_priority",
      FieldValue::Integer(reactor.priority as i64),
    );
    node.set_field(
      "system:reactor_capabilities",
      FieldValue::Json(capabilities_json),
    );
    node.set_field(
      "system:reactor_status",
      FieldValue::String(match reactor.status {
        ReactorStatus::Active => "active".into(),
        ReactorStatus::Disabled => "disabled".into(),
        ReactorStatus::ErrorQuarantined => "error_quarantined".into(),
      }),
    );
    if let Some(rp) = retry_policy_json {
      node.set_field("system:retry_policy", FieldValue::Json(rp));
    }
    if let Some(auth) = &reactor.authorized_by {
      node.set_field(
        "system:reactor_authorized_by",
        FieldValue::String(auth.clone()),
      );
    }
    if let Some(plugin_id) = &reactor.registered_by_plugin {
      node.set_field(
        "system:registered_by_plugin",
        FieldValue::String(plugin_id.clone()),
      );
    }

    node
  }

  /// Attempt to deserialize a Reactor from a Node.
  fn reactor_from_node(&self, node: &Node) -> Option<Reactor> {
    let id = node.id;
    let defined_by_app = match node.get_field("system:defined_by_app") {
      Some(FieldValue::NodeRef(u)) => Some(*u),
      _ => None,
    };
    let owner_schema_id = match node.get_field("system:owner_schema_id") {
      Some(FieldValue::NodeRef(u)) => Some(*u),
      _ => None,
    };
    let mode = match node.get_field("system:reactor_mode") {
      Some(FieldValue::String(s)) if s == "eager" => ReactorMode::Eager,
      _ => ReactorMode::Deferred,
    };
    let trigger: ReactorTrigger = node.get_field("system:reactor_trigger").and_then(|v| {
      if let FieldValue::Json(j) = v {
        serde_json::from_value(j.clone()).ok()
      } else {
        None
      }
    })?;
    let filter = node.get_field("system:reactor_filter").and_then(|v| {
      if let FieldValue::Json(j) = v {
        serde_json::from_value(j.clone()).ok()
      } else {
        None
      }
    });
    let action_kind = match node.get_field("system:action_kind") {
      Some(FieldValue::String(s)) => match s.as_str() {
        "validate" => ActionKind::Validate,
        "transform" => ActionKind::Transform,
        "compute_field" => ActionKind::ComputeField,
        "side_effect" => ActionKind::SideEffect,
        "internal_write" => ActionKind::InternalWrite,
        _ => return None,
      },
      _ => return None,
    };
    let action_target = node.get_field("system:action_target").and_then(|v| {
      if let FieldValue::String(s) = v {
        Some(s.clone())
      } else {
        None
      }
    });
    let action_ref = node.get_field("system:action_ref").and_then(|v| {
      if let FieldValue::Json(j) = v {
        serde_json::from_value(j.clone()).ok()
      } else {
        None
      }
    })?;
    let priority = match node.get_field("system:reactor_priority") {
      Some(FieldValue::Integer(i)) => *i as i32,
      _ => 0,
    };
    let capabilities = node
      .get_field("system:reactor_capabilities")
      .and_then(|v| {
        if let FieldValue::Json(j) = v {
          serde_json::from_value(j.clone()).ok()
        } else {
          None
        }
      })
      .unwrap_or_default();
    let status = match node.get_field("system:reactor_status") {
      Some(FieldValue::String(s)) => match s.as_str() {
        "active" => ReactorStatus::Active,
        "disabled" => ReactorStatus::Disabled,
        "error_quarantined" => ReactorStatus::ErrorQuarantined,
        _ => ReactorStatus::Active,
      },
      _ => ReactorStatus::Active,
    };
    let retry_policy = node.get_field("system:retry_policy").and_then(|v| {
      if let FieldValue::Json(j) = v {
        serde_json::from_value(j.clone()).ok()
      } else {
        None
      }
    });
    let authorized_by = node
      .get_field("system:reactor_authorized_by")
      .and_then(|v| {
        if let FieldValue::String(s) = v {
          Some(s.clone())
        } else {
          None
        }
      });
    let registered_by_plugin = node.get_field("system:registered_by_plugin").and_then(|v| {
      if let FieldValue::String(s) = v {
        Some(s.clone())
      } else {
        None
      }
    });
    let created_at = node.created_at.to_rfc3339();

    Some(Reactor {
      id,
      defined_by_app,
      registered_by_plugin,
      owner_schema_id,
      mode,
      trigger,
      filter,
      action_kind,
      action_target,
      action_ref,
      priority,
      capabilities,
      status,
      retry_policy,
      authorized_by,
      created_at,
    })
  }

  /// Extract a Node from a query result row.
  fn node_from_row(&self, row: &serde_json::Value) -> Option<Node> {
    serde_json::from_value(row.clone()).ok()
  }
}

// ── Key helpers ───────────────────────────────────────────────────────────────

/// Build a stable string key for a reactor trigger, used for indexing.
fn hook_point_key(trigger: &ReactorTrigger) -> String {
  match trigger {
    ReactorTrigger::Hook(hook) => match hook {
      HookPoint::BeforeNodeCreate { scope_schema_id } => {
        format!(
          "hook:before_node_create:{}",
          scope_schema_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "*".into())
        )
      }
      HookPoint::BeforeFieldWrite {
        field_path,
        scope_schema_id,
      } => {
        format!(
          "hook:before_field_write:{}:{}",
          field_path,
          scope_schema_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "*".into())
        )
      }
      HookPoint::BeforeNodeDelete { scope_schema_id } => {
        format!(
          "hook:before_node_delete:{}",
          scope_schema_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "*".into())
        )
      }
      HookPoint::BeforeSchemaInstall { scope_schema_id } => {
        format!("hook:before_schema_install:{}", scope_schema_id)
      }
      HookPoint::BeforeSchemaMigrate { scope_schema_id } => {
        format!("hook:before_schema_migrate:{}", scope_schema_id)
      }
    },
    ReactorTrigger::Watch(watch) => watch_trigger_key(watch),
  }
}

/// Build a stable string key for a watch trigger.
fn watch_trigger_key(watch: &WatchTrigger) -> String {
  match watch {
    WatchTrigger::FieldWatch { field_path, scope } => {
      format!("watch:field:{}:{:?}", field_path, scope)
    }
    WatchTrigger::LifecycleWatch { event, scope } => {
      format!("watch:lifecycle:{:?}:{:?}", event, scope)
    }
  }
}

/// Persist a reactor's delivery state to storage so cursors survive restart.
pub(crate) fn persist_reactor_state(
  registry: &ReactorRegistry,
  reactor_id: Uuid,
  last_sequence: u64,
) -> Result<(), String> {
  use panorama_core::types::FieldValue;
  use std::collections::HashMap;

  // Check if a state node already exists for this reactor
  let query = format!(
    "MATCH (n) IN space(\"default\") WHERE \
     HAS_FIELD(n, \"system\", \"rs_reactor_id\") AND \
     SCAN(n.system.rs_reactor_id = \"{}\") \
     RETURN n LIMIT 1",
    reactor_id
  );

  let rows = registry.storage.query_lang(&query)?;
  if let Some(row) = rows.first() {
    if let Some(node) = panorama_core::query::row_to_node(row) {
      let mut patch = HashMap::new();
      patch.insert(
        "system:rs_last_sequence".to_string(),
        FieldValue::Integer(last_sequence as i64),
      );
      registry.storage.update(node.id, patch)?;
      return Ok(());
    }
  }

  // No existing state node — create one
  let mut state_node = panorama_core::types::Node::new(Uuid::nil());
  state_node.set_field("system:rs_reactor_id", FieldValue::NodeRef(reactor_id));
  state_node.set_field(
    "system:rs_last_sequence",
    FieldValue::Integer(last_sequence as i64),
  );
  registry.storage.create(state_node)?;
  Ok(())
}

/// Extract directed graph edges for a reactor as (from, to) pairs.
///
/// Two kinds of edges:
/// - `(reactor:{id}, field:{name})` — the reactor *writes* this field
/// - `(field:{name}, reactor:{id})` — the reactor *watches* this field
///
/// The cycle detector follows these edges: if reactor A writes X and
/// reactor B watches X, the path is A → field:X → B. If B also writes Y
/// and A watches Y, we get B → field:Y → A, completing the cycle.
fn reactor_graph_edges(reactor: &Reactor) -> Option<Vec<(String, String)>> {
  let mut edges: Vec<(String, String)> = Vec::new();
  let rid = format!("reactor:{}", reactor.id);

  // INPUT edges: field → reactor (watching this field triggers this reactor)
  let mut extract_field_path = |fp: &str| {
    edges.push((format!("field:{}", fp), rid.clone()));
  };
  match &reactor.trigger {
    ReactorTrigger::Hook(hook) => match hook {
      HookPoint::BeforeFieldWrite { field_path, .. } => {
        extract_field_path(field_path);
      }
      _ => {}
    },
    ReactorTrigger::Watch(watch) => match watch {
      WatchTrigger::FieldWatch { field_path, .. } => {
        extract_field_path(field_path);
      }
      _ => {} // LifecycleWatch edges omitted for now — no cycle risk
    },
  }

  // OUTPUT edges: reactor → field (this reactor writes this field)
  if let Some(target) = &reactor.action_target {
    edges.push((rid.clone(), format!("field:{}", target)));
  }
  // InternalWrite reactors can trigger writes to arbitrary fields,
  // so they create a self-loop edge that catches chains.
  if reactor.action_kind == ActionKind::InternalWrite {
    // Internal writes are too broad for static cycle detection
    // beyond simple self-reference. Mark as a potential cycle node.
    edges.push((rid.clone(), format!("field:*")));
  }

  if edges.is_empty() {
    None
  } else {
    Some(edges)
  }
}
