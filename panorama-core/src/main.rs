use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    panorama_core::run().await
}
