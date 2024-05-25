use axum::{
  extract::{Path, State},
  Json,
};
use cozo::ScriptMutability;
use serde_json::Value;

use crate::{error::AppResult, AppState};

pub async fn get_node(
  State(state): State<AppState>,
  Path(node_id): Path<String>,
) -> AppResult<Json<Value>> {
  let result = state.db.run_script(
    "
    j[plaintext] := *journal{ node_id, plaintext }, node_id = $node_id
    j[plaintext] := not *journal{ node_id }, node_id = $node_id, plaintext = null

    jd[day] := *journal_days{ node_id, day }, node_id = $node_id
    jd[day] := not *journal_days{ node_id }, node_id = $node_id, day = null

    ?[extra_data, plaintext, day] := *node{ id, extra_data },
      j[plaintext],
      jd[day],
      id = $node_id
    :limit 1
  ",
    btmap! {"node_id".to_owned() => node_id.clone().into()},
    ScriptMutability::Immutable,
  )?;

  let row = &result.rows[0];
  let extra_data = row[0].get_str();
  let plaintext = row[1].get_str();
  let day = row[2].get_str();

  Ok(Json(json!({
    "node": node_id,
    "extra_data": extra_data,
    "plaintext": plaintext,
    "day": day,
  })))
}
