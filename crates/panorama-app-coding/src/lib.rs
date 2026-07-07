//! Coding Activity Plugin — Full Coding Activity API-compatible heartbeat collection,
//! duration computation, and multi-dimensional stats engine.
//!
//! ## Modules
//!
//! - **schemas** — Schema definitions (Heartbeat, Duration, DailySummary)
//! - **helpers** — Time parsing, node field extraction, duration formatting
//! - **stats** — Stats engine (StatsQuery, aggregations, grouping)
//! - **heartbeat** — Heartbeat ingestion, dedup, placeholder resolution, UA parsing
//! - **durations** — Duration computation (session grouping)
//! - **wakatime** — WakaTime-compatible endpoint handlers + response formatting

mod durations;
mod heartbeat;
mod helpers;
mod schemas;
mod stats;
mod wakatime;

use async_trait::async_trait;
use chrono::Utc;
use matchit::Router;
use panorama_core::*;
use std::sync::OnceLock;
use uuid::Uuid;

use helpers::*;
use stats::StatsQuery;

// ── Plugin struct ─────────────────────────────────────────────────────────────

pub struct CodingPlugin;

impl CodingPlugin {
  pub fn new() -> Self {
    Self
  }
}

// ── Route dispatch table ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Route {
  Heartbeats, // POST (create), GET (read), DELETE — method dispatches in handler
  Durations,  // GET
  Stats,      // GET (query string), POST (JSON body)
  Summaries,  // GET
  WakaTimeStats,
  WakaTimeSummaries,
  WakaTimeAllTime,
  WakaTimeProjects,
  WakaTimeStatusbar,
  WakaTimeUserAgents,
  WakaTimeUserInfo,
  WakaTimeHeartbeatGet,
  WakaTimeLeaders,
  BadgeSvg,
  ShieldsBadge,
  ActivityChart,
}

fn build_router() -> Router<Route> {
  let mut r = Router::new();

  // ── Heartbeats (POST + GET + DELETE) ───────────────────────
  for path in &[
    "/users/current/heartbeats",
    "/users/current/heartbeats.bulk",
    "/heartbeat",
    "/heartbeats",
    "/users/{user}/heartbeats",
    "/users/{user}/heartbeats.bulk",
    "/v1/users/{user}/heartbeats",
    "/v1/users/{user}/heartbeats.bulk",
    "/compat/wakatime/v1/users/{user}/heartbeats",
    "/compat/wakatime/v1/users/{user}/heartbeats.bulk",
  ] {
    r.insert(*path, Route::Heartbeats).ok();
  }

  // ── Durations ──────────────────────────────────────────────
  r.insert("/users/current/durations", Route::Durations).ok();
  r.insert("/durations", Route::Durations).ok();

  // ── Stats ──────────────────────────────────────────────────
  r.insert("/stats", Route::Stats).ok();

  // ── Summaries ──────────────────────────────────────────────
  r.insert("/summaries", Route::Summaries).ok();

  // ── WakaTime compat: stats ─────────────────────────────────
  for path in &[
    "/users/{user}/stats",
    "/users/{user}/stats/{range}",
    "/v1/users/{user}/stats",
    "/v1/users/{user}/stats/{range}",
    "/compat/wakatime/v1/users/{user}/stats",
    "/compat/wakatime/v1/users/{user}/stats/{range}",
  ] {
    r.insert(*path, Route::WakaTimeStats).ok();
  }

  // ── WakaTime compat: summaries ─────────────────────────────
  for path in &[
    "/users/{user}/summaries",
    "/v1/users/{user}/summaries",
    "/compat/wakatime/v1/users/{user}/summaries",
  ] {
    r.insert(*path, Route::WakaTimeSummaries).ok();
  }

  // ── WakaTime compat: all_time ──────────────────────────────
  for path in &[
    "/users/{user}/all_time_since_today",
    "/v1/users/{user}/all_time_since_today",
    "/compat/wakatime/v1/users/{user}/all_time_since_today",
  ] {
    r.insert(*path, Route::WakaTimeAllTime).ok();
  }

  // ── WakaTime compat: projects ──────────────────────────────
  r.insert("/users/{user}/projects", Route::WakaTimeProjects)
    .ok();
  r.insert(
    "/compat/wakatime/v1/users/{user}/projects",
    Route::WakaTimeProjects,
  )
  .ok();

  // ── WakaTime compat: statusbar ─────────────────────────────
  for path in &[
    "/users/{user}/statusbar/{range}",
    "/v1/users/{user}/statusbar/{range}",
    "/compat/wakatime/v1/users/{user}/statusbar/{range}",
    "/users/{user}/statusbar",
    "/v1/users/{user}/statusbar",
    "/compat/wakatime/v1/users/{user}/statusbar",
  ] {
    r.insert(*path, Route::WakaTimeStatusbar).ok();
  }

  // ── WakaTime compat: user_agents ───────────────────────────
  r.insert("/users/{user}/user_agents", Route::WakaTimeUserAgents)
    .ok();
  r.insert(
    "/compat/wakatime/v1/users/{user}/user_agents",
    Route::WakaTimeUserAgents,
  )
  .ok();

  // ── WakaTime compat: user info ─────────────────────────────
  r.insert("/users/{user}", Route::WakaTimeUserInfo).ok();
  r.insert("/compat/wakatime/v1/users/{user}", Route::WakaTimeUserInfo)
    .ok();

  // ── WakaTime compat: heartbeat GET ─────────────────────────
  r.insert(
    "/compat/wakatime/v1/users/{user}/heartbeats",
    Route::WakaTimeHeartbeatGet,
  )
  .ok();

  // ── WakaTime compat: leaders ───────────────────────────────
  r.insert("/compat/wakatime/v1/leaders", Route::WakaTimeLeaders)
    .ok();

  // ── Badges / Charts ───────────────────────────────────────
  r.insert("/badge/{user}/{*rest}", Route::BadgeSvg).ok();
  r.insert(
    "/compat/shields/v1/{user}/{interval}/{filter}",
    Route::ShieldsBadge,
  )
  .ok();
  r.insert("/activity/chart/{user}.svg", Route::ActivityChart)
    .ok();

  r
}

