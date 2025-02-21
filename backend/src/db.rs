use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::functions::FunctionFlags;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use uuid::Uuid;

pub fn init_db_options() -> SqlitePoolOptions {
  SqlitePoolOptions::new().after_connect(|conn, _| {
    Box::pin(async move {
      let mut locked_conn = conn.lock_handle().await?;
      let mut raw_handle = locked_conn.as_raw_handle();

      // SAFETY: We are currently locking the handle
      let rusqlite_handle = unsafe { rusqlite::Connection::from_handle_owned(raw_handle.as_mut()) }
        // ok holy shit sqlx::SqliteError can't be constructed so i need to figure out how to handle this error
        .unwrap();

      rusqlite_handle
        .create_scalar_function("NOW_ISO8601", 0, FunctionFlags::SQLITE_UTF8, |_| {
          let now = Utc::now();
          Ok(now.to_rfc3339())
        })
        // same as above
        .unwrap();

      rusqlite_handle
        .create_scalar_function("UUIDV7_NOW", 0, FunctionFlags::SQLITE_UTF8, |_| {
          let id = Uuid::now_v7();
          Ok(id.to_string())
        })
        // same as above
        .unwrap();

      drop(rusqlite_handle);
      drop(locked_conn);
      // conn is now unlocked

      Ok(())
    })
  })
}

pub async fn get_config(pool: &SqlitePool, key: impl AsRef<str>) -> Result<String> {
  let row = sqlx::query("select panorama_config_value from node where panorama_config_key = ?")
    .bind(key.as_ref())
    .fetch_one(pool)
    .await?;

  Ok(row.get(0))
}

pub async fn get_config_with_default(
  pool: &SqlitePool,
  key: impl AsRef<str>,
  value: impl AsRef<str>,
) -> Result<String> {
  let row = sqlx::query("insert into node (panorama_config_key, panorama_config_value)
  values (?, ?)
  on conflict (panorama_config_key) do update set panorama_config_value = EXCLUDED.panorama_config_value
  returning panorama_config_value")
    .bind(key.as_ref())
    .bind(value.as_ref())
    .fetch_one(pool)
    .await?;

  Ok(row.get(0))
}
