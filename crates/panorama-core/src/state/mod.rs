pub mod apps;
// pub mod codetrack;
// pub mod export;
// pub mod journal;
// pub mod mail;
pub mod node;
pub mod node_raw;
// pub mod utils;

use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use bimap::BiMap;
use sqlx::{
  pool::PoolConnection,
  sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
  Sqlite, SqlitePool,
};
use tantivy::{
  directory::MmapDirectory,
  schema::{Field, Schema, STORED, STRING, TEXT},
  Index,
};
use wasmtime::Module;

use crate::{
  // mail::MailWorker,
  migrations::MIGRATOR,
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

  pub app_wasm_modules: HashMap<String, Module>,
  // TODO: Compile this into a more efficient thing than just iter
  pub app_routes: HashMap<String, Vec<AppRoute>>,
}

#[derive(Clone)]
pub struct AppRoute {
  route: String,
  handler_name: String,
}

impl AppState {
  pub async fn new(panorama_dir: impl AsRef<Path>) -> Result<Self> {
    let panorama_dir = panorama_dir.as_ref().to_path_buf();
    fs::create_dir_all(&panorama_dir)
      .context("Could not create panorama directory")?;

    println!("Panorama dir: {}", panorama_dir.display());

    let (tantivy_index, tantivy_field_map) = {
      let (schema, field_map) = tantivy_schema();
      let tantivy_path = panorama_dir.join("tantivy-index");
      fs::create_dir_all(&tantivy_path)?;
      let dir = MmapDirectory::open(&tantivy_path)?;
      let index = Index::builder().schema(schema).open_or_create(dir)?;
      (index, field_map)
    };

    let db_path = panorama_dir.join("db.sqlite");
    let sqlite_connect_options = SqliteConnectOptions::new()
      .filename(db_path)
      .journal_mode(SqliteJournalMode::Wal)
      .create_if_missing(true);
    let db = SqlitePoolOptions::new()
      .connect_with(sqlite_connect_options)
      .await
      .context("Could not connect to SQLite database")?;

    let state = AppState {
      db,
      tantivy_index,
      tantivy_field_map,
      app_wasm_modules: Default::default(),
      app_routes: Default::default(),
    };
    state.init().await?;

    Ok(state)
  }

  pub async fn conn(&self) -> Result<PoolConnection<Sqlite>> {
    self.db.acquire().await.map_err(|err| err.into())
  }

  async fn init(&self) -> Result<()> {
    // run_migrations(&self.db).await?;
    MIGRATOR
      .run(&self.db)
      .await
      .context("Could not migrate database")?;

    // let state = self.clone();
    // let mail_worker = MailWorker::new(state);
    // tokio::spawn(mail_worker.mail_loop());

    Ok(())
  }

  pub fn handle_app_route() {}
}
