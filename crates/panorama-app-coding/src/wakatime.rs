use chrono::Utc;
use panorama_core::*;
use std::collections::HashMap;
use std::hash::Hasher;

use crate::helpers::*;
use crate::CodingPlugin;

// ═══════════════════════════════════════════════════════════════════════════════
// WakaTime-Compatible Response Types
// ═══════════════════════════════════════════════════════════════════════════════

fn summarize_entry(name: &str, total_seconds: f64, grand_total: f64) -> serde_json::Value {
  let percent = if grand_total > 0.0 {
    (total_seconds / grand_total * 1000.0).round() / 10.0
  } else {
    0.0
  };
  let total_secs_int = total_seconds as i64;
  let hours = total_secs_int / 3600;
  let minutes = (total_secs_int % 3600) / 60;
  let seconds = total_secs_int % 60;

  serde_json::json!({
    "digital": format!("{}:{:02}:{:02}", hours, minutes, seconds),
    "hours": hours,
    "minutes": minutes,
    "name": name,
    "percent": percent,
    "seconds": total_secs_int,
    "text": fmt_wakatime_duration(total_seconds),
    "total_seconds": total_seconds,
  })
}

fn grand_total(seconds: f64) -> serde_json::Value {
  let total = seconds as i64;
  let hours = total / 3600;
  let minutes = (total % 3600) / 60;
  serde_json::json!({
    "digital": format!("{}:{:02}", hours, minutes),
    "hours": hours,
    "minutes": minutes,
    "text": fmt_wakatime_duration(seconds),
    "total_seconds": seconds,
  })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Endpoint Handlers
// ═══════════════════════════════════════════════════════════════════════════════

impl CodingPlugin {
  /// WakaTime-compatible stats endpoint.
  /// Returns projects, languages, editors, OSs, machines, categories, branches
  /// in the standard WakaTime StatsResponse format.
  pub(crate) async fn handle_wakatime_stats(
    &self,
    ctx: &dyn PluginContext,
    _user: &str,
    range: Option<&str>,
    request: &HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    let range = range.unwrap_or("7d");
    let (start, end) = parse_time_range(range);
    let days = (end - start).num_days().max(1) as f64;

    async fn dim(
      plugin: &CodingPlugin,
      ctx: &dyn PluginContext,
      range: &str,
      field: &str,
    ) -> Result<HashMap<String, f64>, PluginError> {
      let rows = plugin
        .execute_aggregate_query(ctx, range, field, "SUM", None)
        .await?;
      let mut map = HashMap::new();
      for row in &rows {
        let key = row
          .get("key")
          .and_then(|v| v.as_str())
          .unwrap_or("(unknown)")
          .to_string();
        let val = row.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        map.insert(key, val);
      }
      Ok(map)
    }

    let by_project = dim(self, ctx, range, "project").await?;
    let by_language = dim(self, ctx, range, "language").await?;
    let by_editor = dim(self, ctx, range, "editor").await?;
    let by_os = dim(self, ctx, range, "operating_system").await?;
    let by_machine = dim(self, ctx, range, "machine_name_id").await?;
    let by_category = dim(self, ctx, range, "category").await?;

    let total_seconds: f64 = by_project.values().sum();
    let daily_average = total_seconds / days;

    let sort_entries = |map: HashMap<String, f64>| -> Vec<serde_json::Value> {
      let mut entries: Vec<(String, f64)> = map.into_iter().collect();
      entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
      entries
        .into_iter()
        .map(|(k, v)| summarize_entry(&k, v, total_seconds))
        .collect()
    };

    // best_day via PQL: group by date, find max
    let best_day = {
      let day_rows = self
        .execute_aggregate_query(ctx, range, "time", "SUM", None)
        .await?;
      day_rows
        .iter()
        .filter_map(|row| {
          let key = row.get("key")?.as_str()?;
          let val = row.get("value")?.as_f64()?;
          let ts = key.parse::<f64>().ok()? as i64;
          let date =
            chrono::DateTime::from_timestamp(ts, 0).map(|d| d.format("%Y-%m-%d").to_string())?;
          Some((date, val))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    };

    HttpResponse::json(&serde_json::json!({
      "data": {
        "username": "current",
        "user_id": "current",
        "start": start.to_rfc3339(),
        "end": end.to_rfc3339(),
        "status": "ok",
        "timezone": "UTC",
        "total_seconds": total_seconds,
        "daily_average": daily_average,
        "days_including_holidays": days as i64,
        "range": range,
        "human_readable_range": range,
        "human_readable_total": fmt_wakatime_duration(total_seconds),
        "human_readable_daily_average": fmt_wakatime_duration(daily_average),
        "is_coding_activity_visible": true,
        "is_other_usage_visible": true,
        "best_day": best_day.map(|(date, secs)| {
          serde_json::json!({
            "date": date,
            "total_seconds": secs,
            "text": fmt_wakatime_duration(secs),
          })
        }),
        "projects": sort_entries(by_project),
        "languages": sort_entries(by_language),
        "editors": sort_entries(by_editor),
        "operating_systems": sort_entries(by_os),
        "machines": sort_entries(by_machine),
        "categories": sort_entries(by_category),
      }
    }))
  }

  /// WakaTime-compatible summaries endpoint.
  /// Returns per-day summaries with breakdowns.
  pub(crate) async fn handle_wakatime_summaries(
    &self,
    ctx: &dyn PluginContext,
    _user: &str,
    request: &HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    let range = request
      .query_params
      .get("range")
      .cloned()
      .unwrap_or_else(|| "7d".to_string());
    let (start, end) = parse_time_range(&range);

    let nodes = self.fetch_heartbeats_in_range(ctx, &range).await?;

    // Group by day
    let mut days: std::collections::BTreeMap<String, Vec<&Node>> =
      std::collections::BTreeMap::new();
    for node in &nodes {
      let ts = node_time_epoch(node) as i64;
      let date = chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
      days.entry(date).or_default();
      // We need owned nodes for computations
    }

    // Re-do with proper grouping
    let mut day_nodes: std::collections::BTreeMap<String, Vec<Node>> =
      std::collections::BTreeMap::new();
    for node in nodes {
      let ts = node_time_epoch(&node) as i64;
      let date = chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
      day_nodes.entry(date).or_default().push(node);
    }

    let mut data = Vec::new();
    let mut cumulative_seconds = 0.0;

    for (date, day_nodes) in &day_nodes {
      let total = day_nodes
        .iter()
        .map(|n| node_duration_seconds(n))
        .sum::<f64>();
      cumulative_seconds += total;

      let by_project = self.group_and_sum(day_nodes, "project", None);
      let by_language = self.group_and_sum(day_nodes, "language", None);
      let by_editor = self.group_and_sum(day_nodes, "editor", None);
      let by_os = self.group_and_sum(day_nodes, "operating_system", None);
      let by_machine = self.group_and_sum(day_nodes, "machine_name_id", None);
      let by_category = self.group_and_sum(day_nodes, "category", None);

      let sort_entries = |map: HashMap<String, f64>| -> Vec<serde_json::Value> {
        let mut entries: Vec<(String, f64)> = map.into_iter().collect();
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        entries
          .into_iter()
          .map(|(k, v)| summarize_entry(&k, v, total))
          .collect()
      };

      data.push(serde_json::json!({
        "categories": sort_entries(by_category),
        "dependencies": [],
        "editors": sort_entries(by_editor),
        "languages": sort_entries(by_language),
        "machines": sort_entries(by_machine),
        "operating_systems": sort_entries(by_os),
        "projects": sort_entries(by_project),
        "grand_total": grand_total(total),
        "range": {
          "date": format!("{}T00:00:00Z", date),
          "end": format!("{}T23:59:59Z", date),
          "start": format!("{}T00:00:00Z", date),
          "text": "",
          "timezone": "UTC",
        }
      }));
    }

    let num_days = day_nodes.len().max(1) as i64;

    HttpResponse::json(&serde_json::json!({
      "data": data,
      "start": start.to_rfc3339(),
      "end": end.to_rfc3339(),
      "cumulative_total": {
        "decimal": format!("{:.2}", cumulative_seconds / 3600.0),
        "digital": fmt_digital(cumulative_seconds),
        "seconds": cumulative_seconds,
        "text": fmt_wakatime_duration(cumulative_seconds),
      },
      "daily_average": {
        "days_including_holidays": num_days,
        "days_minus_holidays": num_days,
        "holidays": 0,
        "seconds": (cumulative_seconds / num_days as f64) as i64,
        "seconds_including_other_language": (cumulative_seconds / num_days as f64) as i64,
        "text": fmt_wakatime_duration(cumulative_seconds / num_days as f64),
        "text_including_other_language": fmt_wakatime_duration(cumulative_seconds / num_days as f64),
      }
    }))
  }

  /// All-time total endpoint.
  pub(crate) async fn handle_all_time(
    &self,
    ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    let nodes = self.fetch_heartbeats_in_range(ctx, "all").await?;
    let total_seconds: f64 = nodes.iter().map(|n| node_duration_seconds(n)).sum();
    let end = Utc::now();

    HttpResponse::json(&serde_json::json!({
      "data": {
        "total_seconds": total_seconds as f32,
        "text": fmt_wakatime_duration(total_seconds),
        "is_up_to_date": true,
        "range": {
          "end": end.to_rfc3339(),
          "end_date": end.format("%Y-%m-%d").to_string(),
          "start": "1970-01-01T00:00:00Z",
          "start_date": "1970-01-01",
          "timezone": "UTC",
        }
      }
    }))
  }

  /// Projects list endpoint.
  pub(crate) async fn handle_projects(
    &self,
    ctx: &dyn PluginContext,
    request: &HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    let nodes = self.fetch_heartbeats_in_range(ctx, "all").await?;
    let prefix = request.query_params.get("q").cloned().unwrap_or_default();
    let mut projects: HashMap<String, (String, String)> = HashMap::new();

    for node in &nodes {
      let name = self.field_value(&node, "project");
      if name == "(unknown)" || name.is_empty() {
        continue;
      }
      if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
        continue;
      }
      let ts = node_time_epoch(&node) as i64;
      let last_str = chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();
      projects
        .entry(name.clone())
        .or_insert((last_str.clone(), last_str.clone()));
      // Update last heartbeat if newer
      if ts > 0 {
        if let Some((ref mut current_last, _)) = projects.get_mut(&name) {
          *current_last = last_str;
        }
      }
    }

    let data: Vec<serde_json::Value> = projects
      .into_iter()
      .map(|(name, (last, created))| {
        serde_json::json!({
          "id": name,
          "name": name,
          "last_heartbeat_at": last,
          "human_readable_last_heartbeat_at": last,
          "urlencoded_name": urlencoding(&name),
          "created_at": created,
        })
      })
      .collect();

    HttpResponse::json(&serde_json::json!({ "data": data }))
  }

  /// Status bar endpoint.
  pub(crate) async fn handle_statusbar(
    &self,
    ctx: &dyn PluginContext,
    range: &str,
  ) -> Result<HttpResponse, PluginError> {
    let nodes = self.fetch_heartbeats_in_range(ctx, range).await?;
    let total = nodes.iter().map(|n| node_duration_seconds(n)).sum::<f64>();
    let now = Utc::now().to_rfc3339();
    let (start, end) = parse_time_range(range);

    HttpResponse::json(&serde_json::json!({
      "cached_at": now,
      "data": {
        "categories": [],
        "dependencies": [],
        "editors": [],
        "languages": [],
        "machines": [],
        "operating_systems": [],
        "projects": [],
        "grand_total": grand_total(total),
        "range": {
          "date": end.to_rfc3339(),
          "end": end.to_rfc3339(),
          "start": start.to_rfc3339(),
          "text": range,
          "timezone": "UTC",
        }
      }
    }))
  }

  /// User agents endpoint.
  pub(crate) async fn handle_user_agents(
    &self,
    ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    let nodes = self.fetch_heartbeats_in_range(ctx, "all").await?;
    let mut agents: HashMap<String, (String, String, String, String)> = HashMap::new();

    for node in &nodes {
      let ua = match node.get_field("coding:user_agent") {
        Some(FieldValue::String(s)) => s.clone(),
        _ => continue,
      };
      let editor = self.field_value(&node, "editor");
      let os = self.field_value(&node, "operating_system");
      let ts = node_time_epoch(&node) as i64;
      let seen = chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();

      agents
        .entry(ua.clone())
        .and_modify(|(_, _, ref mut first, ref mut last)| {
          *last = seen.clone();
          if first.is_empty() {
            *first = seen.clone();
          }
        })
        .or_insert_with(|| (editor, os, seen.clone(), seen));
    }

    let data: Vec<serde_json::Value> = agents
      .into_iter()
      .map(|(ua, (editor, os, first, last))| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&ua, &mut h);
        std::hash::Hash::hash(&editor, &mut h);
        let id = format!("{:x}", h.finish());
        serde_json::json!({
          "id": id,
          "editor": editor,
          "os": os,
          "value": ua,
          "version": "",
          "is_browser_extension": false,
          "is_desktop_app": false,
          "first_seen": first,
          "last_seen": last,
        })
      })
      .collect();

    HttpResponse::json(&serde_json::json!({
      "data": data,
      "total_pages": 1,
    }))
  }

  /// User info endpoint.
  pub(crate) async fn handle_user_info(&self) -> Result<HttpResponse, PluginError> {
    HttpResponse::json(&serde_json::json!({
      "data": {
        "id": "current",
        "display_name": "current",
        "full_name": "",
        "email": "",
        "is_email_public": false,
        "is_email_confirmed": false,
        "timezone": "UTC",
        "last_heartbeat_at": Utc::now().to_rfc3339(),
        "last_project": "",
        "last_plugin_name": "",
        "username": "current",
        "website": "",
        "created_at": Utc::now().to_rfc3339(),
        "modified_at": Utc::now().to_rfc3339(),
        "photo": "",
      }
    }))
  }

  /// Leaders endpoint (single-user leaderboard).
  pub(crate) async fn handle_leaders(
    &self,
    ctx: &dyn PluginContext,
    request: &HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    let range = request
      .query_params
      .get("range")
      .cloned()
      .unwrap_or_else(|| "7d".to_string());
    let language = request
      .query_params
      .get("language")
      .cloned()
      .unwrap_or_default();

    let nodes = self.fetch_heartbeats_in_range(ctx, &range).await?;
    let total_seconds: f64 = nodes.iter().map(|n| node_duration_seconds(n)).sum();
    let days = 7.0_f64.max(1.0);
    let daily_average = total_seconds / days;

    let by_lang = self.group_and_sum(&nodes, "language", None);
    let languages: Vec<serde_json::Value> = by_lang
      .into_iter()
      .filter(|(k, _)| language.is_empty() || k == &language)
      .map(|(k, v)| serde_json::json!({"name": k, "total_seconds": v}))
      .collect();

    let (start, end) = parse_time_range(&range);

    HttpResponse::json(&serde_json::json!({
      "current_user": {
        "rank": 1,
        "page": 1,
        "user": {
          "id": "current",
          "display_name": "current",
          "full_name": "",
          "email": "",
          "is_email_public": false,
          "is_email_confirmed": false,
          "timezone": "UTC",
          "last_heartbeat_at": Utc::now().to_rfc3339(),
          "last_project": "",
          "last_plugin_name": "",
          "username": "current",
          "website": "",
          "created_at": Utc::now().to_rfc3339(),
          "modified_at": Utc::now().to_rfc3339(),
          "photo": "",
        }
      },
      "data": [{
        "rank": 1,
        "running_total": {
          "total_seconds": total_seconds,
          "human_readable_total": fmt_wakatime_duration(total_seconds),
          "daily_average": daily_average,
          "human_readable_daily_average": fmt_wakatime_duration(daily_average),
          "languages": languages,
        },
        "user": {
          "id": "current",
          "display_name": "current",
          "full_name": "",
          "email": "",
          "is_email_public": false,
          "is_email_confirmed": false,
          "timezone": "UTC",
          "last_heartbeat_at": Utc::now().to_rfc3339(),
          "last_project": "",
          "last_plugin_name": "",
          "username": "current",
          "website": "",
          "created_at": Utc::now().to_rfc3339(),
          "modified_at": Utc::now().to_rfc3339(),
          "photo": "",
        }
      }],
      "page": 1,
      "total_pages": 1,
      "language": language,
      "range": {
        "end_text": end.format("%a, %d %b %Y").to_string(),
        "end_date": end.to_rfc3339(),
        "start_text": start.format("%a, %d %b %Y").to_string(),
        "start_date": start.to_rfc3339(),
        "name": range,
        "text": range,
      }
    }))
  }

  /// GitHub-style activity chart SVG.
  pub(crate) async fn handle_activity_chart(
    &self,
    ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    use chrono::{Datelike, Duration};
    let now = Utc::now().date_naive();
    let year_ago = now - Duration::days(365);
    let nodes = self.fetch_heartbeats_in_range(ctx, "365d").await?;
    let mut day_totals: HashMap<String, f64> = HashMap::new();
    for n in &nodes {
      let ts = node_time_epoch(n) as i64;
      if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
        let key = dt.format("%Y-%m-%d").to_string();
        *day_totals.entry(key).or_default() += node_duration_seconds(n);
      }
    }
    let max_secs = day_totals
      .values()
      .cloned()
      .fold(0.0_f64, f64::max)
      .max(1.0);
    let cell = 12;
    let gap = 2;
    let cols = 53;
    let rows = 7;
    let header = 30;
    let w = cols * (cell + gap) + 40;
    let h = rows * (cell + gap) + header + 20;
    let hash = "#";
    let mut cells = String::new();
    let mut cur = year_ago;
    let dow = cur.weekday().num_days_from_monday();
    cur -= Duration::days(dow as i64);
    for week in 0..cols {
      for day in 0..rows {
        let d = cur + Duration::days((week * 7 + day) as i64);
        if d > now {
          break;
        }
        let key = d.format("%Y-%m-%d").to_string();
        let secs = day_totals.get(&key).copied().unwrap_or(0.0);
        let level = if secs <= 0.0 {
          0
        } else {
          ((secs / max_secs * 4.0).ceil() as u8).min(4)
        };
        let colors = ["#161b22", "#0e4429", "#006d32", "#26a641", "#39d353"];
        let x = 20 + week * (cell + gap);
        let y = header + day * (cell + gap);
        if d <= now {
          cells.push_str(&format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="2" fill="{}"/>"#,
            x, y, cell, cell, colors[level as usize],
          ));
        }
      }
    }
    let svg = format!(
      r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><style>text{{font-family:sans-serif;font-size:10px;fill:{hash}8b949e;}}</style><text x="20" y="20">Activity</text>{cells}</svg>"#
    );
    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "image/svg+xml".into());
    Ok(HttpResponse {
      status: 200,
      headers,
      body: bytes::Bytes::from(svg),
    })
  }

  /// SVG badge endpoint.
  pub(crate) async fn handle_badge(
    &self,
    ctx: &dyn PluginContext,
    _endpoint: &str,
    request: &HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    let interval = request
      .query_params
      .get("interval")
      .cloned()
      .unwrap_or_else(|| "7d".to_string());
    let label = request
      .query_params
      .get("label")
      .cloned()
      .unwrap_or_else(|| "coding time".to_string());
    let color = request
      .query_params
      .get("color")
      .cloned()
      .unwrap_or_else(|| "brightgreen".to_string());

    let nodes = self.fetch_heartbeats_in_range(ctx, &interval).await?;
    let total: f64 = nodes.iter().map(|n| node_duration_seconds(n)).sum();
    let text = fmt_wakatime_duration(total);

    // Generate a simple Shields.io-style SVG badge
    let label_width = label.len() as f64 * 6.0 + 10.0;
    let value_width = text.len() as f64 * 6.0 + 10.0;
    let total_width = label_width + value_width;

    let hash = "#";
    let svg = format!(
      r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="20">
  <linearGradient id="b" x2="0" y2="100%">
    <stop offset="0" stop-color="{hash}bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <mask id="a">
    <rect width="{}" height="20" rx="3" fill="{hash}fff"/>
  </mask>
  <g mask="url({hash}a)">
    <path fill="{hash}555" d="M0 0h{}v20H0z"/>
    <path fill="{}" d="M{} 0h{}v20H{}z"/>
    <path fill="url({hash}b)" d="M0 0h{}v20H0z"/>
  </g>
  <g fill="{hash}fff" text-anchor="middle" font-family="DejaVu Sans,Verdana,Geneva,sans-serif" font-size="11">
    <text x="{}" y="15" fill="{hash}010101" fill-opacity=".3">{}</text>
    <text x="{}" y="14">{}</text>
    <text x="{}" y="15" fill="{hash}010101" fill-opacity=".3">{}</text>
    <text x="{}" y="14">{}</text>
  </g>
</svg>"#,
      total_width,
      total_width,
      label_width,
      color,
      label_width,
      value_width,
      label_width,
      total_width,
      label_width / 2.0,
      label,
      label_width / 2.0,
      label,
      label_width + value_width / 2.0,
      text,
      label_width + value_width / 2.0,
      text,
    );

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "image/svg+xml".into());
    Ok(HttpResponse {
      status: 200,
      headers,
      body: bytes::Bytes::from(svg),
    })
  }

  /// Shields.io JSON badge endpoint.
  pub(crate) async fn handle_shields_badge(
    &self,
    ctx: &dyn PluginContext,
    parts: &[&str],
    request: &HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    // parts: [compat, shields, v1, {user}, {interval}, {filter}]
    let interval = parts.get(3).copied().unwrap_or("7d");
    let label = request
      .query_params
      .get("label")
      .cloned()
      .unwrap_or_else(|| "coding time".to_string());

    let nodes = self.fetch_heartbeats_in_range(ctx, interval).await?;
    let total: f64 = nodes.iter().map(|n| node_duration_seconds(n)).sum();
    let text = fmt_wakatime_duration(total);

    HttpResponse::json(&serde_json::json!({
      "schemaVersion": 1,
      "label": label,
      "message": text,
      "color": "brightgreen",
    }))
  }
}

fn urlencoding(s: &str) -> String {
  // Simple URL encoding for project names
  s.replace(' ', "%20")
    .replace('/', "%2F")
    .replace('#', "%23")
    .replace('&', "%26")
}
