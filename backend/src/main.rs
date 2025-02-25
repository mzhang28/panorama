use std::env;

#[cfg(feature = "static-build")]
mod static_build;

use anyhow::Result;
use axum::{Router, extract::Request};
use clap::Parser;
use panorama_backend::{create_context, create_web_server};
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::{fmt::time::uptime, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
struct Opt {
  #[clap(
    long = "port",
    short = 'p',
    default_value = "3000",
    env = "PANORAMA_PORT"
  )]
  port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
  let opt = Opt::parse();

  let format = tracing_subscriber::fmt::format()
    .with_level(true) // don't include levels in formatted output
    .with_target(false) // don't include targets
    .with_thread_ids(false) // include the thread ID of the current thread
    .with_thread_names(false) // include the name of the current thread
    .with_ansi(true)
    .with_timer(uptime())
    .with_source_location(true)
    .pretty(); // use the `Compact` formatting style.

  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // axum logs rejections from built-in extractors with the `axum::rejection`
        // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
        format!(
          "{}=debug,tower_http=debug,axum::rejection=trace",
          env!("CARGO_CRATE_NAME")
        )
        .into()
      }),
    )
    .with(tracing_subscriber::fmt::layer().event_format(format))
    .init();

  let context = create_context().await?;

  let (workflow_router_tx, workflow_router_rx) = mpsc::unbounded_channel();
  let workflow_router = move |req: Request| async move {
    workflow_router_tx.send(req);
  };

  // Spawn services
  // let services_handle = spawn(spawn_services(context.clone()));

  let app = create_web_server(context).await?;

  #[cfg(feature = "static-build")]
  let app = static_build::create_static_router(app);

  let listener = tokio::net::TcpListener::bind(("0.0.0.0", opt.port))
    .await
    .unwrap();

  info!("Listening on {}", listener.local_addr().unwrap());
  axum::serve(listener, app).await?;

  Ok(())
}
