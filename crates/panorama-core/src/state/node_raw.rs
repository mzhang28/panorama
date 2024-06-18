use sqlx::{Executor, FromRow, Sqlite};

use crate::AppState;

#[derive(FromRow)]
pub struct FieldMappingRow {
  pub full_key: String,
  pub app_id: i64,
  pub app_table_name: String,
  pub app_table_field: String,
  pub db_table_name: Option<String>,
}

impl AppState {
  pub(crate) async fn get_related_field_list_for_node_id<'e, 'c: 'e, X>(
    x: X,
    node_id: &str,
  ) -> sqlx::Result<Vec<FieldMappingRow>>
  where
    X: 'e + Executor<'c, Database = Sqlite>,
  {
    sqlx::query_as!(
      FieldMappingRow,
      "
        SELECT
          node_has_key.full_key, key_mapping.app_id,
          key_mapping.app_table_name, app_table_field,
          app_table_mapping.db_table_name
        FROM node_has_key
        INNER JOIN key_mapping
          ON node_has_key.full_key = key_mapping.full_key
        INNER JOIN app_table_mapping
          ON key_mapping.app_id = app_table_mapping.app_id
            AND key_mapping.app_table_name = app_table_mapping.app_table_name
        WHERE node_id = $1
      ",
      node_id
    )
    .fetch_all(x)
    .await
  }
}
