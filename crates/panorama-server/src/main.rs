use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use panorama_server::api::{build_router, AppState};
use panorama_server::object_store::ObjectStorage;
use panorama_server::plugin_loader::{PluginLoadState, PluginLoader, PluginStatusEntry};
use panorama_server::reactor::deferred::DeferredReactorEngine;
use panorama_server::reactor::eager::EagerReactorPipeline;
use panorama_server::reactor::op_stream::OpStream;
use panorama_server::reactor::registry::ReactorRegistry;
use panorama_server::schema_registry::SchemaRegistry;
use panorama_server::storage::{sqlite::SqliteBackend, NodeStorage};
use tokio::sync::RwLock;

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
  let schema_registry = SchemaRegistry::new(Some(storage.clone()));
  let object_storage = ObjectStorage::new(data_path.join("objects"));

  schema_registry
    .register(panorama_core::schema::system_schemas::node_time_schema())
    .expect("node_time schema");
  schema_registry
    .register(panorama_core::schema::system_schemas::node_info_schema())
    .expect("node_info schema");
  schema_registry
    .register(panorama_core::schema::system_schemas::reactors_schema())
    .expect("reactors schema");
  schema_registry
    .register(panorama_core::schema::system_schemas::op_stream_schema())
    .expect("op_stream schema");
  schema_registry
    .register(panorama_core::schema::system_schemas::reactor_state_schema())
    .expect("reactor_state schema");

  let plugin_loader = Arc::new(PluginLoader::new(
    storage.clone(),
    schema_registry.clone(),
    object_storage.clone(),
  ));
  let load_state = Arc::new(RwLock::new(PluginLoadState::new()));

  // ── Reactor subsystem ──────────────────────────────────────────────────
  let reactor_registry = Arc::new(ReactorRegistry::new(
    storage.clone(),
    schema_registry.clone(),
  ));
  let op_stream = Arc::new(OpStream::new(storage.clone()));
  let eager_pipeline = Arc::new(
    EagerReactorPipeline::new(reactor_registry.clone()).with_plugin_loader(plugin_loader.clone()),
  );
  let deferred_engine = Arc::new(
    DeferredReactorEngine::new(reactor_registry.clone(), op_stream.clone())
      .with_plugin_loader(plugin_loader.clone()),
  );

  if let Err(e) = reactor_registry.initialize().await {
    tracing::error!(error = %e, "Failed to initialize reactor registry");
  }
  if let Err(e) = op_stream.initialize().await {
    tracing::error!(error = %e, "Failed to initialize op stream");
  }
  if let Err(e) = deferred_engine.initialize().await {
    tracing::error!(error = %e, "Failed to initialize deferred reactor engine");
  }

  // Start deferred reactor polling loop in background
  let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
  let deferred_engine_bg = deferred_engine.clone();
  tokio::spawn(async move {
    deferred_engine_bg.run_polling_loop(cancel_rx).await;
  });

  #[cfg(frontend_embedded)]
  tracing::info!("Frontend SPA is embedded in the server binary");
  #[cfg(not(frontend_embedded))]
  tracing::warn!("Frontend SPA is NOT embedded in the server binary!");

  let state = AppState {
    storage,
    schema_registry,
    object_storage,
    plugin_loader: plugin_loader.clone(),
    load_state: load_state.clone(),
    reactor_registry,
    eager_pipeline,
    op_stream,
    deferred_engine,
  };
  let app = build_router(state);

  let addr: SocketAddr = std::env::var("PANORAMA_LISTEN")
    .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
    .parse()
    .expect("Invalid PANORAMA_LISTEN address");

  tracing::info!(addr = %addr, "Binding listener");
  let listener = match tokio::net::TcpListener::bind(addr).await {
    Ok(l) => {
      tracing::info!(addr = %addr, "Listener bound, HTTP server is LIVE");
      l
    }
    Err(e) => {
      tracing::error!(addr = %addr, error = %e, "Failed to bind listener");
      std::process::exit(1);
    }
  };

  // ── Plugin loading runs in background after HTTP is live ───────────────
  let plugins_dirs: Vec<PathBuf> = std::env::var("PANORAMA_PLUGINS_DIR")
    .unwrap_or_else(|_| data_path.join("plugins").display().to_string())
    .split(':')
    .map(PathBuf::from)
    .collect();
  tracing::info!(?plugins_dirs, "Plugin search paths");

  let loader_bg = plugin_loader.clone();
  let state_bg = load_state.clone();
  tokio::spawn(async move {
    load_plugins_in_background(loader_bg, state_bg, plugins_dirs).await;
  });

  axum::serve(listener, app).await.unwrap();
}

