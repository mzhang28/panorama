use chrono::DateTime;
use panorama_core::*;

use crate::helpers::*;
use crate::CodingPlugin;

impl CodingPlugin {
  /// Compute durations from heartbeats for a given date.
  /// Groups consecutive heartbeats within 15 minutes of each other.
  pub(crate) async fn compute_durations(
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

  pub(crate) fn build_duration_entry(
    &self,
    start: &Node,
    end: &Node,
    hb_count: u32,
  ) -> serde_json::Value {
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
