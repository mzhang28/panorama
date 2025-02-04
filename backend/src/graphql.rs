use std::{sync::Arc, time::Duration};

use axum::Extension;
use chrono::{DateTime, Utc};
use futures::{stream::BoxStream, StreamExt};
use juniper::{
  graphql_object, graphql_subscription, EmptyMutation, FieldError, FieldResult, GraphQLObject,
  RootNode,
};
use juniper_axum::{extract::JuniperRequest, response::JuniperResponse};
use sqlx::{Row, SqlitePool};
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;

pub type Schema = RootNode<'static, Query, EmptyMutation<Context>, Subscription>;

#[derive(Clone)]
pub struct Context {
  pub(crate) db: SqlitePool,
}

impl juniper::Context for Context {}

#[derive(Clone, Copy, Debug)]
pub struct Query;

#[derive(GraphQLObject)]
struct NodeBase {
  id: String,
  created_at: DateTime<Utc>,
  last_updated_at: DateTime<Utc>,
}

#[graphql_object]
#[graphql(context = Context)]
impl Query {
  async fn node(id: String, #[graphql(ctx)] context: &Context) -> FieldResult<NodeBase> {
    let data = sqlx::query(r#"select id, created_at, last_updated_at from node where id = ?"#)
      .bind(&id)
      .fetch_one(&context.db)
      .await?;
    let created_at = data.get(1);
    let last_updated_at = data.get(2);
    Ok(NodeBase {
      id,
      created_at,
      last_updated_at,
    })
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Subscription;

type NumberStream = BoxStream<'static, Result<i32, FieldError>>;

#[graphql_subscription]
#[graphql(context = Context)]
impl Subscription {
  /// Counts seconds.
  async fn count(context: &Context) -> NumberStream {
    let mut value = 0;
    let stream = IntervalStream::new(interval(Duration::from_secs(1))).map(move |_| {
      value += 1;
      Ok(value)
    });
    Box::pin(stream)
  }
}

pub async fn handler(
  Extension(schema): Extension<Arc<Schema>>,
  Extension(context): Extension<Context>,
  JuniperRequest(req): JuniperRequest, // should be the last argument as consumes `Request`
) -> JuniperResponse {
  JuniperResponse(req.execute(&*schema, &context).await)
}
