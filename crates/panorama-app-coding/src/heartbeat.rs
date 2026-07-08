use chrono::{DateTime, Duration, Utc};
use panorama_core::*;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use uuid::Uuid;

use crate::helpers::*;
use crate::CodingPlugin;

impl CodingPlugin {
  /// Handle POST of heartbeats (single or bulk).
  /// This is the primary write path — compatible with the upstream Coding Activity
  /// API so existing editor plugins and `coding-activity-cli` can submit data.
  pub(crate) async fn handle_post_heartbeats(
    &self,
    request: HttpRequest,
    ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    let body: serde_json::Value = serde_json::from_slice(request.body.as_deref().unwrap_or(&[]))
      .map_err(|e| PluginError::bad_request(&format!("Invalid JSON: {}", e)))?;

    // Extract headers for UA parsing and machine name
    let user_agent = request
      .headers
      .iter()
      .find(|(k, _)| k.to_lowercase() == "user-agent")
      .map(|(_, v)| v.clone());
    let machine_from_header = request
      .headers
      .iter()
      .find(|(k, _)| k.to_lowercase() == "x-machine-name")
      .map(|(_, v)| v.clone());

    // Support both single heartbeat object and bulk array
    let heartbeats: Vec<serde_json::Value> = if body.is_array() {
      let arr = body.as_array().unwrap().clone();
      // Enforce bulk limit of 25 per Coding Activity spec
      if arr.len() > 25 {
        return Err(PluginError::bad_request(
          "Bulk heartbeats limited to 25 per request",
        ));
      }
      arr
    } else {
      vec![body]
    };

    let mut created = Vec::new();
    let mut errors = Vec::new();

    for hb in &heartbeats {
      match self
        .create_heartbeat_node(
          ctx,
          hb,
          user_agent.as_deref(),
          machine_from_header.as_deref(),
        )
        .await
      {
        Ok(node) => created.push(node),
        Err(e) => errors.push(serde_json::json!({
            "entity": hb.get("entity"),
            "error": {
              "message": e.message,
              "code": e.code,
              "backtrace_id": e.backtrace_id,
              "location": e.location.as_ref().map(|l| serde_json::json!({
                "file": l.file,
                "line": l.line,
              })),
            },
        })),
      }
    }

    let response = if errors.is_empty() {
      serde_json::json!({
          "status": "ok",
          "created": created.len(),
          "data": created.iter().map(|n| self.heartbeat_to_api_json(n)).collect::<Vec<_>>(),
      })
    } else {
      serde_json::json!({
          "status": "partial",
          "created": created.len(),
          "errors": errors,
      })
    };

    HttpResponse::json(&response)
  }

