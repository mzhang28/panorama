//! Reference evaluator for the Reactor & Hook subsystem.
//!
//! Pure, in-memory functions that encode the correct behavior of:
//! - Eager reactor hook execution (priority ordering, short-circuiting, quarantine)
//! - Deferred reactor trigger matching and dispatch
//! - Cycle detection on dependency graphs
//!
//! Serves as the **differential oracle**: the real (storage-backed) implementation
//! in `panorama-server` must produce identical results on the same inputs.
//! This follows the same pattern as `query::eval` for the query engine.

use std::collections::HashMap;
use uuid::Uuid;

use crate::reactor::{
    ActionKind, HookPoint, LifecycleEvent, OpStreamEntry, OpType, Reactor,
    ReactorMode, ReactorStatus, ReactorTrigger, WatchScope, WatchTrigger,
};

// ── Eager Hook Evaluation ─────────────────────────────────────────────────────

/// Result of evaluating eager hooks for a single hook point.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalHookResult {
    /// All reactors approved. Contains transformed value (if any) and computed fields.
    Approved {
        transformed_value: Option<crate::types::FieldValue>,
        computed_fields: Vec<(String, crate::types::FieldValue)>,
    },
    /// A validate reactor rejected the write.
    Rejected {
        reason: String,
        reactor_id: Uuid,
    },
    /// No reactors matched this hook point.
    NoMatch,
}

/// Pure reference implementation of the eager reactor pipeline.
///
/// Given a set of reactors and a hook context, evaluates which reactors fire
/// and what the outcome is. This is the ground-truth specification — the
/// real `EagerReactorPipeline` in `panorama-server` must match this output.
pub fn eval_eager_hook(
    reactors: &[Reactor],
    hook_point: &HookPoint,
    current_value: Option<&crate::types::FieldValue>,
    failure_counts: &HashMap<Uuid, u32>,
    quarantine_threshold: u32,
) -> EvalHookResult {
    // 1. Filter: only active, eager reactors matching this hook point
    let mut matching: Vec<&Reactor> = reactors
        .iter()
        .filter(|r| {
            r.mode == ReactorMode::Eager
                && r.status == ReactorStatus::Active
                && hook_matches_reactor(hook_point, r)
        })
        .collect();

    if matching.is_empty() {
        return EvalHookResult::NoMatch;
    }

    // 2. Sort by priority (lower runs first), tie-break by created_at
    matching.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.created_at.cmp(&b.created_at)));

    let current_value = current_value.cloned();
    let computed_fields: Vec<(String, crate::types::FieldValue)> = Vec::new();

    // 3. Execute in priority order with short-circuiting
    for reactor in &matching {
        // Skip quarantined (shouldn't happen since we filtered for Active, but safety)
        if reactor.status == ReactorStatus::ErrorQuarantined {
            continue;
        }

        // Check if reactor should fail (based on failure count)
        let failures = failure_counts.get(&reactor.id).copied().unwrap_or(0);
        let quarantined = failures >= quarantine_threshold;

        match reactor.action_kind {
            ActionKind::Validate => {
                if quarantined {
                    // Fail open after quarantine — skip this reactor
                    continue;
                }
                // In the reference evaluator, validate reactors always approve.
                // The real implementation may execute WASM that rejects.
                // Tests override this by pre-marking reactors as "failing".
                if failures > 0 {
                    if quarantined {
                        continue; // fail open
                    } else {
                        return EvalHookResult::Rejected {
                            reason: format!("Validate reactor '{}' failed", reactor.id),
                            reactor_id: reactor.id,
                        };
                    }
                }
                // Approved — continue to next reactor
            }
            ActionKind::Transform => {
                if quarantined {
                    continue; // fail open
                }
                if failures > 0 {
                    if quarantined {
                        continue;
                    } else {
                        return EvalHookResult::Rejected {
                            reason: format!("Transform reactor '{}' failed", reactor.id),
                            reactor_id: reactor.id,
                        };
                    }
                }
                // In the reference evaluator, transform is a no-op unless
                // a test provides an explicit transformation.
            }
            ActionKind::ComputeField => {
                if quarantined {
                    continue;
                }
                if failures > 0 {
                    // compute_field failures don't block the write
                    continue;
                }
                // In the reference evaluator, compute_field is a no-op unless
                // a test provides an explicit computed value.
            }
            _ => {
                // Deferred-only action kinds on eager reactors — skip
            }
        }
    }

    EvalHookResult::Approved {
        transformed_value: current_value,
        computed_fields,
    }
}

