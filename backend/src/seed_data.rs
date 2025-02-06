use anyhow::Result;
use sqlx::SqlitePool;

pub async fn ensure_seed_data(db: &SqlitePool) -> Result<()> {
  Ok(())
}
