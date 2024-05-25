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
    ?[extra_data] := *node{ id, extra_data }, id = $node_id
  ",
    btmap! {"node_id".to_owned() => node_id.clone().into()},
    ScriptMutability::Immutable,
  )?;

  println!("REUSLT {:?}", result);

  Ok(Json(json!({
    "node": node_id,
  })))
}
