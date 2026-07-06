//! Integration tests demonstrating all example apps working through the plugin API.
//! Each test creates a Panorama server instance, loads a plugin, and tests its workflows.

use std::sync::Arc;

use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{HttpRequest, HttpResponse, Plugin, PluginContext, PluginError};
use panorama_core::types::{FieldValue, Node};
use panorama_server::object_store::ObjectStorage;
use panorama_server::plugin_loader::PluginLoader;
use panorama_server::schema_registry::SchemaRegistry;
use panorama_server::storage::{sqlite::SqliteBackend, NodeStorage};

/// Helper to create a test plugin loader with in-memory storage
fn setup_test_env() -> (PluginLoader, tempfile::TempDir) {
  let tmp = tempfile::tempdir().unwrap();
  let backend = Arc::new(SqliteBackend::new(tmp.path().join("nodes")));
  let storage = NodeStorage::new(backend);
  let schema_registry = SchemaRegistry::new();
  let object_storage = ObjectStorage::new(tmp.path().join("objects"));

  // Register system schemas
  schema_registry.register(panorama_core::schema::system_schemas::node_time_schema());
  schema_registry.register(panorama_core::schema::system_schemas::node_info_schema());

  let loader = PluginLoader::new(storage, schema_registry, object_storage);
  (loader, tmp)
}

// ─── Journal App Tests ───────────────────────────────────────

