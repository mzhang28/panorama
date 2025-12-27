use panorama_core::apps::AppManager;
use panorama_core::db::DbClient;
use std::path::Path;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::{
  http::{Response, StatusCode},
  Manager, Runtime,
};
use tokio::sync::Mutex;

pub struct AppState {
  pub app_manager: Arc<Mutex<AppManager>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .setup(|app| {
      let handle = app.handle().clone();
      tauri::async_runtime::spawn(async move {
        let db = DbClient::new().await.expect("Failed to connect to DB");
        db.init().await.expect("Failed to init DB");
        let mut manager = AppManager::new(db);

        // Load journal app for now
        let journal_path = Path::new("../apps/journal").canonicalize().unwrap();
        let err = manager.load_app(&journal_path).await;
        println!("Loading journal: {:?}", err);

        handle.manage(AppState {
          app_manager: Arc::new(Mutex::new(manager)),
        });
      });
      Ok(())
    })
    .register_asynchronous_uri_scheme_protocol("panorama-static", move |ctx, request, responder| {
      let app_handle = ctx.app_handle().clone();
      tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let uri = request.uri();
        let host = uri.host().unwrap_or("");

        println!("got a request: {uri:?}");

        let manager = state.app_manager.lock().await;
        println!("manager: {:?}", manager);
        if let Some(app_info) = manager.get_app(host) {
          if let Some(ref frontend_server) = app_info.frontend_server {
            // Redirect to the dev server
            // For a real proxy we'd fetch and return, but for now we can try a redirect if supported or just return a simple response
            // Actually Tauri's custom protocol expects a response.
            // We'll do a simple fetch proxy for dev.
            let client = reqwest::Client::new();
            let path = uri.path();
            let target_url = format!("{}{}", frontend_server, path);

            match client.get(&target_url).send().await {
              Ok(res) => {
                let status = res.status().as_u16();
                let content_type = res.headers().get("Content-Type").unwrap().to_owned();
                println!("Content Type: {:?}", content_type);
                let body = res.bytes().await.unwrap_or_default();
                let response = Response::builder()
                  .status(status)
                  .header("Content-Type", content_type)
                  .header("Access-Control-Allow-Origin", "*")
                  .body(body.to_vec())
                  .unwrap();
                responder.respond(response);
              }
              Err(err) => {
                eprintln!("Some other error {err:?}.");
                responder.respond(
                  Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Failed to proxy to dev server".as_bytes().to_vec())
                    .unwrap(),
                );
              }
            }
          } else {
            eprintln!("No frontend_server.");
            responder.respond(
              Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(
                  "App found but no frontend_server defined"
                    .as_bytes()
                    .to_vec(),
                )
                .unwrap(),
            );
          }
        } else {
          eprintln!("App not found.");
          responder.respond(
            Response::builder()
              .status(StatusCode::NOT_FOUND)
              .body("App not found".as_bytes().to_vec())
              .unwrap(),
          );
        }
      });
    })
    .register_asynchronous_uri_scheme_protocol("panorama-api", move |ctx, request, responder| {
      let app_handle = ctx.app_handle().clone();
      tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let uri = request.uri();
        let app_name = uri.host().unwrap_or("");
        let path = uri.path();
        let func_name = path.strip_prefix("/").unwrap_or(path);

        println!("panorama-api call: app={}, func={}", app_name, func_name);

        let method = request.method().as_str().to_string();
        let body_bytes = request.body();
        let body_json: serde_json::Value = if !body_bytes.is_empty() {
          serde_json::from_slice(body_bytes).unwrap_or(serde_json::Value::Null)
        } else {
          serde_json::Value::Null
        };

        let req_obj = serde_json::json!({
          "method": method,
          "body": body_json
        });

        let manager = state.app_manager.lock().await;
        match manager.call_app_function(app_name, func_name, req_obj) {
          Ok(res) => {
            let res_bytes = serde_json::to_vec(&res).unwrap_or_default();
            let response = Response::builder()
              .status(StatusCode::OK)
              .header("Content-Type", "application/json")
              .header("Access-Control-Allow-Origin", "*")
              .body(res_bytes)
              .unwrap();
            responder.respond(response);
          }
          Err(e) => {
            eprintln!("Lua call error: {:?}", e);
            responder.respond(
              Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(format!("Error: {:?}", e).into_bytes())
                .unwrap(),
            );
          }
        }
      });
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
