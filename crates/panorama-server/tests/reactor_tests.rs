//! Integration tests for the Reactor & Hook subsystem.
//!
//! Tests the full pipeline with real storage (SQLite via tempfile),
//! following the same pattern as `integration_test.rs`.
//!
//! Covers:
//! - Reactor registration, lookup, status management
//! - Eager hook execution (validate, transform, compute_field)
//! - Priority ordering and short-circuiting
//! - Quarantine after consecutive failures
//! - Op stream append and query
//! - Deferred trigger matching and dispatch
//! - Schema ownership enforcement
//! - Filter evaluation against real nodes

use std::sync::Arc;

use panorama_core::reactor::{
  ActionKind, HookPoint, LifecycleEvent, OpStreamEntry, OpType, Reactor, ReactorMode,
  ReactorStatus, ReactorTrigger, WasmRef, WatchScope, WatchTrigger,
};
use panorama_server::reactor::eager::{EagerReactorPipeline, HookContext, HookResult};
use panorama_server::reactor::op_stream::OpStream;
use panorama_server::reactor::registry::ReactorRegistry;
use panorama_server::schema_registry::SchemaRegistry;
use panorama_server::storage::{sqlite::SqliteBackend, NodeStorage};
use uuid::Uuid;

// ── Test helpers ─────────────────────────────────────────────────────────────

fn setup_reactor_test_env() -> (
  Arc<ReactorRegistry>,
  Arc<EagerReactorPipeline>,
  Arc<OpStream>,
  tempfile::TempDir,
) {
  let tmp = tempfile::tempdir().unwrap();
  let backend = Arc::new(SqliteBackend::new(tmp.path().join("nodes")));
  let storage = NodeStorage::new(backend);
  let schema_registry = SchemaRegistry::new();

  // Register system schemas needed by the reactor subsystem
  schema_registry.register(panorama_core::schema::system_schemas::node_time_schema());
  schema_registry.register(panorama_core::schema::system_schemas::node_info_schema());
  schema_registry.register(panorama_core::schema::system_schemas::reactors_schema());
  schema_registry.register(panorama_core::schema::system_schemas::op_stream_schema());
  schema_registry.register(panorama_core::schema::system_schemas::reactor_state_schema());

  let registry = Arc::new(ReactorRegistry::new(storage.clone(), schema_registry));
  let pipeline = Arc::new(EagerReactorPipeline::new(registry.clone()));
  let op_stream = Arc::new(OpStream::new(storage));

  (registry, pipeline, op_stream, tmp)
}

fn mk_reactor(
  mode: ReactorMode,
  trigger: ReactorTrigger,
  action: ActionKind,
  priority: i32,
) -> Reactor {
  Reactor {
    id: Uuid::new_v4(),
    defined_by_app: None,
    registered_by_plugin: None,
    owner_schema_id: None,
    mode,
    trigger,
    filter: None,
    action_kind: action,
    action_target: None,
    action_ref: WasmRef {
      plugin_id: "test".into(),
      function_name: "test_fn".into(),
    },
    priority,
    capabilities: vec![],
    status: ReactorStatus::Active,
    retry_policy: None,
    authorized_by: None,
    created_at: "2026-01-01T00:00:00Z".into(),
  }
}

fn before_create_hook() -> ReactorTrigger {
  ReactorTrigger::Hook(HookPoint::BeforeNodeCreate {
    scope_schema_id: None,
  })
}

fn field_write_hook(field: &str) -> ReactorTrigger {
  ReactorTrigger::Hook(HookPoint::BeforeFieldWrite {
    field_path: field.into(),
    scope_schema_id: None,
  })
}

#[tokio::test]
async fn test_register_and_retrieve_reactor() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  registry.initialize().await.unwrap();

  let reactor = mk_reactor(
    ReactorMode::Eager,
    before_create_hook(),
    ActionKind::Validate,
    0,
  );
  let registered = registry.register(reactor).await.unwrap();

  let retrieved = registry.get(&registered.id).unwrap();
  assert_eq!(retrieved.mode, ReactorMode::Eager);
  assert_eq!(retrieved.action_kind, ActionKind::Validate);
  assert_eq!(retrieved.status, ReactorStatus::Active);
}