/// Check whether a hook point triggers a given reactor.
fn hook_matches_reactor(hook: &HookPoint, reactor: &Reactor) -> bool {
    match &reactor.trigger {
        ReactorTrigger::Hook(reactor_hook) => hooks_match(hook, reactor_hook),
        _ => false, // Not an eager hook trigger
    }
}

/// Two hook points match if they are the same variant with compatible scopes.
fn hooks_match(actual: &HookPoint, registered: &HookPoint) -> bool {
    match (actual, registered) {
        (
            HookPoint::BeforeNodeCreate { scope_schema_id: a_scope },
            HookPoint::BeforeNodeCreate { scope_schema_id: r_scope },
        ) => scope_compatible(a_scope, r_scope),
        (
            HookPoint::BeforeFieldWrite {
                field_path: a_field,
                scope_schema_id: a_scope,
            },
            HookPoint::BeforeFieldWrite {
                field_path: r_field,
                scope_schema_id: r_scope,
            },
        ) => {
            // Field path must match exactly, scope must be compatible
            (a_field == r_field || r_field.is_empty())
                && scope_compatible(a_scope, r_scope)
        }
        (
            HookPoint::BeforeNodeDelete { scope_schema_id: a_scope },
            HookPoint::BeforeNodeDelete { scope_schema_id: r_scope },
        ) => scope_compatible(a_scope, r_scope),
        (
            HookPoint::BeforeSchemaInstall { scope_schema_id: a_schema },
            HookPoint::BeforeSchemaInstall { scope_schema_id: r_schema },
        ) => a_schema == r_schema,
        (
            HookPoint::BeforeSchemaMigrate { scope_schema_id: a_schema },
            HookPoint::BeforeSchemaMigrate { scope_schema_id: r_schema },
        ) => a_schema == r_schema,
        _ => false, // Different variants never match
    }
}

/// A registered scope of `None` matches any actual scope (wildcard).
/// A registered scope of `Some(id)` must match exactly.
fn scope_compatible(actual: &Option<Uuid>, registered: &Option<Uuid>) -> bool {
    match registered {
        None => true,  // Wildcard — matches everything
        Some(rid) => actual.as_ref() == Some(rid), // Exact match required
    }
}

// ── Trigger Matching ──────────────────────────────────────────────────────────

/// Pure reference implementation of deferred trigger matching.
///
/// Returns true if the given watch trigger matches the op stream entry.
pub fn eval_trigger_match(trigger: &WatchTrigger, entry: &OpStreamEntry) -> bool {
    match trigger {
        WatchTrigger::FieldWatch { field_path, scope } => {
            // Must be a write-related event
            let is_write = matches!(
                entry.op_type,
                OpType::NodeCreated | OpType::NodeUpdated | OpType::FieldWritten
            );
            if !is_write {
                return false;
            }
            // Check field path
            if let Some(entry_fp) = &entry.field_path {
                if entry_fp != field_path {
                    return false;
                }
            } else {
                // No field path on entry — only match if event implies all fields
                // (NodeCreated/NodeUpdated without a specific field path)
            }
            // Check scope
            scope_matches_entry(scope, entry)
        }
        WatchTrigger::LifecycleWatch { event, scope } => {
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
            scope_matches_entry(scope, entry)
        }
    }
}

fn scope_matches_entry(scope: &WatchScope, entry: &OpStreamEntry) -> bool {
    match scope {
        WatchScope::SchemaId(sid) => entry.schema_id == Some(*sid),
        WatchScope::SpaceId(sid) => entry.space_id == *sid,
        WatchScope::Global => true,
    }
}

// ── Deferred Dispatch ─────────────────────────────────────────────────────────