/// Scan plugin directories (from `PANORAMA_PLUGINS_DIR`, colon-separated)
/// and load all `.panoapp` files concurrently.
///
/// Duplicate plugin IDs are skipped — first directory wins.
///
/// Updates `load_state` throughout so the frontend can observe progress
/// via `GET /api/plugins/status`.
async fn load_plugins_in_background(
  plugin_loader: Arc<PluginLoader>,
  load_state: Arc<RwLock<PluginLoadState>>,
  plugins_dirs: Vec<PathBuf>,
) {
  tracing::info!(?plugins_dirs, "Scanning for plugins");

  // Phase 1: Scan all directories for .panoapp files, first-seen wins per id
  let mut seen_ids = std::collections::HashSet::new();
  let mut packages: Vec<(panorama_server::panoapp::PanoAppPackage, PluginStatusEntry)> = Vec::new();

  for dir in &plugins_dirs {
    if !dir.exists() {
      tracing::warn!(dir = %dir.display(), "Plugin directory does not exist, skipping");
      continue;
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
      let path = entry.path();
      if path.is_file() && path.extension().map_or(false, |e| e == "panoapp") {
        tracing::info!(file = %path.display(), dir = %dir.display(), "Discovered .panoapp");
        match panorama_server::panoapp::PanoAppPackage::load_from_file(&path) {
          Ok(package) => {
            if !seen_ids.insert(package.manifest.id.clone()) {
              tracing::info!(
                id = %package.manifest.id,
                "Skipping duplicate plugin (already found in earlier directory)"
              );
              continue;
            }
            let entry = PluginStatusEntry {
              id: package.manifest.id.clone(),
              name: package.manifest.name.clone(),
              version: package.manifest.version.clone(),
              status: "pending".to_string(),
              error: None,
            };
            packages.push((package, entry));
          }
          Err(e) => {
            tracing::warn!("Skipping {}: {}", path.display(), e);
          }
        }
      }
    }
  }

  if packages.is_empty() {
    let mut state = load_state.write().await;
    state.phase = panorama_server::plugin_loader::LoadPhase::Ready;
    tracing::info!("No plugins found, load phase = Ready");
    return;
  }

  // Phase 2: Transition to Loading and publish the pending list
  {
    let mut state = load_state.write().await;
    state.total = packages.len() as u32;
    state.plugins = packages.iter().map(|(_, e)| e.clone()).collect();
    state.phase = panorama_server::plugin_loader::LoadPhase::Loading;
    tracing::info!(total = state.total, "Plugin loading phase = Loading");
  }

  // Phase 3: Load all plugins concurrently
  let futures: Vec<_> = packages
    .into_iter()
    .map(|(package, entry)| {
      let plugin_loader = plugin_loader.clone();
      let load_state = load_state.clone();
      async move {
        // Mark as loading
        {
          let mut state = load_state.write().await;
          if let Some(e) = state.plugins.iter_mut().find(|p| p.id == entry.id) {
            e.status = "loading".to_string();
          }
        }

        let plugin_id = entry.id.clone();
        let plugin_name = entry.name.clone();
        let plugin_version = entry.version.clone();

        match plugin_loader.load_from_panoapp(package).await {
          Ok(_) => {
            tracing::info!(
              "Loaded .panoapp: {} v{} ({})",
              plugin_name,
              plugin_version,
              plugin_id
            );
            let mut state = load_state.write().await;
            if let Some(e) = state.plugins.iter_mut().find(|p| p.id == plugin_id) {
              e.status = "loaded".to_string();
            }
            state.loaded += 1;
          }
          Err(err) => {
            tracing::error!("Failed .panoapp '{}': {}", plugin_name, err);
            let mut state = load_state.write().await;
            if let Some(e) = state.plugins.iter_mut().find(|p| p.id == plugin_id) {
              e.status = "failed".to_string();
              e.error = Some(err.to_string());
            }
            state.failed += 1;
          }
        }
      }
    })
    .collect();

  futures::future::join_all(futures).await;

  // Phase 4: All done
  {
    let state = load_state.read().await;
    tracing::info!(
      total = state.total,
      loaded = state.loaded,
      failed = state.failed,
      "Plugin loading complete"
    );
  }
  {
    let mut state = load_state.write().await;
    state.phase = panorama_server::plugin_loader::LoadPhase::Ready;
  }
}
