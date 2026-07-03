//! Wakatime App - Receives heartbeats from Wakatime clients.
//! Demonstrates from DESIGN.md: external HTTP endpoint compatibility, time-series data.
//!
//! Workflow:
//! - Exposes /heartbeat and /heartbeats endpoints compatible with Wakatime clients
//! - Translates heartbeats into nodes with a time-series schema
//! - Stores coding activity data (entity, project, language, duration, category)
//! - Supports both single and bulk heartbeat JSON submissions

use async_trait::async_trait;
use chrono::Utc;
use panorama_core::*;
use uuid::Uuid;

pub struct WakatimePlugin;

impl WakatimePlugin {
    pub fn new() -> Self {
        Self
    }

    fn heartbeat_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "wakatime/Heartbeat".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_time".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("When the heartbeat was recorded".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "entity".to_string(),
                    namespace: "wakatime".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("File path or entity being edited".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "project".to_string(),
                    namespace: "wakatime".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Project name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "language".to_string(),
                    namespace: "wakatime".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Programming language".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "duration".to_string(),
                    namespace: "wakatime".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Duration in seconds".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "category".to_string(),
                    namespace: "wakatime".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Activity category (coding/browsing/markup/etc)".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }
}

#[async_trait]
impl Plugin for WakatimePlugin {
    fn id(&self) -> &str {
        "com.panorama.wakatime"
    }

    fn name(&self) -> &str {
        "Wakatime"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Receives Wakatime-compatible heartbeats and stores them as time-series nodes"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::heartbeat_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/heartbeat".to_string(),
                description: "Receive a Wakatime-compatible heartbeat".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/heartbeats".to_string(),
                description: "Receive multiple heartbeats (bulk submission)".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![UiComponent {
            id: "wakatime-dashboard".to_string(),
            name: "Coding Activity".to_string(),
            mount_point: UiMountPoint::Dashboard,
            bundle_path: "ui/wakatime.js".to_string(),
        }]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants {
            field_write: vec![
                "wakatime:*".to_string(),
                "system:node_time".to_string(),
            ],
            field_read: vec!["wakatime:*".to_string()],
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
            ("POST", "heartbeat") | ("POST", "heartbeats") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;

                // Support both single heartbeat and bulk array
                let heartbeats = if body.is_array() {
                    body.as_array().unwrap().clone()
                } else {
                    vec![body]
                };

                let mut created = Vec::new();
                for hb in &heartbeats {
                    let mut node = Node::new(Uuid::nil());
                    let time = hb["time"]
                        .as_f64()
                        .map(|ts| {
                            chrono::DateTime::from_timestamp(ts as i64, 0)
                                .unwrap_or_else(|| Utc::now())
                                .to_rfc3339()
                        })
                        .unwrap_or_else(|| Utc::now().to_rfc3339());

                    node.set_field("system:node_time", FieldValue::DateTime(time));
                    if let Some(entity) = hb["entity"].as_str() {
                        node.set_field("wakatime:entity", FieldValue::String(entity.to_string()));
                    }
                    if let Some(project) = hb["project"].as_str() {
                        node.set_field("wakatime:project", FieldValue::String(project.to_string()));
                    }
                    if let Some(language) = hb["language"].as_str() {
                        node.set_field("wakatime:language", FieldValue::String(language.to_string()));
                    }
                    if let Some(duration) = hb["duration"].as_f64() {
                        node.set_field("wakatime:duration", FieldValue::Float(duration));
                    }
                    if let Some(category) = hb["category"].as_str() {
                        node.set_field("wakatime:category", FieldValue::String(category.to_string()));
                    }
                    created.push(ctx.create_node(node).await?);
                }
                HttpResponse::json(&serde_json::json!({
                    "status": "ok",
                    "created": created.len()
                }))
            }
            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {}",
                endpoint
            ))),
        }
    }
}