/// Pure reference implementation of deferred reactor dispatch.
///
/// Given reactors and op stream entries, returns which (reactor, entry) pairs
/// should fire, in the order they should be processed.
pub fn eval_deferred_dispatch(
    reactors: &[Reactor],
    entries: &[OpStreamEntry],
) -> Vec<(Uuid, u64)> {
    let mut fired: Vec<(Uuid, u64)> = Vec::new();

    for reactor in reactors {
        if reactor.mode != ReactorMode::Deferred || reactor.status != ReactorStatus::Active {
            continue;
        }

        for entry in entries {
            // Check trigger
            let trigger_matches = match &reactor.trigger {
                ReactorTrigger::Watch(watch) => eval_trigger_match(watch, entry),
                _ => false,
            };

            if trigger_matches {
                fired.push((reactor.id, entry.sequence));
            }
        }
    }

    // Sort by entry sequence (causal order)
    fired.sort_by_key(|(_, seq)| *seq);
    fired
}

// ── Cycle Detection ───────────────────────────────────────────────────────────

/// A dependency edge in the reactor/computed-field graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencyEdge {
    /// The source node (what triggers).
    pub from: String,
    /// The target node (what gets triggered).
    pub to: String,
}

/// Pure reference implementation of dependency graph cycle detection.
///
/// Returns true if adding `new_edges` to `existing_edges` would create a cycle.
/// This is the ground truth for HOOK_DESIGN §4.1.
pub fn detect_cycles_in_graph(
    existing_edges: &[(String, String)],  // (from, to) pairs
    new_edges: &[(String, String)],
) -> bool {
    // Build adjacency list: node -> nodes it triggers
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for (from, to) in existing_edges.iter().chain(new_edges.iter()) {
        adjacency
            .entry(from.clone())
            .or_default()
            .push(to.clone());
    }

    // For each new edge, check if there's a path from `to` back to `from`
    for (from, to) in new_edges {
        if has_path(to, from, &adjacency, &mut HashMap::new()) {
            return true;
        }
    }

    false
}

/// DFS to check if there's a path from `start` to `target` in the graph.
fn has_path(
    start: &str,
    target: &str,
    adjacency: &HashMap<String, Vec<String>>,
    visited: &mut HashMap<String, bool>,
) -> bool {
    if start == target {
        return true;
    }
    if visited.contains_key(start) {
        return false;
    }
    visited.insert(start.to_string(), true);

    if let Some(neighbors) = adjacency.get(start) {
        for next in neighbors {
            if has_path(next, target, adjacency, visited) {
                return true;
            }
        }
    }

    false
}

// ── Priority Ordering ─────────────────────────────────────────────────────────

/// Pure reference implementation of reactor priority sorting.
///
/// Sorts by priority (ascending), tie-breaking by created_at (ascending).
pub fn sort_reactors_by_priority(reactors: &mut [Reactor]) {
    reactors.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
}

// ── Quarantine Logic ──────────────────────────────────────────────────────────

/// Pure reference implementation of quarantine decisions.
///
/// Returns true if the reactor should be quarantined based on its failure count.
pub fn eval_quarantine(consecutive_failures: u32, threshold: u32) -> bool {
    consecutive_failures >= threshold
}

// ── Retry Backoff ─────────────────────────────────────────────────────────────

/// Pure reference implementation of exponential backoff calculation.
pub fn eval_backoff(
    attempt: u32,
    base_ms: u64,
    max_ms: u64,
    multiplier: f64,
) -> u64 {
    let backoff = (base_ms as f64 * multiplier.powi(attempt as i32 - 1)) as u64;
    backoff.min(max_ms)
}

// ── Mode × Action Validation ──────────────────────────────────────────────────

