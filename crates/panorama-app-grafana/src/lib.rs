//! Grafana Plugin — Full dashboard system for time-series data visualization,
//! matching the four-panel WakaTime dashboard reference design.
//!
//! ## Architecture
//!
//! ### Dashboard Model
//!
//! Dashboards are stored as nodes with a `grafana:config` field containing the
//! full dashboard JSON. The model mirrors Grafana's dashboard JSON structure:
//!
//! ```json
//! {
//!   "uid": "abc123",
//!   "title": "Coding Overview",
//!   "description": "My coding activity",
//!   "tags": ["coding", "wakatime"],
//!   "time": {"from": "now-7d", "to": "now"},
//!   "refresh": "5m",
//!   "panels": [
//!     {
//!       "id": 1,
//!       "title": "Coding per Project",
//!       "type": "leaderboard",
//!       "gridPos": {"x": 0, "y": 0, "w": 12, "h": 8},
//!       "queries": [{
//!         "dataSource": "io.mzhang.panorama.wakatime",
//!         "groupBy": "project",
//!         "aggregation": "leaderboard",
//!         "limit": 10
//!       }],
//!       "options": {"orientation": "horizontal", "showValues": true}
//!     }
//!   ],
//!   "variables": []
//! }
//! ```
//!
//! ### Panel Types
//!
//! | Type         | Description                        | Screenshot quadrant |
//! |-------------|------------------------------------|---------------------|
//! | leaderboard | Horizontal bar chart, sorted desc  | Top-left, top-right, bottom-right |
//! | timeseries  | Line/area chart over time          | Bottom-left         |
//! | stat        | Single big number with sparkline   | —                   |
//! | piechart    | Pie/donut chart                    | —                   |
//! | table       | Sortable data table                | —                   |
//! | heatmap     | Calendar heatmap (GitHub-style)    | —                   |
//!
//! ### Data Flow
//!
//! 1. Frontend loads dashboard JSON from `GET /api/dashboards/{uid}`
//! 2. For each panel, frontend calls `POST /api/ds/query` with panel queries
//! 3. Grafana plugin delegates to the specified data source plugin
//!    (e.g., wakatime plugin's `/stats` endpoint) via `PluginContext.query()`
//! 4. Results are returned in Grafana-compatible data frame format
//!
//! ### Query Engine
//!
//! The `/api/ds/query` endpoint accepts a batch of queries and resolves each
//! by calling the appropriate data source plugin. It supports:
//! - Time range filtering (applied at query time)
//! - Group-by dimensions
//! - Aggregation functions: sum, count, avg, min, max, leaderboard
//! - Time-series bucketing: hour, day, week, month
//! - Multi-query panels (multiple series on one chart)

pub mod promql;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use panorama_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

// ── Plugin struct ─────────────────────────────────────────────────────────────

pub struct GrafanaPlugin;