  /// Parse a heartbeat JSON value and create a node.
  pub(crate) async fn create_heartbeat_node(
    &self,
    ctx: &dyn PluginContext,
    hb: &serde_json::Value,
    user_agent: Option<&str>,
    machine_from_header: Option<&str>,
  ) -> Result<Node, PluginError> {
    let mut node = Node::new(Uuid::nil());

    // ── Required fields ────────────────────────────────────────
    let entity_raw = hb["entity"]
      .as_str()
      .ok_or_else(|| PluginError::bad_request("entity is required"))?;

    // Placeholder resolution
    let entity = self.resolve_placeholder(entity_raw, "entity", ctx).await;
    node.set_field("coding:entity", FieldValue::String(entity.clone()));

    let hb_type = hb["type"].as_str().unwrap_or("file");
    node.set_field("coding:type", FieldValue::String(hb_type.to_string()));

    let time = hb["time"]
      .as_f64()
      .unwrap_or_else(|| Utc::now().timestamp() as f64);
    node.set_field("coding:time", FieldValue::Float(time));
    node.set_field(
      "system:node_time",
      FieldValue::DateTime(
        DateTime::from_timestamp(time as i64, 0)
          .unwrap_or_else(|| Utc::now())
          .to_rfc3339(),
      ),
    );

    // ── Placeholder resolution for project/language ─────────────
    let project_raw = hb["project"].as_str().unwrap_or("");
    let project = self.resolve_placeholder(project_raw, "project", ctx).await;
    if !project.is_empty() && project != "<<LAST_PROJECT>>" {
      node.set_field("coding:project", FieldValue::String(project.clone()));
    }

    let branch_raw = hb["branch"].as_str().unwrap_or("");
    let branch = self.resolve_placeholder(branch_raw, "branch", ctx).await;
    if !branch.is_empty() && branch != "<<LAST_BRANCH>>" {
      node.set_field("coding:branch", FieldValue::String(branch.clone()));
    }

    let language_raw = hb["language"].as_str().unwrap_or("");
    let language = self
      .resolve_placeholder(language_raw, "language", ctx)
      .await;
    if !language.is_empty() && language != "<<LAST_LANGUAGE>>" {
      node.set_field("coding:language", FieldValue::String(language.clone()));
    }

    // Clear placeholders for url/domain type (Wakapi behavior)
    if hb_type == "url" || hb_type == "domain" {
      if entity == "<<LAST_PROJECT>>" || project == "<<LAST_PROJECT>>" {
        node.set_field("coding:project", FieldValue::String(String::new()));
      }
      if branch == "<<LAST_BRANCH>>" {
        node.set_field("coding:branch", FieldValue::String(String::new()));
      }
    }

    // ── Category (auto-assign if missing) ──────────────────────
    let category = hb["category"]
      .as_str()
      .map(|s| s.to_string())
      .unwrap_or_else(|| {
        if hb_type == "domain" || hb_type == "url" {
          "browsing".to_string()
        } else if !language.is_empty() {
          "coding".to_string()
        } else {
          String::new()
        }
      });
    if !category.is_empty() {
      node.set_field("coding:category", FieldValue::String(category.clone()));
    }

    // ── Editor / OS / UA ─────────────────────────────────────
    // Parse User-Agent for editor+OS, fall back to explicit fields in body
    let (parsed_editor, parsed_os) = user_agent
      .map(|ua| parse_user_agent(ua))
      .unwrap_or(("Unknown".to_string(), "Unknown".to_string()));

    let editor = hb["editor"].as_str().unwrap_or(&parsed_editor);
    node.set_field("coding:editor", FieldValue::String(editor.to_string()));

    let os = hb["operating_system"].as_str().unwrap_or(&parsed_os);
    node.set_field(
      "coding:operating_system",
      FieldValue::String(os.to_string()),
    );

    if let Some(ua) = user_agent {
      node.set_field("coding:user_agent", FieldValue::String(ua.to_string()));
    } else if let Some(ua) = hb["user_agent"].as_str() {
      node.set_field("coding:user_agent", FieldValue::String(ua.to_string()));
    }

    // ── Machine ────────────────────────────────────────────────
    let machine = machine_from_header
      .or_else(|| hb["machine"].as_str())
      .or_else(|| hb["machine_name_id"].as_str())
      .unwrap_or("unknown");
    node.set_field(
      "coding:machine_name_id",
      FieldValue::String(machine.to_string()),
    );

    // ── Origin ─────────────────────────────────────────────────
    if let Some(v) = hb["origin"].as_str() {
      node.set_field("coding:origin", FieldValue::String(v.to_string()));
    }

    // ── Optional core fields ───────────────────────────────────
    if let Some(v) = hb["project_root_count"].as_i64() {
      node.set_field("coding:project_root_count", FieldValue::Integer(v));
    }
    if let Some(v) = hb["dependencies"].as_str() {
      node.set_field("coding:dependencies", FieldValue::String(v.to_string()));
    }

    // ── Line changes ───────────────────────────────────────────
    if let Some(v) = hb["line_additions"].as_i64() {
      node.set_field("coding:line_additions", FieldValue::Integer(v));
    }
    if let Some(v) = hb["line_deletions"].as_i64() {
      node.set_field("coding:line_deletions", FieldValue::Integer(v));
    }

    // ── AI / GenAI fields ──────────────────────────────────────
    if let Some(v) = hb["ai_line_changes"].as_i64() {
      node.set_field("coding:ai_line_changes", FieldValue::Integer(v));
    }
    if let Some(v) = hb["human_line_changes"].as_i64() {
      node.set_field("coding:human_line_changes", FieldValue::Integer(v));
    }
    if let Some(v) = hb["ai_session"].as_str() {
      node.set_field("coding:ai_session", FieldValue::String(v.to_string()));
    }
    if let Some(v) = hb["ai_input_tokens"].as_i64() {
      node.set_field("coding:ai_input_tokens", FieldValue::Integer(v));
    }
    if let Some(v) = hb["ai_output_tokens"].as_i64() {
      node.set_field("coding:ai_output_tokens", FieldValue::Integer(v));
    }
    if let Some(v) = hb["ai_prompt_length"].as_i64() {
      node.set_field("coding:ai_prompt_length", FieldValue::Integer(v));
    }
    if let Some(v) = hb["ai_subscription_plan"].as_str() {
      node.set_field(
        "coding:ai_subscription_plan",
        FieldValue::String(v.to_string()),
      );
    }

    // ── Editor position fields ─────────────────────────────────
    if let Some(v) = hb["lines"].as_i64() {
      node.set_field("coding:lines", FieldValue::Integer(v));
    }
    if let Some(v) = hb["lineno"].as_i64() {
      node.set_field("coding:lineno", FieldValue::Integer(v));
    }
    if let Some(v) = hb["cursorpos"].as_i64() {
      node.set_field("coding:cursorpos", FieldValue::Integer(v));
    }
    if let Some(v) = hb["is_write"].as_bool() {
      node.set_field("coding:is_write", FieldValue::Boolean(v));
    }

    // ── Computed fields ────────────────────────────────────────
    // Duration: use payload value if provided, otherwise default 120s (2 min)
    // Always writing the field so PQL SUM(n.coding.duration) works on all nodes.
    let duration = hb["duration"].as_f64().unwrap_or(120.0);
    node.set_field("coding:duration", FieldValue::Float(duration));
    // AI flag
    node.set_field(
      "coding:is_ai_generated",
      FieldValue::Boolean(heartbeat_is_ai(hb)),
    );

    // ── Deduplication ──────────────────────────────────────────
    let hash = compute_dedup_hash(
      &entity, hb_type, &category, &project, &branch, &language, time, editor, os, machine,
    );
    node.set_field("coding:hash", FieldValue::String(hash.clone()));

    // Check for existing node with same hash — if found, skip creation.
    // CONFORMS TO resolves the schema's index on coding:hash, avoiding a SCAN error.
    let check_query = format!(
      "MATCH (n) IN space(\"default\") WHERE n CONFORMS TO schema(\"coding/Heartbeat\") AND n.coding.hash = \"{}\" RETURN n LIMIT 1",
      hash
    );
    if let Ok(rows) = ctx.query(&check_query).await {
      if !rows.is_empty() {
        if let Some(existing) = rows
          .iter()
          .filter_map(panorama_core::query::row_to_node)
          .next()
        {
          return Ok(existing);
        }
      }
    }

    ctx.create_node(node).await
  }

