pub mod automate;

use anyhow::Result;

use crate::graphql::Context;

pub async fn spawn_services(ctx: Context) -> Result<()> {
  Ok(())
}

pub async fn spawn_repeatable_service() -> Result<()> {
  loop {}

  Ok(())
}