impl GrafanaPlugin {
    pub fn new() -> Self {
        Self
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Dashboard JSON Model
// ═══════════════════════════════════════════════════════════════════════════════

/// A complete dashboard definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    /// Unique identifier (short hash, human-friendly)
    #[serde(default)]
    pub uid: String,
    /// Display title
    pub title: String,
    /// Optional description (markdown)
    #[serde(default)]
    pub description: String,
    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Time range configuration
    #[serde(default)]
    pub time: DashboardTime,
    /// Auto-refresh interval (e.g., "5m", "30s", "1h", "" = off)
    #[serde(default)]
    pub refresh: String,
    /// Schema version for forward compatibility
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Panels in this dashboard
    #[serde(default)]
    pub panels: Vec<Panel>,
    /// Template variables
    #[serde(default)]
    pub variables: Vec<DashboardVariable>,
    /// Annotations
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardTime {
    /// Start time: ISO 8601 or relative like "now-7d", "now-24h"
    #[serde(default = "default_from")]
    pub from: String,
    /// End time: ISO 8601 or "now"
    #[serde(default = "default_to")]
    pub to: String,
}

fn default_from() -> String {
    "now-7d".to_string()
}
fn default_to() -> String {
    "now".to_string()
}

impl Default for DashboardTime {
    fn default() -> Self {
        Self {
            from: default_from(),
            to: default_to(),
        }
    }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Panel {
    /// Unique panel ID within the dashboard
    pub id: u32,
    /// Panel title
    pub title: String,
    /// Panel type: leaderboard, timeseries, stat, piechart, table, heatmap
    #[serde(rename = "type")]
    pub panel_type: String,
    /// Grid position: {x, y, w, h} in 24-column grid
    #[serde(default)]
    pub grid_pos: GridPos,
    /// Data queries for this panel
    #[serde(default)]
    pub queries: Vec<PanelQuery>,
    /// Type-specific options
    #[serde(default)]
    pub options: serde_json::Value,
    /// Field overrides
    #[serde(default)]
    pub field_config: serde_json::Value,
    /// Transparency (0.0 = opaque, 1.0 = full)
    #[serde(default)]
    pub transparent: bool,
    /// Description tooltip
    #[serde(default)]
    pub description: String,
    /// Whether this panel is repeatable (one per variable value)
    #[serde(default)]
    pub repeat: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPos {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Default for GridPos {
    fn default() -> Self {
        Self { x: 0, y: 0, w: 12, h: 8 }
    }
}

// ── Panel Query ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelQuery {
    /// Which data source plugin to query (e.g., "io.mzhang.panorama.wakatime")
    #[serde(default = "default_datasource")]
    pub data_source: String,
    /// Query reference ID (A, B, C...) for multi-query panels
    #[serde(default)]
    pub ref_id: String,
    /// Dimension to group by
    #[serde(default)]
    pub group_by: String,
    /// Aggregation function
    #[serde(default)]
    pub aggregation: String,
    /// Optional key=value filter
    #[serde(default)]
    pub filter: String,
    /// Time range override (uses dashboard time if not set)
    #[serde(default)]
    pub time_range: Option<DashboardTime>,
    /// Time bucket for timeseries: "auto", "hour", "day", "week", "month"
    #[serde(default)]
    pub bucket: String,
    /// Result limit
    #[serde(default = "default_query_limit")]
    pub limit: usize,
    /// Hide this query's results from the visualization
    #[serde(default)]
    pub hide: bool,
    /// Optional PromQL expression (mutually exclusive with group_by/aggregation).
    /// When set, the query engine parses PromQL and translates to PQL.
    #[serde(default)]
    pub promql: Option<String>,
}

fn default_datasource() -> String {
    "io.mzhang.panorama.wakatime".to_string()
}
fn default_query_limit() -> usize {
    25
}

// ── Dashboard Variable ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardVariable {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub var_type: String, // "query", "interval", "custom", "constant"
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub current: serde_json::Value,
    #[serde(default)]
    pub options: Vec<VariableOption>,
    #[serde(default)]
    pub multi: bool,
    #[serde(default)]
    pub include_all: bool,
    #[serde(default)]
    pub hide: u8, // 0 = show, 1 = hide label, 2 = hide all
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableOption {
    pub text: String,
    pub value: String,
}

// ── Annotation ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub name: String,
    #[serde(default)]
    pub data_source: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub icon_color: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query Request / Response (Grafana-compatible data frames)
// ═══════════════════════════════════════════════════════════════════════════════

/// A batch of queries to execute.
#[derive(Debug, Deserialize)]
pub struct DataQueryRequest {
    /// Queries to execute
    pub queries: Vec<PanelQuery>,
    /// Time range for this batch
    #[serde(default)]
    pub range: DashboardTime,
    /// Start time as ISO 8601
    #[serde(default)]
    pub from: String,
    /// End time as ISO 8601
    #[serde(default)]
    pub to: String,
}

/// A data frame in Grafana-compatible format.
#[derive(Debug, Serialize)]
pub struct DataFrame {
    /// Frame name (usually the ref_id)
    pub name: String,
    /// Column names
    pub columns: Vec<String>,
    /// Row data (each row is an array of values)
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Time Range Engine
// ═══════════════════════════════════════════════════════════════════════════════

/// Pre-defined time range presets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangePreset {
    pub label: String,
    pub from: String,
    pub to: String,
}

impl GrafanaPlugin {
    /// All available time range presets.
    fn time_range_presets() -> Vec<TimeRangePreset> {
        vec![
            TimeRangePreset { label: "Last 1 hour".into(), from: "now-1h".into(), to: "now".into() },
            TimeRangePreset { label: "Last 3 hours".into(), from: "now-3h".into(), to: "now".into() },
            TimeRangePreset { label: "Last 6 hours".into(), from: "now-6h".into(), to: "now".into() },
            TimeRangePreset { label: "Last 12 hours".into(), from: "now-12h".into(), to: "now".into() },
            TimeRangePreset { label: "Last 24 hours".into(), from: "now-24h".into(), to: "now".into() },
            TimeRangePreset { label: "Last 2 days".into(), from: "now-2d".into(), to: "now".into() },
            TimeRangePreset { label: "Last 7 days".into(), from: "now-7d".into(), to: "now".into() },
            TimeRangePreset { label: "Last 30 days".into(), from: "now-30d".into(), to: "now".into() },
            TimeRangePreset { label: "Last 90 days".into(), from: "now-90d".into(), to: "now".into() },
            TimeRangePreset { label: "Last 1 year".into(), from: "now-365d".into(), to: "now".into() },
            TimeRangePreset { label: "Year to date".into(), from: "now/y".into(), to: "now".into() },
            TimeRangePreset { label: "Today".into(), from: "now/d".into(), to: "now".into() },
            TimeRangePreset { label: "Yesterday".into(), from: "now-1d/d".into(), to: "now-1d/d".into() },
            TimeRangePreset { label: "This week".into(), from: "now/w".into(), to: "now".into() },
            TimeRangePreset { label: "This month".into(), from: "now/M".into(), to: "now".into() },
        ]
    }

    /// Resolve a relative time string to an absolute DateTime.
    /// Supports: "now", "now-Nh", "now-Nd", "now-NM", "now/y", "now/M", "now/w", "now/d"
    fn resolve_time(expr: &str) -> DateTime<Utc> {
        let now = Utc::now();
        if expr == "now" {
            return now;
        }

        // Handle "now-Nh", "now-Nd", "now-NM"
        if let Some(rest) = expr.strip_prefix("now-") {
            return parse_relative(rest, now);
        }

        // Handle "now/d" (start of today)
        if expr == "now/d" {
            return now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        }
        // Handle "now/w" (start of this week — Monday)
        if expr == "now/w" {
            let weekday = now.date_naive().weekday().num_days_from_monday();
            return (now.date_naive() - chrono::Duration::days(weekday as i64))
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
        }
        // Handle "now/M" (start of this month)
        if expr == "now/M" {
            return now
                .date_naive()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
        }
        // Handle "now/y" (start of this year)
        if expr == "now/y" {
            return NaiveDate::from_ymd_opt(now.year(), 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
        }
        // Handle "now-Nd/d" (start of day N days ago)
        if let Some(rest) = expr.strip_prefix("now-") {
            if let Some(days_str) = rest.strip_suffix("/d") {
                if let Ok(days) = days_str.parse::<i64>() {
                    return (now.date_naive() - chrono::Duration::days(days))
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc();
                }
            }
        }
        // Fallback: assume ISO 8601
        DateTime::parse_from_rfc3339(expr)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or(now)
    }
}

/// Parse a relative time suffix like "7d", "24h", "90d", "12M".
fn parse_relative(s: &str, now: DateTime<Utc>) -> DateTime<Utc> {
    if let Some(days_str) = s.strip_suffix('d') {
        if let Ok(days) = days_str.parse::<i64>() {
            return now - Duration::days(days);
        }
    }
    if let Some(hours_str) = s.strip_suffix('h') {
        if let Ok(hours) = hours_str.parse::<i64>() {
            return now - Duration::hours(hours);
        }
    }
    if let Some(mins_str) = s.strip_suffix('m') {
        if let Ok(mins) = mins_str.parse::<i64>() {
            return now - Duration::minutes(mins);
        }
    }
    if let Some(secs_str) = s.strip_suffix('s') {
        if let Ok(secs) = secs_str.parse::<i64>() {
            return now - Duration::seconds(secs);
        }
    }
    if let Some(months_str) = s.strip_suffix('M') {
        if let Ok(months) = months_str.parse::<i32>() {
            // Approximate months as 30 days
            return now - Duration::days(months as i64 * 30);
        }
    }
    now
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query Engine
// ═══════════════════════════════════════════════════════════════════════════════

impl GrafanaPlugin {
    /// Execute a batch of panel queries and return data frames.
    /// This is the main query engine entry point.
    async fn execute_data_queries(
        &self,
        ctx: &dyn PluginContext,
        request: &DataQueryRequest,
    ) -> Result<Vec<DataFrame>, PluginError> {
        let mut frames = Vec::new();

        for query in &request.queries {
            // Resolve time range: use query override, then request range, then dashboard default
            let range = query.time_range.as_ref().unwrap_or(&request.range);
            let from = GrafanaPlugin::resolve_time(&range.from);
            let to = GrafanaPlugin::resolve_time(&range.to);

            let frame = self.execute_single_query(ctx, query, from, to).await?;
            frames.push(frame);
        }

        Ok(frames)
    }

    /// Execute a single panel query and produce a data frame.
    async fn execute_single_query(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<DataFrame, PluginError> {
        let ref_id = if query.ref_id.is_empty() { "A".to_string() } else { query.ref_id.clone() };

        match query.aggregation.as_str() {
            "leaderboard" => self.query_leaderboard(ctx, query, from, to, &ref_id).await,
            "timeseries" => self.query_timeseries(ctx, query, from, to, &ref_id).await,
            "sum" => self.query_grouped_sum(ctx, query, from, to, &ref_id).await,
            "count" => self.query_grouped_count(ctx, query, from, to, &ref_id).await,
            "stat" => self.query_stat(ctx, query, from, to, &ref_id).await,
            "piechart" => self.query_piechart(ctx, query, from, to, &ref_id).await,
            "table" => self.query_table(ctx, query, from, to, &ref_id).await,
            "heatmap" => self.query_heatmap(ctx, query, from, to, &ref_id).await,
            "promql" => self.execute_promql_query(ctx, query, from, to, &ref_id).await,
            _ => {
                // Fallback: delegate to the data source plugin's stats endpoint
                self.query_datasource_delegate(ctx, query, from, to, &ref_id).await
            }
        }
    }

    /// Delegate a query to the data source plugin's `/stats` endpoint.
    async fn query_datasource_delegate(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        // Build a stats query for the data source plugin
        let _range_str = format!(
            "{}..{}",
            from.format("%Y-%m-%dT%H:%M:%SZ"),
            to.format("%Y-%m-%dT%H:%M:%SZ")
        );

        let stats_query = format!(
            "MATCH (n) IN space(\"default\") \
             WHERE HAS_FIELD(n, \"wakatime\", \"entity\") \
             AND n.system.node_time >= \"{}\" \
             AND n.system.node_time <= \"{}\" \
             RETURN n ORDER BY n.system.node_time ASC",
            from.to_rfc3339(),
            to.to_rfc3339()
        );

        let rows = ctx.query(&stats_query).await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        let group_by = if query.group_by.is_empty() { "project" } else { &query.group_by };
        let group_key = |n: &Node| -> String {
            let field = if group_by.contains(':') {
                group_by.to_string()
            } else {
                format!("wakatime:{}", group_by)
            };
            match n.get_field(&field) {
                Some(FieldValue::String(s)) => s.clone(),
                Some(FieldValue::Integer(i)) => i.to_string(),
                _ => "(unknown)".to_string(),
            }
        };

        // Aggregate
        let mut groups: HashMap<String, f64> = HashMap::new();
        for node in &nodes {
            let key = group_key(node);
            let duration = node_duration_seconds(node);
            *groups.entry(key).or_default() += duration;
        }

        let mut entries: Vec<(String, f64)> = groups.into_iter().collect();
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let limit = query.limit;
        if entries.len() > limit {
            entries.truncate(limit);
        }

        Ok(DataFrame {
            name: ref_id.to_string(),
            columns: vec!["key".into(), "seconds".into(), "hours".into()],
            rows: entries
                .into_iter()
                .map(|(key, secs)| {
                    vec![
                        serde_json::Value::String(key),
                        serde_json::json!(secs),
                        serde_json::json!((secs / 3600.0 * 10.0).round() / 10.0),
                    ]
                })
                .collect(),
            meta: Some(serde_json::json!({
                "aggregation": query.aggregation,
                "group_by": group_by,
                "from": from.to_rfc3339(),
                "to": to.to_rfc3339(),
            })),
        })
    }

    /// Leaderboard query: group + sum, sorted descending.
    async fn query_leaderboard(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let frame = self.query_datasource_delegate(ctx, query, from, to, ref_id).await?;
        Ok(frame)
    }

    /// Time-series query: bucketed data points over time.
    async fn query_timeseries(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let _range_str = format!(
            "{}..{}",
            from.format("%Y-%m-%dT%H:%M:%SZ"),
            to.format("%Y-%m-%dT%H:%M:%SZ")
        );

        let stats_query = format!(
            "MATCH (n) IN space(\"default\") \
             WHERE HAS_FIELD(n, \"wakatime\", \"entity\") \
             RETURN n ORDER BY n.system.node_time ASC"
        );

        let rows = ctx.query(&stats_query).await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        // Filter by time range
        let filtered: Vec<&Node> = nodes
            .iter()
            .filter(|n| {
                let ts = node_time_epoch(n) as i64;
                ts >= from.timestamp() && ts <= to.timestamp()
            })
            .collect();

        // Determine bucket size
        let bucket_secs: i64 = match query.bucket.as_str() {
            "hour" => 3600,
            "week" => 604800,
            "month" => 2592000,
            _ => {
                // "auto": pick based on range
                let range_secs = (to - from).num_seconds();
                if range_secs <= 86400 { 3600 } // <=1 day: hourly
                else if range_secs <= 604800 { 86400 } // <=1 week: daily
                else if range_secs <= 2592000 { 86400 } // <=30 days: daily
                else { 604800 } // >30 days: weekly
            }
        };

        let group_by = if query.group_by.is_empty() { "project" } else { &query.group_by };
        let group_key = |n: &Node| -> String {
            let field = if group_by.contains(':') {
                group_by.to_string()
            } else {
                format!("wakatime:{}", group_by)
            };
            match n.get_field(&field) {
                Some(FieldValue::String(s)) => s.clone(),
                _ => "(unknown)".to_string(),
            }
        };

        // Group by (bucket_ts, key)
        let mut buckets: HashMap<(i64, String), f64> = HashMap::new();
        for node in &filtered {
            let ts = node_time_epoch(node) as i64;
            let bucket_ts = ts - (ts % bucket_secs);
            let key = group_key(node);
            let duration = node_duration_seconds(node);
            *buckets.entry((bucket_ts, key)).or_default() += duration;
        }

        // Build series
        let mut series: BTreeMap<String, Vec<(i64, f64)>> = BTreeMap::new();
        for ((ts, key), secs) in &buckets {
            series.entry(key.clone()).or_default().push((*ts, *secs));
        }

        // Produce one frame per series (Grafana style)
        // For simplicity, we return all series in one wide frame
        let _col_key = group_by.replace(':', "_");
        let mut all_timestamps: Vec<i64> = Vec::new();
        let mut t = from.timestamp();
        let aligned_start = t - (t % bucket_secs);
        t = aligned_start;
        while t <= to.timestamp() {
            all_timestamps.push(t);
            t += bucket_secs;
        }

        let series_keys: Vec<String> = series.keys().cloned().collect();
        let mut columns = vec!["time".to_string(), "iso".to_string()];
        for key in &series_keys {
            columns.push(key.clone());
        }

        let mut rows_data: Vec<Vec<serde_json::Value>> = Vec::new();
        let series_maps: Vec<HashMap<i64, f64>> = series_keys
            .iter()
            .map(|k| {
                series
                    .get(k)
                    .map(|pts| pts.iter().cloned().collect())
                    .unwrap_or_default()
            })
            .collect();

        for ts in &all_timestamps {
            let mut row = vec![
                serde_json::json!(ts),
                serde_json::Value::String(
                    DateTime::from_timestamp(*ts, 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_default(),
                ),
            ];
            for smap in &series_maps {
                row.push(serde_json::json!(smap.get(ts).copied().unwrap_or(0.0)));
            }
            rows_data.push(row);
        }

        Ok(DataFrame {
            name: ref_id.to_string(),
            columns,
            rows: rows_data,
            meta: Some(serde_json::json!({
                "aggregation": "timeseries",
                "bucket_seconds": bucket_secs,
                "from": from.to_rfc3339(),
                "to": to.to_rfc3339(),
            })),
        })
    }

    /// Stat query: single aggregated value.
    async fn query_stat(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let frame = self.query_datasource_delegate(ctx, query, from, to, ref_id).await?;
        // Flatten to single value
        let total: f64 = frame.rows.iter().filter_map(|r| r.get(2)?.as_f64()).sum();
        Ok(DataFrame {
            name: ref_id.to_string(),
            columns: vec!["value".into(), "label".into()],
            rows: vec![vec![
                serde_json::json!((total * 10.0).round() / 10.0),
                serde_json::Value::String(format!("{:.1}h", total)),
            ]],
            meta: Some(serde_json::json!({
                "aggregation": "stat",
                "display_type": "hours",
            })),
        })
    }

    /// Pie chart query: group percentages.
    async fn query_piechart(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let frame = self.query_datasource_delegate(ctx, query, from, to, ref_id).await?;
        let total: f64 = frame.rows.iter().filter_map(|r| r.get(1)?.as_f64()).sum();
        let rows_with_pct: Vec<Vec<serde_json::Value>> = frame
            .rows
            .iter()
            .map(|r| {
                let val = r.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let pct = if total > 0.0 { val / total * 100.0 } else { 0.0 };
                vec![
                    r[0].clone(),
                    r[1].clone(),
                    serde_json::json!((pct * 10.0).round() / 10.0),
                ]
            })
            .collect();
        Ok(DataFrame {
            name: ref_id.to_string(),
            columns: vec!["key".into(), "seconds".into(), "percent".into()],
            rows: rows_with_pct,
            meta: Some(serde_json::json!({"aggregation": "piechart"})),
        })
    }

    /// Table query: raw data rows.
    async fn query_table(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        self.query_datasource_delegate(ctx, query, from, to, ref_id).await
    }

    /// Heatmap query: calendar heatmap data (GitHub contribution graph style).
    async fn query_heatmap(
        &self,
        ctx: &dyn PluginContext,
        _query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let stats_query = format!(
            "MATCH (n) IN space(\"default\") \
             WHERE HAS_FIELD(n, \"wakatime\", \"entity\") \
             RETURN n ORDER BY n.system.node_time ASC"
        );
        let rows = ctx.query(&stats_query).await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        // Aggregate by day
        let mut daily: HashMap<String, f64> = HashMap::new();
        for n in &nodes {
            let ts = node_time_epoch(n) as i64;
            if ts < from.timestamp() || ts > to.timestamp() {
                continue;
            }
            let day = DateTime::from_timestamp(ts, 0)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            *daily.entry(day).or_default() += node_duration_seconds(n);
        }

        let mut days: Vec<(String, f64)> = daily.into_iter().collect();
        days.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(DataFrame {
            name: ref_id.to_string(),
            columns: vec!["date".into(), "seconds".into(), "hours".into()],
            rows: days
                .into_iter()
                .map(|(date, secs)| {
                    vec![
                        serde_json::Value::String(date),
                        serde_json::json!(secs),
                        serde_json::json!((secs / 3600.0 * 10.0).round() / 10.0),
                    ]
                })
                .collect(),
            meta: Some(serde_json::json!({"aggregation": "heatmap"})),
        })
    }

    /// Execute a PromQL query: parse, translate to PQL, execute, apply post-steps.
    async fn execute_promql_query(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let promql_str = query.promql.as_deref().unwrap_or_default();
        if promql_str.is_empty() {
            return Err(PluginError::bad_request("PromQL expression is empty"));
        }

        // 1. Parse PromQL
        let expr = promql::parser::parse(promql_str)?;

        // 2. Load metric registry (default for now, extensible later)
        let registry = promql::registry::MetricRegistry::with_wakatime_defaults();

        // 3. Translate to PQL + post-steps
        let tq = promql::translator::translate(
            &expr,
            &registry,
            from.timestamp(),
            to.timestamp(),
        )
        .map_err(|e| PluginError::bad_request(&format!("PromQL translation error: {}", e)))?;

        // 4. Execute PQL query
        let rows = ctx.query(&tq.pql).await?;
        let nodes: Vec<Node> = rows
            .iter()
            .filter_map(panorama_core::query::row_to_node)
            .collect();

        // 5. Apply post-processing steps and produce DataFrame
        let mut df = promql::translator::execute_translated(&tq, &nodes)
            .map_err(|e| PluginError::internal(format!("PromQL execution error: {}", e)))?;

        // Override the name with the ref_id
        df.name = ref_id.to_string();

        // Add metadata
        df.meta = Some(serde_json::json!({
            "aggregation": "promql",
            "expression": promql_str,
            "from": from.to_rfc3339(),
            "to": to.to_rfc3339(),
        }));

        Ok(df)
    }

    /// Group nodes by dimension and sum values.
    async fn query_grouped_sum(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        self.query_datasource_delegate(ctx, query, from, to, ref_id).await
    }

    /// Group nodes by dimension and count occurrences.
    async fn query_grouped_count(
        &self,
        ctx: &dyn PluginContext,
        query: &PanelQuery,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        ref_id: &str,
    ) -> Result<DataFrame, PluginError> {
        let mut frame = self.query_datasource_delegate(ctx, query, from, to, ref_id).await?;
        // Replace "seconds"/"hours" with "count"
        frame.columns = vec!["key".into(), "count".into()];
        for row in &mut frame.rows {
            if row.len() >= 2 {
                // The leaderboard delegate returns (key, seconds, hours)
                // We count occurrences instead
                row.truncate(2);
            }
        }
        Ok(frame)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn node_time_epoch(node: &Node) -> f64 {
    if let Some(FieldValue::Float(ts)) = node.get_field("wakatime:time") {
        return *ts;
    }
    if let Some(FieldValue::Integer(ts)) = node.get_field("wakatime:time") {
        return *ts as f64;
    }
    if let Some(FieldValue::DateTime(s)) = node.get_field("system:node_time") {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return dt.timestamp() as f64;
        }
    }
    0.0
}

fn node_duration_seconds(node: &Node) -> f64 {
    if let Some(FieldValue::Float(d)) = node.get_field("wakatime:duration") {
        if *d > 0.0 {
            return *d;
        }
    }
    if let Some(FieldValue::Integer(d)) = node.get_field("wakatime:duration") {
        if *d > 0 {
            return *d as f64;
        }
    }
    120.0
}

// ═══════════════════════════════════════════════════════════════════════════════
// Schemas
// ═══════════════════════════════════════════════════════════════════════════════

impl GrafanaPlugin {
    fn dashboard_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "grafana/Dashboard".to_string(),
            version: SchemaVersion::new(2, 0),
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
                    description: Some("Full dashboard JSON configuration (panels, variables, time, etc.)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "uid".to_string(),
                    namespace: "grafana".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Unique dashboard identifier (short hash)".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "tags".to_string(),
                    namespace: "grafana".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Comma-separated tags for organization".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "folder_uid".to_string(),
                    namespace: "grafana".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Folder this dashboard belongs to".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "version".to_string(),
                    namespace: "grafana".to_string(),
                    field_type: None,
                    required: false,
                    default: Some(FieldValue::Integer(1)),
                    description: Some("Dashboard revision version number".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    fn folder_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "grafana/Folder".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Folder name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "uid".to_string(),
                    namespace: "grafana".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Unique folder identifier".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin Trait
// ═══════════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Plugin for GrafanaPlugin {
    fn id(&self) -> &str {
        "io.mzhang.panorama.grafana"
    }

    fn name(&self) -> &str {
        "Dashboards"
    }

    fn version(&self) -> &str {
        "0.2.0"
    }

    fn description(&self) -> &str {
        "Full dashboard system: leaderboards, time-series, stats, pie charts, tables, and heatmaps with variables and annotations"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::dashboard_schema(), Self::folder_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            // ── Dashboard CRUD ────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/dashboards".to_string(),
                description: "List all dashboards".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/dashboards".to_string(),
                description: "Create a new dashboard".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/dashboards/{uid}".to_string(),
                description: "Get a dashboard by UID".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::PUT,
                path: "/api/dashboards/{uid}".to_string(),
                description: "Update a dashboard".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::DELETE,
                path: "/api/dashboards/{uid}".to_string(),
                description: "Delete a dashboard".to_string(),
            },
            // ── Query engine ──────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/ds/query".to_string(),
                description: "Execute a batch of panel queries (Grafana-compatible)".to_string(),
            },
            // ── Time range presets ────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/query/options".to_string(),
                description: "Get time range presets and aggregation options".to_string(),
            },
            // ── Dashboard operations ──────────────────────────────
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/dashboards/import".to_string(),
                description: "Import a dashboard from JSON".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/dashboards/export/{uid}".to_string(),
                description: "Export a dashboard as JSON".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/dashboards/{uid}/duplicate".to_string(),
                description: "Duplicate a dashboard".to_string(),
            },
            // ── Folders ───────────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/folders".to_string(),
                description: "List dashboard folders".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/folders".to_string(),
                description: "Create a dashboard folder".to_string(),
            },
            // ── Home dashboard ────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/dashboards/home".to_string(),
                description: "Get the home dashboard (auto-created if missing)".to_string(),
            },
            // ── PromQL ─────────────────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/promql/validate".to_string(),
                description: "Validate a PromQL expression".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/promql/metrics".to_string(),
                description: "List registered PromQL metrics".to_string(),
            },
            // ── Backward compat ───────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/query".to_string(),
                description: "Execute a dashboard query (legacy)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/dashboards".to_string(),
                description: "Save a dashboard (legacy)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/dashboards".to_string(),
                description: "List saved dashboards (legacy)".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![
            UiComponent {
                id: "grafana-main".to_string(),
                name: "Dashboard View".to_string(),
                mount_point: UiMountPoint::MainPage,
                bundle_path: "ui/grafana.js".to_string(),
            },
            UiComponent {
                id: "grafana-dashboard".to_string(),
                name: "Dashboard Widget".to_string(),
                mount_point: UiMountPoint::Dashboard,
                bundle_path: "ui/grafana.js".to_string(),
            },
        ]
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
            // ── Dashboard CRUD ────────────────────────────────────
            ("GET", "api/dashboards") => self.handle_list_dashboards(ctx).await,
            ("POST", "api/dashboards") => self.handle_create_dashboard(ctx, request).await,
            ("GET", "api/dashboards/home") => self.handle_get_home_dashboard(ctx).await,
            ("GET", _) if endpoint.starts_with("api/dashboards/export/") => {
                let uid = &endpoint["api/dashboards/export/".len()..];
                self.handle_export_dashboard(ctx, uid).await
            }
            ("POST", "api/dashboards/import") => self.handle_import_dashboard(ctx, request).await,
            ("POST", _) if endpoint.ends_with("/duplicate") => {
                let path = endpoint.strip_suffix("/duplicate").unwrap_or(endpoint);
                let uid = path.strip_prefix("api/dashboards/").unwrap_or("");
                self.handle_duplicate_dashboard(ctx, uid).await
            }
            ("GET", _) if endpoint.starts_with("api/dashboards/") => {
                let uid = &endpoint["api/dashboards/".len()..];
                self.handle_get_dashboard(ctx, uid).await
            }
            ("PUT", _) if endpoint.starts_with("api/dashboards/") => {
                let uid = &endpoint["api/dashboards/".len()..];
                self.handle_update_dashboard(ctx, uid, request).await
            }
            ("DELETE", _) if endpoint.starts_with("api/dashboards/") => {
                let uid = &endpoint["api/dashboards/".len()..];
                self.handle_delete_dashboard(ctx, uid).await
            }

            // ── Query engine ──────────────────────────────────────
            ("POST", "api/ds/query") => {
                let req: DataQueryRequest = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&format!("Invalid query request: {}", e)))?;
                let frames = self.execute_data_queries(ctx, &req).await?;
                HttpResponse::json(&frames)
            }

            // ── Query options ─────────────────────────────────────
            ("GET", "api/query/options") => {
                let presets = Self::time_range_presets();
                let aggregations = vec![
                    serde_json::json!({"value": "leaderboard", "label": "Leaderboard (sum + sort desc)"}),
                    serde_json::json!({"value": "timeseries", "label": "Time Series"}),
                    serde_json::json!({"value": "sum", "label": "Sum"}),
                    serde_json::json!({"value": "count", "label": "Count"}),
                    serde_json::json!({"value": "stat", "label": "Single Stat"}),
                    serde_json::json!({"value": "piechart", "label": "Pie Chart"}),
                    serde_json::json!({"value": "table", "label": "Table"}),
                    serde_json::json!({"value": "heatmap", "label": "Heatmap"}),
                    serde_json::json!({"value": "promql", "label": "PromQL (Prometheus Query Language)"}),
                ];
                let group_bys = vec![
                    serde_json::json!({"value": "project", "label": "Project"}),
                    serde_json::json!({"value": "language", "label": "Language"}),
                    serde_json::json!({"value": "entity", "label": "File / Entity"}),
                    serde_json::json!({"value": "category", "label": "Category"}),
                    serde_json::json!({"value": "machine_name_id", "label": "Machine"}),
                    serde_json::json!({"value": "branch", "label": "Branch"}),
                ];
                let buckets = vec![
                    serde_json::json!({"value": "auto", "label": "Auto"}),
                    serde_json::json!({"value": "hour", "label": "Hour"}),
                    serde_json::json!({"value": "day", "label": "Day"}),
                    serde_json::json!({"value": "week", "label": "Week"}),
                    serde_json::json!({"value": "month", "label": "Month"}),
                ];
                HttpResponse::json(&serde_json::json!({
                    "time_ranges": presets,
                    "aggregations": aggregations,
                    "group_by_options": group_bys,
                    "buckets": buckets,
                }))
            }

            // ── PromQL ────────────────────────────────────────────
            ("POST", "api/promql/validate") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&format!("Invalid JSON: {}", e)))?;
                let expr_str = body["expression"].as_str().unwrap_or("");
                match promql::parser::parse(expr_str) {
                    Ok(expr) => {
                        let registry = promql::registry::MetricRegistry::with_wakatime_defaults();
                        let from_ts = Utc::now().timestamp();
                        let to_ts = from_ts + 3600;
                        match promql::translator::translate(&expr, &registry, from_ts, to_ts) {
                            Ok(tq) => HttpResponse::json(&serde_json::json!({
                                "valid": true,
                                "pql": tq.pql,
                                "value_field": tq.value_field,
                                "time_field": tq.time_field,
                                "label_fields": tq.label_fields,
                                "post_steps_count": tq.post_steps.len(),
                            })),
                            Err(e) => HttpResponse::json(&serde_json::json!({
                                "valid": false,
                                "error": format!("Translation error: {}", e),
                            })),
                        }
                    }
                    Err(e) => HttpResponse::json(&serde_json::json!({
                        "valid": false,
                        "error": format!("Parse error: {}", e),
                    })),
                }
            }
            ("GET", "api/promql/metrics") => {
                let registry = promql::registry::MetricRegistry::with_wakatime_defaults();
                let metrics: Vec<serde_json::Value> = registry.mappings.values().map(|m| {
                    serde_json::json!({
                        "metric_name": m.metric_name,
                        "namespace": m.namespace,
                        "value_field": m.value_field,
                        "time_field": m.time_field,
                        "required_fields": m.required_fields,
                        "description": m.description,
                    })
                }).collect();
                HttpResponse::json(&metrics)
            }

