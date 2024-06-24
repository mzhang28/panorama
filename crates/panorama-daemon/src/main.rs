use anyhow::Result;
use clap::{Parser, Subcommand};
use panorama_core::state::apps::manifest::AppManifest;
use schemars::schema_for;

#[derive(Debug, Parser)]
struct Opt {
  #[clap(subcommand)]
  command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
  GenerateConfigSchema,
}

#[tokio::main]
async fn main() -> Result<()> {
  let opt = Opt::parse();

  tracing_subscriber::fmt::init();

  match opt.command {
    Some(Command::GenerateConfigSchema) => {
      let schema = schema_for!(AppManifest);
      println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    }
    None => panorama_daemon::run().await?,
  }

  Ok(())
}
