use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::{
    db::Dal,
    plugin::{PluginInfo, call_on_plugins_loaded, ipc},
};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;

pub async fn server_main(dal: Dal, plugins: Vec<PluginInfo>) -> Result<()> {
    println!("Server main");
    // Create a broadcast channel for sending events to frontend websocket clients
    let (broad_tx, _) = broadcast::channel::<String>(16);

    // Make sender available to Lua prelude and other code
    ipc::set_sender(broad_tx.clone());

    let state = AppState {
        dal,
        plugins,
        broadcaster: broad_tx.clone(),
    };

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/graphql", post(post_graphql))
        .route("/plugins", get(get_plugins))
        .route("/ws", get(ws_handler))
        .route("/plugins_loaded", post(plugins_loaded_handler))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4141").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    dal: Dal,
    plugins: Vec<PluginInfo>,
    broadcaster: broadcast::Sender<String>,
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

async fn get_plugins(State(state): State<AppState>) -> axum::response::Json<serde_json::Value> {
    // Return the loaded plugin manifests as JSON. We wrap in an object with
    // a "plugins" array for compatibility with the UI which accepts either
    // an array or an object containing a "plugins" key.
    axum::response::Json(serde_json::json!({"plugins": state.plugins}))
}

async fn plugins_loaded_handler(
    State(state): State<AppState>,
) -> axum::response::Json<serde_json::Value> {
    // Call all plugins' on_plugins_loaded handlers
    match call_on_plugins_loaded(&state.plugins) {
        Ok(_) => axum::response::Json(serde_json::json!({"ok": true})),
        Err(e) => axum::response::Json(serde_json::json!({"error": format!("{e:?}")})),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.broadcaster.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    println!("Websocket connection open!");
    // Split socket into sender/receiver
    let (mut sender, mut receiver) = socket.split();

    // Forward broadcast messages to the websocket
    let mut rx2 = rx.resubscribe();
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx2.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Drain incoming messages (we don't handle them)
    while let Some(Ok(_msg)) = receiver.next().await {}

    let _ = send_task.await;
}
