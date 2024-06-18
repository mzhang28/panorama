#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
struct Opt {
  #[clap(long = "no-embedded-daemon")]
  no_embedded_daemon: bool,

  #[clap(subcommand)]
  command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
  Daemon,
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt::init();
  let opt = Opt::parse();

  match opt.command {
    Some(Command::Daemon) => {
      panorama_daemon::run().await;
    }
    None => {
      if !opt.no_embedded_daemon {
        tokio::spawn(panorama_daemon::run());
      }
      app_lib::run();
    }
  }
}