            // ── Folders ───────────────────────────────────────────
            ("GET", "api/folders") => self.handle_list_folders(ctx).await,
            ("POST", "api/folders") => self.handle_create_folder(ctx, request).await,

            // ── Legacy backward compat ────────────────────────────
            ("POST", "query") => {
                // Auto-detect format: new {queries:[...]} vs old {group_by, aggregation}
                let body_val: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&format!("Invalid JSON: {}", e)))?;

                let req = if body_val.get("queries").is_some() {
                    serde_json::from_value::<DataQueryRequest>(body_val)
                        .map_err(|e| PluginError::bad_request(&format!("Invalid query request: {}", e)))?
                } else {
                    // Wrap old format
                    let group_by = body_val["group_by"].as_str().unwrap_or("project").to_string();
                    let aggregation = body_val["aggregation"].as_str().unwrap_or("leaderboard").to_string();
                    let filter = body_val["filter"].as_str().map(String::from).unwrap_or_default();
                    let limit = body_val["limit"].as_u64().unwrap_or(25) as usize;
                    DataQueryRequest {
                        queries: vec![PanelQuery {
                            data_source: "io.mzhang.panorama.wakatime".to_string(),
                            ref_id: "A".to_string(),
                            group_by,
                            aggregation,
                            filter,
                            time_range: None,
                            bucket: String::new(),
                            limit,
                            hide: false,
                            promql: None,
                        }],
                        range: DashboardTime::default(),
                        from: String::new(),
                        to: String::new(),
                    }
                };
                let frames = self.execute_data_queries(ctx, &req).await?;
                HttpResponse::json(&frames)
            }
            ("POST", "dashboards") => {
                // Legacy: wrap bare panels into a full dashboard if needed
                self.handle_create_dashboard(ctx, request).await
            }
            ("GET", "dashboards") => self.handle_list_dashboards(ctx).await,

            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {} {}",
                request.method, endpoint
            ))),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Dashboard CRUD Handlers
