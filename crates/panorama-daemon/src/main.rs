use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::init();
  panorama_daemon::run().await?;
  Ok(())
}
