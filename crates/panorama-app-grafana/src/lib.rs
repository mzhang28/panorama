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
//!         "refId": "A",
//!         "dataSource": "io.mzhang.panorama.wakatime",
//!         "promql": "sum by (project) (wakatime_duration)",
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
//! | Type         | Description                        |
//! |-------------|------------------------------------|
//! | leaderboard | Horizontal bar chart, sorted desc  |
//! | timeseries  | Line/area chart over time          |
//! | stat        | Single big number                  |
//! | piechart    | Pie/donut chart                    |
//! | table       | Sortable data table                |
//! | heatmap     | Calendar heatmap (GitHub-style)    |
//!
//! ### Data Flow
//!
//! 1. Frontend loads dashboard JSON from `GET /api/dashboards/{uid}`
//! 2. For each panel, frontend calls `POST /api/ds/query` with PromQL expressions
//! 3. The plugin parses PromQL, translates to PQL, executes, and post-processes
//! 4. Results are returned in data frame format

pub mod promql;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use panorama_core::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// PromQL expression that defines what data to fetch
    #[serde(default)]
    pub promql: Option<String>,
    /// Time range override (uses dashboard time if not set)
    #[serde(default)]
    pub time_range: Option<DashboardTime>,
    /// Result limit
    #[serde(default = "default_query_limit")]
    pub limit: usize,
    /// Hide this query's results from the visualization
    #[serde(default)]
    pub hide: bool,
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
// Query Request / Response
// ═══════════════════════════════════════════════════════════════════════════════

/// A batch of queries to execute.
#[derive(Debug, Deserialize)]
pub struct DataQueryRequest {
    pub queries: Vec<PanelQuery>,
    #[serde(default)]
    pub range: DashboardTime,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

/// A data frame in Grafana-compatible format.
#[derive(Debug, Serialize)]
pub struct DataFrame {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Time Range Engine
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangePreset {
    pub label: String,
    pub from: String,
    pub to: String,
}

impl GrafanaPlugin {
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
    fn resolve_time(expr: &str) -> DateTime<Utc> {
        let now = Utc::now();
        if expr == "now" {
            return now;
        }

        if let Some(rest) = expr.strip_prefix("now-") {
            return parse_relative(rest, now);
        }

        if expr == "now/d" {
            return now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        }
        if expr == "now/w" {
            let weekday = now.date_naive().weekday().num_days_from_monday();
            return (now.date_naive() - chrono::Duration::days(weekday as i64))
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
        }
        if expr == "now/M" {
            return now
                .date_naive()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
        }
        if expr == "now/y" {
            return NaiveDate::from_ymd_opt(now.year(), 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
        }
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
            return now - Duration::days(months as i64 * 30);
        }
    }
    now
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query Engine — PromQL only
// ═══════════════════════════════════════════════════════════════════════════════

impl GrafanaPlugin {
    /// Execute a batch of panel queries.
    async fn execute_data_queries(
        &self,
        ctx: &dyn PluginContext,
        request: &DataQueryRequest,
    ) -> Result<Vec<DataFrame>, PluginError> {
        let mut frames = Vec::new();

        for query in &request.queries {
            let range = query.time_range.as_ref().unwrap_or(&request.range);
            let from = Self::resolve_time(&range.from);
            let to = Self::resolve_time(&range.to);
            let ref_id = if query.ref_id.is_empty() { "A".to_string() } else { query.ref_id.clone() };

            let frame = self.execute_promql_query(ctx, query, from, to, &ref_id).await?;
            frames.push(frame);
        }

        Ok(frames)
    }

    /// Execute a PromQL panel query: parse → translate → execute PQL → post-process.
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
            return Err(PluginError::bad_request("PromQL expression is required"));
        }

        // 1. Parse PromQL
        let expr = promql::parser::parse(promql_str)?;

        // 2. Load metric registry
        let registry = promql::registry::MetricRegistry::with_wakatime_defaults();

        // 3. Translate to PQL + post-steps
        let tq = promql::translator::translate(
            &expr,
            &registry,
            &from.to_rfc3339(),
            &to.to_rfc3339(),
        )
        .map_err(|e| PluginError::bad_request(&format!("PromQL translation error: {}", e)))?;

