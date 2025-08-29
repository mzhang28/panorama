use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
    response::IntoResponse,
};

use crate::db::Dal;
use serde_yaml;
use std::path::PathBuf;
use serde_json::json;
use std::time::Duration;
use std::sync::mpsc as std_mpsc;
use serde::Deserialize as SerdeDeserialize;

pub async fn server_main(dal: Dal) -> Result<()> {
    println!("Server main");
    let state = AppState { dal };

    // Create a broadcast channel for pushing UI events to websocket clients.
    let (tx, _rx) = tokio::sync::broadcast::channel::<crate::apps::UiEvent>(100);
    crate::apps::set_ui_broadcast(tx.clone());

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/plugins", get(get_plugins))
        .route("/startup", get(get_startup))
        .route("/call_app_function", post(call_app_function))
        .route("/ui/events", get(get_ui_events))
        .route("/ws/ui", get(ws_ui_handler))
        .route("/graphql", post(post_graphql))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4141").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    dal: Dal,
}

#[derive(Debug, Deserialize)]
struct PostGraphqlReq {
    query: String,
    variables: Option<serde_json::Value>,
}

async fn post_graphql(
    State(state): State<AppState>,
    request: Json<PostGraphqlReq>,
) -> axum::response::Json<serde_json::Value> {
    println!("request: {:?}", request);

    match crate::graphql::process_graphql_request(
        state.dal.clone(),
        request.query.clone(),
        request.variables.clone(),
    )
    .await
    {
        Ok(v) => axum::response::Json(v),
        Err(e) => axum::response::Json(serde_json::json!({"error": format!("{e:?}")})),
    }
}

async fn get_plugins() -> axum::response::Json<serde_json::Value> {
    // Scan ./apps for manifest.yaml entries and return matching ui_plugins
    let mut out = Vec::new();
    let apps_dir = PathBuf::from("apps");
    if apps_dir.exists() && apps_dir.is_dir() {
        if let Ok(dir) = std::fs::read_dir(&apps_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("manifest.yaml");
                    if manifest_path.exists() {
                        if let Ok(s) = std::fs::read_to_string(&manifest_path) {
                            if let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&s) {
                                if let Some(ui_plugins) = v.get("ui_plugins") {
                                    if let Some(arr) = ui_plugins.as_sequence() {
                                        // determine current target triple
                                        let triple = {
                                            let arch = if cfg!(target_arch = "x86_64") {
                                                "x86_64"
                                            } else if cfg!(target_arch = "aarch64") {
                                                "aarch64"
                                            } else if cfg!(target_arch = "arm") {
                                                "arm"
                                            } else {
                                                "unknown"
                                            };
                                            let os_part = if cfg!(target_os = "macos") {
                                                "apple-darwin"
                                            } else if cfg!(target_os = "linux") {
                                                if cfg!(target_env = "musl") {
                                                    "unknown-linux-musl"
                                                } else {
                                                    "unknown-linux-gnu"
                                                }
                                            } else if cfg!(target_os = "windows") {
                                                "pc-windows-msvc"
                                            } else {
                                                "unknown"
                                            };
                                            format!("{}-{}", arch, os_part)
                                        };

                                        for item in arr.iter() {
                                            if let Some(t) = item.get("target").and_then(|x| x.as_str()) {
                                                if t == triple {
                                                    if let Some(p) = item.get("path").and_then(|x| x.as_str()) {
                                                        let plugin_path = path.join(p).to_string_lossy().to_string();
                                                        out.push(json!({"app_dir": path.to_string_lossy(), "path": plugin_path}));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    axum::response::Json(serde_json::Value::Array(out))
}

async fn get_startup() -> axum::response::Json<serde_json::Value> {
    // Read panorama-system/config.yaml and return the startup list
    let cfg = std::fs::read_to_string("panorama-system/config.yaml").unwrap_or_default();
    match serde_yaml::from_str::<serde_yaml::Value>(&cfg) {
        Ok(v) => {
            if let Some(startup) = v.get("startup") {
                axum::response::Json(serde_json::to_value(startup).unwrap_or(serde_json::json!([])))
            } else {
                axum::response::Json(serde_json::json!([]))
            }
        }
        Err(_) => axum::response::Json(serde_json::json!([])),
    }
}

#[derive(SerdeDeserialize)]
struct CallAppReq {
    app: String,
    function: String,
    args: Option<Vec<String>>,
    origin_widget: Option<String>,
}

async fn call_app_function(
    State(_state): State<AppState>,
    axum::Json(req): axum::Json<CallAppReq>,
) -> axum::response::Json<serde_json::Value> {
    // find worker sender for app
    let key = format!("apps/{}", req.app);
    let sender_opt = {
        let reg = crate::apps::LUA_WORKERS.lock().unwrap();
        reg.get(&key).cloned()
    };

    if sender_opt.is_none() {
        return axum::response::Json(serde_json::json!({"error": "no such app worker"}));
    }

    let sender = sender_opt.unwrap();
    let (tx, rx) = std_mpsc::channel();
    let args = req.args.unwrap_or_default();
    if let Err(_) = sender.send((req.function.clone(), args, req.origin_widget.clone(), tx)) {
        return axum::response::Json(serde_json::json!({"error": "failed to send request to app"}));
    }

    // Block for a short timeout waiting for the Lua worker's reply
    let res = tokio::task::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(2))).await;
    match res {
        Ok(Ok(Ok(s))) => axum::response::Json(serde_json::json!({"result": s})),
        Ok(Ok(Err(e))) => axum::response::Json(serde_json::json!({"error": e})),
        Ok(Err(std_mpsc::RecvTimeoutError::Timeout)) => axum::response::Json(serde_json::json!({"error": "timeout"})),
        _ => axum::response::Json(serde_json::json!({"error": "unknown"})),
    }
}

async fn get_ui_events() -> axum::response::Json<serde_json::Value> {
    // Drain the UI_EVENTS queue and return the events as JSON
    let mut out = Vec::new();
    {
        let mut q = crate::apps::UI_EVENTS.lock().unwrap();
        for ev in q.drain(..) {
            out.push(json!({"source": ev.source, "url": ev.url, "widget_id": ev.widget_id, "direction": ev.direction}));
        }
    }
    axum::response::Json(serde_json::Value::Array(out))
}

async fn ws_ui_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ui_ws)
}

async fn handle_ui_ws(mut socket: WebSocket) {
    // Subscribe to the broadcast channel (if available)
    let mut rx_opt = crate::apps::subscribe_ui_broadcast();
    if rx_opt.is_none() {
        // No broadcast channel available; simply return and drop the socket
        return;
    }

    let mut rx = rx_opt.unwrap();

    // Split the websocket stream so we can read and write concurrently
    use futures_util::{SinkExt, StreamExt};
    let (mut sender, mut receiver) = socket.split();

    // Task: forward broadcast->socket
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if let Ok(text) = serde_json::to_string(&ev) {
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    // Task: drain incoming messages (keep connection alive)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(_t) => {
                    // Currently ignore client messages
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    let _ = tokio::select! {
        _ = (&mut send_task) => { recv_task.abort(); }
        _ = (&mut recv_task) => { send_task.abort(); }
    };
}
