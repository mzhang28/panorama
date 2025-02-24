use std::sync::Arc;

use object_store::ObjectStore;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Context {
  pub(crate) db: SqlitePool,
  pub(crate) object_store: Arc<dyn ObjectStore>,
  pub(crate) tantivy_index: tantivy::Index,
}