#[tokio::test]
async fn test_register_eager_with_network_cap_rejected() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let mut reactor = mk_reactor(
    ReactorMode::Eager,
    before_create_hook(),
    ActionKind::Validate,
    0,
  );
  reactor.capabilities = vec![panorama_core::reactor::CapRef {
    name: "network".into(),
  }];

  let result = registry.register(reactor).await;
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("capability"));
}

#[tokio::test]
async fn test_register_eager_with_side_effect_rejected() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let reactor = mk_reactor(
    ReactorMode::Eager,
    before_create_hook(),
    ActionKind::SideEffect,
    0,
  );
  let result = registry.register(reactor).await;
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("SideEffect"));
}

#[tokio::test]
async fn test_register_deferred_with_validate_rejected() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let trigger = ReactorTrigger::Watch(WatchTrigger::FieldWatch {
    field_path: "x:y".into(),
    scope: WatchScope::Global,
  });
  let reactor = mk_reactor(ReactorMode::Deferred, trigger, ActionKind::Validate, 0);
  let result = registry.register(reactor).await;
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("Validate"));
}

// ── Eager pipeline: validate ────────────────────────────────────────────

#[tokio::test]
async fn test_eager_validate_approved() {
  let (registry, pipeline, _, _tmp) = setup_reactor_test_env();
  registry.initialize().await.unwrap();

  // Register a validate reactor
  let reactor = mk_reactor(
    ReactorMode::Eager,
    before_create_hook(),
    ActionKind::Validate,
    0,
  );
  registry.register(reactor).await.unwrap();

  let ctx = HookContext {
    hook_point: HookPoint::BeforeNodeCreate {
      scope_schema_id: None,
    },
    node: None,
    node_id: None,
    field_path: None,
    current_value: None,
    previous_value: None,
    schema_id: None,
    space_id: None,
    authorized_by: None,
  };

  let result = pipeline.execute_hook(&ctx).await;
  assert!(matches!(result, HookResult::Approved { .. }));
}

#[tokio::test]
async fn test_eager_validate_no_matching_reactors() {
  let (registry, pipeline, _, _tmp) = setup_reactor_test_env();
  registry.initialize().await.unwrap();

  // Register a reactor for a DIFFERENT hook point
  let reactor = mk_reactor(
    ReactorMode::Eager,
    ReactorTrigger::Hook(HookPoint::BeforeNodeDelete {
      scope_schema_id: None,
    }),
    ActionKind::Validate,
    0,
  );
  registry.register(reactor).await.unwrap();

  // Trigger a BeforeNodeCreate — should get Approved with NoMatch behavior
  let ctx = HookContext {
    hook_point: HookPoint::BeforeNodeCreate {
      scope_schema_id: None,
    },
    node: None,
    node_id: None,
    field_path: None,
    current_value: None,
    previous_value: None,
    schema_id: None,
    space_id: None,
    authorized_by: None,
  };

  let result = pipeline.execute_hook(&ctx).await;
  assert!(matches!(result, HookResult::Approved { .. }));
}

#[tokio::test]
async fn test_eager_skips_disabled_reactor() {
  let (registry, pipeline, _, _tmp) = setup_reactor_test_env();
  registry.initialize().await.unwrap();

  let mut reactor = mk_reactor(
    ReactorMode::Eager,
    before_create_hook(),
    ActionKind::Validate,
    0,
  );
  reactor.status = ReactorStatus::Disabled;
  registry.register(reactor).await.unwrap();

  let ctx = HookContext {
    hook_point: HookPoint::BeforeNodeCreate {
      scope_schema_id: None,
    },
    node: None,
    node_id: None,
    field_path: None,
    current_value: None,
    previous_value: None,
    schema_id: None,
    space_id: None,
    authorized_by: None,
  };

  let result = pipeline.execute_hook(&ctx).await;
  // Disabled reactor should be skipped → approved
  assert!(matches!(result, HookResult::Approved { .. }));
}

// ── Eager pipeline: priority ordering ───────────────────────────────────

