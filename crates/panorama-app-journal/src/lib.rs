//! Journal App - Daily journal with markdown entries.
//! Demonstrates from DESIGN.md: node CRUD, daily journal entries, block-level node references.
//!
//! Workflow:
//! - Create daily journal entries as nodes with markdown content
//! - Entries are stacked with most recent on top (sorted by node_time descending)
//! - Paragraphs can optionally be separate nodes for referencing (block-level breakdown)
//! - Mutable mood tagging per entry

use async_trait::async_trait;
use chrono::Utc;
use panorama_core::*;
use uuid::Uuid;

pub struct JournalPlugin;

impl JournalPlugin {
    pub fn new() -> Self {
        Self
    }

    fn journal_entry_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "journal/JournalEntry".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: false,
                    default: Some(FieldValue::String("Untitled Entry".to_string())),
                    description: Some("Journal entry title".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "node_time".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Date of the journal entry".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "content".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Markdown content of the journal entry".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "mood".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Optional mood tag".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "paragraph_refs".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("References to paragraph-level nodes for block-level linking".to_string()),
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
impl Plugin for JournalPlugin {
    fn id(&self) -> &str {
        "com.panorama.journal"
    }

    fn name(&self) -> &str {
        "Journal"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Daily journal with markdown entries and block-level references"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::journal_entry_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/entries".to_string(),
                description: "Create a new journal entry".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/entries".to_string(),
                description: "List all journal entries".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/entries/{id}".to_string(),
                description: "Get a specific journal entry".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![
            UiComponent {
                id: "journal-main".to_string(),
                name: "Journal View".to_string(),
                mount_point: UiMountPoint::MainPage,
                bundle_path: "ui/journal.js".to_string(),
            },
            UiComponent {
                id: "journal-sidebar".to_string(),
                name: "Journal Sidebar".to_string(),
                mount_point: UiMountPoint::Sidebar,
                bundle_path: "ui/journal-sidebar.js".to_string(),
            },
        ]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants {
            field_read: vec![
                "journal:*".to_string(),
                "system:node_title".to_string(),
                "system:node_time".to_string(),
            ],
            field_write: vec![
                "journal:*".to_string(),
                "system:node_title".to_string(),
                "system:node_time".to_string(),
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
            ("POST", "entries") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;

                let title = body["title"].as_str().unwrap_or("Untitled");
                let content = body["content"].as_str().unwrap_or("");
                let mood = body["mood"].as_str();

                let mut node = Node::new(Uuid::nil());
                node.set_field("system:node_title", FieldValue::String(title.to_string()));
                node.set_field(
                    "system:node_time",
                    FieldValue::DateTime(Utc::now().to_rfc3339()),
                );
                node.set_field("journal:content", FieldValue::String(content.to_string()));
                if let Some(m) = mood {
                    node.set_field("journal:mood", FieldValue::String(m.to_string()));
                }

                let entry = ctx.create_node(node).await?;
                HttpResponse::json(&entry)
            }
            ("GET", "entries") => {
                // Use the Panorama Query Language — filters & sorts in SQL
                let rows = ctx
                    .query(
                        "MATCH (n) IN space(\"default\") \
                         WHERE HAS_FIELD(n, \"journal\", \"content\") \
                         RETURN n \
                         ORDER BY n.system.node_time DESC \
                         LIMIT 100",
                    )
                    .await?;
                HttpResponse::json(&rows)
            }
            ("GET", _) if endpoint.starts_with("entries/") => {
                let id_str = &endpoint["entries/".len()..];
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;
                let node = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Entry not found"))?;
                HttpResponse::json(&node)
            }
            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {}",
                endpoint
            ))),
        }
    }
}
