use panorama_core::*;
use uuid::Uuid;

/// Full Coding Activity heartbeat schema covering every field in the upstream API
/// plus computed fields for local use.
pub(crate) fn heartbeat_schema() -> Schema {
  Schema {
    node_id: Uuid::nil(),
    name: "coding/Heartbeat".to_string(),
    version: SchemaVersion::new(2, 0),
    fields: vec![
      // ── System fields ──────────────────────────────────────
      schema_field(
        "node_time",
        "system",
        true,
        "When the heartbeat was recorded",
      ),
      // ── Core Coding Activity fields ───────────────────────────────
      schema_field(
        "entity",
        "coding",
        true,
        "File path, domain, or app identifier",
      ),
      schema_field(
        "type",
        "coding",
        true,
        "Entity type: file, app, url, or domain",
      ),
      schema_field(
        "category",
        "coding",
        false,
        "Activity category (coding, building, debugging, etc.)",
      ),
      schema_field(
        "time",
        "coding",
        true,
        "UNIX epoch timestamp with fractional seconds",
      ),
      schema_field("project", "coding", false, "Project name"),
      schema_field(
        "project_root_count",
        "coding",
        false,
        "Number of folders in project root path",
      ),
      schema_field("branch", "coding", false, "Git branch name"),
      schema_field("language", "coding", false, "Programming language name"),
      schema_field(
        "dependencies",
        "coding",
        false,
        "Comma-separated dependency list",
      ),
      schema_field(
        "machine_name_id",
        "coding",
        false,
        "Unique machine identifier",
      ),
      // ── Editor / OS fields ──────────────────────────────────
      schema_field("editor", "coding", false, "Editor/IDE name"),
      schema_field("operating_system", "coding", false, "Operating system name"),
      schema_field("user_agent", "coding", false, "Raw User-Agent string"),
      // ── Dedup / origin fields ───────────────────────────────
      schema_field("hash", "coding", false, "Deduplication hash"),
      schema_field(
        "origin",
        "coding",
        false,
        "Source of the heartbeat (e.g. wakatime)",
      ),
      // ── AI / GenAI fields ──────────────────────────────────
      schema_field(
        "ai_line_changes",
        "coding",
        false,
        "Lines added/removed by GenAI since last heartbeat",
      ),
      schema_field(
        "human_line_changes",
        "coding",
        false,
        "Lines added/removed by manual typing since last heartbeat",
      ),
      schema_field("ai_session", "coding", false, "AI session identifier"),
      schema_field(
        "ai_input_tokens",
        "coding",
        false,
        "User input tokens consumed since last heartbeat",
      ),
      schema_field(
        "ai_output_tokens",
        "coding",
        false,
        "Model output tokens consumed since last heartbeat",
      ),
      schema_field(
        "ai_prompt_length",
        "coding",
        false,
        "Prompt characters typed to AI since last heartbeat",
      ),
      schema_field(
        "ai_subscription_plan",
        "coding",
        false,
        "GenAI tool subscription plan tier",
      ),
      // ── Line change fields ──────────────────────────────────
      schema_field(
        "line_additions",
        "coding",
        false,
        "Lines added since last heartbeat",
      ),
      schema_field(
        "line_deletions",
        "coding",
        false,
        "Lines deleted since last heartbeat",
      ),
      // ── Editor position fields ─────────────────────────────
      schema_field("lines", "coding", false, "Total lines in the entity file"),
      schema_field(
        "lineno",
        "coding",
        false,
        "Current cursor line number (1-based)",
      ),
      schema_field(
        "cursorpos",
        "coding",
        false,
        "Current cursor column position (1-based)",
      ),
      schema_field(
        "is_write",
        "coding",
        false,
        "Whether this heartbeat was triggered by a write",
      ),
      // ── Computed fields (set by this plugin, not submitted by clients) ─
      schema_field(
        "duration",
        "coding",
        false,
        "Duration in seconds since previous heartbeat",
      ),
      schema_field(
        "is_ai_generated",
        "coding",
        false,
        "Whether this heartbeat involved AI code generation",
      ),
    ],
    schema_mode: SchemaMode::Preferred,
    previous_versions: vec![],
    migrations: vec![],
    indexes: vec![SchemaIndex {
      name: Some("idx_coding_heartbeat_hash".into()),
      fields: vec!["hash".into()],
      unique: false,
    }],
  }
}

