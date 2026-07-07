use chrono::{DateTime, Duration, NaiveDate, Utc};
use panorama_core::*;

/// Parse a time range string into (start, end) DateTime<Utc>.
///
/// Supported formats:
/// - `24h`, `7d`, `30d`, `90d`, `365d`, `all` — relative to now
/// - `YYYY-MM-DD` — single day
/// - `YYYY-MM-DD..YYYY-MM-DD` — explicit range
pub(crate) fn parse_time_range(range: &str) -> (DateTime<Utc>, DateTime<Utc>) {
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

/// Check if a node's time falls within [start, end].
pub(crate) fn node_time_in_range(node: &Node, start: DateTime<Utc>, end: DateTime<Utc>) -> bool {
  let ts = node_time_epoch(node);
  let start_ts = start.timestamp() as f64;
  let end_ts = end.timestamp() as f64;
  ts >= start_ts && ts <= end_ts
}

/// Extract the epoch timestamp from a node in seconds.
pub(crate) fn node_time_epoch(node: &Node) -> f64 {
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
pub(crate) fn node_duration_seconds(node: &Node) -> f64 {
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
pub(crate) fn heartbeat_is_ai(hb: &serde_json::Value) -> bool {
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

/// Format seconds as a WakaTime-style human readable duration.
/// Rounds to nearest minute. Examples:
/// - 12345.0 → "3 hrs 25 mins"
/// - 3600.0  → "1 hr 0 mins"
/// - 60.0    → "1 min"
/// - 30.0    → "0 mins"
pub(crate) fn fmt_wakatime_duration(total_seconds: f64) -> String {
  let total_minutes = (total_seconds / 60.0).round() as i64;
  let hours = total_minutes / 60;
  let minutes = total_minutes % 60;
  if hours > 0 {
    format!(
      "{} hr{} {} min{}",
      hours,
      if hours == 1 { "" } else { "s" },
      minutes,
      if minutes == 1 { "" } else { "s" }
    )
  } else {
    format!("{} min{}", minutes, if minutes == 1 { "" } else { "s" })
  }
}

/// Format seconds as a digital time string like "1:30:15".
pub(crate) fn fmt_digital(total_seconds: f64) -> String {
  let total = total_seconds as i64;
  let hours = total / 3600;
  let minutes = (total % 3600) / 60;
  let seconds = total % 60;
  format!("{}:{:02}:{:02}", hours, minutes, seconds)
}
