use std::{fs, path::Path};

use anyhow::Result;
use cozo::DbInstance;
use tantivy::{
  directory::MmapDirectory,
  schema::{self, Schema, STORED, STRING, TEXT},
  Index,
};

use crate::{mail::mail_loop, migrations::run_migrations};

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
      let mut schema_builder = Schema::builder();
      let node_id = schema_builder.add_text_field("node_id", STRING | STORED);
      let title = schema_builder.add_text_field("title", TEXT | STORED);
      let body = schema_builder.add_text_field("body", TEXT);
      let schema = schema_builder.build();
      let tantivy_path = panorama_dir.join("tantivy-index");
      fs::create_dir_all(&tantivy_path)?;
      let dir = MmapDirectory::open(&tantivy_path)?;
      Index::builder().schema(schema).open_or_create(dir)?
    };

    let db_path = panorama_dir.join("db.sqlite");
    let db = DbInstance::new(
      "sqlite",
      db_path.display().to_string(),
      Default::default(),
    )
    .unwrap();

    run_migrations(&db).await?;

    tokio::spawn(mail_loop(db.clone()));

    Ok(AppState { db, tantivy_index })
  }
}
