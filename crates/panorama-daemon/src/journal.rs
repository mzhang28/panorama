use axum::{extract::State, Json};
use chrono::Local;
use cozo::ScriptMutability;
use serde_json::Value;
use uuid::Uuid;

use crate::{error::AppResult, AppState};

pub async fn get_todays_journal_id(
  State(state): State<AppState>,
) -> AppResult<Json<Value>> {
  let today = todays_date();

  let result = state.db.run_script(
    "
      ?[node_id] := *journal_day[day, node_id], day = $day
    ",
    btmap! {
      "day".to_owned() => today.clone().into(),
    },
    ScriptMutability::Immutable,
  )?;

  // TODO: Do this check on the server side
  if result.rows.len() == 0 {
    // Insert a new one
    let uuid = Uuid::now_v7();
    let node_id = uuid.to_string();

    state.db.run_script(
      "
      {
        ?[id, title, type] <- [[$node_id, $title, 'panorama/journal/page']]
        :put node { id, title, type }
      }
      {
        ?[node_id, content] <- [[$node_id, '']]
        :put journal { node_id => content }
      }
      {
        ?[day, node_id] <- [[$day, $node_id]]
        :put journal_day { day => node_id }
      }
    ",
      btmap! {
        "node_id".to_owned() => node_id.clone().into(),
        "day".to_owned() => today.clone().into(),
        "title".to_owned() => today.clone().into(),
      },
      ScriptMutability::Mutable,
    )?;

    return Ok(Json(json!({
      "node_id": node_id
    })));
  }

  let node_id = result.rows[0][0].get_str().unwrap();
  Ok(Json(json!({
    "node_id": node_id,
    "day": today,
  })))
}

fn todays_date() -> String {
  let now = Local::now();
  let date = now.date_naive();
  date.format("%Y-%m-%d").to_string()
}
