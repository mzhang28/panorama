use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;

pub struct DbClient {
  db: Surreal<Client>,
}

impl DbClient {
  pub async fn new() -> Result<Self, surrealdb::Error> {
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root {
      username: "root",
      password: "root",
    })
    .await?;
    db.use_ns("panorama").use_db("main").await?;
    Ok(Self { db })
  }

  pub async fn init(&self) -> Result<(), surrealdb::Error> {
    // Ensure 'nodes' table exists.
    // In SurrealDB tables are schemaless by default, but we can define them to enforce schema
    // or just to be explicit.
    self
      .db
      .query("DEFINE TABLE IF NOT EXISTS nodes SCHEMAFULL")
      .await?
      .check()?;
    Ok(())
  }

  pub async fn ensure_field(
    &self,
    table: &str,
    field: &str,
    type_: &str,
  ) -> Result<(), surrealdb::Error> {
    let query = format!(
      "DEFINE FIELD IF NOT EXISTS `{}` ON TABLE {} TYPE {}",
      field, table, type_
    );
    self.db.query(query).await?.check()?;
    Ok(())
  }
}
