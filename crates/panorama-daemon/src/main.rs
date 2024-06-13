use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
  panorama_daemon::run().await?;
  Ok(())
}
