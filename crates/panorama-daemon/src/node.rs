use axum::{
  extract::{Path, State},
  http::StatusCode,
  Json,
};
use cozo::ScriptMutability;
use serde_json::Value;

use crate::{error::AppResult, AppState};

pub async fn get_node(
  State(state): State<AppState>,
  Path(node_id): Path<String>,
) -> AppResult<(StatusCode, Json<Value>)> {
  let result = state.db.run_script(
    "
    j[content] := *journal{ node_id, content }, node_id = $node_id
    j[content] := not *journal{ node_id }, node_id = $node_id, content = null

    jd[day] := *journal_days{ node_id, day }, node_id = $node_id
    jd[day] := not *journal_days{ node_id }, node_id = $node_id, day = null

    ?[
      extra_data, content, day, created_at, updated_at, type
    ] := *node{ id, type, created_at, updated_at, extra_data },
      j[content],
      jd[day],
      id = $node_id
    :limit 1
  ",
    btmap! {"node_id".to_owned() => node_id.clone().into()},
    ScriptMutability::Immutable,
  )?;

  if result.rows.len() == 0 {
    return Ok((StatusCode::NOT_FOUND, Json(json!(null))));
  }

  let row = &result.rows[0];
  let extra_data = row[0].get_str();
  let day = row[2].get_str();

  Ok((
    StatusCode::OK,
    Json(json!({
      "node": node_id,
      "extra_data": extra_data,
      "content": row[1].get_str(),
      "day": day,
      "created_at": row[3].get_float(),
      "updated_at": row[4].get_float(),
      "type": row[5].get_str(),
    })),
  ))
}

#[derive(Deserialize)]
struct NodeUpdate {}

pub async fn update_node(
  State(state): State<AppState>,
  Path(node_id): Path<String>,
) -> AppResult<Json<Value>> {
  Ok(Json(json!({})))
}
