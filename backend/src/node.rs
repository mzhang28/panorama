use std::collections::BTreeMap;

use axum::{
  extract::{Path, State},
  Json,
};
use serde_json::{json, Value as JsonValue};
use sqlx::{Column, Row, TypeInfo};

use crate::context::Context;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RecentNodesResponse {
  nodes: Vec<JsonValue>,
}

pub async fn recent(State(ctx): State<Context>) -> Json<RecentNodesResponse> {
  let rows = sqlx::query("select id from node order by last_updated_at desc limit 5")
    .fetch_all(&ctx.db)
    .await
    .unwrap();

  let nodes = rows
    .into_iter()
    .map(|row| {
      json!({
        "id": row.get::<String,_>("id"),
      })
    })
    .collect();
  Json(RecentNodesResponse { nodes })
}

pub async fn fetch(
  State(ctx): State<Context>,
  Path(id): Path<String>,
) -> Json<Option<BTreeMap<String, JsonValue>>> {
  let row = sqlx::query("select * from node where id = ?")
    .bind(&id)
    .fetch_one(&ctx.db)
    .await
    .unwrap();

  let columns = row
    .columns()
    .into_iter()
    .map(|c| {
      let cname = c.name().to_owned();
      let value: JsonValue = match c.type_info().name() {
        "NULL" => JsonValue::Null,
        "TEXT" => serde_json::to_value(row.get::<Option<String>, _>(&*cname)).unwrap(),
        "INTEGER" => serde_json::to_value(row.get::<Option<i64>, _>(&*cname)).unwrap(),
        name => todo!("wtf? {name}"),
      };
      (cname, value)
    })
    .filter(|(c, v)| !v.is_null())
    .collect::<BTreeMap<_, _>>();

  Json(Some(columns))
}
