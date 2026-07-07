//! Coding Activity Plugin — Full Coding Activity API-compatible heartbeat collection,
//! duration computation, and multi-dimensional stats engine.
//!
//! ## Architecture
//!
//! This plugin implements the complete [Coding Activity v1 Heartbeat API](
//! compatible heartbeat API) for both read and write
//! operations as well as status/stats endpoints.
//!
//! ### Data Model
//!
//! Three node schemas:
//! - **Heartbeat** — raw editor heartbeat with all 25+ Coding Activity API fields
//!   plus computed fields (`duration`, `is_ai_generated`).
//! - **Duration** — materialized coding session produced by grouping
//!   heartbeats within a configurable timeout (default: 15 min).
//! - **DailySummary** — rolled-up stats per (date, project, language,
//!   entity, category) for fast dashboard queries.
//!
//! ### Stats Engine
//!
//! The `/stats` endpoint powers the four dashboard panels visible in the
//! reference screenshot:
//! 1. **Per-project leaderboard** — hours per project, sorted descending
//! 2. **Per-file leaderboard** — hours per entity, sorted descending
//! 3. **Time-series** — bucketed activity (by hour / day / week) over a range
//! 4. **Per-language leaderboard** — hours per language, sorted descending
//!
//! ### Coding Activity API Compatibility
//!
//! Endpoints mirror the upstream API so existing Coding Activity editor plugins and
//! the `coding-activity-cli` can submit heartbeats without modification:
//! - `POST /users/current/heartbeats` (Coding Activity-compatible path)
//! - `POST /users/current/heartbeats.bulk` (bulk, up to 25)
//! - `GET /users/current/heartbeats?date=YYYY-MM-DD`
//! - `GET /users/current/durations?date=YYYY-MM-DD`
//!
//! Convenience paths (no `/users/current` prefix):
//! - `POST /heartbeat`, `POST /heartbeats`
//! - `GET /heartbeats?date=YYYY-MM-DD`
//! - `GET /stats?range=7d&group_by=project`

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use panorama_core::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

// ── Plugin struct ─────────────────────────────────────────────────────────────

pub struct CodingPlugin;

impl CodingPlugin {
  pub fn new() -> Self {
    Self
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Schemas
// ═══════════════════════════════════════════════════════════════════════════════

impl CodingPlugin {
  /// Full Coding Activity heartbeat schema covering every field in the upstream API
  /// plus computed fields for local use.
  fn heartbeat_schema() -> Schema {
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
    }
  }

  /// A Duration represents a continuous coding session formed by grouping
  /// consecutive heartbeats within the keystroke timeout window.
  fn duration_schema() -> Schema {
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
    }
  }

  /// Pre-aggregated daily stats for fast dashboard queries.
  fn daily_summary_schema() -> Schema {
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
    }
  }
}

