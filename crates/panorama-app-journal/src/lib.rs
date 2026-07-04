//! Journal App -- Logseq-inspired block-based notebook with daily journal,
//! outliner tree structure, bidirectional `[[references]]`, backlinks, and a
//! typed properties system.
//!
//! ## Data Model
//!
//! A single **Block** schema replaces the old Entry + Paragraph split:
//!
//! ```text
//! Block {
//!   title        -- optional display text
//!   content      -- markdown body
//!   parent_id    -- UUID of parent block (null = root / page)
//!   order        -- fractional index ("a0", "a1"...) for sibling ordering
//!   page_id      -- UUID of the root page this block belongs to
//!   journal_day  -- date string "2026-07-04" (only on journal page roots)
//!   properties   -- JSON typed key-value pairs
//!   tags         -- array of tag strings
//!   refs         -- array of UUIDs referenced via [[links]]
//!   deleted      -- soft-delete flag
//! }
//! ```
//!
//! A "page" is a block with `parent_id = null`.  A "journal page" additionally
//! has `journal_day` set.  Nested blocks form a tree via `parent_id`.

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use panorama_core::*;
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::sync::LazyLock;
use uuid::Uuid;

pub struct JournalPlugin;

impl JournalPlugin {
    pub fn new() -> Self {
        Self
    }

    // ── Schemas ────────────────────────────────────────────────────────────