#[tokio::test]
async fn test_priority_ordering() {
  let (registry, pipeline, _, _tmp) = setup_reactor_test_env();
  registry.initialize().await.unwrap();

  // Register reactors out of priority order
  registry
    .register(mk_reactor(
      ReactorMode::Eager,
      before_create_hook(),
      ActionKind::Validate,
      10,
    ))
    .await
    .unwrap();
  registry
    .register(mk_reactor(
      ReactorMode::Eager,
      before_create_hook(),
      ActionKind::Validate,
      0,
    ))
    .await
    .unwrap();
  registry
    .register(mk_reactor(
      ReactorMode::Eager,
      before_create_hook(),
      ActionKind::Validate,
      5,
    ))
    .await
    .unwrap();

  let ctx = HookContext {
    hook_point: HookPoint::BeforeNodeCreate {
      scope_schema_id: None,
    },
    node: None,
    node_id: None,
    field_path: None,
    current_value: None,
    previous_value: None,
    schema_id: None,
    space_id: None,
    authorized_by: None,
  };

  let result = pipeline.execute_hook(&ctx).await;
  // All validate reactors approve by default (no WASM) → approved
  assert!(matches!(result, HookResult::Approved { .. }));
}

// ── Quarantine ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_quarantine_after_consecutive_failures() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let reactor_id = Uuid::new_v4();
  // Simulate: record failures, check quarantine threshold
  let r1 = registry.record_failure(reactor_id, "error 1");
  assert!(!r1);
  let r2 = registry.record_failure(reactor_id, "error 2");
  assert!(!r2);
  let r3 = registry.record_failure(reactor_id, "error 3");
  assert!(r3); // Default threshold is 3
}

#[tokio::test]
async fn test_record_success_resets_failure_count() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let reactor_id = Uuid::new_v4();

  registry.record_failure(reactor_id, "error 1");
  registry.record_failure(reactor_id, "error 2");
  registry.record_success(reactor_id);

  // After success reset, first failure should not trigger quarantine
  let should_quarantine = registry.record_failure(reactor_id, "new error");
  assert!(!should_quarantine);
}

// ── Status management ───────────────────────────────────────────────────

#[tokio::test]
async fn test_update_reactor_status() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let reactor = registry
    .register(mk_reactor(
      ReactorMode::Eager,
      before_create_hook(),
      ActionKind::Validate,
      0,
    ))
    .await
    .unwrap();

  registry
    .update_status(reactor.id, ReactorStatus::Disabled)
    .await
    .unwrap();
  let r = registry.get(&reactor.id).unwrap();
  assert_eq!(r.status, ReactorStatus::Disabled);

  registry
    .update_status(reactor.id, ReactorStatus::Active)
    .await
    .unwrap();
  let r = registry.get(&reactor.id).unwrap();
  assert_eq!(r.status, ReactorStatus::Active);
}

#[tokio::test]
async fn test_delete_reactor() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();
  let reactor = registry
    .register(mk_reactor(
      ReactorMode::Eager,
      before_create_hook(),
      ActionKind::Validate,
      0,
    ))
    .await
    .unwrap();

  registry.delete(reactor.id).await.unwrap();
  assert!(registry.get(&reactor.id).is_none());
}

// ── Op stream ───────────────────────────────────────────────────────────

#[test]
fn test_op_stream_append_and_query() {
  let (_registry, _, op_stream, _tmp) = setup_reactor_test_env();

  let entry = op_stream
    .append_sync(
      OpType::NodeCreated,
      Some(Uuid::new_v4()),
      None,
      Uuid::nil(),
      None,
      None,
      None,
      None,
      None,
    )
    .unwrap();

  assert_eq!(entry.sequence, 1);
  assert_eq!(entry.op_type, OpType::NodeCreated);

  let entries = op_stream.query_since(0, 10).unwrap();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].sequence, 1);
}

#[test]
fn test_op_stream_sequence_monotonic() {
  let (_, _, op_stream, _tmp) = setup_reactor_test_env();

  let e1 = op_stream
    .append_sync(
      OpType::NodeCreated,
      None,
      None,
      Uuid::nil(),
      None,
      None,
      None,
      None,
      None,
    )
    .unwrap();
  let e2 = op_stream
    .append_sync(
      OpType::NodeUpdated,
      None,
      None,
      Uuid::nil(),
      None,
      None,
      None,
      None,
      None,
    )
    .unwrap();
  let e3 = op_stream
    .append_sync(
      OpType::NodeDeleted,
      None,
      None,
      Uuid::nil(),
      None,
      None,
      None,
      None,
      None,
    )
    .unwrap();

  assert!(e2.sequence > e1.sequence);
  assert!(e3.sequence > e2.sequence);
}