/// Convenience helper for building schema fields.
fn schema_field(name: &str, namespace: &str, required: bool, description: &str) -> SchemaField {
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
fn ft(name: &str) -> &str {
  match name {
    // DateTime fields
    "node_time" | "node_end_time" | "time" | "created_at" | "updated_at" => "DateTime",
    // Integer fields
    "cursorpos" | "lineno" | "heartbeat_count" | "session_count" | "lines_added"
    | "lines_removed" | "total_lines" => "Integer",
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

// ═══════════════════════════════════════════════════════════════════════════════
// Stats Engine
// ═══════════════════════════════════════════════════════════════════════════════

/// Parameters for a stats query.
#[derive(Debug, Deserialize)]
struct StatsQuery {
  /// Time range: `24h`, `7d`, `30d`, `90d`, `all`, or custom
  /// `YYYY-MM-DD..YYYY-MM-DD`.
  #[serde(default = "default_range")]
  range: String,
  /// Dimension to group by: `project`, `language`, `entity`, `category`,
  /// `date`, `hour`, `machine`, `branch`.
  #[serde(default)]
  group_by: Option<String>,
  /// Sub-dimension for two-level grouping (e.g. group_by=project,
  /// sub_group_by=language).
  #[serde(default)]
  sub_group_by: Option<String>,
  /// Aggregation: `sum` (total seconds), `count` (heartbeat count),
  /// `leaderboard` (sum + sort desc), `avg_daily`, `timeseries`.
  #[serde(default = "default_aggregation")]
  aggregation: String,
  /// Optional filter: `project=panorama`, `language=Rust`, `category=coding`.
  #[serde(default)]
  filter: Option<String>,
  /// For timeseries: bucket size. `hour`, `day`, `week`. Default: `day`.
  #[serde(default = "default_bucket")]
  bucket: String,
  /// Limit results (for leaderboard). Default: 25.
  #[serde(default = "default_limit")]
  limit: usize,
}

fn default_range() -> String {
  "7d".to_string()
}
fn default_aggregation() -> String {
  "leaderboard".to_string()
}
fn default_bucket() -> String {
  "day".to_string()
}
fn default_limit() -> usize {
  25
}

impl CodingPlugin {
  /// Execute a stats query against stored heartbeats using PQL aggregation.
  ///
  /// Instead of loading all heartbeats into memory, this constructs a PQL query
  /// with aggregate functions (COUNT, SUM) and implicit GROUP BY so that the
  /// storage engine (SQLite) performs the aggregation and returns only the
  /// final summary rows.
  async fn execute_stats(
    &self,
    ctx: &dyn PluginContext,
    query: &StatsQuery,
  ) -> Result<serde_json::Value, PluginError> {
    let group_by = query.group_by.as_deref().unwrap_or("project");

    // Map leaderboard/sum aggregations to DB-side PQL queries.
    match query.aggregation.as_str() {
      "leaderboard" | "sum" => {
        self.execute_aggregated_leaderboard(ctx, query, group_by).await
      }
      "count" => {
        self.execute_aggregated_count(ctx, query, group_by).await
      }
      "avg_daily" => {
        // Compute via PQL sum first, then divide by day count.
        let sum_result = self
          .execute_aggregated_leaderboard(ctx, query, group_by)
          .await?;
        let (start, end) = parse_time_range(&query.range);
        let days = (end - start).num_days().max(1) as f64;
        let adjusted: Vec<serde_json::Value> = match sum_result {
          serde_json::Value::Array(entries) => {
            entries
              .into_iter()
              .map(|mut entry| {
                if let Some(obj) = entry.as_object_mut() {
                  if let Some(total) = obj.get("total_seconds").and_then(|v| v.as_f64()) {
                    obj.insert(
                      "avg_seconds_per_day".into(),
                      serde_json::json!(total / days),
                    );
                    obj.insert(
                      "avg_hours_per_day".into(),
                      serde_json::json!(total / days / 3600.0),
                    );
                  }
                }
                entry
              })
              .collect()
          }
          _ => return Ok(sum_result),
        };
        Ok(serde_json::json!(adjusted))
      }
      "timeseries" => {
        // Timeseries is complex (time bucketing). Fall back to in-memory
        // for now; future work can push bucketing into SQL.
        let all_nodes = self.fetch_heartbeats_in_range(ctx, &query.range).await?;
        if all_nodes.is_empty() {
          return Ok(serde_json::json!([]));
        }
        let filtered = self.apply_filter(&all_nodes, &query.filter);
        self.compute_timeseries(&filtered, group_by, &query.bucket, &query.range)
      }
      _ => Err(PluginError::bad_request(&format!(
        "Unknown aggregation: {}. Supported: leaderboard, sum, count, avg_daily, timeseries",
        query.aggregation
      ))),
    }
  }

  /// Execute a PQL SUM aggregation query grouped by a dimension.
  /// Used for leaderboard and sum aggregations.
  async fn execute_aggregated_leaderboard(
    &self,
    ctx: &dyn PluginContext,
    query: &StatsQuery,
    group_by: &str,
  ) -> Result<serde_json::Value, PluginError> {
    let (start_epoch, end_epoch) = parse_time_range_epoch(&query.range);

    // Build time filter predicates
    let time_filter = format!(
      "AND SCAN(n.coding.time >= {}) AND SCAN(n.coding.time <= {})",
      start_epoch, end_epoch
    );

    // Optional key=value filter
    let filter_clause = build_filter_clause(&query.filter);

    let pql = format!(
      "MATCH (n) IN space(\"default\") \
       WHERE HAS_FIELD(n, \"coding\", \"entity\") \
         {} {} \
       RETURN n.coding.{} AS key, SUM(n.coding.duration) AS total_seconds",
      time_filter, filter_clause, group_by
    );

    let rows = ctx.query(&pql).await?;

    // Post-process: sort descending, apply limit, compute hours
    let mut entries: Vec<serde_json::Value> = rows
      .into_iter()
      .map(|row| {
        let total_seconds = row
          .get("total_seconds")
          .and_then(|v| v.as_f64())
          .unwrap_or(0.0);
        let key = row
          .get("key")
          .and_then(|v| v.as_str())
          .unwrap_or("(unknown)")
          .to_string();
        serde_json::json!({
          "key": key,
          "total_seconds": total_seconds,
          "hours": (total_seconds / 3600.0 * 10.0).round() / 10.0,
        })
      })
      .collect();

    // Sort descending by total_seconds
    entries.sort_by(|a, b| {
      b["total_seconds"]
        .as_f64()
        .unwrap_or(0.0)
        .partial_cmp(&a["total_seconds"].as_f64().unwrap_or(0.0))
        .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Apply limit
    let limit = query.limit;
    if entries.len() > limit {
      entries.truncate(limit);
    }

    Ok(serde_json::json!(entries))
  }

  /// Execute a PQL COUNT aggregation query grouped by a dimension.
  async fn execute_aggregated_count(
    &self,
    ctx: &dyn PluginContext,
    query: &StatsQuery,
    group_by: &str,
  ) -> Result<serde_json::Value, PluginError> {
    let (start_epoch, end_epoch) = parse_time_range_epoch(&query.range);

    let time_filter = format!(
      "AND SCAN(n.coding.time >= {}) AND SCAN(n.coding.time <= {})",
      start_epoch, end_epoch
    );

    let filter_clause = build_filter_clause(&query.filter);

    let pql = format!(
      "MATCH (n) IN space(\"default\") \
       WHERE HAS_FIELD(n, \"coding\", \"entity\") \
         {} {} \
       RETURN n.coding.{} AS key, COUNT(n) AS count",
      time_filter, filter_clause, group_by
    );

    let rows = ctx.query(&pql).await?;

    let mut entries: Vec<serde_json::Value> = rows
      .into_iter()
      .map(|row| {
        let count = row.get("count").and_then(|v| v.as_f64()).unwrap_or(0.0) as u64;
        let key = row
          .get("key")
          .and_then(|v| v.as_str())
          .unwrap_or("(unknown)")
          .to_string();
        serde_json::json!({ "key": key, "count": count })
      })
      .collect();

    // Sort descending by count
    entries.sort_by(|a, b| {
      b["count"]
        .as_u64()
        .unwrap_or(0)
        .cmp(&a["count"].as_u64().unwrap_or(0))
    });

    let limit = query.limit;
    if entries.len() > limit {
      entries.truncate(limit);
    }

    Ok(serde_json::json!(entries))
  }

  /// Fetch all heartbeat nodes within a time range.
  async fn fetch_heartbeats_in_range(
    &self,
    ctx: &dyn PluginContext,
    range: &str,
  ) -> Result<Vec<Node>, PluginError> {
    let rows = ctx
            .query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"coding\", \"entity\") RETURN n ORDER BY n.system.node_time ASC")
            .await?;
    let all_nodes: Vec<Node> = rows
      .iter()
      .filter_map(panorama_core::query::row_to_node)
      .collect();

    let (start, end) = parse_time_range(range);
    let filtered: Vec<Node> = all_nodes
      .into_iter()
      .filter(|n| node_time_in_range(n, start, end))
      .collect();
    Ok(filtered)
  }

  /// Apply a simple `key=value` filter.
  fn apply_filter(&self, nodes: &[Node], filter: &Option<String>) -> Vec<Node> {
    let Some(f) = filter else {
      return nodes.to_vec();
    };
    let Some((key, value)) = f.split_once('=') else {
      return nodes.to_vec();
    };
    let field_key = if key.contains(':') {
      key.to_string()
    } else {
      format!("coding:{}", key)
    };

    nodes
      .iter()
      .filter(|n| {
        n.get_field(&field_key)
          .map(|v| match v {
            FieldValue::String(s) => s == value,
            _ => false,
          })
          .unwrap_or(false)
      })
      .cloned()
      .collect()
  }

  /// Time-series: bucketed data points over the time range.
  fn compute_timeseries(
    &self,
    nodes: &[Node],
    group_by: &str,
    bucket: &str,
    range: &str,
  ) -> Result<serde_json::Value, PluginError> {
    let (start, end) = parse_time_range(range);

    // Build bucketed time series keyed by (bucket_ts, group_key) -> seconds
    let mut buckets: HashMap<(i64, String), f64> = HashMap::new();

    for node in nodes {
      let ts = node_time_epoch(node) as i64;
      let bucket_ts: i64 = match bucket {
        "hour" => ts - (ts % 3600),
        "week" => {
          // Align to Monday 00:00 UTC
          let dt = DateTime::from_timestamp(ts, 0).unwrap_or_else(|| Utc::now());
          let weekday = dt.weekday().num_days_from_monday() as i64;
          let day_start = ts - (ts % 86400);
          day_start - weekday * 86400
        }
        _ => ts - (ts % 86400), // day default
      };
      let key = self.group_key(node, group_by, None);
      let duration = node_duration_seconds(node);
      *buckets.entry((bucket_ts, key)).or_default() += duration;
    }

    // Convert to sorted series
    let mut series: BTreeMap<String, Vec<(i64, f64)>> = BTreeMap::new();
    for ((ts, key), secs) in &buckets {
      series.entry(key.clone()).or_default().push((*ts, *secs));
    }

    // Sort each series by timestamp and fill gaps with zeros
    let interval_secs: i64 = match bucket {
      "hour" => 3600,
      "week" => 604800,
      _ => 86400,
    };

    let result: Vec<serde_json::Value> = series
      .into_iter()
      .map(|(key, mut points)| {
        points.sort_by_key(|(ts, _)| *ts);

        // Fill gaps
        let filled =
          self.fill_time_gaps(&points, start.timestamp(), end.timestamp(), interval_secs);

        serde_json::json!({
            "key": key,
            "bucket": bucket,
            "data": filled.into_iter().map(|(ts, v)| {
                serde_json::json!({
                    "time": ts,
                    "iso": DateTime::from_timestamp(ts, 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_default(),
                    "seconds": v,
                    "hours": (v / 3600.0 * 10.0).round() / 10.0,
                })
            }).collect::<Vec<_>>(),
        })
      })
      .collect();

    Ok(serde_json::json!(result))
  }

  /// Fill gaps in time series with zero values.
  fn fill_time_gaps(
    &self,
    points: &[(i64, f64)],
    start: i64,
    end: i64,
    interval: i64,
  ) -> Vec<(i64, f64)> {
    if points.is_empty() {
      let mut result = Vec::new();
      let mut t = start;
      while t <= end {
        result.push((t, 0.0));
        t += interval;
      }
      return result;
    }

    let mut result = Vec::new();
    let point_map: HashMap<i64, f64> = points.iter().cloned().collect();
    let mut t = start;
    while t <= end {
      result.push((t, point_map.get(&t).copied().unwrap_or(0.0)));
      t += interval;
    }
    result
  }

  /// Build a grouping key for a node.
  fn group_key(&self, node: &Node, group_by: &str, sub_group_by: Option<&str>) -> String {
    let primary = self.field_value(node, group_by);
    if let Some(sg) = sub_group_by {
      let secondary = self.field_value(node, sg);
      format!("{}::{}", primary, secondary)
    } else {
      primary
    }
  }

  /// Extract a field value as a string for grouping.
  fn field_value(&self, node: &Node, field: &str) -> String {
    let key = if field.contains(':') {
      field.to_string()
    } else {
      format!("coding:{}", field)
    };
    match node.get_field(&key) {
      Some(FieldValue::String(s)) => s.clone(),
      Some(FieldValue::Integer(i)) => i.to_string(),
      Some(FieldValue::Float(f)) => f.to_string(),
      Some(FieldValue::Boolean(b)) => b.to_string(),
      _ => "(unknown)".to_string(),
    }
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Duration Computation
// ═══════════════════════════════════════════════════════════════════════════════

impl CodingPlugin {
  /// Compute durations from heartbeats for a given date.
  /// Groups consecutive heartbeats within 15 minutes of each other.
  async fn compute_durations(
    &self,
    ctx: &dyn PluginContext,
    date: &str,
  ) -> Result<serde_json::Value, PluginError> {
    let all_nodes = self.fetch_heartbeats_in_range(ctx, date).await?;

    // Sort by time
    let mut sorted: Vec<&Node> = all_nodes.iter().collect();
    sorted.sort_by(|a, b| {
      node_time_epoch(a)
        .partial_cmp(&node_time_epoch(b))
        .unwrap_or(std::cmp::Ordering::Equal)
    });

    let timeout_secs: f64 = 900.0; // 15 minutes
    let mut durations: Vec<serde_json::Value> = Vec::new();
    let mut session_start: Option<&Node> = None;
    let mut session_end: Option<&Node> = None;
    let mut hb_count: u32 = 0;

    for hb in &sorted {
      match session_start {
        None => {
          session_start = Some(hb);
          session_end = Some(hb);
          hb_count = 1;
        }
        Some(start) => {
          let prev_time = node_time_epoch(session_end.unwrap());
          let curr_time = node_time_epoch(hb);
          if curr_time - prev_time <= timeout_secs {
            session_end = Some(hb);
            hb_count += 1;
          } else {
            // Flush current session
            durations.push(self.build_duration_entry(start, session_end.unwrap(), hb_count));
            session_start = Some(hb);
            session_end = Some(hb);
            hb_count = 1;
          }
        }
      }
    }
    // Flush last session
    if let (Some(start), Some(end)) = (session_start, session_end) {
      durations.push(self.build_duration_entry(start, end, hb_count));
    }

    Ok(serde_json::json!(durations))
  }

  fn build_duration_entry(&self, start: &Node, end: &Node, hb_count: u32) -> serde_json::Value {
    let start_epoch = node_time_epoch(start);
    let end_epoch = node_time_epoch(end);
    let duration = end_epoch - start_epoch;

    serde_json::json!({
        "start_time": DateTime::from_timestamp(start_epoch as i64, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        "end_time": DateTime::from_timestamp(end_epoch as i64, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        "duration_seconds": duration,
        "entity": self.field_value(start, "entity"),
        "project": self.field_value(start, "project"),
        "language": self.field_value(start, "language"),
        "category": self.field_value(start, "category"),
        "machine_name_id": self.field_value(start, "machine_name_id"),
        "heartbeat_count": hb_count,
    })
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Parse a time range string into (start, end) DateTime<Utc>.
///
/// Supported formats:
/// - `24h`, `7d`, `30d`, `90d`, `365d`, `all` — relative to now
/// - `YYYY-MM-DD` — single day
/// - `YYYY-MM-DD..YYYY-MM-DD` — explicit range
fn parse_time_range(range: &str) -> (DateTime<Utc>, DateTime<Utc>) {
  let now = Utc::now();
  match range {
    "24h" => (now - Duration::hours(24), now),
    "7d" => (now - Duration::days(7), now),
    "30d" => (now - Duration::days(30), now),
    "90d" => (now - Duration::days(90), now),
    "365d" => (now - Duration::days(365), now),
    "all" => (DateTime::from_timestamp(0, 0).unwrap(), now),
    other => {
      // Try explicit range: YYYY-MM-DD..YYYY-MM-DD
      if let Some((from, to)) = other.split_once("..") {
        let start = NaiveDate::parse_from_str(from.trim(), "%Y-%m-%d")
          .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
          .map(|d| d.and_utc())
          .unwrap_or(now - Duration::days(7));
        let end = NaiveDate::parse_from_str(to.trim(), "%Y-%m-%d")
          .map(|d| d.and_hms_opt(23, 59, 59).unwrap())
          .map(|d| d.and_utc())
          .unwrap_or(now);
        (start, end)
      } else {
        // Single date
        NaiveDate::parse_from_str(other.trim(), "%Y-%m-%d")
          .map(|d| {
            let start = d.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = d.and_hms_opt(23, 59, 59).unwrap().and_utc();
            (start, end)
          })
          .unwrap_or_else(|_| (now - Duration::days(7), now))
      }
    }
  }
}

/// Parse a time range string into (start_epoch, end_epoch) as f64 epoch seconds.
/// Uses the same range format as `parse_time_range`.
fn parse_time_range_epoch(range: &str) -> (f64, f64) {
  let (start, end) = parse_time_range(range);
  (start.timestamp() as f64, end.timestamp() as f64)
}

/// Build an optional filter clause from a `key=value` string.
/// Returns a PQL snippet like `AND n.coding.project = 'panorama'` or empty.
fn build_filter_clause(filter: &Option<String>) -> String {
  match filter {
    Some(f) if f.contains('=') => {
      let (key, value) = f.split_once('=').unwrap();
      format!("AND n.coding.{} = '{}'", key, value)
    }
    _ => String::new(),
  }
}

/// Check if a node's time falls within [start, end].
fn node_time_in_range(node: &Node, start: DateTime<Utc>, end: DateTime<Utc>) -> bool {
  let ts = node_time_epoch(node);
  let start_ts = start.timestamp() as f64;
  let end_ts = end.timestamp() as f64;
  ts >= start_ts && ts <= end_ts
}

/// Extract the epoch timestamp from a node in seconds.
fn node_time_epoch(node: &Node) -> f64 {
  // Try coding:time (the raw heartbeat timestamp) first
  if let Some(FieldValue::Float(ts)) = node.get_field("coding:time") {
    return *ts;
  }
  if let Some(FieldValue::Integer(ts)) = node.get_field("coding:time") {
    return *ts as f64;
  }
  // Fall back to system:node_time (ISO 8601 string)
  if let Some(FieldValue::DateTime(s)) = node.get_field("system:node_time") {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
      return dt.timestamp() as f64;
    }
  }
  0.0
}

/// Extract duration in seconds from a node.
fn node_duration_seconds(node: &Node) -> f64 {
  // Use explicit duration field if present
  if let Some(FieldValue::Float(d)) = node.get_field("coding:duration") {
    if *d > 0.0 {
      return *d;
    }
  }
  if let Some(FieldValue::Integer(d)) = node.get_field("coding:duration") {
    if *d > 0 {
      return *d as f64;
    }
  }
  // Default: each heartbeat = 120 seconds (2 min) of activity
  120.0
}

/// Check if a heartbeat involves AI code generation.
fn heartbeat_is_ai(hb: &serde_json::Value) -> bool {
  // If ai_session is set or ai_line_changes > 0, it's AI-assisted
  hb.get("ai_session")
    .and_then(|v| v.as_str())
    .map(|s| !s.is_empty())
    .unwrap_or(false)
    || hb
      .get("ai_line_changes")
      .and_then(|v| v.as_i64())
      .map(|n| n > 0)
      .unwrap_or(false)
    || hb
      .get("ai_input_tokens")
      .and_then(|v| v.as_i64())
      .map(|n| n > 0)
      .unwrap_or(false)
    || hb
      .get("ai_output_tokens")
      .and_then(|v| v.as_i64())
      .map(|n| n > 0)
      .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin Trait Implementation
// ═══════════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Plugin for CodingPlugin {
  fn id(&self) -> &str {
    "io.mzhang.panorama.coding"
  }

  fn name(&self) -> &str {
    "Coding Activity"
  }

  fn version(&self) -> &str {
    "0.2.0"
  }

  fn description(&self) -> &str {
    "Full Coding Activity API-compatible heartbeat collection with stats, durations, and leaderboards"
  }

  fn schemas(&self) -> Vec<Schema> {
    vec![
      Self::heartbeat_schema(),
      Self::duration_schema(),
      Self::daily_summary_schema(),
    ]
  }

  fn http_endpoints(&self) -> Vec<HttpEndpoint> {
    vec![
      // ── Coding Activity-compatible paths ──────────────────────────
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/users/current/heartbeats".to_string(),
        description: "Coding Activity API: receive a single heartbeat".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/users/current/heartbeats.bulk".to_string(),
        description: "Coding Activity API: receive bulk heartbeats (max 25)".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/users/current/heartbeats".to_string(),
        description: "Coding Activity API: get heartbeats for a date".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/users/current/durations".to_string(),
        description: "Coding Activity API: get durations for a date".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::DELETE,
        path: "/users/current/heartbeats.bulk".to_string(),
        description: "Coding Activity API: delete heartbeats for a date".to_string(),
      },
      // ── Convenience paths ──────────────────────────────────
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/heartbeat".to_string(),
        description: "Receive a single heartbeat (shorthand)".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/heartbeats".to_string(),
        description: "Receive bulk heartbeats (shorthand, max 25)".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/heartbeats".to_string(),
        description: "Get heartbeats for a date (shorthand)".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/durations".to_string(),
        description: "Get durations for a date (shorthand)".to_string(),
      },
      // ── Stats endpoint ─────────────────────────────────────
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/stats".to_string(),
        description: "Multi-dimensional stats: leaderboard, timeseries, sums".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/stats".to_string(),
        description: "Multi-dimensional stats with JSON query body".to_string(),
      },
      // ── Summary endpoints ──────────────────────────────────
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/summaries".to_string(),
        description: "Get daily summaries".to_string(),
      },
    ]
  }

  fn background_tasks(&self) -> Vec<BackgroundTask> {
    vec![BackgroundTask {
      name: "daily-summary-rollup".to_string(),
      interval_seconds: Some(3600), // Every hour
      description: "Roll up heartbeats into daily summary nodes for fast dashboard queries"
        .to_string(),
    }]
  }

  fn ui_components(&self) -> Vec<UiComponent> {
    vec![
      UiComponent {
        id: "coding-dashboard".to_string(),
        name: "Coding Activity Dashboard".to_string(),
        mount_point: UiMountPoint::Dashboard,
        bundle_path: "ui/coding.js".to_string(),
      },
      UiComponent {
        id: "coding-main".to_string(),
        name: "Coding Activity".to_string(),
        mount_point: UiMountPoint::MainPage,
        bundle_path: "ui/coding.js".to_string(),
      },
    ]
  }

  fn required_capabilities(&self) -> CapabilityGrants {
    CapabilityGrants {
      field_write: vec![
        "coding:*".to_string(),
        "system:node_time".to_string(),
        "system:node_end_time".to_string(),
      ],
      field_read: vec!["*".to_string()],
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
      // ── POST heartbeat(s) — Coding Activity-compatible ────────────
      ("POST", "users/current/heartbeats")
      | ("POST", "heartbeat")
      | ("POST", "heartbeats")
      | ("POST", "users/current/heartbeats.bulk") => {
        self.handle_post_heartbeats(request, ctx).await
      }

      // ── GET heartbeats ─────────────────────────────────────
      ("GET", "users/current/heartbeats") | ("GET", "heartbeats") => {
        let date = request
          .query_params
          .get("date")
          .cloned()
          .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
        let nodes = self.fetch_heartbeats_in_range(ctx, &date).await?;
        let result: Vec<serde_json::Value> = nodes
          .iter()
          .map(|n| self.heartbeat_to_api_json(n))
          .collect();

        let (start, end) = parse_time_range(&date);
        HttpResponse::json(&serde_json::json!({
            "data": result,
            "start": start.to_rfc3339(),
            "end": end.to_rfc3339(),
            "timezone": "UTC",
        }))
      }

      // ── GET durations ──────────────────────────────────────
      ("GET", "users/current/durations") | ("GET", "durations") => {
        let date = request
          .query_params
          .get("date")
          .cloned()
          .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
        let result = self.compute_durations(ctx, &date).await?;
        HttpResponse::json(&serde_json::json!({
            "data": result,
            "timezone": "UTC",
        }))
      }

      // ── GET / POST stats ────────────────────────────────────
      ("GET", "stats") => {
        let query = StatsQuery {
          range: request
            .query_params
            .get("range")
            .cloned()
            .unwrap_or_default(),
          group_by: request.query_params.get("group_by").cloned(),
          sub_group_by: request.query_params.get("sub_group_by").cloned(),
          aggregation: request
            .query_params
            .get("aggregation")
            .cloned()
            .unwrap_or_default(),
          filter: request.query_params.get("filter").cloned(),
          bucket: request
            .query_params
            .get("bucket")
            .cloned()
            .unwrap_or_default(),
          limit: request
            .query_params
            .get("limit")
            .and_then(|l| l.parse().ok())
            .unwrap_or(25),
        };
        let result = self.execute_stats(ctx, &query).await?;
        HttpResponse::json(&result)
      }
      ("POST", "stats") => {
        let query: StatsQuery = serde_json::from_slice(request.body.as_deref().unwrap_or(&[]))
          .map_err(|e| PluginError::bad_request(&format!("Invalid stats query: {}", e)))?;
        let result = self.execute_stats(ctx, &query).await?;
        HttpResponse::json(&result)
      }

      // ── DELETE heartbeats.bulk ─────────────────────────────
      ("DELETE", "users/current/heartbeats.bulk") => {
        let body: serde_json::Value =
          serde_json::from_slice(request.body.as_deref().unwrap_or(&[]))
            .map_err(|e| PluginError::bad_request(&e.to_string()))?;

        let date = body["date"]
          .as_str()
          .ok_or_else(|| PluginError::bad_request("date field required (YYYY-MM-DD)"))?;
        let ids: Vec<String> = body["ids"]
          .as_array()
          .map(|a| {
            a.iter()
              .filter_map(|v| v.as_str().map(String::from))
              .collect()
          })
          .unwrap_or_default();

        let mut deleted = 0;
        if !ids.is_empty() {
          for id_str in &ids {
            if let Ok(id) = Uuid::parse_str(id_str) {
              if ctx.delete_node(id).await.is_ok() {
                deleted += 1;
              }
            }
          }
        } else {
          // Delete all heartbeats for the date
          let nodes = self.fetch_heartbeats_in_range(ctx, date).await?;
          for node in &nodes {
            if ctx.delete_node(node.id).await.is_ok() {
              deleted += 1;
            }
          }
        }

        HttpResponse::json(&serde_json::json!({
            "data": {"deleted": deleted},
        }))
      }

      // ── GET summaries ──────────────────────────────────────
      ("GET", "summaries") => {
        let range = request
          .query_params
          .get("range")
          .cloned()
          .unwrap_or_else(|| "7d".to_string());
        let rows = ctx
                    .query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"coding\", \"total_seconds\") RETURN n ORDER BY n.system.node_time DESC")
                    .await?;
        let mut summaries: Vec<Node> = rows
          .iter()
          .filter_map(panorama_core::query::row_to_node)
          .collect();
        let (start, end) = parse_time_range(&range);
        summaries.retain(|n| node_time_in_range(n, start, end));
        HttpResponse::json(&summaries)
      }

      _ => Err(PluginError::not_found(&format!(
        "Unknown endpoint: {} {}",
        request.method, endpoint
      ))),
    }
  }

  async fn run_background_task(
    &self,
    task_name: &str,
    ctx: &dyn PluginContext,
  ) -> Result<(), PluginError> {
    match task_name {
      "daily-summary-rollup" => self.rollup_daily_summaries(ctx).await,
      _ => Err(PluginError::not_found(&format!(
        "Unknown background task: {}",
        task_name
      ))),
    }
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HTTP Handler Implementations
// ═══════════════════════════════════════════════════════════════════════════════

impl CodingPlugin {
  /// Handle POST of heartbeats (single or bulk).
  /// This is the primary write path — compatible with the upstream Coding Activity
  /// API so existing editor plugins and `coding-activity-cli` can submit data.
  async fn handle_post_heartbeats(
    &self,
    request: HttpRequest,
    ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    let body: serde_json::Value = serde_json::from_slice(request.body.as_deref().unwrap_or(&[]))
      .map_err(|e| PluginError::bad_request(&format!("Invalid JSON: {}", e)))?;

    // Support both single heartbeat object and bulk array
    // The Coding Activity API spec says: bulk endpoint accepts array, single
    // endpoint accepts object. We handle both in either case.
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
      match self.create_heartbeat_node(ctx, hb).await {
        Ok(node) => created.push(node),
        Err(e) => errors.push(serde_json::json!({
            "entity": hb.get("entity"),
            "error": e.message,
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
  async fn create_heartbeat_node(
    &self,
    ctx: &dyn PluginContext,
    hb: &serde_json::Value,
  ) -> Result<Node, PluginError> {
    let mut node = Node::new(Uuid::nil());

    // ── Required fields ────────────────────────────────────────
    let entity = hb["entity"]
      .as_str()
      .ok_or_else(|| PluginError::bad_request("entity is required"))?;
    node.set_field("coding:entity", FieldValue::String(entity.to_string()));

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

    // ── Optional core fields ───────────────────────────────────
    if let Some(v) = hb["category"].as_str() {
      node.set_field("coding:category", FieldValue::String(v.to_string()));
    }
    if let Some(v) = hb["project"].as_str() {
      node.set_field("coding:project", FieldValue::String(v.to_string()));
    }
    if let Some(v) = hb["project_root_count"].as_i64() {
      node.set_field("coding:project_root_count", FieldValue::Integer(v));
    }
    if let Some(v) = hb["branch"].as_str() {
      node.set_field("coding:branch", FieldValue::String(v.to_string()));
    }
    if let Some(v) = hb["language"].as_str() {
      node.set_field("coding:language", FieldValue::String(v.to_string()));
    }
    if let Some(v) = hb["dependencies"].as_str() {
      node.set_field("coding:dependencies", FieldValue::String(v.to_string()));
    }
    if let Some(v) = hb["machine_name_id"].as_str() {
      node.set_field("coding:machine_name_id", FieldValue::String(v.to_string()));
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
    // Duration from the heartbeat payload if provided
    if let Some(v) = hb["duration"].as_f64() {
      node.set_field("coding:duration", FieldValue::Float(v));
    }
    // AI flag
    node.set_field(
      "coding:is_ai_generated",
      FieldValue::Boolean(heartbeat_is_ai(hb)),
    );

    ctx.create_node(node).await
  }

  /// Convert a stored heartbeat node back to Coding Activity API-compatible JSON.
  fn heartbeat_to_api_json(&self, node: &Node) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "id": node.id.to_string(),
        "entity": self.field_value(node, "entity"),
        "type": self.field_value(node, "type"),
        "time": node_time_epoch(node),
    });

    // Add optional fields that are present
    for field in &[
      "category",
      "project",
      "branch",
      "language",
      "dependencies",
      "machine_name_id",
      "ai_session",
      "ai_subscription_plan",
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
  async fn rollup_daily_summaries(&self, ctx: &dyn PluginContext) -> Result<(), PluginError> {
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

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_time_range_relative() {
    let (start, end) = parse_time_range("7d");
    let diff = end - start;
    assert!(diff.num_days() == 7);

    let (start, end) = parse_time_range("24h");
    let diff = end - start;
    assert!(diff.num_hours() == 24);
  }

  #[test]
  fn test_parse_time_range_explicit() {
    let (start, end) = parse_time_range("2026-01-01..2026-01-07");
    let diff = end - start;
    assert!(diff.num_days() == 6); // 23:59:59 end time
    assert_eq!(start.format("%Y-%m-%d").to_string(), "2026-01-01");
  }

  #[test]
  fn test_parse_time_range_single_date() {
    let (start, end) = parse_time_range("2026-06-15");
    assert_eq!(start.format("%Y-%m-%d").to_string(), "2026-06-15");
    assert_eq!(end.format("%Y-%m-%d").to_string(), "2026-06-15");
  }

  #[test]
  fn test_heartbeat_is_ai() {
    assert!(heartbeat_is_ai(
      &serde_json::json!({"ai_session": "sess-123"})
    ));
    assert!(heartbeat_is_ai(&serde_json::json!({"ai_line_changes": 5})));
    assert!(heartbeat_is_ai(
      &serde_json::json!({"ai_input_tokens": 100})
    ));
    assert!(!heartbeat_is_ai(
      &serde_json::json!({"entity": "/src/main.rs"})
    ));
  }
}
