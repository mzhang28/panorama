pub mod mail;
pub mod node;

use std::{fs, path::Path};

use cozo::DbInstance;
use miette::{IntoDiagnostic, Result};
use tantivy::{
  directory::MmapDirectory,
  schema::{self, Schema, STORED, STRING, TEXT},
  Index,
};

use crate::migrations::run_migrations;

pub fn tantivy_schema() -> Schema {
  let mut schema_builder = Schema::builder();
  let node_id = schema_builder.add_text_field("node_id", STRING | STORED);
  let title = schema_builder.add_text_field("title", TEXT | STORED);
  let body = schema_builder.add_text_field("body", TEXT);
  schema_builder.build()
}

#[derive(Clone)]
pub struct AppState {
  pub db: DbInstance,
  pub tantivy_index: Index,
}

impl AppState {
  pub async fn new(panorama_dir: impl AsRef<Path>) -> Result<Self> {
    let panorama_dir = panorama_dir.as_ref().to_path_buf();
    println!("Panorama dir: {}", panorama_dir.display());

    let tantivy_index = {
      let schema = tantivy_schema();
      let tantivy_path = panorama_dir.join("tantivy-index");
      fs::create_dir_all(&tantivy_path).into_diagnostic()?;
      let dir = MmapDirectory::open(&tantivy_path).into_diagnostic()?;
      Index::builder()
        .schema(schema)
        .open_or_create(dir)
        .into_diagnostic()?
    };

    let db_path = panorama_dir.join("db.sqlite");
    let db = DbInstance::new(
      "sqlite",
      db_path.display().to_string(),
      Default::default(),
    )
    .unwrap();

    let state = AppState { db, tantivy_index };
    state.init().await?;

    Ok(state)
  }

  async fn init(&self) -> Result<()> {
    run_migrations(&self.db).await?;

    let state = self.clone();
    tokio::spawn(async move { state.mail_loop().await });

    Ok(())
  }
}
