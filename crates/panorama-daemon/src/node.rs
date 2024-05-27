use std::collections::HashMap;

use axum::{
  extract::{Path, State},
  http::StatusCode,
  Json,
};
use cozo::{DataValue, ScriptMutability};
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

    jd[day] := *journal_day{ node_id, day }, node_id = $node_id
    jd[day] := not *journal_day{ node_id }, node_id = $node_id, day = null

    ?[
      extra_data, content, day, created_at, updated_at, type, title
    ] := *node{ id, type, title, created_at, updated_at, extra_data },
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
      "title": row[6].get_str(),
    })),
  ))
}

#[derive(Deserialize, Debug)]
pub struct UpdateData {
  title: Option<String>,
  extra_data: Option<HashMap<String, Value>>,
}

pub async fn update_node(
  State(state): State<AppState>,
  Path(node_id): Path<String>,
  Json(update_data): Json<UpdateData>,
) -> AppResult<Json<Value>> {
  println!("Update data: {:?}", update_data);

  let tx = state.db.multi_transaction(true);

  if let Some(extra_data) = update_data.extra_data {
    let result = tx.run_script(
      "
      ?[relation, field_name, type] :=
        *fqkey_to_dbkey{key, relation, field_name, type},
        key = $key
    ",
      btmap! {
        "key".to_owned() => DataValue::List(
          extra_data
            .keys()
            .map(|s| DataValue::from(s.as_str()))
            .collect::<Vec<_>>()
        ),
      },
    )?;

    println!("Result: {result:?}");
  }

  tx.commit()?;

  Ok(Json(json!({})))
}

pub async fn node_types() -> AppResult<Json<Value>> {
  Ok(Json(json!({
    "types": [
      { "id": "panorama/journal/page", "display": "Journal Entry" },
    ]
  })))
}
