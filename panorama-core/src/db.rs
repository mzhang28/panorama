use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::{QueryBuilder, SqlitePool};

pub struct Dal {
    pub(crate) pool: SqlitePool,
}

impl Dal {
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!().run(&self.pool).await?;
        Ok(())
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
