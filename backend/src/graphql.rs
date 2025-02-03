use std::{sync::Arc, time::Duration};

use axum::Extension;
use futures::{stream::BoxStream, StreamExt};
use juniper::{
  graphql_object, graphql_subscription, EmptyMutation, FieldError, FieldResult, GraphQLObject,
  RootNode,
};
use juniper_axum::{extract::JuniperRequest, response::JuniperResponse};
use sqlx::SqlitePool;
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
}

#[graphql_object]
#[graphql(context = Context)]
impl Query {
  /// Adds two `a` and `b` numbers.
  fn add(a: i32, b: i32) -> i32 {
    a + b
  }

  fn node(id: String, context: &Context) -> FieldResult<NodeBase> {
    Ok(NodeBase { id })
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
