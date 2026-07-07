use chrono::{DateTime, Datelike, Utc};
use panorama_core::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

use crate::helpers::*;
use crate::CodingPlugin;

/// Parameters for a stats query.
#[derive(Debug, Deserialize)]
pub(crate) struct StatsQuery {
  /// Time range: `24h`, `7d`, `30d`, `90d`, `all`, or custom
  /// `YYYY-MM-DD..YYYY-MM-DD`.
  #[serde(default = "default_range")]
  pub(crate) range: String,
  /// Dimension to group by: `project`, `language`, `entity`, `category`,
  /// `date`, `hour`, `machine`, `branch`, `editor`, `operating_system`.
  #[serde(default)]
  pub(crate) group_by: Option<String>,
  /// Sub-dimension for two-level grouping (e.g. group_by=project,
  /// sub_group_by=language).
  #[serde(default)]
  pub(crate) sub_group_by: Option<String>,
  /// Aggregation: `sum` (total seconds), `count` (heartbeat count),
  /// `leaderboard` (sum + sort desc), `avg_daily`, `timeseries`.
  #[serde(default = "default_aggregation")]
  pub(crate) aggregation: String,
  /// Optional filter: `project=panorama`, `language=Rust`, `category=coding`.
  /// Supports comma-separated OR: `project=a,b` matches a or b.
  #[serde(default)]
  pub(crate) filter: Option<String>,
  /// For timeseries: bucket size. `hour`, `day`, `week`. Default: `day`.
  #[serde(default = "default_bucket")]
  pub(crate) bucket: String,
  /// Limit results (for leaderboard). Default: 25.
  #[serde(default = "default_limit")]
  pub(crate) limit: usize,
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
  /// Execute a stats query against stored heartbeats.
  pub(crate) async fn execute_stats(
    &self,
    ctx: &dyn PluginContext,
    query: &StatsQuery,
  ) -> Result<serde_json::Value, PluginError> {
    let all_nodes = self.fetch_heartbeats_in_range(ctx, &query.range).await?;

    if all_nodes.is_empty() {
      return Ok(serde_json::json!([]));
    }

    // Apply field filter if present
    let filtered = self.apply_filter(&all_nodes, &query.filter);

    // Build grouping key function
    let group_by = query.group_by.as_deref().unwrap_or("project");
    let sub_group = query.sub_group_by.as_deref();

    match query.aggregation.as_str() {
      "leaderboard" => self.compute_leaderboard(&filtered, group_by, sub_group, query.limit),
      "sum" => self.compute_sum(&filtered, group_by, sub_group),
      "count" => self.compute_count(&filtered, group_by, sub_group, query.limit),
      "avg_daily" => self.compute_avg_daily(&filtered, group_by, &query.range),
      "timeseries" => self.compute_timeseries(&filtered, group_by, &query.bucket, &query.range),
      _ => Err(PluginError::bad_request(&format!(
        "Unknown aggregation: {}. Supported: leaderboard, sum, count, avg_daily, timeseries",
        query.aggregation
      ))),
    }
  }

