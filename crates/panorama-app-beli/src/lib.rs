//! Beli Alternative App - Rate restaurants with PARTIAL ORDERING.
//! Demonstrates from DESIGN.md: partial order data model, pairwise comparison rankings.
//!
//! Workflow:
//! - Add restaurants as nodes with cuisine, location, and notes
//! - Rate restaurants via pairwise comparisons (partial order, not total order)
//! - Query rankings using topological sort (Kahn's algorithm) producing tiers
//! - This is fundamentally different from a 5-star rating system

use async_trait::async_trait;
use panorama_core::*;
use std::collections::HashMap;
use uuid::Uuid;

pub struct BeliPlugin;

impl BeliPlugin {
    pub fn new() -> Self {
        Self
    }

    fn restaurant_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "beli/Restaurant".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Restaurant name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "cuisine".to_string(),
                    namespace: "beli".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Type of cuisine".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "location".to_string(),
                    namespace: "beli".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Restaurant address/location".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "notes".to_string(),
                    namespace: "beli".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("User notes".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    fn comparison_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "beli/Comparison".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "better_id".to_string(),
                    namespace: "beli".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("ID of the preferred restaurant".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "worse_id".to_string(),
                    namespace: "beli".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("ID of the less preferred restaurant".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "context".to_string(),
                    namespace: "beli".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Optional context (e.g., 'best pizza')".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    /// Compute a partial order ranking from pairwise comparisons.
    /// Uses topological sort (Kahn's algorithm) to produce tiers.
    async fn compute_rankings(
        &self,
        ctx: &dyn PluginContext,
    ) -> Result<serde_json::Value, PluginError> {
        // Query restaurants: nodes with cuisine or location fields
        let restaurant_rows = ctx
            .query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"beli\", \"cuisine\") RETURN n")
            .await?;

        // Query comparisons: nodes with better_id field
        let comparison_rows = ctx
            .query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"beli\", \"better_id\") RETURN n")
            .await?;

        let restaurants: Vec<Node> = restaurant_rows.iter().filter_map(panorama_core::query::row_to_node).collect();
        let comparisons: Vec<Node> = comparison_rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        // Build directed graph: better -> worse
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut names: HashMap<String, String> = HashMap::new();

        for r in &restaurants {
            let id = r.id.to_string();
            let title = r.title().unwrap_or("Unknown").to_string();
            in_degree.entry(id.clone()).or_insert(0);
            names.insert(id, title);
        }

        for c in &comparisons {
            let better = match c.get_field("beli:better_id") {
                Some(FieldValue::String(s)) => s.clone(),
                _ => continue,
            };
            let worse = match c.get_field("beli:worse_id") {
                Some(FieldValue::String(s)) => s.clone(),
                _ => continue,
            };
            edges.entry(better.clone()).or_default().push(worse.clone());
            *in_degree.entry(worse).or_default() += 1;
            in_degree.entry(better).or_insert(0);
        }

        // Topological sort into tiers (Kahn's algorithm)
        let mut tiers: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut current_tier: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        while !current_tier.is_empty() {
            let tier_items: Vec<serde_json::Value> = current_tier
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "id": id,
                        "name": names.get(id).cloned().unwrap_or_default(),
                    })
                })
                .collect();
            tiers.push(tier_items);

            let mut next_tier = Vec::new();
            for id in &current_tier {
                if let Some(neighbors) = edges.get(id) {
                    for neighbor in neighbors {
                        if let Some(deg) = in_degree.get_mut(neighbor) {
                            *deg -= 1;
                            if *deg == 0 {
                                next_tier.push(neighbor.clone());
                            }
                        }
                    }
                }
            }
            current_tier = next_tier;
        }

        Ok(serde_json::json!({
            "tiers": tiers,
            "total_restaurants": restaurant_rows.len(),
            "total_comparisons": comparison_rows.len(),
        }))
    }
}

#[async_trait]
impl Plugin for BeliPlugin {
    fn id(&self) -> &str {
        "com.panorama.beli"
    }

    fn name(&self) -> &str {
        "Beli"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Restaurant ratings with partial ordering (pairwise comparisons)"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::restaurant_schema(), Self::comparison_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/restaurants".to_string(),
                description: "Add a restaurant".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/restaurants".to_string(),
                description: "List restaurants".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/compare".to_string(),
                description: "Record a pairwise comparison (A > B)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/rankings".to_string(),
                description: "Get partial order rankings (topological tiers)".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![UiComponent {
            id: "beli-main".to_string(),
            name: "Restaurant Rankings".to_string(),
            mount_point: UiMountPoint::MainPage,
            bundle_path: "ui/beli.js".to_string(),
        }]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants {
            field_read: vec![
                "beli:*".to_string(),
                "system:node_title".to_string(),
            ],
            field_write: vec![
                "beli:*".to_string(),
                "system:node_title".to_string(),
            ],
            write_own_nodes: true,
            ..Default::default()
        }
    }

    async fn handle_http_request(
        &self,
        endpoint: &str,
        request: HttpRequest,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        match (request.method.as_str(), endpoint) {
            ("POST", "restaurants") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let mut node = Node::new(Uuid::nil());
                node.set_field(
                    "system:node_title",
                    FieldValue::String(
                        body["name"].as_str().unwrap_or("Unknown Restaurant").to_string(),
                    ),
                );
                if let Some(c) = body["cuisine"].as_str() {
                    node.set_field("beli:cuisine", FieldValue::String(c.to_string()));
                }
                if let Some(l) = body["location"].as_str() {
                    node.set_field("beli:location", FieldValue::String(l.to_string()));
                }
                if let Some(n) = body["notes"].as_str() {
                    node.set_field("beli:notes", FieldValue::String(n.to_string()));
                }
                let restaurant = ctx.create_node(node).await?;
                HttpResponse::json(&restaurant)
            }
            ("GET", "restaurants") => {
                let rows = ctx.query(
                    "MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"beli\", \"cuisine\") RETURN n"
                ).await?;
                HttpResponse::json(&rows)
            }
            ("POST", "compare") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let better = body["better_id"]
                    .as_str()
                    .ok_or_else(|| PluginError::bad_request("better_id required"))?;
                let worse = body["worse_id"]
                    .as_str()
                    .ok_or_else(|| PluginError::bad_request("worse_id required"))?;
                let mut node = Node::new(Uuid::nil());
                node.set_field("beli:better_id", FieldValue::String(better.to_string()));
                node.set_field("beli:worse_id", FieldValue::String(worse.to_string()));
                if let Some(ctx_str) = body["context"].as_str() {
                    node.set_field("beli:context", FieldValue::String(ctx_str.to_string()));
                }
                let comparison = ctx.create_node(node).await?;
                HttpResponse::json(&comparison)
            }
            ("GET", "rankings") => {
                let rankings = self.compute_rankings(ctx).await?;
                HttpResponse::json(&rankings)
            }
            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {}",
                endpoint
            ))),
        }
    }
}
