use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use panorama_server::api::{build_router, AppState};
use panorama_server::object_store::ObjectStorage;
use panorama_server::plugin_loader::PluginLoader;
use panorama_server::schema_registry::SchemaRegistry;
use panorama_server::storage::{sqlite::SqliteBackend, NodeStorage};

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
    .with_target(false)
    .init();

  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  tracing::info!(cwd = %cwd.display(), "Server process started");

  let data_dir = std::env::var("PANORAMA_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
  let data_path = PathBuf::from(&data_dir);
  tracing::info!(data_dir = %data_path.display(), "Data directory");

  let backend = std::sync::Arc::new(SqliteBackend::new(data_path.join("nodes")));
  let storage = NodeStorage::new(backend);
  let schema_registry = SchemaRegistry::new();
  let object_storage = ObjectStorage::new(data_path.join("objects"));

  schema_registry.register(panorama_core::schema::system_schemas::node_time_schema());
  schema_registry.register(panorama_core::schema::system_schemas::node_info_schema());

  let plugin_loader = Arc::new(PluginLoader::new(
    storage.clone(),
    schema_registry.clone(),
    object_storage.clone(),
  ));

  // The ONLY hardcoded behavior: the directory to load .panoapp files from
  let plugins_dir = data_path.join("plugins");
  tracing::info!(plugins_dir = %plugins_dir.display(), exists = plugins_dir.exists(), "Scanning for plugins");
  if plugins_dir.exists() {
    let mut count = 0u32;
    for entry in std::fs::read_dir(&plugins_dir)
      .into_iter()
      .flatten()
      .flatten()
    {
      let path = entry.path();
      if path.is_file() && path.extension().map_or(false, |e| e == "panoapp") {
        count += 1;
        tracing::info!(file = %path.display(), "Loading .panoapp");
        match panorama_server::panoapp::PanoAppPackage::load_from_file(&path) {
          Ok(package) => {
            let name = package.manifest.name.clone();
            let version = package.manifest.version.clone();
            let id = package.manifest.id.clone();
            match plugin_loader.load_from_panoapp(package).await {
              Ok(_) => {
                tracing::info!("Loaded .panoapp: {} v{} ({})", name, version, id)
              }
              Err(e) => tracing::error!("Failed .panoapp '{}': {}", name, e),
            }
          }
          Err(e) => tracing::warn!("Skipping {}: {}", path.display(), e),
        }
      }
    }
    tracing::info!(count = count, "Finished scanning plugins dir");
  } else {
    tracing::warn!("Plugins directory does not exist");
  }

  let state = AppState {
    storage,
    schema_registry,
    object_storage,
    plugin_loader,
  };
  let app = build_router(state);

  let addr: SocketAddr = std::env::var("PANORAMA_LISTEN")
    .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
    .parse()
    .expect("Invalid PANORAMA_LISTEN address");

  tracing::info!(addr = %addr, "Binding listener");
  let listener = match tokio::net::TcpListener::bind(addr).await {
    Ok(l) => {
      tracing::info!(addr = %addr, "Listener bound, starting serve");
      l
    }
    Err(e) => {
      tracing::error!(addr = %addr, error = %e, "Failed to bind listener");
      std::process::exit(1);
    }
  };
  axum::serve(listener, app).await.unwrap();
}
