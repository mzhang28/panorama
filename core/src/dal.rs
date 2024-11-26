use anyhow::Result;
use sqlx::SqlitePool;

pub struct PanoramaDatabase(SqlitePool);

pub struct EnsureMailAccount {
  imap_server_host: String,
  imap_server_port: u16,
}

impl PanoramaDatabase {
  pub async fn ensure_mail_account(&self) -> Result<()> {
    Ok(())
  }
}