/// A Duration represents a continuous coding session formed by grouping
/// consecutive heartbeats within the keystroke timeout window.
pub(crate) fn duration_schema() -> Schema {
  Schema {
    node_id: Uuid::nil(),
    name: "coding/Duration".to_string(),
    version: SchemaVersion::new(1, 0),
    fields: vec![
      schema_field(
        "node_time",
        "system",
        true,
        "Start time of the coding session",
      ),
      schema_field(
        "node_end_time",
        "system",
        true,
        "End time of the coding session",
      ),
      schema_field("entity", "coding", true, "Primary entity (file/domain)"),
      schema_field("project", "coding", false, "Project name"),
      schema_field("language", "coding", false, "Programming language"),
      schema_field("category", "coding", false, "Activity category"),
      schema_field(
        "duration_seconds",
        "coding",
        true,
        "Total duration in seconds",
      ),
      schema_field(
        "heartbeat_count",
        "coding",
        false,
        "Number of heartbeats in this session",
      ),
      schema_field(
        "machine_name_id",
        "coding",
        false,
        "Machine that produced this session",
      ),
    ],
    schema_mode: SchemaMode::Preferred,
    previous_versions: vec![],
    migrations: vec![],
    indexes: vec![],
  }
}

/// Pre-aggregated daily stats for fast dashboard queries.
pub(crate) fn daily_summary_schema() -> Schema {
  Schema {
    node_id: Uuid::nil(),
    name: "coding/DailySummary".to_string(),
    version: SchemaVersion::new(1, 0),
    fields: vec![
      schema_field("node_time", "system", true, "The date this summary covers"),
      schema_field("date", "coding", true, "ISO date string (YYYY-MM-DD)"),
      schema_field("project", "coding", false, "Project name"),
      schema_field("language", "coding", false, "Programming language"),
      schema_field("entity", "coding", false, "File path / entity"),
      schema_field("category", "coding", false, "Activity category"),
      schema_field(
        "total_seconds",
        "coding",
        true,
        "Total coding seconds for this dimension",
      ),
      schema_field("heartbeat_count", "coding", false, "Number of heartbeats"),
      schema_field(
        "session_count",
        "coding",
        false,
        "Number of coding sessions",
      ),
      schema_field(
        "ai_seconds",
        "coding",
        false,
        "Seconds involving AI-generated code",
      ),
      schema_field("lines_added", "coding", false, "Total human lines added"),
      schema_field(
        "lines_removed",
        "coding",
        false,
        "Total human lines removed",
      ),
    ],
    schema_mode: SchemaMode::Preferred,
    previous_versions: vec![],
    migrations: vec![],
    indexes: vec![],
  }
}

/// Convenience helper for building schema fields.
pub(crate) fn schema_field(
  name: &str,
  namespace: &str,
  required: bool,
  description: &str,
) -> SchemaField {
  let type_tag = ft(name);
  SchemaField {
    name: name.to_string(),
    namespace: namespace.to_string(),
    field_type: Some(FieldTypeConstraint {
      type_tag: type_tag.into(),
      element_type: None,
    }),
    required,
    default: None,
    description: Some(description.to_string()),
    computed: None,
  }
}

/// Infer the FieldTypeConstraint type_tag from a field name.
pub(crate) fn ft(name: &str) -> &str {
  match name {
    // DateTime fields
    "node_time" | "node_end_time" | "time" | "created_at" | "updated_at" => "DateTime",
    // Integer fields
    "cursorpos" | "lineno" | "heartbeat_count" | "session_count" | "lines_added"
    | "lines_removed" | "total_lines" | "line_additions" | "line_deletions"
    | "project_root_count" | "ai_line_changes" | "ai_input_tokens" | "ai_output_tokens"
    | "ai_prompt_length" | "human_line_changes" => "Integer",
    // Float fields
    "duration" | "duration_seconds" | "total_seconds" | "ai_seconds" => "Float",
    // Boolean fields
    "is_write" | "is_debugging" | "is_ai_generated" | "is_unsaved_entity" => "Boolean",
    // Integer (epoch) fields — stored as Float for precision
    "sent_at" => "Float",
    // Everything else is a String
    _ => "String",
  }
}