    fn block_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "journal/Block".to_string(),
            version: SchemaVersion::new(2, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: false,
                    default: Some(FieldValue::String("".to_string())),
                    description: Some("Block title (display text for pages)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "node_time".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Creation / last-edit timestamp".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "content".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Markdown content of the block".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "parent_id".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("UUID of the parent block (null = root page)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "order".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: Some(FieldValue::String("a0".to_string())),
                    description: Some("Fractional index for sibling ordering".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "page_id".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("UUID of the root page this block belongs to".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "journal_day".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("ISO date string for journal pages (e.g. 2026-07-04)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "properties".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("JSON map of typed key-value properties".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "tags".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Array of tag strings".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "refs".to_string(),
                    namespace: "journal".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Array of UUIDs this block references via [[links]]".to_string()),
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
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Regex for `[[page reference]]` wiki-style links.
static REF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\[([^\]]+)\]\]").unwrap()
});

/// Extract `[[page names]]` from markdown content.
fn extract_refs(content: &str) -> Vec<String> {
    REF_RE
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// Render markdown to HTML via Sattleri.
fn render_markdown(content: &str) -> String {
    satteri::markdown_to_html(content)
}

/// Generate the next fractional index between two existing orders.
/// Uses simple midpoint string insertion — "a0", "a1", ..., "a0a0" etc.
fn next_order(existing: &[String]) -> String {
    if existing.is_empty() {
        return "a0".to_string();
    }
    // Take the last order and increment its last segment
    let last = &existing[existing.len() - 1];
    // Simple strategy: append "a0" to the last one
    format!("{}a0", last)
}

/// Resolve a page name to its UUID by searching for a block with matching title
/// and no parent_id. Returns None if not found.
async fn resolve_page_ref(
    ctx: &dyn PluginContext,
    page_name: &str,
) -> Result<Option<Uuid>, PluginError> {
    let query = format!(
        "MATCH (n) IN space(\"default\") \
         WHERE HAS_FIELD(n, \"journal\", \"content\") \
         AND n.system.node_title = \"{}\" \
         RETURN n LIMIT 1",
        page_name.replace('"', "\\\"")
    );
    let rows = ctx.query(&query).await?;
    for row in &rows {
        if let Some(node) = panorama_core::query::row_to_node(row) {
            if !node.fields.contains_key("journal:parent_id") {
                return Ok(Some(node.id));
            }
        }
    }
    Ok(None)
}

// ── Plugin trait ───────────────────────────────────────────────────────────

#[async_trait]
impl Plugin for JournalPlugin {
    fn id(&self) -> &str {
        "io.mzhang.panorama.journal"
    }

    fn name(&self) -> &str {
        "Journal"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Logseq-inspired block-based notebook with daily journal, [[references]], backlinks, and properties"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::block_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            // Blocks
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/blocks".to_string(),
                description: "Create a block (page or child)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/blocks".to_string(),
                description: "List blocks (?page_id=, ?date=, ?tag=)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/blocks/{id}".to_string(),
                description: "Get a single block".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::PUT,
                path: "/blocks/{id}".to_string(),
                description: "Update a block".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::DELETE,
                path: "/blocks/{id}".to_string(),
                description: "Soft-delete a block".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/blocks/{id}/children".to_string(),
                description: "Get child blocks".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/blocks/render".to_string(),
                description: "Render markdown to HTML".to_string(),
            },
            // Pages
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/pages".to_string(),
                description: "List root pages (blocks with no parent)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/pages/today".to_string(),
                description: "Get or create today's journal page".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/pages/{id}/backlinks".to_string(),
                description: "Get blocks that reference this page".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![UiComponent {
            id: "journal-main".to_string(),
            name: "Journal".to_string(),
            mount_point: UiMountPoint::MainPage,
            bundle_path: "ui/journal.js".to_string(),
        }]
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
            // ── POST /blocks ────────────────────────────────────────
            ("POST", "blocks") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;

                let title = body["title"].as_str().unwrap_or("");
                let content = body["content"].as_str().unwrap_or("");
                let parent_id = body["parent_id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
                let journal_day = body["journal_day"].as_str().map(String::from);
                let tags: Vec<String> = body["tags"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let properties = body.get("properties").cloned();

                // Extract [[references]] from content
                let ref_names = extract_refs(content);

                // Resolve refs to page UUIDs
                let mut ref_uuids: Vec<Uuid> = Vec::new();
                for name in &ref_names {
                    if let Some(id) = resolve_page_ref(ctx, name).await? {
                        ref_uuids.push(id);
                    }
                }

                let now = Utc::now().to_rfc3339();

                let mut node = Node::new(Uuid::nil());
                if !title.is_empty() {
                    node.set_field("system:node_title", FieldValue::String(title.to_string()));
                }
                node.set_field("system:node_time", FieldValue::DateTime(now));
                node.set_field("journal:content", FieldValue::String(content.to_string()));
                node.set_field("journal:deleted", FieldValue::Boolean(false));

                if let Some(pid) = parent_id {
                    node.set_field("journal:parent_id", FieldValue::NodeRef(pid));
                }
                if let Some(ref day) = journal_day {
                    node.set_field("journal:journal_day", FieldValue::String(day.clone()));
                }
                if !tags.is_empty() {
                    node.set_field(
                        "journal:tags",
                        FieldValue::Json(json!(tags)),
                    );
                }
                if let Some(props) = &properties {
                    node.set_field("journal:properties", FieldValue::Json(props.clone()));
                }
                if !ref_uuids.is_empty() {
                    node.set_field(
                        "journal:refs",
                        FieldValue::Array(
                            ref_uuids.iter().map(|id| FieldValue::NodeRef(*id)).collect(),
                        ),
                    );
                }

                // Determine page_id: if parent is set, inherit from parent
                let page_id = if let Some(pid) = parent_id {
                    if let Some(parent) = ctx.get_node(pid).await? {
                        parent
                            .fields
                            .get("journal:page_id")
                            .and_then(|v| {
                                if let FieldValue::NodeRef(id) = v { Some(*id) } else { None }
                            })
                            .unwrap_or(pid)
                    } else {
                        // parent not found, this block is its own page
                        Uuid::nil() // will be set after creation
                    }
                } else {
                    Uuid::nil() // will be set after creation
                };

                // Assign order
                let order = "a0".to_string(); // first child — frontend can reorder later
                node.set_field("journal:order", FieldValue::String(order));

                let created = ctx.create_node(node).await?;

                // Set page_id after creation (self-reference for root pages)
                let actual_page_id = if parent_id.is_some() && page_id != Uuid::nil() {
                    page_id
                } else {
                    created.id
                };

                let mut patch = HashMap::new();
                patch.insert(
                    "journal:page_id".to_string(),
                    FieldValue::NodeRef(actual_page_id),
                );
                let updated = ctx.update_node(created.id, patch).await?;

                HttpResponse::json(&updated)
            }

            // ── GET /blocks ─────────────────────────────────────────
            ("GET", "blocks") => {
                let page_id = request.query_params.get("page_id").cloned();
                let date = request.query_params.get("date").cloned();
                let tag = request.query_params.get("tag").cloned();

                let mut preds: Vec<String> = Vec::new();
                preds.push("HAS_FIELD(n, \"journal\", \"content\")".to_string());

                if let Some(ref pid) = page_id {
                    preds.push(format!(
                        "n.journal.page_id = \"{}\"",
                        pid.replace('"', "\\\"")
                    ));
                }
                if let Some(ref d) = date {
                    preds.push(format!(
                        "n.journal.journal_day = \"{}\"",
                        d.replace('"', "\\\"")
                    ));
                }
                if let Some(ref t) = tag {
                    // tags is a JSON array — use LIKE for simple matching
                    preds.push(format!(
                        "n.journal.tags LIKE \"%{}%\"",
                        t.replace('"', "\\\"")
                    ));
                }

                let query_str = format!(
                    "MATCH (n) IN space(\"default\") WHERE {} RETURN n ORDER BY n.journal.order ASC LIMIT 200",
                    preds.join(" AND ")
                );
                let rows = ctx.query(&query_str).await?;
                HttpResponse::json(&rows)
            }

            // ── GET /blocks/{id} ────────────────────────────────────
            ("GET", _) if endpoint.starts_with("blocks/")
                && !endpoint.contains("/children")
                && !endpoint.contains("/render") =>
            {
                let id_str = endpoint.strip_prefix("blocks/").unwrap_or(endpoint);
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;
                let node = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Block not found"))?;
                HttpResponse::json(&node)
            }

            // ── GET /blocks/{id}/children ───────────────────────────
            ("GET", _) if endpoint.ends_with("/children") => {
                let id_str = endpoint
                    .strip_suffix("/children")
                    .unwrap_or(endpoint)
                    .strip_prefix("blocks/")
                    .unwrap_or("");
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                let query = format!(
                    "MATCH (n) IN space(\"default\") \
                     WHERE HAS_FIELD(n, \"journal\", \"parent_id\") \
                     AND n.journal.parent_id = \"{}\" \
                     RETURN n ORDER BY n.journal.order ASC",
                    id
                );
                let rows = ctx.query(&query).await?;
                HttpResponse::json(&rows)
            }

            // ── PUT /blocks/{id} ────────────────────────────────────
            ("PUT", _) if endpoint.starts_with("blocks/")
                && !endpoint.contains("/children") =>
            {
                let id_str = endpoint.strip_prefix("blocks/").unwrap_or(endpoint);
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                let existing = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Block not found"))?;

                if let Some(FieldValue::Boolean(true)) = existing.fields.get("journal:deleted") {
                    return Err(PluginError::bad_request("Cannot update a deleted block"));
                }

                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;

                let mut updates: HashMap<String, FieldValue> = HashMap::new();
                updates.insert(
                    "system:node_time".to_string(),
                    FieldValue::DateTime(Utc::now().to_rfc3339()),
                );

                if let Some(title) = body.get("title").and_then(|v| v.as_str()) {
                    updates.insert(
                        "system:node_title".to_string(),
                        FieldValue::String(title.to_string()),
                    );
                }
                if let Some(content) = body.get("content").and_then(|v| v.as_str()) {
                    // Re-extract [[refs]]
                    let ref_names = extract_refs(content);
                    let mut ref_uuids: Vec<Uuid> = Vec::new();
                    for name in &ref_names {
                        if let Some(rid) = resolve_page_ref(ctx, name).await? {
                            ref_uuids.push(rid);
                        }
                    }
                    updates.insert(
                        "journal:content".to_string(),
                        FieldValue::String(content.to_string()),
                    );
                    updates.insert(
                        "journal:refs".to_string(),
                        FieldValue::Array(
                            ref_uuids.iter().map(|rid| FieldValue::NodeRef(*rid)).collect(),
                        ),
                    );
                }
                if let Some(order) = body.get("order").and_then(|v| v.as_str()) {
                    updates.insert(
                        "journal:order".to_string(),
                        FieldValue::String(order.to_string()),
                    );
                }
                if let Some(tags) = body.get("tags") {
                    updates.insert("journal:tags".to_string(), FieldValue::Json(tags.clone()));
                }
                if let Some(props) = body.get("properties") {
                    updates.insert("journal:properties".to_string(), FieldValue::Json(props.clone()));
                }

                if updates.len() <= 1 {
                    // only node_time
                    return Err(PluginError::bad_request("No fields to update"));
                }

                let updated = ctx.update_node(id, updates).await?;
                HttpResponse::json(&updated)
            }

            // ── DELETE /blocks/{id} ─────────────────────────────────
            ("DELETE", _) if endpoint.starts_with("blocks/") => {
                let id_str = endpoint.strip_prefix("blocks/").unwrap_or(endpoint);
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                let existing = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Block not found"))?;

                if let Some(FieldValue::Boolean(true)) = existing.fields.get("journal:deleted") {
                    return Err(PluginError::bad_request("Block is already deleted"));
                }

                let mut updates = HashMap::new();
                updates.insert("journal:deleted".to_string(), FieldValue::Boolean(true));
                let updated = ctx.update_node(id, updates).await?;
                HttpResponse::json(&updated)
            }

            // ── POST /blocks/render ─────────────────────────────────
            ("POST", "blocks/render") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let content = body["content"].as_str().unwrap_or("");
                let html = render_markdown(content);
                HttpResponse::json(&json!({ "html": html }))
            }

            // ── GET /pages ──────────────────────────────────────────
            ("GET", "pages") => {
                let journal_only = request.query_params.get("journal")
                    .map(|v| v == "true")
                    .unwrap_or(false);

                let mut preds = vec![
                    "HAS_FIELD(n, \"journal\", \"content\")".to_string(),
                ];

                if journal_only {
                    preds.push("n.journal.journal_day IS NOT NULL".to_string());
                } else {
                    // Pages have no parent_id
                    preds.push("n.journal.parent_id IS NULL".to_string());
                }

                let query = format!(
                    "MATCH (n) IN space(\"default\") WHERE {} RETURN n ORDER BY n.system.node_time DESC LIMIT 100",
                    preds.join(" AND ")
                );
                let rows = ctx.query(&query).await?;
                HttpResponse::json(&rows)
            }

            // ── GET /pages/today ────────────────────────────────────
            ("GET", "pages/today") => {
                let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();

                // Look for existing today page
                let query = format!(
                    "MATCH (n) IN space(\"default\") \
                     WHERE HAS_FIELD(n, \"journal\", \"journal_day\") \
                     AND n.journal.journal_day = \"{}\" \
                     RETURN n LIMIT 1",
                    today
                );
                let rows = ctx.query(&query).await?;
                if let Some(row) = rows.first() {
                    return HttpResponse::json(row);
                }

                // Auto-create today's journal page
                let mut node = Node::new(Uuid::nil());
                let title = Utc::now().format("%B %d, %Y").to_string();
                node.set_field("system:node_title", FieldValue::String(title.clone()));
                node.set_field("system:node_time", FieldValue::DateTime(Utc::now().to_rfc3339()));
                node.set_field("journal:content", FieldValue::String(String::new()));
                node.set_field("journal:journal_day", FieldValue::String(today));
                node.set_field("journal:order", FieldValue::String("a0".to_string()));
                node.set_field("journal:deleted", FieldValue::Boolean(false));

                let created = ctx.create_node(node).await?;

                let mut patch = HashMap::new();
                patch.insert(
                    "journal:page_id".to_string(),
                    FieldValue::NodeRef(created.id),
                );
                let updated = ctx.update_node(created.id, patch).await?;

                HttpResponse::json(&updated)
            }

            // ── GET /pages/{id}/backlinks ───────────────────────────
            ("GET", _) if endpoint.starts_with("pages/") && endpoint.contains("/backlinks") => {
                let path = endpoint.strip_suffix("/backlinks").unwrap_or(endpoint);
                let id_str = path.strip_prefix("pages/").unwrap_or("");
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| PluginError::bad_request("Invalid UUID"))?;

                // Find blocks whose refs contain this page UUID
                let query = format!(
                    "MATCH (n) IN space(\"default\") \
                     WHERE HAS_FIELD(n, \"journal\", \"refs\") \
                     AND n.journal.refs LIKE \"%{}%\" \
                     RETURN n ORDER BY n.system.node_time DESC LIMIT 50",
                    id
                );
                let rows = ctx.query(&query).await?;
                HttpResponse::json(&rows)
            }

            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {} {}",
                request.method, endpoint
            ))),
        }
    }
}
