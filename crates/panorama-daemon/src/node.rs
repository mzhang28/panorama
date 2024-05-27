use std::collections::{BTreeMap, HashMap};

use axum::{
  extract::{Path, Query, State},
  http::StatusCode,
  Json,
};
use cozo::{DataValue, DbInstance, MultiTransaction, ScriptMutability, Vector};
use itertools::Itertools;
use serde_json::Value;
use uuid::Uuid;

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
  extra_data: Option<ExtraData>,
}

pub async fn update_node(
  State(state): State<AppState>,
  Path(node_id): Path<String>,
  Json(update_data): Json<UpdateData>,
) -> AppResult<Json<Value>> {
  println!("Update data: {:?}", update_data);
  let node_id_data = DataValue::from(node_id.clone());

  // TODO: Combine these into the same script

  let tx = state.db.multi_transaction(true);

  if let Some(title) = update_data.title {
    let title = DataValue::from(title);

    tx.run_script(
      "
    # Always update the time
    ?[ id, title ] <- [[ $node_id, $title ]]
    :update node { id, title }
  ",
      btmap! {
        "node_id".to_owned() => node_id_data.clone(),
        "title".to_owned() => title,
      },
    )?;
  }

  if let Some(extra_data) = update_data.extra_data {
    let result = get_rows_for_extra_keys(&tx, &extra_data)?;

    for (key, (relation, field_name, ty)) in result.iter() {
      let new_value = extra_data.get(key).unwrap();

      // TODO: Make this more generic
      let new_value = DataValue::from(new_value.as_str().unwrap());

      let query = format!(
        "
          ?[ node_id, {field_name} ] <- [[$node_id, $input_data]]
          :update {relation} {{ node_id, {field_name} }}
        "
      );

      let result = tx.run_script(
        &query,
        btmap! {
          "node_id".to_owned() => node_id_data.clone(),
          "input_data".to_owned() => new_value,
        },
      )?;
    }
  }

  tx.run_script(
    "
    # Always update the time
    ?[ id, updated_at ] <- [[ $node_id, now() ]]
    :update node { id, updated_at }
  ",
    btmap! {
      "node_id".to_owned() => node_id_data,
    },
  )?;

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

#[derive(Debug, Deserialize)]
pub struct CreateNodeOpts {
  // TODO: Allow submitting a string
  // id: Option<String>,
  #[serde(rename = "type")]
  ty: String,
  extra_data: Option<ExtraData>,
}

pub async fn create_node(
  State(state): State<AppState>,
  Json(opts): Json<CreateNodeOpts>,
) -> AppResult<Json<Value>> {
  let node_id = Uuid::now_v7();
  let node_id = node_id.to_string();
  println!("Opts: {opts:?}");

  let tx = state.db.multi_transaction(true);

  let result = tx.run_script(
    "
    ?[id, type] <- [[$node_id, $type]]
    :put node { id, type }
  ",
    btmap! {
      "node_id".to_owned() => DataValue::from(node_id.clone()),
      "type".to_owned() => DataValue::from(opts.ty),
    },
  );

  if let Some(extra_data) = opts.extra_data {
    let result = get_rows_for_extra_keys(&tx, &extra_data)?;

    let result_by_relation = result
      .iter()
      .into_group_map_by(|(key, (relation, field_name, ty))| relation);
    println!("Result by relation: {result_by_relation:?}");

    for (relation, fields) in result_by_relation.iter() {
      let fields_mapping = fields
        .into_iter()
        .map(|(key, (_, field_name, _))| {
          let new_value = extra_data.get(*key).unwrap();
          // TODO: Make this more generic
          let new_value = DataValue::from(new_value.as_str().unwrap());
          (field_name.to_owned(), new_value)
        })
        .collect::<BTreeMap<_, _>>();

      let keys = fields_mapping.keys().collect::<Vec<_>>();
      let keys_joined = keys.iter().join(", ");

      let query = format!(
        "
          ?[ node_id, {keys_joined} ] <- [$input_data]
          :insert {relation} {{ node_id, {keys_joined} }}
        "
      );

      let mut params = vec![];
      params.push(DataValue::from(node_id.clone()));
      for key in keys {
        params.push(fields_mapping[key].clone());
      }

      let result = tx.run_script(
        &query,
        btmap! {
          "input_data".to_owned() => DataValue::List(params),
        },
      )?;
    }
  }

  tx.commit()?;

  Ok(Json(json!({
    "node_id": node_id,
  })))
}

#[derive(Deserialize)]
pub struct SearchQuery {
  query: String,
}

pub async fn search_nodes(
  State(state): State<AppState>,
  Query(query): Query<SearchQuery>,
) -> AppResult<Json<Value>> {
  // TODO: This is temporary, there may be more ways to search so tacking on *
  // at the end may destroy some queries
  let query = format!("{}*", query.query);

  let results = state.db.run_script(
    "
      results[node_id, content, score] := ~journal:text_index {node_id, content, |
        query: $q,
        k: 10,
        score_kind: 'tf_idf',
        bind_score: score
      }

      ?[node_id, content, title, score] :=
        results[node_id, content, score],
        *node{ id: node_id, title }

      :order -score
      ",
    btmap! {
      "q".to_owned() => DataValue::from(query),
    },
    ScriptMutability::Immutable,
  )?;

  let results = results
    .rows
    .into_iter()
    .map(|row| {
      json!({
        "node_id": row[0].get_str().unwrap(),
        "content": row[1].get_str().unwrap(),
        "title": row[2].get_str().unwrap(),
        "score": row[3].get_float().unwrap(),
      })
    })
    .collect::<Vec<_>>();

  Ok(Json(json!({
    "results": results
  })))
}

type ExtraData = HashMap<String, Value>;

fn get_rows_for_extra_keys(
  tx: &MultiTransaction,
  extra_data: &ExtraData,
) -> AppResult<HashMap<String, (String, String, String)>> {
  let result = tx.run_script(
    "
      ?[key, relation, field_name, type] :=
        *fqkey_to_dbkey{key, relation, field_name, type},
        is_in(key, $keys)
    ",
    btmap! {
      "keys".to_owned() => DataValue::List(
        extra_data
          .keys()
          .map(|s| DataValue::from(s.as_str()))
          .collect::<Vec<_>>()
      ),
    },
  )?;

  let s = |s: &DataValue| s.get_str().unwrap().to_owned();

  Ok(
    result
      .rows
      .into_iter()
      .map(|row| (s(&row[0]), (s(&row[1]), s(&row[2]), s(&row[3]))))
      .collect::<HashMap<_, _>>(),
  )
}