  /// Resolve placeholder values like <<LAST_PROJECT>> from recent heartbeats.
  async fn resolve_placeholder(&self, value: &str, field: &str, ctx: &dyn PluginContext) -> String {
    if value.starts_with("<<LAST_") && value.ends_with(">>") {
      // Query the most recent heartbeat for this value
      let coding_field = format!("coding:{}", field);
      let query = format!(
        "MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"{}\") RETURN n ORDER BY n.system.node_time DESC LIMIT 1",
        coding_field
      );
      if let Ok(rows) = ctx.query(&query).await {
        for row in &rows {
          if let Some(node) = panorama_core::query::row_to_node(row) {
            if let Some(v) = node.get_field(&coding_field) {
              match v {
                FieldValue::String(s) if !s.is_empty() => return s.clone(),
                _ => {}
              }
            }
          }
        }
      }
    }
    value.to_string()
  }

  /// Convert a stored heartbeat node back to Coding Activity API-compatible JSON.
  pub(crate) fn heartbeat_to_api_json(&self, node: &Node) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "id": node.id.to_string(),
        "entity": self.field_value(node, "entity"),
        "type": self.field_value(node, "type"),
        "time": node_time_epoch(node),
    });

    // Add optional string fields that are present
    for field in &[
      "category",
      "project",
      "branch",
      "language",
      "dependencies",
      "machine_name_id",
      "editor",
      "operating_system",
      "user_agent",
      "ai_session",
      "ai_subscription_plan",
      "origin",
    ] {
      if let Some(v) = node.get_field(&format!("coding:{}", field)) {
        match v {
          FieldValue::String(s) => {
            obj[field] = serde_json::Value::String(s.clone());
          }
          _ => {}
        }
      }
    }

    for field in &[
      "project_root_count",
      "ai_line_changes",
      "human_line_changes",
      "ai_input_tokens",
      "ai_output_tokens",
      "ai_prompt_length",
      "line_additions",
      "line_deletions",
      "lines",
      "lineno",
      "cursorpos",
    ] {
      if let Some(v) = node.get_field(&format!("coding:{}", field)) {
        match v {
          FieldValue::Integer(i) => {
            obj[field] = serde_json::Value::Number((*i).into());
          }
          _ => {}
        }
      }
    }

    if let Some(FieldValue::Boolean(b)) = node.get_field("coding:is_write") {
      obj["is_write"] = serde_json::Value::Bool(*b);
    }

    obj
  }

  /// Background task: roll up heartbeats into daily summaries.
  /// This runs periodically to pre-compute stats for fast dashboard queries.
  pub(crate) async fn rollup_daily_summaries(
    &self,
    ctx: &dyn PluginContext,
  ) -> Result<(), PluginError> {
    // Get yesterday's heartbeats
    let yesterday = (Utc::now() - Duration::days(1))
      .format("%Y-%m-%d")
      .to_string();
    let nodes = self.fetch_heartbeats_in_range(ctx, &yesterday).await?;

    if nodes.is_empty() {
      return Ok(());
    }

    // Group by (date, project, language, entity, category)
    let mut groups: HashMap<String, (f64, usize, usize, f64, i64, i64)> = HashMap::new();
    let mut seen_sessions: HashMap<String, HashSet<String>> = HashMap::new();

    for node in &nodes {
      let project = self.field_value(&node, "project");
      let language = self.field_value(&node, "language");
      let entity = self.field_value(&node, "entity");
      let category = self.field_value(&node, "category");
      let machine = self.field_value(&node, "machine_name_id");

      let key = format!(
        "{}|{}|{}|{}|{}",
        yesterday, project, language, entity, category
      );

      let duration = node_duration_seconds(&node);
      let is_ai = match node.get_field("coding:is_ai_generated") {
        Some(FieldValue::Boolean(b)) => *b,
        _ => false,
      };
      let human_lines = match node.get_field("coding:human_line_changes") {
        Some(FieldValue::Integer(n)) => *n,
        _ => 0,
      };

      let entry = groups.entry(key.clone()).or_insert((0.0, 0, 0, 0.0, 0, 0));
      entry.0 += duration;
      entry.1 += 1; // heartbeat count
      if is_ai {
        entry.3 += duration;
      }
      entry.4 += human_lines.max(0); // lines added (positive)
      entry.5 += (-human_lines).max(0); // lines removed (negative values)

      // Count sessions per project
      let session_key = format!("{}|{}", yesterday, machine);
      seen_sessions
        .entry(session_key)
        .or_default()
        .insert(project);
    }

    // Count sessions per project
    let mut project_sessions: HashMap<String, usize> = HashMap::new();
    for (_, projects) in &seen_sessions {
      for p in projects {
        *project_sessions.entry(p.clone()).or_default() += 1;
      }
    }

    // Create/update summary nodes
    for (key, (total_secs, hb_count, _, ai_secs, lines_added, lines_removed)) in &groups {
      let parts: Vec<&str> = key.split('|').collect();
      if parts.len() < 5 {
        continue;
      }
      let project = parts[1];
      let session_count = project_sessions.get(project).copied().unwrap_or(0) as u64;

      let mut node = Node::new(Uuid::nil());
      node.set_field(
        "system:node_time",
        FieldValue::DateTime(format!("{}T00:00:00Z", yesterday)),
      );
      node.set_field("coding:date", FieldValue::String(yesterday.clone()));
      node.set_field("coding:project", FieldValue::String(parts[1].to_string()));
      node.set_field("coding:language", FieldValue::String(parts[2].to_string()));
      node.set_field("coding:entity", FieldValue::String(parts[3].to_string()));
      node.set_field("coding:category", FieldValue::String(parts[4].to_string()));
      node.set_field("coding:total_seconds", FieldValue::Float(*total_secs));
      node.set_field(
        "coding:heartbeat_count",
        FieldValue::Integer(*hb_count as i64),
      );
      node.set_field(
        "coding:session_count",
        FieldValue::Integer(session_count as i64),
      );
      node.set_field("coding:ai_seconds", FieldValue::Float(*ai_secs));
      node.set_field("coding:lines_added", FieldValue::Integer(*lines_added));
      node.set_field("coding:lines_removed", FieldValue::Integer(*lines_removed));

      ctx.create_node(node).await?;
    }

    ctx
      .log(
        LogLevel::Info,
        &format!(
          "Daily summary rollup for {}: {} groups from {} heartbeats",
          yesterday,
          groups.len(),
          nodes.len()
        ),
      )
      .await;

    Ok(())
  }
}

