pub mod automate;
pub mod bun_download;

use anyhow::Result;

use self::automate::run_automate_service;
use self::bun_download::download_bun;
use crate::graphql::Context;

pub async fn spawn_services(ctx: Context) -> Result<()> {
  tokio::spawn(download_bun());
  tokio::spawn(run_automate_service(ctx.clone()));

  Ok(())
}

pub async fn spawn_repeatable_service() -> Result<()> {
  loop {}

  Ok(())
}