/// Pure reference implementation of the mode/action_kind compatibility matrix.
///
/// Returns true if the given mode and action kind are compatible.
pub fn is_valid_mode_action(mode: &ReactorMode, action: &ActionKind) -> bool {
    matches!(
        (mode, action),
        (ReactorMode::Eager, ActionKind::Validate)
            | (ReactorMode::Eager, ActionKind::Transform)
            | (ReactorMode::Eager, ActionKind::ComputeField)
            | (ReactorMode::Deferred, ActionKind::ComputeField)
            | (ReactorMode::Deferred, ActionKind::SideEffect)
            | (ReactorMode::Deferred, ActionKind::InternalWrite)
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactor::{
        ActionKind, HookPoint, LifecycleEvent, OpStreamEntry, OpType, Reactor, ReactorMode,
        ReactorStatus, ReactorTrigger, WasmRef, WatchScope, WatchTrigger,
    };
    use uuid::Uuid;

    // ── Helpers ───────────────────────────────────────────────────────────

    fn mk_reactor(
        id: Uuid,
        mode: ReactorMode,
        trigger: ReactorTrigger,
        action: ActionKind,
        priority: i32,
    ) -> Reactor {
        Reactor {
            id,
            defined_by_app: None,
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

    fn mk_entry(sequence: u64, op_type: OpType, field_path: Option<&str>, schema_id: Option<Uuid>) -> OpStreamEntry {
        OpStreamEntry {
            sequence,
            op_type,
            node_id: Some(Uuid::new_v4()),
            schema_id,
            space_id: Uuid::nil(),
            field_path: field_path.map(|s| s.to_string()),
            new_value: None,
            previous_value: None,
            authorized_by: None,
            committed_at: "2026-01-01T00:00:00Z".into(),
            source_app_id: None,
        }
    }

    // ── Hook matching tests ───────────────────────────────────────────────

    #[test]
    fn test_before_node_create_matches_with_wildcard_scope() {
        let actual = HookPoint::BeforeNodeCreate {
            scope_schema_id: Some(Uuid::new_v4()),
        };
        let registered = HookPoint::BeforeNodeCreate {
            scope_schema_id: None, // wildcard
        };
        assert!(hooks_match(&actual, &registered));
    }

    #[test]
    fn test_before_node_create_matches_with_exact_scope() {
        let id = Uuid::new_v4();
        let actual = HookPoint::BeforeNodeCreate {
            scope_schema_id: Some(id),
        };
        let registered = HookPoint::BeforeNodeCreate {
            scope_schema_id: Some(id),
        };
        assert!(hooks_match(&actual, &registered));
    }

    #[test]
    fn test_before_node_create_mismatches_different_scope() {
        let actual = HookPoint::BeforeNodeCreate {
            scope_schema_id: Some(Uuid::new_v4()),
        };
        let registered = HookPoint::BeforeNodeCreate {
            scope_schema_id: Some(Uuid::new_v4()),
        };
        assert!(!hooks_match(&actual, &registered));
    }

    #[test]
    fn test_different_hook_variants_never_match() {
        let actual = HookPoint::BeforeNodeCreate {
            scope_schema_id: None,
        };
        let registered = HookPoint::BeforeNodeDelete {
            scope_schema_id: None,
        };
        assert!(!hooks_match(&actual, &registered));
    }

    #[test]
    fn test_before_field_write_matches_same_field() {
        let actual = HookPoint::BeforeFieldWrite {
            field_path: "journal:content".into(),
            scope_schema_id: None,
        };
        let registered = HookPoint::BeforeFieldWrite {
            field_path: "journal:content".into(),
            scope_schema_id: None,
        };
        assert!(hooks_match(&actual, &registered));
    }

    #[test]
    fn test_before_field_write_mismatches_different_field() {
        let actual = HookPoint::BeforeFieldWrite {
            field_path: "journal:content".into(),
            scope_schema_id: None,
        };
        let registered = HookPoint::BeforeFieldWrite {
            field_path: "journal:mood".into(),
            scope_schema_id: None,
        };
        assert!(!hooks_match(&actual, &registered));
    }

    #[test]
    fn test_before_field_write_wildcard_field_matches_anything() {
        // An empty field_path in the registered reactor acts as wildcard
        let actual = HookPoint::BeforeFieldWrite {
            field_path: "any:field".into(),
            scope_schema_id: None,
        };
        let registered = HookPoint::BeforeFieldWrite {
            field_path: "".into(),
            scope_schema_id: None,
        };
        assert!(hooks_match(&actual, &registered));
    }

    // ── Trigger matching tests ────────────────────────────────────────────

    #[test]
    fn test_field_watch_matches_correct_field() {
        let trigger = WatchTrigger::FieldWatch {
            field_path: "journal:content".into(),
            scope: WatchScope::Global,
        };
        let entry = mk_entry(1, OpType::FieldWritten, Some("journal:content"), None);
        assert!(eval_trigger_match(&trigger, &entry));
    }

    #[test]
    fn test_field_watch_does_not_match_different_field() {
        let trigger = WatchTrigger::FieldWatch {
            field_path: "journal:content".into(),
            scope: WatchScope::Global,
        };
        let entry = mk_entry(1, OpType::FieldWritten, Some("journal:mood"), None);
        assert!(!eval_trigger_match(&trigger, &entry));
    }

    #[test]
    fn test_field_watch_matches_node_created() {
        let trigger = WatchTrigger::FieldWatch {
            field_path: "journal:content".into(),
            scope: WatchScope::Global,
        };
        let entry = mk_entry(1, OpType::NodeCreated, Some("journal:content"), None);
        assert!(eval_trigger_match(&trigger, &entry));
    }

    #[test]
    fn test_field_watch_does_not_match_node_deleted() {
        let trigger = WatchTrigger::FieldWatch {
            field_path: "journal:content".into(),
            scope: WatchScope::Global,
        };
        let entry = mk_entry(1, OpType::NodeDeleted, None, None);
        assert!(!eval_trigger_match(&trigger, &entry));
    }

    #[test]
    fn test_lifecycle_watch_matches_node_created() {
        let trigger = WatchTrigger::LifecycleWatch {
            event: LifecycleEvent::NodeCreated,
            scope: WatchScope::Global,
        };
        let entry = mk_entry(1, OpType::NodeCreated, None, None);
        assert!(eval_trigger_match(&trigger, &entry));
    }

    #[test]
    fn test_lifecycle_watch_does_not_match_wrong_event() {
        let trigger = WatchTrigger::LifecycleWatch {
            event: LifecycleEvent::NodeCreated,
            scope: WatchScope::Global,
        };
        let entry = mk_entry(1, OpType::NodeDeleted, None, None);
        assert!(!eval_trigger_match(&trigger, &entry));
    }

    #[test]
    fn test_scope_filtering_schema_id() {
        let schema_a = Uuid::new_v4();
        let schema_b = Uuid::new_v4();

        let trigger = WatchTrigger::FieldWatch {
            field_path: "x:y".into(),
            scope: WatchScope::SchemaId(schema_a),
        };
        let entry_match = OpStreamEntry {
            schema_id: Some(schema_a),
            ..mk_entry(1, OpType::FieldWritten, Some("x:y"), None)
        };
        let entry_no_match = OpStreamEntry {
            schema_id: Some(schema_b),
            ..mk_entry(2, OpType::FieldWritten, Some("x:y"), None)
        };

        assert!(eval_trigger_match(&trigger, &entry_match));
        assert!(!eval_trigger_match(&trigger, &entry_no_match));
    }

    // ── Cycle detection tests ────────────────────────────────────────────

    #[test]
    fn test_no_cycle_in_linear_chain() {
        let existing: Vec<(String, String)> = vec![
            ("A".into(), "B".into()),
            ("B".into(), "C".into()),
        ];
        let new: Vec<(String, String)> = vec![("C".into(), "D".into())];
        assert!(!detect_cycles_in_graph(&existing, &new));
    }

    #[test]
    fn test_simple_cycle_detected() {
        let existing: Vec<(String, String)> = vec![
            ("A".into(), "B".into()),
        ];
        let new: Vec<(String, String)> = vec![("B".into(), "A".into())];
        assert!(detect_cycles_in_graph(&existing, &new));
    }

    #[test]
    fn test_self_cycle_detected() {
        let existing: Vec<(String, String)> = vec![];
        let new: Vec<(String, String)> = vec![("A".into(), "A".into())];
        assert!(detect_cycles_in_graph(&existing, &new));
    }

    #[test]
    fn test_long_cycle_detected() {
        let existing: Vec<(String, String)> = vec![
            ("A".into(), "B".into()),
            ("B".into(), "C".into()),
            ("C".into(), "D".into()),
        ];
        let new: Vec<(String, String)> = vec![("D".into(), "A".into())];
        assert!(detect_cycles_in_graph(&existing, &new));
    }

    #[test]
    fn test_no_cycle_in_dag_with_shared_descendant() {
        // A → C, B → C → D (diamond pattern, no cycle)
        let existing: Vec<(String, String)> = vec![
            ("A".into(), "C".into()),
            ("B".into(), "C".into()),
            ("C".into(), "D".into()),
        ];
        let new: Vec<(String, String)> = vec![];
        assert!(!detect_cycles_in_graph(&existing, &new));
    }

    // ── Priority ordering tests ──────────────────────────────────────────

    #[test]
    fn test_priority_ordering_lower_first() {
        let mut reactors = vec![
            mk_reactor(Uuid::new_v4(), ReactorMode::Eager, ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }), ActionKind::Validate, 10),
            mk_reactor(Uuid::new_v4(), ReactorMode::Eager, ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }), ActionKind::Validate, 0),
            mk_reactor(Uuid::new_v4(), ReactorMode::Eager, ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }), ActionKind::Validate, 5),
        ];
        sort_reactors_by_priority(&mut reactors);
        assert_eq!(reactors[0].priority, 0);
        assert_eq!(reactors[1].priority, 5);
        assert_eq!(reactors[2].priority, 10);
    }

    // ── Quarantine tests ─────────────────────────────────────────────────

    #[test]
    fn test_quarantine_at_threshold() {
        assert!(!eval_quarantine(2, 3));
        assert!(eval_quarantine(3, 3));
        assert!(eval_quarantine(5, 3));
    }

    // ── Mode × Action validation tests ───────────────────────────────────

    #[test]
    fn test_mode_action_compatibility_matrix() {
        let cases = vec![
            (ReactorMode::Eager, ActionKind::Validate, true),
            (ReactorMode::Eager, ActionKind::Transform, true),
            (ReactorMode::Eager, ActionKind::ComputeField, true),
            (ReactorMode::Eager, ActionKind::SideEffect, false),
            (ReactorMode::Eager, ActionKind::InternalWrite, false),
            (ReactorMode::Deferred, ActionKind::Validate, false),
            (ReactorMode::Deferred, ActionKind::Transform, false),
            (ReactorMode::Deferred, ActionKind::ComputeField, true),
            (ReactorMode::Deferred, ActionKind::SideEffect, true),
            (ReactorMode::Deferred, ActionKind::InternalWrite, true),
        ];
        for (mode, action, expected) in &cases {
            assert_eq!(
                is_valid_mode_action(mode, action),
                *expected,
                "mode={:?} action={:?} expected={}",
                mode,
                action,
                expected
            );
        }
    }

    // ── Backoff calculation tests ────────────────────────────────────────

    #[test]
    fn test_backoff_exponential() {
        // Default policy: base=1000, max=60000, multiplier=2.0
        assert_eq!(eval_backoff(1, 1000, 60000, 2.0), 1000);
        assert_eq!(eval_backoff(2, 1000, 60000, 2.0), 2000);
        assert_eq!(eval_backoff(3, 1000, 60000, 2.0), 4000);
        assert_eq!(eval_backoff(4, 1000, 60000, 2.0), 8000);
    }

    #[test]
    fn test_backoff_respects_max() {
        assert_eq!(eval_backoff(10, 1000, 5000, 2.0), 5000);
    }

    // ── Eager hook evaluation tests ──────────────────────────────────────

    #[test]
    fn test_eager_hook_no_reactors_returns_no_match() {
        let result = eval_eager_hook(
            &[],
            &HookPoint::BeforeNodeCreate { scope_schema_id: None },
            None,
            &HashMap::new(),
            3,
        );
        assert_eq!(result, EvalHookResult::NoMatch);
    }

    #[test]
    fn test_eager_hook_single_validate_approves() {
        let reactor = mk_reactor(
            Uuid::new_v4(),
            ReactorMode::Eager,
            ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }),
            ActionKind::Validate,
            0,
        );
        let result = eval_eager_hook(
            &[reactor],
            &HookPoint::BeforeNodeCreate { scope_schema_id: None },
            None,
            &HashMap::new(),
            3,
        );
        assert!(matches!(result, EvalHookResult::Approved { .. }));
    }

    #[test]
    fn test_eager_hook_failing_validate_below_threshold_rejects() {
        let reactor_id = Uuid::new_v4();
        let reactor = mk_reactor(
            reactor_id,
            ReactorMode::Eager,
            ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }),
            ActionKind::Validate,
            0,
        );
        let mut failures = HashMap::new();
        failures.insert(reactor_id, 1); // 1 failure, below threshold of 3
        let result = eval_eager_hook(
            &[reactor],
            &HookPoint::BeforeNodeCreate { scope_schema_id: None },
            None,
            &failures,
            3,
        );
        assert!(matches!(result, EvalHookResult::Rejected { .. }));
    }

    #[test]
    fn test_eager_hook_failing_validate_at_threshold_fails_open() {
        let reactor_id = Uuid::new_v4();
        let reactor = mk_reactor(
            reactor_id,
            ReactorMode::Eager,
            ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }),
            ActionKind::Validate,
            0,
        );
        let mut failures = HashMap::new();
        failures.insert(reactor_id, 3); // At threshold → quarantined → fail open
        let result = eval_eager_hook(
            &[reactor],
            &HookPoint::BeforeNodeCreate { scope_schema_id: None },
            None,
            &failures,
            3,
        );
        assert!(matches!(result, EvalHookResult::Approved { .. }));
    }

    #[test]
    fn test_eager_hook_skips_disabled_reactor() {
        let mut reactor = mk_reactor(
            Uuid::new_v4(),
            ReactorMode::Eager,
            ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }),
            ActionKind::Validate,
            0,
        );
        reactor.status = ReactorStatus::Disabled;
        let result = eval_eager_hook(
            &[reactor],
            &HookPoint::BeforeNodeCreate { scope_schema_id: None },
            None,
            &HashMap::new(),
            3,
        );
        assert_eq!(result, EvalHookResult::NoMatch);
    }

    #[test]
    fn test_eager_hook_skips_deferred_reactor() {
        let reactor = mk_reactor(
            Uuid::new_v4(),
            ReactorMode::Deferred,
            ReactorTrigger::Hook(HookPoint::BeforeNodeCreate { scope_schema_id: None }),
            ActionKind::ComputeField,
            0,
        );
        let result = eval_eager_hook(
            &[reactor],
            &HookPoint::BeforeNodeCreate { scope_schema_id: None },
            None,
            &HashMap::new(),
            3,
        );
        assert_eq!(result, EvalHookResult::NoMatch);
    }

    // ── Deferred dispatch tests ──────────────────────────────────────────

    #[test]
    fn test_deferred_dispatch_empty() {
        let result = eval_deferred_dispatch(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deferred_dispatch_field_watch_fires() {
        let reactor = mk_reactor(
            Uuid::new_v4(),
            ReactorMode::Deferred,
            ReactorTrigger::Watch(WatchTrigger::FieldWatch {
                field_path: "journal:content".into(),
                scope: WatchScope::Global,
            }),
            ActionKind::SideEffect,
            0,
        );
        let reactor_id = reactor.id;
        let entry = mk_entry(1, OpType::FieldWritten, Some("journal:content"), None);
        let result = eval_deferred_dispatch(&[reactor], &[entry]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], (reactor_id, 1));
    }

    #[test]
    fn test_deferred_dispatch_skips_non_matching_entries() {
        let reactor = mk_reactor(
            Uuid::new_v4(),
            ReactorMode::Deferred,
            ReactorTrigger::Watch(WatchTrigger::FieldWatch {
                field_path: "journal:content".into(),
                scope: WatchScope::Global,
            }),
            ActionKind::SideEffect,
            0,
        );
        let entry = mk_entry(1, OpType::FieldWritten, Some("other:field"), None);
        let result = eval_deferred_dispatch(&[reactor], &[entry]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deferred_dispatch_sorts_by_sequence() {
        let reactor = mk_reactor(
            Uuid::new_v4(),
            ReactorMode::Deferred,
            ReactorTrigger::Watch(WatchTrigger::FieldWatch {
                field_path: "x:y".into(),
                scope: WatchScope::Global,
            }),
            ActionKind::SideEffect,
            0,
        );
        let e1 = mk_entry(10, OpType::FieldWritten, Some("x:y"), None);
        let e2 = mk_entry(5, OpType::FieldWritten, Some("x:y"), None);
        let result = eval_deferred_dispatch(&[reactor], &[e1, e2]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, 5); // Lower sequence first
        assert_eq!(result[1].1, 10);
    }
}
