use axum::{extract::State, Json};
use chrono::Local;
use cozo::{DataValue, ScriptMutability};
use serde_json::Value;
use uuid::Uuid;

use crate::{ensure_ok, error::AppResult, AppState};

pub async fn get_todays_journal_id(
  State(state): State<AppState>,
) -> AppResult<Json<Value>> {
  let today = todays_date();
  println!("Getting journal id for {today}!");

  let result = state.db.run_script(
    "
      ?[node_id] := *journal_days[day, node_id], day = $day
    ",
    btmap! {
      "day".to_owned() => today.clone().into(),
    },
    ScriptMutability::Immutable,
  )?;

  println!("Result: {:?}", result);

  if result.rows.len() == 0 {
    // Insert a new one
    let uuid = Uuid::now_v7();
    let node_id = uuid.to_string();

    let result = state.db.run_script_fold_err(
      "
      {
        ?[id] <- [[$node_id]]
        :put node { id }
      }
      {
        ?[node_id, plaintext] <- [[$node_id, '']]
        :put journal { node_id => plaintext }
      }
      {
        ?[day, node_id] <- [[$day, $node_id]]
        :put journal_days { day => node_id }
      }
    ",
      btmap! {
        "node_id".to_owned() => node_id.into(),
        "day".to_owned() => today.clone().into(),
      },
      ScriptMutability::Mutable,
    );
  }

  let node_id = result.rows[0][0].get_str().unwrap();
  Ok(Json(json!({
    "node_id": node_id
  })))
}

fn todays_date() -> String {
  let now = Local::now();
  let date = now.date_naive();
  date.format("%Y-%m-%d").to_string()
}
