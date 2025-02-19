use chrono::{DateTime, Utc};
use rusqlite::functions::FunctionFlags;
use sqlx::sqlite::SqlitePoolOptions;
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

      rusqlite_handle
        .create_scalar_function("DATE_BEFORE", 2, FunctionFlags::SQLITE_UTF8, |ctx| {
          let date_a: String = ctx.get(0)?;
          let date_b: String = ctx.get(0)?;
          let date_a = DateTime::parse_from_rfc3339(&date_a).unwrap();
          let date_b = DateTime::parse_from_rfc3339(&date_b).unwrap();
          return Ok(if date_a < date_b { 1 } else { 0 });
        })
        .unwrap();

      rusqlite_handle
        .create_scalar_function("DATE_AFTER", 2, FunctionFlags::SQLITE_UTF8, |ctx| {
          let date_a: String = ctx.get(0)?;
          let date_b: String = ctx.get(0)?;
          let date_a = DateTime::parse_from_rfc3339(&date_a).unwrap();
          let date_b = DateTime::parse_from_rfc3339(&date_b).unwrap();
          return Ok(if date_a > date_b { 1 } else { 0 });
        })
        .unwrap();

      drop(rusqlite_handle);
      drop(locked_conn);
      // conn is now unlocked

      Ok(())
    })
  })
}
