use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::{Execute, QueryBuilder, SqlitePool};
use uuid::Uuid;

#[derive(Clone)]
pub struct Dal {
    pub(crate) pool: SqlitePool,
}

impl Dal {
    pub fn new(pool: SqlitePool) -> Self {
        Dal { pool }
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!().run(&self.pool).await?;
        Ok(())
    }

    pub async fn create_table(
        &self,
        name: impl AsRef<str>,
        fields: HashMap<String, String>,
    ) -> Result<()> {
        // TODO: Validate this stuff lmao

        // Create the new table
        let table_id = format!("{}-{}", Uuid::now_v7().to_string(), name.as_ref());
        let mut qb = QueryBuilder::new(format!("create table \"{table_id}\" (node_id text,"));
        let mut s = qb.separated(", ");
        let augmented_fields = fields
            .into_iter()
            .map(|(name, ty)| (format!("{}-{}", Uuid::now_v7().to_string(), name), name, ty))
            .collect::<Vec<_>>();
        for (column_name, _, field_ty) in augmented_fields.iter() {
            // QueryBuilder will insert the separators between entries, so don't
            // include a trailing comma here.
            s.push(format!("\"{column_name}\" {field_ty}"));
        }
        s.push_unseparated(", primary key (node_id))");
        let query = qb.build();
        println!("Query: {:?}", query.sql());
        let _result = query.execute(&self.pool).await?;

        // Document the updates into the schema table
        let mut qb = QueryBuilder::new(
            "insert into _panorama_schema_columns (key, sqlite_table_name, sqlite_column_name)",
        );
        qb.push_values(
            augmented_fields.iter(),
            |mut b, (column_name, field_name, _)| {
                b.push_bind(field_name)
                    .push_bind(&table_id)
                    .push_bind(column_name);
            },
        );
        qb.build().execute(&self.pool).await?;

        Ok(())
    }

    pub async fn has_schema_key(&self, key: &str) -> Result<bool> {
        let rec: Option<(String,)> = sqlx::query_as("select key from _panorama_schema_columns where key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(rec.is_some())
    }

    pub async fn schema_count(&self, key: &str) -> Result<i64> {
        let rec: (i64,) = sqlx::query_as("select count(1) from _panorama_schema_columns where key = ?")
            .bind(key)
            .fetch_one(&self.pool)
            .await?;
        Ok(rec.0)
    }

    pub async fn schema_entry(&self, key: &str) -> Result<Option<(String, String)>> {
        let rec: Option<(String, String)> = sqlx::query_as("select sqlite_table_name, sqlite_column_name from _panorama_schema_columns where key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(rec)
    }

    pub async fn insert_nodes(&self, nodes: Vec<InsertNode>) -> Result<Vec<String>> {
        let all_keys = nodes
            .iter()
            .flat_map(|node| node.0.keys())
            .collect::<Vec<_>>();

        let mut qb = QueryBuilder::new("select * from _panorama_schema where key in (");
        let mut s = qb.separated(", ");
        for key in all_keys {
            s.push_bind(key);
        }
        s.push_unseparated(")");
        let query = qb.build();
        let result = query.execute(&self.pool).await?;
        println!("Result: {result:?}");

        let ids = vec![];

        Ok(ids)
    }

    pub async fn query(&self) -> Result<()> {
        Ok(())
    }
}

pub struct InsertNode(HashMap<String, JsonValue>);
