use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::db::Dal;
use serde_yaml;
use std::path::PathBuf;
use serde_json::json;

pub async fn server_main(dal: Dal) -> Result<()> {
    println!("Server main");
    let state = AppState { dal };
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/plugins", get(get_plugins))
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
