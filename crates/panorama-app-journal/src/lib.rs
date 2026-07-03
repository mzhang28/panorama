//! Journal App — Daily journal with markdown entries, block-level paragraph
//! decomposition, mood tagging, and rich filtering.
//!
//! Features (aligned with PROGRESS.md §2.1):
//! 1. **Block-Level Paragraph Decomposition** — markdown content is split into
//!    paragraph child nodes on create/update, with bidirectional refs.
//! 2. **Rich Markdown Content** — entries store full markdown; the frontend
//!    renders it with headings, lists, code blocks, etc.
//! 3. **Entry Editing & Soft Deletion** — PUT updates entries in place;
//!    DELETE marks `journal:deleted` rather than removing the node.
//! 4. **Timeline & Mood Filtering** — GET /entries supports `?mood=`,
//!    `?from=`, and `?to=` query parameters.

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
                    description: Some("Optional mood tag (happy, thoughtful, excited, etc.)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "paragraph_refs".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Array of paragraph child node UUIDs for block-level referencing".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "deleted".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: Some(FieldValue::Boolean(false)),
                    description: Some("Soft-delete flag".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    fn paragraph_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "journal/Paragraph".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: false,
                    default: Some(FieldValue::String("Paragraph".to_string())),
                    description: Some("Auto-generated paragraph title".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "node_time".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Same date as the parent entry".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "content".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Plain-text paragraph content (no markdown wrapper)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "entry_ref".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("NodeRef pointing back to the parent journal entry".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "paragraph_index".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("0-based position within the parent entry".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }
}

// ── Paragraph decomposition ──────────────────────────────────────────────────

/// Split markdown content into logical paragraph blocks.
///
/// Paragraphs are separated by one or more blank lines.  Each paragraph is
/// returned as plain text (the markdown formatting is preserved — it will
/// be rendered by the frontend when viewing individual paragraphs).
fn split_into_paragraphs(markdown: &str) -> Vec<String> {
    markdown
        .split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Create child Paragraph nodes for a journal entry and return their UUIDs.
async fn decompose_paragraphs(
    ctx: &dyn PluginContext,
    entry_id: Uuid,
    entry_time: &str,
    markdown: &str,
) -> Result<Vec<Uuid>, PluginError> {
    // First, remove old paragraph nodes (if any — e.g. on update)
    // We don't have a direct "query paragraphs by entry_ref" yet, so we
    // rely on the frontend to pass previous paragraph_refs on update and
    // we delete those.  For now, orphaned paragraph nodes are harmless;
    // the entry's paragraph_refs field is the source of truth.

    let paragraphs = split_into_paragraphs(markdown);
    let mut refs = Vec::with_capacity(paragraphs.len());

    for (i, para_text) in paragraphs.iter().enumerate() {
        let mut para_node = Node::new(Uuid::nil());
        para_node.set_field(
            "system:node_title",
            FieldValue::String(format!("Paragraph {}", i + 1)),
        );
        para_node.set_field(
            "system:node_time",
            FieldValue::DateTime(entry_time.to_string()),
        );
        para_node.set_field(
            "journal:content",
            FieldValue::String(para_text.clone()),
        );
        para_node.set_field(
            "journal:entry_ref",
            FieldValue::NodeRef(entry_id),
        );
        para_node.set_field(
            "journal:paragraph_index",
            FieldValue::Integer(i as i64),
        );

        let created = ctx.create_node(para_node).await?;
        refs.push(created.id);
    }

    Ok(refs)
}

/// Delete paragraph child nodes given their UUIDs.
async fn delete_paragraphs(ctx: &dyn PluginContext, refs: &[Uuid]) -> Result<(), PluginError> {
    for id in refs {
        // Best-effort — don't fail the whole operation if one delete fails
        let _ = ctx.delete_node(*id).await;
    }
    Ok(())
}

// ── Plugin trait implementation ──────────────────────────────────────────────

#[async_trait]
impl Plugin for JournalPlugin {
    fn id(&self) -> &str {
        "io.mzhang.panorama.journal"
    }

    fn name(&self) -> &str {
        "Journal"
    }

    fn version(&self) -> &str {
        "0.2.0"
    }

    fn description(&self) -> &str {
        "Daily journal with markdown, block-level paragraph references, mood tagging, and rich filtering"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::journal_entry_schema(), Self::paragraph_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/entries".to_string(),
                description: "Create a new journal entry with paragraph decomposition".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/entries".to_string(),
                description: "List journal entries (supports ?mood=, ?from=, ?to=)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/entries/{id}".to_string(),
                description: "Get a specific journal entry".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::PUT,
                path: "/entries/{id}".to_string(),
                description: "Update a journal entry (title, content, mood)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::DELETE,
                path: "/entries/{id}".to_string(),
                description: "Soft-delete a journal entry".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/entries/{id}/paragraphs".to_string(),
                description: "Get paragraph child nodes for an entry".to_string(),
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
            // ── POST /entries — create entry with paragraph decomposition ──
            ("POST", "entries") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;

                let title = body["title"].as_str().unwrap_or("Untitled");
                let content = body["content"].as_str().unwrap_or("");
                let mood = body["mood"].as_str();
                let entry_time = body["time"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Utc::now().to_rfc3339());

                let mut node = Node::new(Uuid::nil());
                node.set_field("system:node_title", FieldValue::String(title.to_string()));
                node.set_field("system:node_time", FieldValue::DateTime(entry_time.clone()));
                node.set_field("journal:content", FieldValue::String(content.to_string()));
                node.set_field("journal:deleted", FieldValue::Boolean(false));
                if let Some(m) = mood {
                    if !m.is_empty() {
                        node.set_field("journal:mood", FieldValue::String(m.to_string()));
                    }
                }

                // 1. Create the entry node first (need its ID for paragraph refs)
                let entry = ctx.create_node(node).await?;

                // 2. Decompose paragraphs into child nodes
                let para_refs = decompose_paragraphs(ctx, entry.id, &entry_time, content).await?;

                // 3. Update the entry with paragraph_refs
                let mut refs_update = std::collections::HashMap::new();
                refs_update.insert(
                    "journal:paragraph_refs".to_string(),
                    FieldValue::Array(
                        para_refs
                            .iter()
                            .map(|id| FieldValue::NodeRef(*id))
                            .collect(),
                    ),
                );
                let entry = ctx.update_node(entry.id, refs_update).await?;

                HttpResponse::json(&entry)
            }

            // ── GET /entries — list with optional mood & date filters ──
            ("GET", "entries") => {
                let mood_filter = request.query_params.get("mood").cloned();
                let from = request.query_params.get("from").cloned();
                let to = request.query_params.get("to").cloned();

                // Build the query dynamically depending on which filters are present
                let mut preds: Vec<String> = Vec::new();
                preds.push("HAS_FIELD(n, \"journal\", \"content\")".to_string());

                if let Some(ref mood) = mood_filter {
                    preds.push(format!(
                        "n.journal.mood = \"{}\"",
                        mood.replace('"', "\\\"")
                    ));
                }
                if let Some(ref from_date) = from {
                    preds.push(format!(
                        "n.system.node_time >= \"{}\"",
                        from_date.replace('"', "\\\"")
                    ));
                }
                if let Some(ref to_date) = to {
                    preds.push(format!(
                        "n.system.node_time <= \"{}\"",
                        to_date.replace('"', "\\\"")
                    ));
                }

                let query_str = format!(
                    "MATCH (n) IN space(\"default\") WHERE {} RETURN n ORDER BY n.system.node_time DESC LIMIT 100",
                    preds.join(" AND ")
                );

                let rows = ctx.query(&query_str).await?;
                HttpResponse::json(&rows)
            }

            // ── GET /entries/{id} — get single entry ──
            ("GET", _) if endpoint.starts_with("entries/")
                && !endpoint.ends_with("/paragraphs") =>
            {
                let id_str = &endpoint["entries/".len()..];
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;
                let node = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Entry not found"))?;
                HttpResponse::json(&node)
            }

            // ── GET /entries/{id}/paragraphs — get paragraph children ──
            ("GET", _) if endpoint.ends_with("/paragraphs") => {
                // endpoint looks like "entries/{id}/paragraphs"
                let path = endpoint
                    .strip_suffix("/paragraphs")
                    .unwrap_or(endpoint);
                let id_str = &path["entries/".len()..];
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                // Get the entry to find its paragraph_refs
                let entry = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Entry not found"))?;

                // Extract paragraph refs and fetch each paragraph
                let para_refs = match entry.fields.get("journal:paragraph_refs") {
                    Some(FieldValue::Array(refs)) => refs.clone(),
                    _ => vec![],
                };

                let mut paragraphs = Vec::new();
                for pref in &para_refs {
                    if let FieldValue::NodeRef(pid) = pref {
                        if let Some(pnode) = ctx.get_node(*pid).await? {
                            paragraphs.push(serde_json::to_value(&pnode).unwrap_or_default());
                        }
                    }
                }

                HttpResponse::json(&paragraphs)
            }

            // ── PUT /entries/{id} — update entry ──
            ("PUT", _) if endpoint.starts_with("entries/")
                && !endpoint.ends_with("/paragraphs") =>
            {
                let id_str = &endpoint["entries/".len()..];
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;

                // Verify entry exists
                let existing = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Entry not found"))?;

                // Check soft-delete flag
                if let Some(FieldValue::Boolean(true)) =
                    existing.fields.get("journal:deleted")
                {
                    return Err(PluginError::bad_request(
                        "Cannot update a deleted entry. Undelete first.",
                    ));
                }

                let mut updates: std::collections::HashMap<String, FieldValue> =
                    std::collections::HashMap::new();

                if let Some(title) = body.get("title").and_then(|v| v.as_str()) {
                    updates.insert(
                        "system:node_title".to_string(),
                        FieldValue::String(title.to_string()),
                    );
                }
                if let Some(content) = body.get("content").and_then(|v| v.as_str()) {
                    updates.insert(
                        "journal:content".to_string(),
                        FieldValue::String(content.to_string()),
                    );
                    // Content changed — update node_time for "last edited" tracking
                    updates.insert(
                        "system:node_time".to_string(),
                        FieldValue::DateTime(Utc::now().to_rfc3339()),
                    );

                    // Delete old paragraph nodes if they exist
                    if let Some(FieldValue::Array(old_refs)) =
                        existing.fields.get("journal:paragraph_refs")
                    {
                        let old_ids: Vec<Uuid> = old_refs
                            .iter()
                            .filter_map(|fv| {
                                if let FieldValue::NodeRef(id) = fv {
                                    Some(*id)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        delete_paragraphs(ctx, &old_ids).await?;
                    }

                    // Re-decompose
                    let entry_time = Utc::now().to_rfc3339();
                    let new_refs =
                        decompose_paragraphs(ctx, id, &entry_time, content).await?;
                    updates.insert(
                        "journal:paragraph_refs".to_string(),
                        FieldValue::Array(
                            new_refs
                                .iter()
                                .map(|pid| FieldValue::NodeRef(*pid))
                                .collect(),
                        ),
                    );
                }
                if let Some(mood) = body.get("mood").and_then(|v| v.as_str()) {
                    updates.insert(
                        "journal:mood".to_string(),
                        FieldValue::String(mood.to_string()),
                    );
                }

                if updates.is_empty() {
                    return Err(PluginError::bad_request(
                        "No fields to update. Provide title, content, and/or mood.",
                    ));
                }

                let updated = ctx.update_node(id, updates).await?;
                HttpResponse::json(&updated)
            }

            // ── DELETE /entries/{id} — soft delete ──
            ("DELETE", _) if endpoint.starts_with("entries/")
                && !endpoint.ends_with("/paragraphs") =>
            {
                let id_str = &endpoint["entries/".len()..];
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                let existing = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Entry not found"))?;

                // Check if already deleted
                if let Some(FieldValue::Boolean(true)) =
                    existing.fields.get("journal:deleted")
                {
                    return Err(PluginError::bad_request("Entry is already deleted"));
                }

                let mut updates = std::collections::HashMap::new();
                updates.insert(
                    "journal:deleted".to_string(),
                    FieldValue::Boolean(true),
                );
                let updated = ctx.update_node(id, updates).await?;
                HttpResponse::json(&updated)
            }

            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {} {}",
                request.method, endpoint
            ))),
        }
    }
}
