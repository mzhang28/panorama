pub mod automate;

use anyhow::Result;
use automate::run_automate_service;

use crate::graphql::Context;

pub async fn run_services(ctx: Context) -> Result<()> {
  run_automate_service(ctx).await?;

  Ok(())
}