fn router() -> &'static Router<Route> {
  static ROUTER: OnceLock<Router<Route>> = OnceLock::new();
  ROUTER.get_or_init(build_router)
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
    "0.3.0"
  }

  fn description(&self) -> &str {
    "Full Coding Activity API-compatible heartbeat collection with stats, durations, leaderboards, and WakaTime compat"
  }

  fn schemas(&self) -> Vec<Schema> {
    vec![
      schemas::heartbeat_schema(),
      schemas::duration_schema(),
      schemas::daily_summary_schema(),
    ]
  }

  fn http_endpoints(&self) -> Vec<HttpEndpoint> {
    // List every route pattern for manifest registration
    let paths = [
      // Original
      ("POST", "/users/current/heartbeats"),
      ("POST", "/users/current/heartbeats.bulk"),
      ("GET", "/users/current/heartbeats"),
      ("GET", "/users/current/durations"),
      ("DELETE", "/users/current/heartbeats.bulk"),
      ("POST", "/heartbeat"),
      ("POST", "/heartbeats"),
      ("GET", "/heartbeats"),
      ("GET", "/durations"),
      ("GET", "/stats"),
      ("POST", "/stats"),
      ("GET", "/summaries"),
      // WakaTime compat: heartbeat POST aliases
      ("POST", "/users/{user}/heartbeats"),
      ("POST", "/users/{user}/heartbeats.bulk"),
      ("POST", "/v1/users/{user}/heartbeats"),
      ("POST", "/v1/users/{user}/heartbeats.bulk"),
      ("POST", "/compat/wakatime/v1/users/{user}/heartbeats"),
      ("POST", "/compat/wakatime/v1/users/{user}/heartbeats.bulk"),
      // WakaTime compat: stats
      ("GET", "/users/{user}/stats/{range}"),
      ("GET", "/users/{user}/stats"),
      ("GET", "/v1/users/{user}/stats/{range}"),
      ("GET", "/v1/users/{user}/stats"),
      ("GET", "/compat/wakatime/v1/users/{user}/stats/{range}"),
      ("GET", "/compat/wakatime/v1/users/{user}/stats"),
      // WakaTime compat: summaries
      ("GET", "/users/{user}/summaries"),
      ("GET", "/v1/users/{user}/summaries"),
      ("GET", "/compat/wakatime/v1/users/{user}/summaries"),
      // WakaTime compat: all_time
      ("GET", "/users/{user}/all_time_since_today"),
      ("GET", "/v1/users/{user}/all_time_since_today"),
      (
        "GET",
        "/compat/wakatime/v1/users/{user}/all_time_since_today",
      ),
      // WakaTime compat: projects
      ("GET", "/users/{user}/projects"),
      ("GET", "/compat/wakatime/v1/users/{user}/projects"),
      // WakaTime compat: statusbar
      ("GET", "/users/{user}/statusbar/{range}"),
      ("GET", "/v1/users/{user}/statusbar/{range}"),
      ("GET", "/compat/wakatime/v1/users/{user}/statusbar/{range}"),
      // WakaTime compat: user_agents
      ("GET", "/users/{user}/user_agents"),
      ("GET", "/compat/wakatime/v1/users/{user}/user_agents"),
      // WakaTime compat: user info
      ("GET", "/users/{user}"),
      ("GET", "/compat/wakatime/v1/users/{user}"),
      // WakaTime compat: heartbeat GET
      ("GET", "/compat/wakatime/v1/users/{user}/heartbeats"),
      // WakaTime compat: leaders
      ("GET", "/compat/wakatime/v1/leaders"),
      // Badges
      ("GET", "/badge/{user}/{*rest}"),
      ("GET", "/activity/chart/{user}.svg"),
      ("GET", "/compat/shields/v1/{user}/{interval}/{filter}"),
    ];
    paths
      .iter()
      .map(|(method, path)| HttpEndpoint {
        method: match *method {
          "POST" => HttpMethod::POST,
          "GET" => HttpMethod::GET,
          "DELETE" => HttpMethod::DELETE,
          _ => HttpMethod::GET,
        },
        path: path.to_string(),
        description: "".to_string(),
      })
      .collect()
  }

  fn background_tasks(&self) -> Vec<BackgroundTask> {
    vec![BackgroundTask {
      name: "daily-summary-rollup".to_string(),
      interval_seconds: Some(3600),
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
    let method = request.method.as_str();

    // Strip leading slash for matchit (our patterns include it)
    let path = if endpoint.starts_with('/') {
      endpoint.to_string()
    } else {
      format!("/{}", endpoint)
    };

    let Ok(matched) = router().at(&path) else {
      return Err(PluginError::not_found(&format!(
        "Unknown endpoint: {} {}",
        method, endpoint
      )));
    };

    let route = *matched.value;
    let params = &matched.params;

    match (method, route) {
      // ── Heartbeats (POST/GET/DELETE dispatch by method) ─────
      ("POST", Route::Heartbeats) => self.handle_post_heartbeats(request, ctx).await,
      ("GET", Route::Heartbeats) | ("GET", Route::WakaTimeHeartbeatGet) => {
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
      ("DELETE", Route::Heartbeats) => {
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

      // ── Durations ──────────────────────────────────────────
      ("GET", Route::Durations) => {
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

      // ── Stats (GET query string, POST JSON body) ────────────
      ("GET", Route::Stats) => {
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
      ("POST", Route::Stats) => {
        let query: StatsQuery = serde_json::from_slice(request.body.as_deref().unwrap_or(&[]))
          .map_err(|e| PluginError::bad_request(&format!("Invalid stats query: {}", e)))?;
        let result = self.execute_stats(ctx, &query).await?;
        HttpResponse::json(&result)
      }

      // ── Summaries ──────────────────────────────────────────
      ("GET", Route::Summaries) => {
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

      // ── WakaTime stats ─────────────────────────────────────
      ("GET", Route::WakaTimeStats) => {
        let user = params.get("user").unwrap_or("current").to_string();
        let range = params.get("range").map(|s| s.to_string());
        self
          .handle_wakatime_stats(ctx, &user, range.as_deref(), &request)
          .await
      }

      // ── WakaTime summaries ─────────────────────────────────
      ("GET", Route::WakaTimeSummaries) => {
        let user = params.get("user").unwrap_or("current").to_string();
        self.handle_wakatime_summaries(ctx, &user, &request).await
      }

      // ── WakaTime all_time ──────────────────────────────────
      ("GET", Route::WakaTimeAllTime) => self.handle_all_time(ctx).await,

      // ── WakaTime projects ──────────────────────────────────
      ("GET", Route::WakaTimeProjects) => self.handle_projects(ctx, &request).await,

      // ── WakaTime statusbar ─────────────────────────────────
      ("GET", Route::WakaTimeStatusbar) => {
        let range = params.get("range").unwrap_or("today").to_string();
        self.handle_statusbar(ctx, &range).await
      }

      // ── WakaTime user_agents ───────────────────────────────
      ("GET", Route::WakaTimeUserAgents) => self.handle_user_agents(ctx).await,

      // ── WakaTime user info ─────────────────────────────────
      ("GET", Route::WakaTimeUserInfo) => self.handle_user_info().await,

      // ── Leaders ────────────────────────────────────────────
      ("GET", Route::WakaTimeLeaders) => self.handle_leaders(ctx, &request).await,

      // ── Badges & Charts ────────────────────────────────────
      ("GET", Route::BadgeSvg) => self.handle_badge(ctx, endpoint, &request).await,
      ("GET", Route::ActivityChart) => self.handle_activity_chart(ctx).await,
      ("GET", Route::ShieldsBadge) => {
        let parts: Vec<&str> = endpoint.split('/').filter(|s| !s.is_empty()).collect();
        self.handle_shields_badge(ctx, &parts, &request).await
      }

      _ => Err(PluginError::not_found(&format!(
        "Method {} not allowed for {}",
        method, endpoint
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
    assert!(diff.num_days() == 6);
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

  #[test]
  fn test_fmt_wakatime_duration() {
    assert_eq!(fmt_wakatime_duration(12345.0), "3 hrs 26 mins");
    assert_eq!(fmt_wakatime_duration(3600.0), "1 hr 0 mins");
    assert_eq!(fmt_wakatime_duration(60.0), "1 min");
    assert_eq!(fmt_wakatime_duration(30.0), "1 min");
    assert_eq!(fmt_wakatime_duration(0.0), "0 mins");
  }

  #[test]
  fn test_fmt_digital() {
    assert_eq!(fmt_digital(5415.0), "1:30:15");
    assert_eq!(fmt_digital(3600.0), "1:00:00");
  }

  #[test]
  fn test_parse_user_agent() {
    let (editor, os) =
      heartbeat::parse_user_agent("wakatime/v1.2.3 (linux-x86_64) VSCode/v1.80.0 vscode");
    assert_eq!(editor, "VSCode");
    assert_eq!(os, "linux");

    let (editor, os) = heartbeat::parse_user_agent("vim/9.0 (macos)");
    assert_eq!(editor, "vim");
    assert_eq!(os, "macos");

    let (editor, os) = heartbeat::parse_user_agent("curl/7.68.0");
    assert_eq!(editor, "curl");
    assert_eq!(os, "Unknown");
  }

  #[test]
  fn test_compute_dedup_hash() {
    let hash1 = heartbeat::compute_dedup_hash(
      "/src/main.rs",
      "file",
      "coding",
      "panorama",
      "main",
      "Rust",
      1234567.0,
      "VSCode",
      "Linux",
      "my-machine",
    );
    let hash2 = heartbeat::compute_dedup_hash(
      "/src/main.rs",
      "file",
      "coding",
      "panorama",
      "main",
      "Rust",
      1234567.0,
      "VSCode",
      "Linux",
      "my-machine",
    );
    assert_eq!(hash1, hash2);

    let hash3 = heartbeat::compute_dedup_hash(
      "/src/lib.rs",
      "file",
      "coding",
      "panorama",
      "main",
      "Rust",
      1234567.0,
      "VSCode",
      "Linux",
      "my-machine",
    );
    assert_ne!(hash1, hash3);
  }

  #[test]
  fn test_parse_time_range_all_variants() {
    for range in &["24h", "7d", "30d", "90d", "365d", "all"] {
      let (start, end) = parse_time_range(range);
      assert!(start < end, "range {}: start should be before end", range);
    }
    let (start, end) = parse_time_range("garbage");
    let diff = end - start;
    assert!(diff.num_days() == 7);
  }

  #[test]
  fn test_router_matches_all_endpoints() {
    let r = build_router();
    // Test that common WakaTime paths match
    assert!(r.at("/users/current/heartbeats").is_ok());
    assert!(r.at("/heartbeat").is_ok());
    assert!(r.at("/users/test-user/stats").is_ok());
    assert!(r.at("/users/test-user/stats/last_7_days").is_ok());
    assert!(r.at("/v1/users/test-user/stats").is_ok());
    assert!(r
      .at("/compat/wakatime/v1/users/test-user/stats/last_7_days")
      .is_ok());
    assert!(r
      .at("/compat/wakatime/v1/users/test-user/summaries")
      .is_ok());
    assert!(r
      .at("/compat/wakatime/v1/users/test-user/all_time_since_today")
      .is_ok());
    assert!(r.at("/compat/wakatime/v1/leaders").is_ok());
    assert!(r.at("/users/test-user/projects").is_ok());
    assert!(r.at("/users/test-user/statusbar/today").is_ok());
    assert!(r
      .at("/badge/test-user/interval:any/project:panorama")
      .is_ok());
  }
}