// ═══════════════════════════════════════════════════════════════════════════════

impl GrafanaPlugin {
    /// Generate a short UID for dashboards (8 chars, alphanumeric).
    fn generate_uid() -> String {
        let uuid = Uuid::new_v4();
        let hex = uuid.as_simple().to_string();
        hex[..8].to_string()
    }

    /// Find a dashboard node by its UID.
    async fn find_dashboard_by_uid(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
    ) -> Result<Option<(Node, Dashboard)>, PluginError> {
        let rows = ctx
            .query(
                "MATCH (n) IN space(\"default\") \
                 WHERE HAS_FIELD(n, \"grafana\", \"uid\") \
                 RETURN n",
            )
            .await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        for node in &nodes {
            if let Some(FieldValue::String(node_uid)) = node.get_field("grafana:uid") {
                if node_uid == uid {
                    if let Some(FieldValue::Json(config)) = node.get_field("grafana:config") {
                        if let Ok(dashboard) = serde_json::from_value::<Dashboard>(config.clone()) {
                            return Ok(Some((node.clone(), dashboard)));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Find a dashboard node by its node ID.
    async fn find_dashboard_node(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
    ) -> Result<Option<Node>, PluginError> {
        let rows = ctx
            .query(
                "MATCH (n) IN space(\"default\") \
                 WHERE HAS_FIELD(n, \"grafana\", \"uid\") \
                 RETURN n",
            )
            .await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();
        Ok(nodes.into_iter().find(|n| {
            matches!(n.get_field("grafana:uid"), Some(FieldValue::String(u)) if u == uid)
        }))
    }

    /// List all dashboards.
    async fn handle_list_dashboards(
        &self,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        let rows = ctx
            .query(
                "MATCH (n) IN space(\"default\") \
                 WHERE HAS_FIELD(n, \"grafana\", \"uid\") \
                 RETURN n ORDER BY n.system.updated_at DESC",
            )
            .await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        let dashboards: Vec<serde_json::Value> = nodes
            .iter()
            .filter_map(|n| {
                let uid = match n.get_field("grafana:uid") {
                    Some(FieldValue::String(s)) => s.clone(),
                    _ => return None,
                };
                let title = n.title().unwrap_or("Untitled");
                let tags = match n.get_field("grafana:tags") {
                    Some(FieldValue::String(s)) => {
                        s.split(',').map(|t| t.trim().to_string()).collect::<Vec<_>>()
                    }
                    _ => vec![],
                };
                Some(serde_json::json!({
                    "uid": uid,
                    "title": title,
                    "tags": tags,
                    "id": n.id.to_string(),
                    "updated_at": n.updated_at.to_rfc3339(),
                }))
            })
            .collect();

        HttpResponse::json(&dashboards)
    }

    /// Create a new dashboard.
    async fn handle_create_dashboard(
        &self,
        ctx: &dyn PluginContext,
        request: HttpRequest,
    ) -> Result<HttpResponse, PluginError> {
        let mut dashboard: Dashboard = serde_json::from_slice(
            request.body.as_deref().unwrap_or(&[]),
        )
        .map_err(|e| PluginError::bad_request(&format!("Invalid dashboard JSON: {}", e)))?;

        if dashboard.title.is_empty() {
            return Err(PluginError::bad_request("Dashboard title is required"));
        }

        // Generate UID if not provided
        if dashboard.uid.is_empty() {
            dashboard.uid = Self::generate_uid();
        }

        // Ensure schema version is set
        if dashboard.schema_version == 0 {
            dashboard.schema_version = 1;
        }

        let mut node = Node::new(Uuid::nil());
        node.set_field(
            "system:node_title",
            FieldValue::String(dashboard.title.clone()),
        );
        node.set_field("grafana:uid", FieldValue::String(dashboard.uid.clone()));
        node.set_field(
            "grafana:config",
            FieldValue::Json(serde_json::to_value(&dashboard).map_err(|e| {
                PluginError::internal(format!("Failed to serialize dashboard: {}", e))
            })?),
        );
        node.set_field("grafana:version", FieldValue::Integer(1));
        if !dashboard.tags.is_empty() {
            node.set_field(
                "grafana:tags",
                FieldValue::String(dashboard.tags.join(", ")),
            );
        }

        let created = ctx.create_node(node).await?;
        ctx.log(
            LogLevel::Info,
            &format!("Created dashboard: {} ({})", dashboard.title, dashboard.uid),
        )
        .await;

        HttpResponse::json(&serde_json::json!({
            "uid": dashboard.uid,
            "id": created.id.to_string(),
            "title": dashboard.title,
            "url": format!("/d/{}", dashboard.uid),
        }))
    }

    /// Get a dashboard by UID.
    async fn handle_get_dashboard(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
    ) -> Result<HttpResponse, PluginError> {
        match self.find_dashboard_by_uid(ctx, uid).await? {
            Some((_node, dashboard)) => HttpResponse::json(&dashboard),
            None => Err(PluginError::not_found(&format!(
                "Dashboard '{}' not found",
                uid
            ))),
        }
    }

    /// Update a dashboard.
    async fn handle_update_dashboard(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
        request: HttpRequest,
    ) -> Result<HttpResponse, PluginError> {
        let existing = self
            .find_dashboard_node(ctx, uid)
            .await?
            .ok_or_else(|| PluginError::not_found(&format!("Dashboard '{}' not found", uid)))?;

        let dashboard: Dashboard = serde_json::from_slice(
            request.body.as_deref().unwrap_or(&[]),
        )
        .map_err(|e| PluginError::bad_request(&format!("Invalid dashboard JSON: {}", e)))?;

        // Bump version
        let current_version = match existing.get_field("grafana:version") {
            Some(FieldValue::Integer(v)) => *v,
            _ => 1,
        };

        let mut updates: HashMap<String, FieldValue> = HashMap::new();
        updates.insert(
            "system:node_title".to_string(),
            FieldValue::String(dashboard.title.clone()),
        );
        updates.insert("grafana:uid".to_string(), FieldValue::String(uid.to_string()));
        updates.insert(
            "grafana:config".to_string(),
            FieldValue::Json(serde_json::to_value(&dashboard).map_err(|e| {
                PluginError::internal(format!("Failed to serialize: {}", e))
            })?),
        );
        updates.insert("grafana:version".to_string(), FieldValue::Integer(current_version + 1));
        if !dashboard.tags.is_empty() {
            updates.insert(
                "grafana:tags".to_string(),
                FieldValue::String(dashboard.tags.join(", ")),
            );
        }

        let updated = ctx.update_node(existing.id, updates).await?;
        ctx.log(
            LogLevel::Info,
            &format!("Updated dashboard: {} ({})", dashboard.title, uid),
        )
        .await;

        HttpResponse::json(&serde_json::json!({
            "uid": uid,
            "id": updated.id.to_string(),
            "title": dashboard.title,
            "version": current_version + 1,
        }))
    }

    /// Delete a dashboard.
    async fn handle_delete_dashboard(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
    ) -> Result<HttpResponse, PluginError> {
        let existing = self
            .find_dashboard_node(ctx, uid)
            .await?
            .ok_or_else(|| PluginError::not_found(&format!("Dashboard '{}' not found", uid)))?;

        let title = existing.title().unwrap_or("Untitled");
        ctx.delete_node(existing.id).await?;
        ctx.log(LogLevel::Info, &format!("Deleted dashboard: {} ({})", title, uid)).await;

        HttpResponse::json(&serde_json::json!({
            "deleted": true,
            "uid": uid,
        }))
    }

    /// Export a dashboard as JSON.
    async fn handle_export_dashboard(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
    ) -> Result<HttpResponse, PluginError> {
        match self.find_dashboard_by_uid(ctx, uid).await? {
            Some((_node, dashboard)) => HttpResponse::json(&dashboard),
            None => Err(PluginError::not_found(&format!(
                "Dashboard '{}' not found",
                uid
            ))),
        }
    }

    /// Import a dashboard from JSON.
    async fn handle_import_dashboard(
        &self,
        ctx: &dyn PluginContext,
        request: HttpRequest,
    ) -> Result<HttpResponse, PluginError> {
        let mut dashboard: Dashboard = serde_json::from_slice(
            request.body.as_deref().unwrap_or(&[]),
        )
        .map_err(|e| PluginError::bad_request(&format!("Invalid dashboard JSON: {}", e)))?;

        // Generate new UID to avoid conflicts
        let old_uid = dashboard.uid.clone();
        dashboard.uid = Self::generate_uid();

        let mut node = Node::new(Uuid::nil());
        node.set_field(
            "system:node_title",
            FieldValue::String(dashboard.title.clone()),
        );
        node.set_field("grafana:uid", FieldValue::String(dashboard.uid.clone()));
        node.set_field(
            "grafana:config",
            FieldValue::Json(serde_json::to_value(&dashboard).map_err(|e| {
                PluginError::internal(format!("Failed to serialize: {}", e))
            })?),
        );
        node.set_field("grafana:version", FieldValue::Integer(1));
        if !dashboard.tags.is_empty() {
            node.set_field(
                "grafana:tags",
                FieldValue::String(dashboard.tags.join(", ")),
            );
        }

        let created = ctx.create_node(node).await?;

        HttpResponse::json(&serde_json::json!({
            "uid": dashboard.uid,
            "id": created.id.to_string(),
            "title": dashboard.title,
            "imported_from": old_uid,
        }))
    }

    /// Duplicate a dashboard.
    async fn handle_duplicate_dashboard(
        &self,
        ctx: &dyn PluginContext,
        uid: &str,
    ) -> Result<HttpResponse, PluginError> {
        let (_existing_node, mut dashboard) = self
            .find_dashboard_by_uid(ctx, uid)
            .await?
            .ok_or_else(|| PluginError::not_found(&format!("Dashboard '{}' not found", uid)))?;

        dashboard.uid = Self::generate_uid();
        dashboard.title = format!("{} (copy)", dashboard.title);

        let mut node = Node::new(Uuid::nil());
        node.set_field(
            "system:node_title",
            FieldValue::String(dashboard.title.clone()),
        );
        node.set_field("grafana:uid", FieldValue::String(dashboard.uid.clone()));
        node.set_field(
            "grafana:config",
            FieldValue::Json(serde_json::to_value(&dashboard).map_err(|e| {
                PluginError::internal(format!("Failed to serialize: {}", e))
            })?),
        );
        node.set_field("grafana:version", FieldValue::Integer(1));

        let created = ctx.create_node(node).await?;

        HttpResponse::json(&serde_json::json!({
            "uid": dashboard.uid,
            "id": created.id.to_string(),
            "title": dashboard.title,
            "source_uid": uid,
        }))
    }

    /// Get auto-created home dashboard
    async fn handle_get_home_dashboard(
        &self,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        // Look for existing home dashboard
        let rows = ctx
            .query(
                "MATCH (n) IN space(\"default\") \
                 WHERE HAS_FIELD(n, \"grafana\", \"uid\") \
                 RETURN n",
            )
            .await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        for node in &nodes {
            if let Some(FieldValue::String(uid)) = node.get_field("grafana:uid") {
                if uid == "home" {
                    if let Some(FieldValue::Json(config)) = node.get_field("grafana:config") {
                        if let Ok(dashboard) = serde_json::from_value::<Dashboard>(config.clone()) {
                            return HttpResponse::json(&dashboard);
                        }
                    }
                }
            }
        }

        // Auto-create a default home dashboard
        let home = Dashboard {
            uid: "home".to_string(),
            title: "Home".to_string(),
            description: "Auto-generated home dashboard. Edit to customize.".to_string(),
            tags: vec!["home".to_string()],
            time: DashboardTime::default(),
            refresh: "5m".to_string(),
            schema_version: 1,
            panels: vec![
                Panel {
                    id: 1,
                    title: "Coding per Project".to_string(),
                    panel_type: "leaderboard".to_string(),
                    grid_pos: GridPos { x: 0, y: 0, w: 12, h: 8 },
                    queries: vec![PanelQuery {
                        data_source: "io.mzhang.panorama.wakatime".to_string(),
                        ref_id: "A".to_string(),
                        group_by: "project".to_string(),
                        aggregation: "leaderboard".to_string(),
                        filter: String::new(),
                        time_range: None,
                        bucket: String::new(),
                        limit: 10,
                        hide: false,
                        promql: None,
                    }],
                    options: serde_json::json!({"orientation": "horizontal", "showValues": true}),
                    field_config: serde_json::json!({}),
                    transparent: false,
                    description: String::new(),
                    repeat: None,
                },
                Panel {
                    id: 2,
                    title: "Coding per Language".to_string(),
                    panel_type: "leaderboard".to_string(),
                    grid_pos: GridPos { x: 12, y: 0, w: 12, h: 8 },
                    queries: vec![PanelQuery {
                        data_source: "io.mzhang.panorama.wakatime".to_string(),
                        ref_id: "A".to_string(),
                        group_by: "language".to_string(),
                        aggregation: "leaderboard".to_string(),
                        filter: String::new(),
                        time_range: None,
                        bucket: String::new(),
                        limit: 10,
                        hide: false,
                        promql: None,
                    }],
                    options: serde_json::json!({"orientation": "horizontal", "showValues": true}),
                    field_config: serde_json::json!({}),
                    transparent: false,
                    description: String::new(),
                    repeat: None,
                },
                Panel {
                    id: 3,
                    title: "Coding Activity".to_string(),
                    panel_type: "timeseries".to_string(),
                    grid_pos: GridPos { x: 0, y: 8, w: 24, h: 10 },
                    queries: vec![PanelQuery {
                        data_source: "io.mzhang.panorama.wakatime".to_string(),
                        ref_id: "A".to_string(),
                        group_by: "project".to_string(),
                        aggregation: "timeseries".to_string(),
                        filter: String::new(),
                        time_range: None,
                        bucket: "auto".to_string(),
                        limit: 10,
                        hide: false,
                        promql: None,
                    }],
                    options: serde_json::json!({"fill": 1, "lineWidth": 2}),
                    field_config: serde_json::json!({}),
                    transparent: false,
                    description: String::new(),
                    repeat: None,
                },
                Panel {
                    id: 4,
                    title: "Per File".to_string(),
                    panel_type: "leaderboard".to_string(),
                    grid_pos: GridPos { x: 0, y: 18, w: 24, h: 8 },
                    queries: vec![PanelQuery {
                        data_source: "io.mzhang.panorama.wakatime".to_string(),
                        ref_id: "A".to_string(),
                        group_by: "entity".to_string(),
                        aggregation: "leaderboard".to_string(),
                        filter: String::new(),
                        time_range: None,
                        bucket: String::new(),
                        limit: 15,
                        hide: false,
                        promql: None,
                    }],
                    options: serde_json::json!({"orientation": "horizontal", "showValues": true}),
                    field_config: serde_json::json!({}),
                    transparent: false,
                    description: String::new(),
                    repeat: None,
                },
            ],
            variables: vec![],
            annotations: vec![],
        };

        // Save it
        let mut node = Node::new(Uuid::nil());
        node.set_field("system:node_title", FieldValue::String("Home".to_string()));
        node.set_field("grafana:uid", FieldValue::String("home".to_string()));
        node.set_field(
            "grafana:config",
            FieldValue::Json(serde_json::to_value(&home).map_err(|e| {
                PluginError::internal(format!("Failed to serialize: {}", e))
            })?),
        );
        node.set_field("grafana:version", FieldValue::Integer(1));
        node.set_field("grafana:tags", FieldValue::String("home".to_string()));
        ctx.create_node(node).await?;
        ctx.log(LogLevel::Info, "Auto-created home dashboard").await;

        HttpResponse::json(&home)
    }

    // ── Folders ──────────────────────────────────────────────────

    async fn handle_list_folders(
        &self,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        let rows = ctx
            .query(
                "MATCH (n) IN space(\"default\") \
                 WHERE HAS_FIELD(n, \"grafana\", \"uid\") \
                 AND HAS_FIELD(n, \"system\", \"node_title\") \
                 RETURN n ORDER BY n.system.node_title ASC",
            )
            .await?;
        let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();

        // Folders are identified by having grafana:uid but no grafana:config with panels
        let folders: Vec<serde_json::Value> = nodes
            .iter()
            .filter(|n| {
                // A folder has no config with panels
                match n.get_field("grafana:config") {
                    None => true, // No config = folder
                    Some(FieldValue::Json(v)) => {
                        v.get("panels").map(|p| p.as_array().map(|a| a.is_empty()).unwrap_or(true)).unwrap_or(true)
                    }
                    _ => true,
                }
            })
            .map(|n| {
                let uid = match n.get_field("grafana:uid") {
                    Some(FieldValue::String(s)) => s.clone(),
                    _ => String::new(),
                };
                serde_json::json!({
                    "uid": uid,
                    "title": n.title().unwrap_or("Untitled"),
                    "id": n.id.to_string(),
                })
            })
            .collect();

        HttpResponse::json(&folders)
    }

    async fn handle_create_folder(
        &self,
        ctx: &dyn PluginContext,
        request: HttpRequest,
    ) -> Result<HttpResponse, PluginError> {
        let body: serde_json::Value = serde_json::from_slice(
            request.body.as_deref().unwrap_or(&[]),
        )
        .map_err(|e| PluginError::bad_request(&e.to_string()))?;

        let title = body["title"].as_str().unwrap_or("New Folder");
        let uid = body["uid"].as_str().map(String::from).unwrap_or_else(|| Self::generate_uid());

        let mut node = Node::new(Uuid::nil());
        node.set_field("system:node_title", FieldValue::String(title.to_string()));
        node.set_field("grafana:uid", FieldValue::String(uid.clone()));

        let created = ctx.create_node(node).await?;

        HttpResponse::json(&serde_json::json!({
            "uid": uid,
            "id": created.id.to_string(),
            "title": title,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_time_now() {
        let before = Utc::now().timestamp();
        let resolved = GrafanaPlugin::resolve_time("now");
        let after = Utc::now().timestamp();
        assert!(resolved.timestamp() >= before - 1);
        assert!(resolved.timestamp() <= after + 1);
    }

    #[test]
    fn test_resolve_time_relative_days() {
        let now = Utc::now();
        let resolved = GrafanaPlugin::resolve_time("now-7d");
        let diff = now - resolved;
        assert!(diff.num_days() >= 6 && diff.num_days() <= 8);
    }

    #[test]
    fn test_resolve_time_relative_hours() {
        let now = Utc::now();
        let resolved = GrafanaPlugin::resolve_time("now-24h");
        let diff = now - resolved;
        assert!(diff.num_hours() >= 23 && diff.num_hours() <= 25);
    }

    #[test]
    fn test_generate_uid() {
        let uid1 = GrafanaPlugin::generate_uid();
        let uid2 = GrafanaPlugin::generate_uid();
        assert_eq!(uid1.len(), 8);
        assert_ne!(uid1, uid2);
    }

    #[test]
    fn test_time_range_presets() {
        let presets = GrafanaPlugin::time_range_presets();
        assert!(!presets.is_empty());
        assert!(presets.iter().any(|p| p.label == "Last 7 days"));
    }
}