  /// Fetch all heartbeat nodes within a time range.
  pub(crate) async fn fetch_heartbeats_in_range(
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

  /// Apply a filter to nodes. Supports comma-separated OR values.
  pub(crate) fn apply_filter(&self, nodes: &[Node], filter: &Option<String>) -> Vec<Node> {
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

    // Support comma-separated OR values
    let values: Vec<&str> = value.split(',').map(|s| s.trim()).collect();

    nodes
      .iter()
      .filter(|n| {
        n.get_field(&field_key)
          .map(|v| match v {
            FieldValue::String(s) => values.iter().any(|val| s == val),
            _ => false,
          })
          .unwrap_or(false)
      })
      .cloned()
      .collect()
  }

  /// Leaderboard: group by dimension, sum durations, sort descending.
  pub(crate) fn compute_leaderboard(
    &self,
    nodes: &[Node],
    group_by: &str,
    sub_group_by: Option<&str>,
    limit: usize,
  ) -> Result<serde_json::Value, PluginError> {
    let grouped = self.group_and_sum(nodes, group_by, sub_group_by);
    let mut entries: Vec<serde_json::Value> = grouped
      .into_iter()
      .map(|(key, seconds)| {
        let hours = seconds / 3600.0;
        if let Some(sg) = sub_group_by {
          // key is "primary::secondary"
          let parts: Vec<&str> = key.splitn(2, "::").collect();
          serde_json::json!({
              group_by: parts.first().copied().unwrap_or(""),
              sg: parts.get(1).copied().unwrap_or(""),
              "total_seconds": seconds,
              "hours": (hours * 10.0).round() / 10.0,
          })
        } else {
          serde_json::json!({
              "key": key,
              "total_seconds": seconds,
              "hours": (hours * 10.0).round() / 10.0,
          })
        }
      })
      .collect();

    // Sort descending by hours
    entries.sort_by(|a, b| {
      b["total_seconds"]
        .as_f64()
        .unwrap_or(0.0)
        .partial_cmp(&a["total_seconds"].as_f64().unwrap_or(0.0))
        .unwrap_or(std::cmp::Ordering::Equal)
    });

    if entries.len() > limit {
      entries.truncate(limit);
    }

    Ok(serde_json::json!(entries))
  }

  /// Sum aggregation.
  pub(crate) fn compute_sum(
    &self,
    nodes: &[Node],
    group_by: &str,
    sub_group_by: Option<&str>,
  ) -> Result<serde_json::Value, PluginError> {
    let grouped = self.group_and_sum(nodes, group_by, sub_group_by);
    let result: BTreeMap<String, f64> = grouped.into_iter().collect();
    Ok(serde_json::json!(result))
  }

  /// Count aggregation.
  pub(crate) fn compute_count(
    &self,
    nodes: &[Node],
    group_by: &str,
    sub_group_by: Option<&str>,
    limit: usize,
  ) -> Result<serde_json::Value, PluginError> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for node in nodes {
      let key = self.group_key(node, group_by, sub_group_by);
      *counts.entry(key).or_default() += 1;
    }
    let mut entries: Vec<serde_json::Value> = counts
      .into_iter()
      .map(|(key, count)| serde_json::json!({"key": key, "count": count}))
      .collect();
    entries.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));
    if entries.len() > limit {
      entries.truncate(limit);
    }
    Ok(serde_json::json!(entries))
  }

  /// Average daily duration.
  pub(crate) fn compute_avg_daily(
    &self,
    nodes: &[Node],
    group_by: &str,
    range: &str,
  ) -> Result<serde_json::Value, PluginError> {
    let (start, end) = parse_time_range(range);
    let days = (end - start).num_days().max(1) as f64;

    let grouped = self.group_and_sum(nodes, group_by, None);
    let result: Vec<serde_json::Value> = grouped
      .into_iter()
      .map(|(key, total_seconds)| {
        serde_json::json!({
            "key": key,
            "avg_seconds_per_day": total_seconds / days,
            "avg_hours_per_day": total_seconds / days / 3600.0,
        })
      })
      .collect();
    Ok(serde_json::json!(result))
  }

  /// Time-series: bucketed data points over the time range.
  pub(crate) fn compute_timeseries(
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
  pub(crate) fn fill_time_gaps(
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

  /// Group nodes by dimension(s) and sum their durations.
  pub(crate) fn group_and_sum(
    &self,
    nodes: &[Node],
    group_by: &str,
    sub_group_by: Option<&str>,
  ) -> HashMap<String, f64> {
    let mut groups: HashMap<String, f64> = HashMap::new();
    for node in nodes {
      let key = self.group_key(node, group_by, sub_group_by);
      let duration = node_duration_seconds(node);
      *groups.entry(key).or_default() += duration;
    }
    groups
  }

  /// Build a grouping key for a node.
  pub(crate) fn group_key(
    &self,
    node: &Node,
    group_by: &str,
    sub_group_by: Option<&str>,
  ) -> String {
    let primary = self.field_value(node, group_by);
    if let Some(sg) = sub_group_by {
      let secondary = self.field_value(node, sg);
      format!("{}::{}", primary, secondary)
    } else {
      primary
    }
  }

  /// Extract a field value as a string for grouping.
  pub(crate) fn field_value(&self, node: &Node, field: &str) -> String {
    // Handle "date" and "hour" as computed group dimensions
    match field {
      "date" => {
        let ts = node_time_epoch(node) as i64;
        return DateTime::from_timestamp(ts, 0)
          .map(|d| d.format("%Y-%m-%d").to_string())
          .unwrap_or_else(|| "(unknown)".to_string());
      }
      "hour" => {
        let ts = node_time_epoch(node) as i64;
        return DateTime::from_timestamp(ts, 0)
          .map(|d| d.format("%Y-%m-%dT%H:00:00Z").to_string())
          .unwrap_or_else(|| "(unknown)".to_string());
      }
      _ => {}
    }

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

  /// Compute the best day (date with max total seconds).
  pub(crate) fn compute_best_day(&self, nodes: &[Node]) -> Option<(String, f64)> {
    let mut days: HashMap<String, f64> = HashMap::new();
    for node in nodes {
      let ts = node_time_epoch(node) as i64;
      let date = DateTime::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
      *days.entry(date).or_default() += node_duration_seconds(node);
    }
    days
      .into_iter()
      .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
  }
}