        // 4. Execute PQL query
        let rows = ctx.query(&tq.pql).await?;
        let nodes: Vec<Node> = rows
            .iter()
            .filter_map(panorama_core::query::row_to_node)
            .collect();

        // 5. Apply post-processing and produce DataFrame
        let mut df = promql::translator::execute_translated(&tq, &nodes)
            .map_err(|e| PluginError::internal(format!("PromQL execution error: {}", e)))?;

        df.name = ref_id.to_string();
        df.meta = Some(serde_json::json!({
            "query_type": "promql",
            "expression": promql_str,
            "from": from.to_rfc3339(),
            "to": to.to_rfc3339(),
        }));

        Ok(df)
    }
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
                    description: Some("Full dashboard JSON configuration".to_string()),
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
                    description: Some("Comma-separated tags".to_string()),
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
        "0.3.0"
    }

    fn description(&self) -> &str {
        "Full dashboard system powered by PromQL — query with Prometheus syntax"
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
                description: "Execute PromQL panel queries".to_string(),
            },
            // ── Options ────────────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/query/options".to_string(),
                description: "Get time range presets and panel type options".to_string(),
            },
            // ── PromQL tools ──────────────────────────────────────
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/api/promql/validate".to_string(),
                description: "Validate a PromQL expression and show its PQL translation".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/api/promql/metrics".to_string(),
                description: "List registered PromQL metrics".to_string(),
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
                let panel_types = vec![
                    serde_json::json!({"value": "leaderboard", "label": "Leaderboard"}),
                    serde_json::json!({"value": "timeseries", "label": "Time Series"}),
                    serde_json::json!({"value": "stat", "label": "Single Stat"}),
                    serde_json::json!({"value": "piechart", "label": "Pie Chart"}),
                    serde_json::json!({"value": "table", "label": "Table"}),
                    serde_json::json!({"value": "heatmap", "label": "Heatmap"}),
                ];
                HttpResponse::json(&serde_json::json!({
                    "time_ranges": presets,
                    "panel_types": panel_types,
                }))
            }

            // ── PromQL validation ──────────────────────────────────
            ("POST", "api/promql/validate") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&format!("Invalid JSON: {}", e)))?;
                let expr_str = body["expression"].as_str().unwrap_or("");
                match promql::parser::parse(expr_str) {
                    Ok(expr) => {
                        let registry = promql::registry::MetricRegistry::with_wakatime_defaults();
                        let now = Utc::now();
                        let from_str = now.to_rfc3339();
                        let to_str = (now + chrono::Duration::hours(1)).to_rfc3339();
                        match promql::translator::translate(&expr, &registry, &from_str, &to_str) {
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
                let body_val: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&format!("Invalid JSON: {}", e)))?;

                // Accept both new {queries:[{promql:"..."}]} and old {promql:"..."} formats
                let req = if body_val.get("queries").is_some() {
                    serde_json::from_value::<DataQueryRequest>(body_val)
                        .map_err(|e| PluginError::bad_request(&format!("Invalid query request: {}", e)))?
                } else {
                    // Wrap old flat format
                    let promql = body_val["promql"].as_str().map(String::from);
                    let limit = body_val["limit"].as_u64().unwrap_or(25) as usize;
                    DataQueryRequest {
                        queries: vec![PanelQuery {
                            data_source: "io.mzhang.panorama.wakatime".to_string(),
                            ref_id: "A".to_string(),
                            promql,
                            time_range: None,
                            limit,
                            hide: false,
                        }],
                        range: DashboardTime::default(),
                        from: String::new(),
                        to: String::new(),
                    }
                };
                let frames = self.execute_data_queries(ctx, &req).await?;
                HttpResponse::json(&frames)
            }
            ("POST", "dashboards") => self.handle_create_dashboard(ctx, request).await,
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
    fn generate_uid() -> String {
        let uuid = Uuid::new_v4();
        let hex = uuid.as_simple().to_string();
        hex[..8].to_string()
    }

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

        if dashboard.uid.is_empty() {
            dashboard.uid = Self::generate_uid();
        }

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

    async fn handle_import_dashboard(
        &self,
        ctx: &dyn PluginContext,
        request: HttpRequest,
    ) -> Result<HttpResponse, PluginError> {
        let mut dashboard: Dashboard = serde_json::from_slice(
            request.body.as_deref().unwrap_or(&[]),
        )
        .map_err(|e| PluginError::bad_request(&format!("Invalid dashboard JSON: {}", e)))?;

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

    async fn handle_get_home_dashboard(
        &self,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
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

        // Auto-create default home dashboard with PromQL-powered panels
        let home = default_home_dashboard();

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

        let folders: Vec<serde_json::Value> = nodes
            .iter()
            .filter(|n| {
                match n.get_field("grafana:config") {
                    None => true,
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
// Default Home Dashboard
// ═══════════════════════════════════════════════════════════════════════════════

fn default_home_dashboard() -> Dashboard {
    Dashboard {
        uid: "home".to_string(),
        title: "Home".to_string(),
        description: "Auto-generated home dashboard powered by PromQL. Edit to customize.".to_string(),
        tags: vec!["home".to_string()],
        time: DashboardTime::default(),
        refresh: "5m".to_string(),
        schema_version: 1,
        panels: vec![
            // Panel 1: Coding per Project (leaderboard)
            Panel {
                id: 1,
                title: "Coding per Project".to_string(),
                panel_type: "leaderboard".to_string(),
                grid_pos: GridPos { x: 0, y: 0, w: 12, h: 8 },
                queries: vec![PanelQuery {
                    data_source: "io.mzhang.panorama.wakatime".to_string(),
                    ref_id: "A".to_string(),
                    promql: Some("sum by (project) (wakatime_duration)".to_string()),
                    time_range: None,
                    limit: 10,
                    hide: false,
                }],
                options: serde_json::json!({"orientation": "horizontal", "showValues": true}),
                field_config: serde_json::json!({}),
                transparent: false,
                description: String::new(),
                repeat: None,
            },
            // Panel 2: Coding per Language (leaderboard)
            Panel {
                id: 2,
                title: "Coding per Language".to_string(),
                panel_type: "leaderboard".to_string(),
                grid_pos: GridPos { x: 12, y: 0, w: 12, h: 8 },
                queries: vec![PanelQuery {
                    data_source: "io.mzhang.panorama.wakatime".to_string(),
                    ref_id: "A".to_string(),
                    promql: Some("sum by (language) (wakatime_duration)".to_string()),
                    time_range: None,
                    limit: 10,
                    hide: false,
                }],
                options: serde_json::json!({"orientation": "horizontal", "showValues": true}),
                field_config: serde_json::json!({}),
                transparent: false,
                description: String::new(),
                repeat: None,
            },
            // Panel 3: Coding Activity (timeseries)
            Panel {
                id: 3,
                title: "Coding Activity".to_string(),
                panel_type: "timeseries".to_string(),
                grid_pos: GridPos { x: 0, y: 8, w: 24, h: 10 },
                queries: vec![PanelQuery {
                    data_source: "io.mzhang.panorama.wakatime".to_string(),
                    ref_id: "A".to_string(),
                    promql: Some("sum by (project) (wakatime_duration)".to_string()),
                    time_range: None,
                    limit: 10,
                    hide: false,
                }],
                options: serde_json::json!({"fill": 1, "lineWidth": 2}),
                field_config: serde_json::json!({}),
                transparent: false,
                description: String::new(),
                repeat: None,
            },
            // Panel 4: Per File (leaderboard)
            Panel {
                id: 4,
                title: "Per File".to_string(),
                panel_type: "leaderboard".to_string(),
                grid_pos: GridPos { x: 0, y: 18, w: 24, h: 8 },
                queries: vec![PanelQuery {
                    data_source: "io.mzhang.panorama.wakatime".to_string(),
                    ref_id: "A".to_string(),
                    promql: Some("topk(15, sum by (entity) (wakatime_duration))".to_string()),
                    time_range: None,
                    limit: 15,
                    hide: false,
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

    #[test]
    fn test_default_home_dashboard_panels_use_promql() {
        let home = default_home_dashboard();
        assert_eq!(home.panels.len(), 4);
        for panel in &home.panels {
            for query in &panel.queries {
                assert!(query.promql.is_some(), "Panel '{}' has no promql", panel.title);
                assert!(!query.promql.as_ref().unwrap().is_empty());
            }
        }
    }
}
