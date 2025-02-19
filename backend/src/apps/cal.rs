use std::collections::BTreeMap;

use axum::extract::{Multipart, State};
use axum::Json;
use chrono::{DateTime, Utc};
use icalendar::{Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, Event};
use serde_json::{json, Value as JsonValue};
use sqlx::Row;

use crate::graphql::Context;

pub async fn ics_upload(State(ctx): State<Context>, mut multipart: Multipart) {
  println!("multipart: {:?}", multipart);

  let field = multipart.next_field().await.unwrap().unwrap();
  let data = field.bytes().await.unwrap();
  let data_utf8 = String::from_utf8(data.to_vec()).unwrap();
  let ics_data = data_utf8.parse::<Calendar>().unwrap();
  println!("ICS: {ics_data:?}");

  for component in ics_data.into_iter() {
    match component {
      CalendarComponent::Event(event) => {
        let start_date = event.get_start().and_then(extract_datetime);

        let row = sqlx::query(
          "insert into node (title, cal_date, json)
          values (?, ?, ?)
          returning id",
        )
        .bind(event.get_summary().unwrap())
        .bind(start_date)
        .bind(&event_to_json(&event))
        .fetch_one(&ctx.db)
        .await
        .unwrap();
        let id = row.get::<String, _>(0usize);
        println!("id: {id}");
      }
      _ => {
        // TODO:
      }
    }
  }
}

fn event_to_json(event: &Event) -> String {
  let res = event
    .properties()
    .into_iter()
    .map(|(k, v)| (k, v.value()))
    .collect::<BTreeMap<_, _>>();
  serde_json::to_string(&res).unwrap()
}

fn extract_datetime(date: DatePerhapsTime) -> Option<DateTime<Utc>> {
  match date {
    DatePerhapsTime::DateTime(dt) => match dt {
      CalendarDateTime::Floating(_) => None,
      CalendarDateTime::Utc(dt) => Some(dt),
      // TODO: This is handleable
      CalendarDateTime::WithTimezone { date_time, tzid } => None,
    },
    DatePerhapsTime::Date(_) => None,
  }
}

// fn iso_represent(dpt: DatePerhapsTime) -> (String, bool) {
//   match dpt {
//     DatePerhapsTime::Date(date) => {}
//     DatePerhapsTime::DateTime(datetime) => match datetime {
//       CalendarDateTime::Floating(naive_date_time) => {}
//       CalendarDateTime::Utc(date_time) => todo!(),
//       CalendarDateTime::WithTimezone { date_time, tzid } => todo!(),
//     },
//   }
// }

#[derive(Debug, Serialize)]
pub struct QueryEventsResponse {
  events: Vec<JsonValue>,
}

pub async fn query_events(State(ctx): State<Context>) -> Json<QueryEventsResponse> {
  let rows = sqlx::query("select title, cal_date, json from node where cal_date is not null")
    .fetch_all(&ctx.db)
    .await
    .unwrap();

  let data = rows
    .into_iter()
    .map(|row| {
      json!({
        "title": row.get::<String,usize>(0),
        "start_date": row.get::<DateTime<Utc>,usize>(1).to_rfc3339(),
        "other_data": serde_json::from_str::<JsonValue>(row.get(2)).unwrap(),
      })
    })
    .collect();

  Json(QueryEventsResponse { events: data })
}
