// pub mod codetrack;
// pub mod export;
// pub mod journal;
// pub mod mail;
// pub mod node;
// pub mod utils;

use std::{collections::HashMap, fs, path::Path};

use bimap::BiMap;
use miette::{Context, IntoDiagnostic, Result};
use sqlx::{
  sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
  SqlitePool,
};
use tantivy::{
  directory::MmapDirectory,
  schema::{Field, Schema, STORED, STRING, TEXT},
  Index,
};

use crate::{
  // mail::MailWorker,
  migrations::{self, MIGRATOR},
};

pub fn tantivy_schema() -> (Schema, BiMap<String, Field>) {
  let mut schema_builder = Schema::builder();

  let mut field_map = BiMap::new();

  let node_id = schema_builder.add_text_field("node_id", STRING | STORED);
  field_map.insert("node_id".to_owned(), node_id);

  let journal_content = schema_builder.add_text_field("title", TEXT | STORED);
  field_map.insert("panorama/journal/page/content".to_owned(), journal_content);

  (schema_builder.build(), field_map)
}

#[derive(Clone)]
pub struct AppState {
  pub db: SqlitePool,
  pub tantivy_index: Index,
  pub tantivy_field_map: BiMap<String, Field>,
}

impl AppState {
  pub async fn new(panorama_dir: impl AsRef<Path>) -> Result<Self> {
    let panorama_dir = panorama_dir.as_ref().to_path_buf();
    fs::create_dir_all(&panorama_dir)
      .into_diagnostic()
      .context("Could not create panorama directory")?;

    println!("Panorama dir: {}", panorama_dir.display());

    let (tantivy_index, tantivy_field_map) = {
      let (schema, field_map) = tantivy_schema();
      let tantivy_path = panorama_dir.join("tantivy-index");
      fs::create_dir_all(&tantivy_path).into_diagnostic()?;
      let dir = MmapDirectory::open(&tantivy_path).into_diagnostic()?;
      let index = Index::builder()
        .schema(schema)
        .open_or_create(dir)
        .into_diagnostic()?;
      (index, field_map)
    };

    let db_path = panorama_dir.join("db.sqlite");
    let sqlite_connect_options = SqliteConnectOptions::new()
      .filename(db_path)
      .journal_mode(SqliteJournalMode::Wal);
    let db = SqlitePoolOptions::new()
      .connect_with(sqlite_connect_options)
      .await
      .into_diagnostic()
      .context("Could not connect to SQLite database")?;

    let state = AppState {
      db,
      tantivy_index,
      tantivy_field_map,
    };
    state.init().await?;

    Ok(state)
  }

  async fn init(&self) -> Result<()> {
    // run_migrations(&self.db).await?;
    MIGRATOR
      .run(&self.db)
      .await
      .into_diagnostic()
      .context("Could not migrate database")?;

    // let state = self.clone();
    // let mail_worker = MailWorker::new(state);
    // tokio::spawn(mail_worker.mail_loop());

    Ok(())
  }
}
