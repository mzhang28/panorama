use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::{db::Dal, plugin::PluginInfo};

pub async fn server_main(dal: Dal, plugins: Vec<PluginInfo>) -> Result<()> {
    println!("Server main");
    let state = AppState { dal, plugins };
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/graphql", post(post_graphql))
        .route("/plugins", get(get_plugins))
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