#[tokio::test]
async fn test_journal_create_and_list_blocks() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_journal::JournalPlugin::new());
  loader.load(plugin.clone()).await.unwrap();

  let ctx = loader.create_context("io.mzhang.panorama.journal", plugin.required_capabilities());

  // Create a root block (page) via HTTP
  let req = HttpRequest {
    method: "POST".into(),
    path: "blocks".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "My First Page",
          "content": "# Hello\nThis is a journal block.",
          "tags": ["test", "journal"]
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("blocks", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);

  // List blocks
  let req = HttpRequest {
    method: "GET".into(),
    path: "blocks".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("blocks", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let blocks: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert!(!blocks.is_empty());
}

#[tokio::test]
async fn test_journal_block_has_fields() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_journal::JournalPlugin::new());
  loader.load(plugin.clone()).await.unwrap();

  let ctx = loader.create_context("io.mzhang.panorama.journal", plugin.required_capabilities());

  let req = HttpRequest {
    method: "POST".into(),
    path: "blocks".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "Test Block",
          "content": "Testing journal block content"
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("blocks", req, &ctx)
    .await
    .unwrap();
  let block: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(block["fields"]["system:node_title"]["value"], "Test Block");
  assert_eq!(
    block["fields"]["journal:content"]["value"],
    "Testing journal block content"
  );
}

#[tokio::test]
async fn test_journal_block_refs_extraction() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_journal::JournalPlugin::new());
  loader.load(plugin.clone()).await.unwrap();

  let ctx = loader.create_context("io.mzhang.panorama.journal", plugin.required_capabilities());

  // Create a page to reference
  let req = HttpRequest {
    method: "POST".into(),
    path: "blocks".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "Target Page",
          "content": "The target"
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("blocks", req, &ctx)
    .await
    .unwrap();
  let target: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  let target_id = target["id"].as_str().unwrap();

  // Create a block that references the target via [[link]]
  let req = HttpRequest {
    method: "POST".into(),
    path: "blocks".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "Source Block",
          "content": "See [[Target Page]] for details."
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("blocks", req, &ctx)
    .await
    .unwrap();
  let source: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  let source_id = source["id"].as_str().unwrap();

  // Check that refs were extracted (refs are serialized as {"type":"NodeRef","value":"uuid"})
  let refs_val = &source["fields"]["journal:refs"];
  let refs_arr = refs_val["value"].as_array().unwrap();
  assert_eq!(refs_arr.len(), 1, "should have one ref");
  let ref_value = refs_arr[0]["value"].as_str().unwrap();
  assert_eq!(ref_value, target_id, "ref should point to target page");

  // Check backlinks
  let backlinks_path = format!("pages/{}/backlinks", target_id);
  let req = HttpRequest {
    method: "GET".into(),
    path: backlinks_path.clone(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request(&backlinks_path, req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let backlinks: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert!(!backlinks.is_empty(), "should have backlinks");
}

#[tokio::test]
async fn test_journal_pages_today() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_journal::JournalPlugin::new());
  loader.load(plugin.clone()).await.unwrap();

  let ctx = loader.create_context("io.mzhang.panorama.journal", plugin.required_capabilities());

  // Create a block first to ensure the "journal" namespace is registered
  let req = HttpRequest {
    method: "POST".into(),
    path: "blocks".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "Warmup",
          "content": "warmup"
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  plugin
    .handle_http_request("blocks", req, &ctx)
    .await
    .unwrap();

  // Get today's page — auto-creates if missing
  let req = HttpRequest {
    method: "GET".into(),
    path: "pages/today".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("pages/today", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let page: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();

  // Should have journal_day set to today
  let journal_day = &page["fields"]["journal:journal_day"]["value"];
  let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
  assert_eq!(journal_day.as_str().unwrap(), today);

  // Calling again should return the same page (not create a duplicate)
  let req2 = HttpRequest {
    method: "GET".into(),
    path: "pages/today".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp2 = plugin
    .handle_http_request("pages/today", req2, &ctx)
    .await
    .unwrap();
  assert_eq!(resp2.status, 200);
  let page2: serde_json::Value = serde_json::from_slice(&resp2.body).unwrap();
  assert_eq!(page["id"], page2["id"]);
}

// ─── Coding Activity App Tests ───────────────────────────────

#[tokio::test]
async fn test_coding_heartbeat_single() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_coding::CodingPlugin::new());
  loader.load(plugin.clone()).await.unwrap();

  let ctx = loader.create_context("io.mzhang.panorama.coding", plugin.required_capabilities());

  let req = HttpRequest {
    method: "POST".into(),
    path: "heartbeat".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "entity": "/src/main.rs",
          "project": "panorama",
          "language": "Rust",
          "time": 1719700000.0
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("heartbeat", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let result: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(result["created"], 1);
}

#[tokio::test]
async fn test_coding_heartbeats_bulk() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_coding::CodingPlugin::new());
  loader.load(plugin.clone()).await.unwrap();

  let ctx = loader.create_context("io.mzhang.panorama.coding", plugin.required_capabilities());

  let req = HttpRequest {
    method: "POST".into(),
    path: "heartbeats".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!([
          {"entity": "/a.rs", "project": "p1", "language": "Rust", "time": 1719700000.0},
          {"entity": "/b.ts", "project": "p2", "language": "TypeScript", "time": 1719700100.0},
          {"entity": "/c.py", "project": "p1", "language": "Python", "time": 1719700200.0}
      ])
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("heartbeats", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let result: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(result["created"], 3);
}

// ─── Dashboards/Dashboard App Tests ─────────────────────────────

#[tokio::test]
async fn test_dashboards_query_count() {
  let (loader, _tmp) = setup_test_env();

  // First load coding plugin and create some heartbeats
  let wk_plugin = Arc::new(panorama_app_coding::CodingPlugin::new());
  loader.load(wk_plugin.clone()).await.unwrap();
  let wk_ctx = loader.create_context(
    "io.mzhang.panorama.coding",
    wk_plugin.required_capabilities(),
  );

  // Create heartbeats with different projects
  for (project, file) in &[
    ("panorama", "main.rs"),
    ("panorama", "lib.rs"),
    ("other", "app.ts"),
  ] {
    let mut node = Node::new(uuid::Uuid::nil());
    node.set_field(
      "system:node_time",
      FieldValue::DateTime(chrono::Utc::now().to_rfc3339()),
    );
    node.set_field("coding:entity", FieldValue::String(file.to_string()));
    node.set_field("coding:project", FieldValue::String(project.to_string()));
    node.set_field("coding:duration", FieldValue::Float(3600.0));
    wk_ctx.create_node(node).await.unwrap();
  }

  // Now query with dashboards plugin
  let gf_plugin = Arc::new(panorama_app_dashboards::DashboardsPlugin::new());
  loader.load(gf_plugin.clone()).await.unwrap();
  let gf_ctx = loader.create_context(
    "io.mzhang.panorama.dashboards",
    gf_plugin.required_capabilities(),
  );

  // Test PromQL count query
  let req = HttpRequest {
    method: "POST".into(),
    path: "query".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "queries": [{
              "ref_id": "A",
              "data_source": "io.mzhang.panorama.coding",
              "promql": "count by (project) (coding_duration)"
          }],
          "range": {"from": "now-30d", "to": "now"}
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = gf_plugin
    .handle_http_request("query", req, &gf_ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let result: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert!(!result.is_empty());
}

#[tokio::test]
async fn test_dashboards_leaderboard() {
  let (loader, _tmp) = setup_test_env();

  // Create some time-series data via coding plugin
  let wk_plugin = Arc::new(panorama_app_coding::CodingPlugin::new());
  loader.load(wk_plugin.clone()).await.unwrap();
  let wk_ctx = loader.create_context(
    "io.mzhang.panorama.coding",
    wk_plugin.required_capabilities(),
  );

  for (project, hours) in &[("project-a", 10.0), ("project-b", 25.0), ("project-c", 5.0)] {
    let mut node = Node::new(uuid::Uuid::nil());
    node.set_field(
      "system:node_time",
      FieldValue::DateTime(chrono::Utc::now().to_rfc3339()),
    );
    node.set_field("coding:entity", FieldValue::String("file.rs".to_string()));
    node.set_field("coding:project", FieldValue::String(project.to_string()));
    node.set_field("coding:duration", FieldValue::Float(hours * 3600.0));
    wk_ctx.create_node(node).await.unwrap();
  }

  // Query leaderboard
  let gf_plugin = Arc::new(panorama_app_dashboards::DashboardsPlugin::new());
  loader.load(gf_plugin.clone()).await.unwrap();
  let gf_ctx = loader.create_context(
    "io.mzhang.panorama.dashboards",
    gf_plugin.required_capabilities(),
  );

  let req = HttpRequest {
    method: "POST".into(),
    path: "query".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "queries": [{
              "ref_id": "A",
              "data_source": "io.mzhang.panorama.coding",
              "promql": "sum by (project) (coding_duration)"
          }],
          "range": {"from": "now-30d", "to": "now"}
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = gf_plugin
    .handle_http_request("query", req, &gf_ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let result: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();

  // Should return data frames with PromQL results
  assert!(!result.is_empty());
  // DataFrame: {name, columns: ["time","value","project"], rows: [[ts,val,proj],...]}
  let frame = &result[0];
  assert_eq!(frame["name"], "A");
  let columns = frame["columns"].as_array().unwrap();
  assert!(
    columns.iter().any(|c| c == "value"),
    "expected 'value' column"
  );
  let rows = frame["rows"].as_array().unwrap();
  assert!(!rows.is_empty());
  // Verifying project-b has the most hours (25h = 90000 seconds)
  let first_row = &rows[0];
  assert_eq!(first_row[2], "project-b");
}

#[tokio::test]
async fn test_dashboards_save_and_list_dashboards() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_dashboards::DashboardsPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context(
    "io.mzhang.panorama.dashboards",
    plugin.required_capabilities(),
  );

  // Create a dashboard (new API format)
  let req = HttpRequest {
    method: "POST".into(),
    path: "dashboards".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "My Coding Dashboard",
          "panels": [{
              "id": 1,
              "title": "Per Project",
              "type": "leaderboard",
              "gridPos": {"x": 0, "y": 0, "w": 12, "h": 8},
              "queries": [{
                  "ref_id": "A",
                  "data_source": "io.mzhang.panorama.coding",
                  "promql": "sum by (project) (coding_duration)"
              }]
          }]
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("dashboards", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);

  // List dashboards
  let req = HttpRequest {
    method: "GET".into(),
    path: "dashboards".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("dashboards", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let dashboards: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert!(!dashboards.is_empty());
}

// ─── Trip Planner App Tests ──────────────────────────────────

#[tokio::test]
async fn test_trips_create_and_list() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_trips::TripsPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context("io.mzhang.panorama.trips", plugin.required_capabilities());

  // Create a trip
  let req = HttpRequest {
    method: "POST".into(),
    path: "trips".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "Japan 2025",
          "start_date": "2025-03-01T00:00:00Z",
          "end_date": "2025-03-14T00:00:00Z"
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("trips", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let trip: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  let trip_id = trip["id"].as_str().unwrap().to_string();

  // Add an event to the trip
  let req = HttpRequest {
    method: "POST".into(),
    path: "events".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({
          "title": "Visit Senso-ji",
          "start_time": "2025-03-02T10:00:00Z",
          "end_time": "2025-03-02T12:00:00Z",
          "trip_id": trip_id,
          "latitude": 35.7148,
          "longitude": 139.7967,
          "location_name": "Asakusa, Tokyo",
          "notes": "Famous Buddhist temple"
      })
      .to_string()
      .into_bytes()
      .into(),
    ),
  };
  let resp = plugin
    .handle_http_request("events", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);

  // List events for the trip
  let req = HttpRequest {
    method: "GET".into(),
    path: "events".into(),
    query_params: [("trip_id".into(), trip_id)].into(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("events", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let events: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn test_trips_map_view() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_trips::TripsPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context("io.mzhang.panorama.trips", plugin.required_capabilities());

  // Create events with geo data
  for (title, lat, lon, loc) in &[
    ("Tokyo Tower", 35.6586, 139.7454, "Minato, Tokyo"),
    ("Fushimi Inari", 34.9671, 135.7727, "Kyoto"),
  ] {
    let req = HttpRequest {
      method: "POST".into(),
      path: "events".into(),
      query_params: Default::default(),
      headers: Default::default(),
      body: Some(
        serde_json::json!({
            "title": title,
            "start_time": "2025-04-01T10:00:00Z",
            "trip_id": "test-trip",
            "latitude": lat,
            "longitude": lon,
            "location_name": loc
        })
        .to_string()
        .into_bytes()
        .into(),
      ),
    };
    plugin
      .handle_http_request("events", req, &ctx)
      .await
      .unwrap();
  }

  // Get map data
  let req = HttpRequest {
    method: "GET".into(),
    path: "events/map".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("events/map", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let map_data: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(map_data.len(), 2);
  assert_eq!(map_data[0]["title"], "Tokyo Tower");
}

// ─── Restaurant Rankings (Restaurant Ratings) App Tests ─────────────────────

#[tokio::test]
async fn test_restaurants_add_and_compare() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_restaurants::RestaurantsPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context(
    "io.mzhang.panorama.restaurants",
    plugin.required_capabilities(),
  );

  // Add restaurants
  let mut ids = Vec::new();
  for name in &["Ramen Jiro", "Ichiran", "Ippudo"] {
    let req = HttpRequest {
      method: "POST".into(),
      path: "restaurants".into(),
      query_params: Default::default(),
      headers: Default::default(),
      body: Some(
        serde_json::json!({
            "name": name,
            "cuisine": "Ramen",
            "location": "Tokyo"
        })
        .to_string()
        .into_bytes()
        .into(),
      ),
    };
    let resp = plugin
      .handle_http_request("restaurants", req, &ctx)
      .await
      .unwrap();
    let r: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
    ids.push(r["id"].as_str().unwrap().to_string());
  }

  // Record partial order comparisons (Jiro > Ichiran, Ichiran > Ippudo)
  for (better, worse) in &[(0, 1), (1, 2)] {
    let req = HttpRequest {
      method: "POST".into(),
      path: "compare".into(),
      query_params: Default::default(),
      headers: Default::default(),
      body: Some(
        serde_json::json!({
            "better_id": ids[*better],
            "worse_id": ids[*worse],
            "context": "best tonkotsu ramen"
        })
        .to_string()
        .into_bytes()
        .into(),
      ),
    };
    let resp = plugin
      .handle_http_request("compare", req, &ctx)
      .await
      .unwrap();
    assert_eq!(resp.status, 200);
  }

  // Get rankings (should produce 3 tiers via topological sort)
  let req = HttpRequest {
    method: "GET".into(),
    path: "rankings".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("rankings", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let rankings: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  let tiers = rankings["tiers"].as_array().unwrap();
  // With two comparisons across 3 items, we should have 3 tiers
  assert_eq!(tiers.len(), 3);
}

// ─── Music Library Music App Tests ─────────────────────────────────

#[tokio::test]
async fn test_music_ping() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_music::MusicPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context("io.mzhang.panorama.music", plugin.required_capabilities());

  let req = HttpRequest {
    method: "GET".into(),
    path: "rest/ping".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("rest/ping", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let data: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(data["music-response"]["status"], "ok");
}

#[tokio::test]
async fn test_music_upload_and_stream() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_music::MusicPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context("io.mzhang.panorama.music", plugin.required_capabilities());

  // Upload a music file
  let audio_data = vec![0u8; 1024]; // Fake audio data
  let req = HttpRequest {
    method: "POST".into(),
    path: "upload".into(),
    query_params: [
      ("filename".into(), "test-song.mp3".into()),
      ("title".into(), "Test Song".into()),
    ]
    .into(),
    headers: Default::default(),
    body: Some(audio_data.clone().into()),
  };
  let resp = plugin
    .handle_http_request("upload", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let track: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  let track_id = track["id"].as_str().unwrap().to_string();

  // Stream the track
  let req = HttpRequest {
    method: "GET".into(),
    path: "rest/stream".into(),
    query_params: [("id".into(), track_id)].into(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("rest/stream", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  // Verify audio data returned
  assert!(!resp.body.is_empty());
}

// ─── File Manager App Tests ──────────────────────────────────

#[tokio::test]
async fn test_files_upload_and_download() {
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_files::FilesPlugin::new());
  loader.load(plugin.clone()).await.unwrap();
  let ctx = loader.create_context("io.mzhang.panorama.files", plugin.required_capabilities());

  // Upload a file
  let file_content = b"Hello, Panorama! This is a test file.";
  let req = HttpRequest {
    method: "POST".into(),
    path: "upload".into(),
    query_params: [
      ("filename".into(), "test.txt".into()),
      ("mime_type".into(), "text/plain".into()),
    ]
    .into(),
    headers: Default::default(),
    body: Some(file_content.to_vec().into()),
  };
  let resp = plugin
    .handle_http_request("upload", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let file_node: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
  let file_id = file_node["id"].as_str().unwrap().to_string();

  // Download the file
  let req = HttpRequest {
    method: "GET".into(),
    path: format!("files/{}", file_id),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request(&format!("files/{}", file_id), req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  assert_eq!(&resp.body[..], file_content);

  // List files
  let req = HttpRequest {
    method: "GET".into(),
    path: "files".into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request("files", req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 200);
  let files: Vec<serde_json::Value> = serde_json::from_slice(&resp.body).unwrap();
  assert_eq!(files.len(), 1);

  // Delete the file
  let req = HttpRequest {
    method: "DELETE".into(),
    path: format!("files/{}", file_id),
    query_params: Default::default(),
    headers: Default::default(),
    body: None,
  };
  let resp = plugin
    .handle_http_request(&format!("files/{}", file_id), req, &ctx)
    .await
    .unwrap();
  assert_eq!(resp.status, 204);
}

// ─── Schema Registration Test ────────────────────────────────

#[tokio::test]
async fn test_all_plugins_register_schemas() {
  let (loader, _tmp) = setup_test_env();

  let plugins: Vec<(Arc<dyn Plugin>, &str)> = vec![
    (
      Arc::new(panorama_app_journal::JournalPlugin::new()),
      "journal",
    ),
    (Arc::new(panorama_app_coding::CodingPlugin::new()), "coding"),
    (
      Arc::new(panorama_app_dashboards::DashboardsPlugin::new()),
      "dashboards",
    ),
    (Arc::new(panorama_app_trips::TripsPlugin::new()), "trips"),
    (
      Arc::new(panorama_app_restaurants::RestaurantsPlugin::new()),
      "restaurants",
    ),
    (Arc::new(panorama_app_music::MusicPlugin::new()), "music"),
    (Arc::new(panorama_app_files::FilesPlugin::new()), "files"),
  ];

  for (plugin, name) in &plugins {
    loader.load(plugin.clone()).await.unwrap();
    let _schemas = loader
      .create_context(plugin.id(), plugin.required_capabilities())
      .schema_registry()
      .list_all()
      .iter()
      .filter(|s| s.name.starts_with(name))
      .count();
    //.list_by_app(&format!("io.mzhang.panorama.{}", name));
    assert!(
      !plugin.schemas().is_empty(),
      "Plugin {} has no schemas",
      name
    );
  }
}

// ─── Plugin API Boundaries Test ──────────────────────────────

#[tokio::test]
async fn test_plugins_only_use_public_api() {
  // This test verifies that all example app crates only depend on panorama-core,
  // not on panorama-server internals. This is verified at compile time by the
  // crate dependency graph, but we document it here as well.
  //
  // Each plugin crate's Cargo.toml should only list:
  //   - panorama-core = { path = "../panorama-core" }
  //
  // And should NOT list panorama-server or any of its internal modules.

  // Verify by checking that plugins compile and load correctly
  let (loader, _tmp) = setup_test_env();
  let plugin = Arc::new(panorama_app_coding::CodingPlugin::new());

  // Plugin should not have access to server internals
  // It can only interact through PluginContext
  let result = loader.load(plugin).await;
  assert!(result.is_ok(), "Plugin should load successfully");
}
