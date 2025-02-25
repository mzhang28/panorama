use std::collections::BTreeMap;

use axum::{
  Json,
  extract::{Path, State},
};
use serde_json::{Value as JsonValue, json};
use sqlx::{Column, QueryBuilder, Row, TypeInfo};

use crate::context::Context;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RecentNodesResponse {
  nodes: Vec<JsonValue>,
}

pub async fn recent(State(ctx): State<Context>) -> Json<RecentNodesResponse> {
  let rows = sqlx::query(
    "select id, title, last_updated_at from node order by last_updated_at desc limit 20",
  )
  .fetch_all(&ctx.db)
  .await
  .unwrap();

  let nodes = rows
    .into_iter()
    .map(|row| {
      json!({
        "id": row.get::<String,_>("id"),
        "title": row.get::<Option<String>, _>("title"),
        "last_updated_at": row.get::<String, _>("last_updated_at"),
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

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CreateNodeResponse {
  id: String,
}

pub async fn create(
  State(ctx): State<Context>,
  Json(payload): Json<BTreeMap<String, Option<String>>>,
) -> Json<CreateNodeResponse> {
  // TODO: Have a stricter schema for the payload
  // based on the node definition separately

  // Fetch column names from the db
  let node_columns = sqlx::query("pragma table_info(node)")
    .fetch_all(&ctx.db)
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.get::<String, _>("name"))
    .collect::<Vec<_>>();

  let payload = payload
    .into_iter()
    .filter(|(c, _)| node_columns.contains(c))
    .filter(|(c, _)| !["id", "created_at", "last_updated_at"].contains(&c.as_str()))
    .filter_map(|(c, v)| v.map(|v| (c, v)))
    .collect::<Vec<_>>();

  let mut builder = QueryBuilder::new("insert into node (");
  {
    let mut sep = builder.separated(", ");
    for (cname, _) in payload.iter() {
      sep.push(cname);
    }
  }
  builder.push(") values (");
  {
    let mut sep = builder.separated(",");
    for (_, value) in payload.iter() {
      sep.push_bind(value);
    }
  }
  builder.push(") returning id");

  info!(query = builder.sql(), "Runnign query");

  let results = builder.build().fetch_one(&ctx.db).await.unwrap();

  Json(CreateNodeResponse {
    id: results.get::<String, _>("id"),
  })
}

pub async fn update(
  State(ctx): State<Context>,
  Path(id): Path<String>,
  Json(payload): Json<BTreeMap<String, Option<String>>>,
) -> Json<()> {
  // TODO: Have a stricter schema for the payload
  // based on the node definition separately

  // Fetch column names from the db
  let node_columns = sqlx::query("pragma table_info(node)")
    .fetch_all(&ctx.db)
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.get::<String, _>("name"))
    .collect::<Vec<_>>();

  let payload = payload
    .into_iter()
    .filter(|(c, _)| node_columns.contains(c))
    .filter(|(c, _)| !["id", "created_at", "last_updated_at"].contains(&c.as_str()))
    .filter_map(|(c, v)| v.map(|v| (c, v)))
    .collect::<Vec<_>>();

  let mut builder = QueryBuilder::new("update node set ");
  {
    for (cname, value) in payload.iter() {
      builder.push(cname);
      builder.push(" = ");
      builder.push_bind(value);
    }
  }
  builder.push(" where id = ");
  builder.push_bind(&id);

  info!(query = builder.sql(), "Runnign query");

  let results = builder.build().execute(&ctx.db).await.unwrap();

  Json(())
}
