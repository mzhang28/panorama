use serde::{Deserialize, Serialize};

/// A field namespace identifier.
/// Namespaces map to apps but are stored as opaque IDs in the data layer,
/// allowing apps to be swapped without changing stored data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FieldNamespace {
    /// Opaque namespace ID
    pub id: String,
    /// Human-readable name (for display only)
    pub display_name: String,
    /// The app that owns this namespace (if any)
    pub owner_app_id: Option<String>,
}

impl FieldNamespace {
    /// Create a new namespace
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            owner_app_id: None,
        }
    }

    /// Create a namespace owned by an app
    pub fn app_owned(
        id: impl Into<String>,
        display_name: impl Into<String>,
        app_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            owner_app_id: Some(app_id.into()),
        }
    }
}

/// Well-known system namespaces
pub mod system_namespaces {
    /// The system namespace for built-in fields
    pub const SYSTEM: &str = "system";
    /// The user namespace for user-created fields
    pub const USER: &str = "user";
}

/// Helper to construct namespaced field keys
pub fn field_key(namespace: &str, field_name: &str) -> String {
    format!("{}:{}", namespace, field_name)
}

/// Parse a namespaced field key into (namespace, field_name)
pub fn parse_field_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// Computed field evaluation context
#[derive(Debug, Clone)]
pub struct ComputeContext {
    /// The current node's fields
    pub node_fields: Vec<(String, serde_json::Value)>,
    /// Referenced nodes (for NodeRef field resolution)
    pub referenced_nodes: Vec<(String, serde_json::Value)>,
}

/// Simple expression evaluator for computed fields.
/// In v0.0, this is a basic reference resolver:
/// - "field:namespace:name" -> get the value of that field
/// - "ref:field_name.target_field" -> follow a NodeRef and get target's field
pub fn evaluate_simple_expression(
    expression: &str,
    context: &ComputeContext,
) -> Option<serde_json::Value> {
    if expression.starts_with("field:") {
        let field_key = &expression[6..];
        context
            .node_fields
            .iter()
            .find(|(k, _)| k == field_key)
            .map(|(_, v)| v.clone())
    } else if expression.starts_with("ref:") {
        let rest = &expression[4..];
        let parts: Vec<&str> = rest.splitn(2, '.').collect();
        if parts.len() == 2 {
            let source_field = format!("user:{}", parts[0]);
            let target_field = parts[1];
            // Find the NodeRef value, then look it up in referenced nodes
            let ref_node_id = context
                .node_fields
                .iter()
                .find(|(k, _)| k == &source_field)
                .and_then(|(_, v)| v.as_str().map(|s| s.to_string()));
            if let Some(node_id) = ref_node_id {
                context
                    .referenced_nodes
                    .iter()
                    .find(|(id, _)| id == &node_id)
                    .and_then(|(_, fields)| {
                        // fields is a JSON object; extract target field
                        fields.get(target_field).cloned()
                    })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        // Literal value
        Some(serde_json::Value::String(expression.to_string()))
    }
}

/// Cycle detection for computed field dependencies
pub fn detect_cycles(
    field_name: &str,
    dependencies: &[String],
    all_computed: &[(String, Vec<String>)],
) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![field_name.to_string()];
    visited.insert(field_name.to_string());

    while let Some(current) = stack.pop() {
        for dep in dependencies.iter().filter(|d| d.starts_with(&current)) {
            if visited.contains(dep) {
                return true; // Cycle detected
            }
            visited.insert(dep.clone());
            // Find dep's own dependencies
            if let Some((_, dep_deps)) = all_computed.iter().find(|(n, _)| n == dep) {
                stack.extend(dep_deps.iter().cloned());
            }
        }
    }
    false
}