/// Compute a deduplication hash from heartbeat fields.
/// Uses std DefaultHasher (SipHash-1-3) — no extra crate needed.
pub(crate) fn compute_dedup_hash(
  entity: &str,
  hb_type: &str,
  category: &str,
  project: &str,
  branch: &str,
  language: &str,
  time: f64,
  editor: &str,
  os: &str,
  machine: &str,
) -> String {
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  entity.hash(&mut hasher);
  hb_type.hash(&mut hasher);
  category.hash(&mut hasher);
  project.hash(&mut hasher);
  branch.hash(&mut hasher);
  language.hash(&mut hasher);
  time.to_bits().hash(&mut hasher);
  editor.hash(&mut hasher);
  os.hash(&mut hasher);
  machine.hash(&mut hasher);
  format!("{:x}", hasher.finish())
}

/// Parse a User-Agent string to extract editor and OS.
/// WakaTime plugin format:
///   wakatime/v1.x.x (os-arch) editor-name/vX.Y.Z editor-version
pub(crate) fn parse_user_agent(ua: &str) -> (String, String) {
  let ua = ua.trim();

  // Try after-paren first (WakaTime format), then before-paren
  let editor = if let Some(paren_close) = ua.find(')') {
    let after = ua[paren_close + 1..].trim();
    let from_after = after
      .split_whitespace()
      .next()
      .map(|t| t.split('/').next().unwrap_or(t))
      .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("wakatime"));

    from_after.unwrap_or_else(|| {
      // Fallback: token before paren
      let before = if let Some(paren_open) = ua.find('(') {
        ua[..paren_open].trim()
      } else {
        ua
      };
      before
        .split_whitespace()
        .last()
        .map(|t| t.split('/').next().unwrap_or(t))
        .unwrap_or("Unknown")
    })
  } else {
    ua.split_whitespace()
      .next()
      .map(|t| t.split('/').next().unwrap_or(t))
      .unwrap_or("Unknown")
  }
  .to_string();

  // Extract OS from parenthesized group
  let os = if let Some(start) = ua.find('(') {
    if let Some(end) = ua[start..].find(')') {
      let inner = &ua[start + 1..start + end];
      inner
        .split('-')
        .next()
        .unwrap_or(inner)
        .split('/')
        .next()
        .unwrap_or(inner)
        .trim()
        .to_string()
    } else {
      "Unknown".to_string()
    }
  } else {
    "Unknown".to_string()
  };

  (editor, os)
}
