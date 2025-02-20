use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::{Execute, Row};

use crate::context::Context;

pub struct SummaryData {}

pub async fn statusbar() -> StatusCode {
  StatusCode::INTERNAL_SERVER_ERROR
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatsData {
  time: f64,
  project: Option<String>,
  language: Option<String>,

  #[serde(flatten)]
  extra: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BulkHeartbeatsData {
  Single(HeartbeatsData),
  Many(Vec<HeartbeatsData>),
}

pub async fn bulk_heartbeats(
  State(ctx): State<Context>,
  Json(data): Json<BulkHeartbeatsData>,
) -> StatusCode {
  println!("{:?}", data);
  // let data: BulkHeartbeatsData = serde_json::from_value(data).unwrap();

  let heartbeats = match data {
    BulkHeartbeatsData::Single(record) => vec![record],
    BulkHeartbeatsData::Many(records) => records,
  };

  let mut ids = vec![];
  for heartbeat in heartbeats.into_iter() {
    let time = {
      let secs = heartbeat.time.trunc() as i64;
      let nsecs = (heartbeat.time.fract() * 1e9) as u32;
      DateTime::<Utc>::from_timestamp(secs, nsecs)
    };

    let query = sqlx::query(
      r#"insert into "node"
      ("cal_date", "wakatime_project", "wakatime_language", "json")
      values (? , ? , ? , ?)
      returning id
    "#,
    )
    .bind(time.map(|s| s.to_rfc3339()))
    .bind(&heartbeat.project)
    .bind(&heartbeat.language)
    .bind(serde_json::to_string(&heartbeat).unwrap());

    println!("Query:{:?}", query.sql());
    let row = query.fetch_one(&ctx.db).await.unwrap();

    let id: String = row.get(0);
    ids.push(id);
  }

  println!("Pushed ids: {:?}", ids);
  StatusCode::INTERNAL_SERVER_ERROR
}