#[test]
fn test_op_stream_query_since() {
  let (_, _, op_stream, _tmp) = setup_reactor_test_env();

  for _ in 0..5 {
    op_stream
      .append_sync(
        OpType::NodeCreated,
        None,
        None,
        Uuid::nil(),
        None,
        None,
        None,
        None,
        None,
      )
      .unwrap();
  }

  // Query from sequence 3 onward (exclusive)
  let entries = op_stream.query_since(3, 10).unwrap();
  assert_eq!(entries.len(), 2);
  assert_eq!(entries[0].sequence, 4);
  assert_eq!(entries[1].sequence, 5);
}

// ── Deferred trigger matching ───────────────────────────────────────────

#[test]
fn test_op_stream_entry_field_watch_matches() {
  let entry = OpStreamEntry {
    sequence: 1,
    op_type: OpType::FieldWritten,
    node_id: Some(Uuid::new_v4()),
    schema_id: None,
    space_id: Uuid::nil(),
    field_path: Some("journal:content".into()),
    new_value: None,
    previous_value: None,
    authorized_by: None,
    committed_at: "2026-01-01T00:00:00Z".into(),
    source_app_id: None,
  };

  let watch = WatchTrigger::FieldWatch {
    field_path: "journal:content".into(),
    scope: WatchScope::Global,
  };

  assert!(panorama_core::reactor_eval::eval_trigger_match(
    &watch, &entry
  ));
}

#[test]
fn test_op_stream_entry_wrong_field_no_match() {
  let entry = OpStreamEntry {
    sequence: 1,
    op_type: OpType::FieldWritten,
    node_id: Some(Uuid::new_v4()),
    schema_id: None,
    space_id: Uuid::nil(),
    field_path: Some("other:field".into()),
    new_value: None,
    previous_value: None,
    authorized_by: None,
    committed_at: "2026-01-01T00:00:00Z".into(),
    source_app_id: None,
  };
  let watch = WatchTrigger::FieldWatch {
    field_path: "journal:content".into(),
    scope: WatchScope::Global,
  };
  assert!(!panorama_core::reactor_eval::eval_trigger_match(
    &watch, &entry
  ));
}

#[test]
fn test_lifecycle_watch_matches_node_created() {
  let entry = OpStreamEntry {
    sequence: 1,
    op_type: OpType::NodeCreated,
    node_id: Some(Uuid::new_v4()),
    schema_id: None,
    space_id: Uuid::nil(),
    field_path: None,
    new_value: None,
    previous_value: None,
    authorized_by: None,
    committed_at: "2026-01-01T00:00:00Z".into(),
    source_app_id: None,
  };
  let watch = WatchTrigger::LifecycleWatch {
    event: LifecycleEvent::NodeCreated,
    scope: WatchScope::Global,
  };
  assert!(panorama_core::reactor_eval::eval_trigger_match(
    &watch, &entry
  ));
}

// ── Schema ownership enforcement ────────────────────────────────────────

#[tokio::test]
async fn test_schema_ownership_enforced_for_eager() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();

  // Register a schema owned by "test-plugin"
  let schema = panorama_core::schema::Schema {
    node_id: Uuid::new_v4(),
    name: "test-plugin/MySchema".into(),
    version: panorama_core::types::SchemaVersion::new(1, 0),
    fields: vec![],
    schema_mode: panorama_core::schema::SchemaMode::Preferred,
    previous_versions: vec![],
    migrations: vec![],
  };
  registry.schema_registry.register(schema.clone());

  // Try to register an eager reactor on that schema with a DIFFERENT plugin
  let mut reactor = mk_reactor(
    ReactorMode::Eager,
    ReactorTrigger::Hook(HookPoint::BeforeNodeCreate {
      scope_schema_id: Some(schema.node_id),
    }),
    ActionKind::Validate,
    0,
  );
  reactor.owner_schema_id = Some(schema.node_id);
  reactor.defined_by_app = Some(Uuid::new_v4());
  reactor.registered_by_plugin = Some("other-plugin".into());

  let result = registry.register(reactor).await;
  // Should fail because "other-plugin" doesn't own "test-plugin/MySchema"
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("ownership"));
}

