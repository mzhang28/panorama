//! Grafana-like Dashboard App - Query and visualize time-series data.
//! Demonstrates from DESIGN.md: time-series queries, aggregations, dashboard creation.
//!
//! Workflow:
//! - Query nodes by time range and field values
//! - Aggregate data (counts, sum_duration, leaderboard by hours descending)
//! - Create and list dashboard configurations
//! - Supports group_by and key=value filtering

use async_trait::async_trait;
use chrono::DateTime;
use panorama_core::*;
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

pub struct GrafanaPlugin;

#[derive(Debug, Deserialize)]
struct DashboardQuery {
    /// Time range start (ISO 8601)
    pub from: Option<String>,
    /// Time range end (ISO 8601)
    pub to: Option<String>,
    /// Field to group by (e.g., "wakatime:project")
    pub group_by: Option<String>,
    /// Aggregation: "count", "sum_duration", "leaderboard"
    pub aggregation: Option<String>,
    /// Filter expression (simple key=value)
    pub filter: Option<String>,
}

impl GrafanaPlugin {
    pub fn new() -> Self {
        Self
    }

    fn dashboard_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "grafana/Dashboard".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Dashboard name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "config".to_string(),
                    namespace: "grafana".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Dashboard JSON configuration".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    async fn execute_query(
        &self,
        ctx: &dyn PluginContext,
        query: &DashboardQuery,
    ) -> Result<serde_json::Value, PluginError> {
        // Fetch all nodes with time fields, sorted
        let rows = ctx
            .query("MATCH (n) IN space(\"default\") RETURN n ORDER BY n.system.node_time ASC")
            .await?;
        let mut all_nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        // Filter by time range
        if let Some(from) = &query.from {
            if let Ok(from_dt) = DateTime::parse_from_rfc3339(from) {
                all_nodes.retain(|n| {
                    n.effective_time()
                        .and_then(|v| match v {
                            FieldValue::DateTime(s) => DateTime::parse_from_rfc3339(s).ok(),
                            _ => None,
                        })
                        .map(|t| t >= from_dt)
                        .unwrap_or(false)
                });
            }
        }
        if let Some(to) = &query.to {
            if let Ok(to_dt) = DateTime::parse_from_rfc3339(to) {
                all_nodes.retain(|n| {
                    n.effective_time()
                        .and_then(|v| match v {
                            FieldValue::DateTime(s) => DateTime::parse_from_rfc3339(s).ok(),
                            _ => None,
                        })
                        .map(|t| t <= to_dt)
                        .unwrap_or(false)
                });
            }
        }

        // Apply key=value filter
        if let Some(filter) = &query.filter {
            if let Some((key, value)) = filter.split_once('=') {
                all_nodes.retain(|n| {
                    n.get_field(key)
                        .map(|v| match v {
                            FieldValue::String(s) => s == value,
                            _ => false,
                        })
                        .unwrap_or(false)
                });
            }
        }

        // Group and aggregate
        match query.aggregation.as_deref() {
            Some("count") => {
                if let Some(group_by) = &query.group_by {
                    let mut groups: HashMap<String, usize> = HashMap::new();
                    for node in &all_nodes {
                        if let Some(FieldValue::String(key)) = node.get_field(group_by) {
                            *groups.entry(key.clone()).or_default() += 1;
                        }
                    }
                    let result: Vec<serde_json::Value> = groups
                        .into_iter()
                        .map(|(key, count)| {
                            serde_json::json!({"key": key, "count": count})
                        })
                        .collect();
                    Ok(serde_json::json!(result))
                } else {
                    Ok(serde_json::json!({"total": all_nodes.len()}))
                }
            }
            Some("sum_duration") => {
                let total: f64 = all_nodes
                    .iter()
                    .filter_map(|n| n.get_field("wakatime:duration"))
                    .filter_map(|v| match v {
                        FieldValue::Float(f) => Some(*f),
                        FieldValue::Integer(i) => Some(*i as f64),
                        FieldValue::String(s) => s.parse::<f64>().ok(),
                        _ => None,
                    })
                    .sum();
                Ok(serde_json::json!({"total_duration_seconds": total}))
            }
            Some("leaderboard") => {
                let mut groups: HashMap<String, f64> = HashMap::new();
                let group_by = query.group_by.as_deref().unwrap_or("wakatime:project");
                for node in &all_nodes {
                    if let Some(FieldValue::String(key)) = node.get_field(group_by) {
                        let duration = node
                            .get_field("wakatime:duration")
                            .and_then(|v| match v {
                                FieldValue::Float(f) => Some(*f),
                                FieldValue::Integer(i) => Some(*i as f64),
                                FieldValue::String(s) => s.parse::<f64>().ok(),
                                _ => None,
                            })
                            .unwrap_or(0.0);
                        *groups.entry(key.clone()).or_default() += duration;
                    }
                }
                let mut result: Vec<serde_json::Value> = groups
                    .into_iter()
                    .map(|(key, seconds)| {
                        serde_json::json!({"project": key, "hours": seconds / 3600.0})
                    })
                    .collect();
                // Sort leaderboard by hours descending
                result.sort_by(|a, b| {
                    b["hours"]
                        .as_f64()
                        .unwrap_or(0.0)
                        .partial_cmp(&a["hours"].as_f64().unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                Ok(serde_json::json!(result))
            }
            _ => {
                // No aggregation specified, return raw nodes
                Ok(serde_json::json!(all_nodes))
            }
        }
    }
}

#[async_trait]
impl Plugin for GrafanaPlugin {
    fn id(&self) -> &str {
        "com.panorama.grafana"
    }

    fn name(&self) -> &str {
        "Dashboards"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Grafana-like dashboards for time-series data visualization"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::dashboard_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/query".to_string(),
                description: "Execute a dashboard query".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/dashboards".to_string(),
                description: "Save a dashboard configuration".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/dashboards".to_string(),
                description: "List saved dashboards".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![UiComponent {
            id: "grafana-main".to_string(),
            name: "Dashboard View".to_string(),
            mount_point: UiMountPoint::MainPage,
            bundle_path: "ui/grafana.js".to_string(),
        }]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants {
            field_read: vec!["*".to_string()],
            field_write: vec![
                "grafana:*".to_string(),
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
            ("POST", "query") => {
                let query: DashboardQuery = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let result = self.execute_query(ctx, &query).await?;
                HttpResponse::json(&result)
            }
            ("POST", "dashboards") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let title = body["title"].as_str().unwrap_or("Untitled Dashboard");
                let mut node = Node::new(Uuid::nil());
                node.set_field(
                    "system:node_title",
                    FieldValue::String(title.to_string()),
                );
                node.set_field("grafana:config", FieldValue::Json(body));
                let saved = ctx.create_node(node).await?;
                HttpResponse::json(&saved)
            }
            ("GET", "dashboards") => {
                let rows = ctx.query(
                    "MATCH (n) IN space(\"default\") \
                     WHERE HAS_FIELD(n, \"grafana\", \"config\") \
                     RETURN n \
                     ORDER BY n.system.updated_at DESC"
                ).await?;
                HttpResponse::json(&rows)
            }
            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {}",
                endpoint
            ))),
        }
    }
}