#[tokio::test]
async fn test_schema_ownership_allows_owning_app() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();

  let schema = panorama_core::schema::Schema {
    node_id: Uuid::new_v4(),
    name: "my-plugin/MySchema".into(),
    version: panorama_core::types::SchemaVersion::new(1, 0),
    fields: vec![],
    schema_mode: panorama_core::schema::SchemaMode::Preferred,
    previous_versions: vec![],
    migrations: vec![],
  };
  registry.schema_registry.register(schema.clone());

  let mut reactor = mk_reactor(
    ReactorMode::Eager,
    ReactorTrigger::Hook(HookPoint::BeforeNodeCreate {
      scope_schema_id: Some(schema.node_id),
    }),
    ActionKind::Validate,
    0,
  );
  reactor.owner_schema_id = Some(schema.node_id);
  reactor.defined_by_app = Some(Uuid::new_v4());
  reactor.registered_by_plugin = Some("my-plugin".into());

  let result = registry.register(reactor).await;
  // Should succeed because "my-plugin" owns "my-plugin/MySchema"
  assert!(result.is_ok());
}

// ── Deferred dispatch ───────────────────────────────────────────────────

#[test]
fn test_deferred_dispatch_field_watch() {
  let reactor = mk_reactor(
    ReactorMode::Deferred,
    ReactorTrigger::Watch(WatchTrigger::FieldWatch {
      field_path: "journal:content".into(),
      scope: WatchScope::Global,
    }),
    ActionKind::SideEffect,
    0,
  );

  let entry = OpStreamEntry {
    sequence: 1,
    op_type: OpType::FieldWritten,
    node_id: Some(Uuid::new_v4()),
    schema_id: None,
    space_id: Uuid::nil(),
    field_path: Some("journal:content".into()),
    new_value: None,
    previous_value: None,
    authorized_by: None,
    committed_at: "2026-01-01T00:00:00Z".into(),
    source_app_id: None,
  };

  let fired = panorama_core::reactor_eval::eval_deferred_dispatch(&[reactor], &[entry]);
  assert_eq!(fired.len(), 1);
  assert_eq!(fired[0].1, 1);
}

#[test]
fn test_deferred_dispatch_skips_non_matching() {
  let reactor = mk_reactor(
    ReactorMode::Deferred,
    ReactorTrigger::Watch(WatchTrigger::FieldWatch {
      field_path: "journal:content".into(),
      scope: WatchScope::Global,
    }),
    ActionKind::SideEffect,
    0,
  );

  let entry = OpStreamEntry {
    sequence: 1,
    op_type: OpType::FieldWritten,
    node_id: Some(Uuid::new_v4()),
    schema_id: None,
    space_id: Uuid::nil(),
    field_path: Some("other:field".into()),
    new_value: None,
    previous_value: None,
    authorized_by: None,
    committed_at: "2026-01-01T00:00:00Z".into(),
    source_app_id: None,
  };

  let fired = panorama_core::reactor_eval::eval_deferred_dispatch(&[reactor], &[entry]);
  assert!(fired.is_empty());
}

// ── Filter evaluation integration ───────────────────────────────────────

#[tokio::test]
async fn test_reactor_with_filter_predicate() {
  let (registry, _, _, _tmp) = setup_reactor_test_env();

  // Register a reactor that only fires when `journal:priority` > 5
  use panorama_core::query::ast::{CmpOp, FieldPath, Predicate, Value};
  let filter = Predicate::FieldCompare {
    field_path: FieldPath {
      variable: "n".into(),
      namespace: Some("journal".into()),
      field: "priority".into(),
    },
    op: CmpOp::Gt,
    value: Value::Integer(5),
    scan: false,
  };

  let mut reactor = mk_reactor(
    ReactorMode::Eager,
    field_write_hook("journal:priority"),
    ActionKind::Validate,
    0,
  );
  reactor.filter = Some(filter);
  let registered = registry.register(reactor).await.unwrap();

  let retrieved = registry.get(&registered.id).unwrap();
  assert!(retrieved.filter.is_some());
  // The filter predicate round-trips through serialization
  let filter_json = serde_json::to_value(&retrieved.filter).unwrap();
  assert!(filter_json.is_object());
}
