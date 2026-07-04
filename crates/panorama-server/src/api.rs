use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode, Uri},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use panorama_core::plugin::HttpRequest;
use panorama_core::types::{FieldValue, Node};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::plugin_loader::PluginLoader;
use crate::storage::NodeStorage;
use crate::schema_registry::SchemaRegistry;
use crate::object_store::ObjectStorage;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub storage: NodeStorage,
    pub schema_registry: SchemaRegistry,
    pub object_storage: ObjectStorage,
    pub plugin_loader: Arc<PluginLoader>,
}

// -- Error types --

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

impl ApiError {
    fn response(status: StatusCode, code: &str, msg: &str) -> (StatusCode, Json<Self>) {
        (status, Json(Self { error: msg.to_string(), code: code.into() }))
    }
    fn not_found(msg: &str) -> (StatusCode, Json<Self>) { Self::response(StatusCode::NOT_FOUND, "NOT_FOUND", msg) }
    fn internal(msg: String) -> (StatusCode, Json<Self>) { Self::response(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &msg) }
    fn bad_request(msg: &str) -> (StatusCode, Json<Self>) { Self::response(StatusCode::BAD_REQUEST, "BAD_REQUEST", msg) }
}

// -- Router --

pub fn build_router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        // Node CRUD
        .route("/api/nodes", post(create_node).get(query_nodes))
        .route("/api/nodes/{id}", get(get_node).put(update_node).delete(delete_node))
        // Query language
        .route("/api/query", post(query_handler))
        // Schema API
        .route("/api/schemas", get(list_schemas))
        .route("/api/schemas/{id}", get(get_schema))
        // Object storage
        .route("/api/objects/{bucket}/{*key}", get(get_object).put(put_object).delete(delete_object))
        .route("/api/objects/{bucket}", get(list_objects))
        .route("/api/uploads", post(initiate_upload))
        .route("/api/uploads/{id}/chunks", post(upload_chunk))
        .route("/api/uploads/{id}/complete", post(complete_upload))
        // Plugin metadata
        .route("/api/plugins", get(list_plugins))
        .route("/api/plugins/{id}", get(get_plugin))
        .route("/api/plugins/{id}/static", get(get_plugin_static_files))
        .route("/api/plugins/{id}/files", get(get_plugin_static_files))
        // Plugin UI asset serving (must come before catch-all dispatch)
        .route("/plugin/{plugin_id}/ui/{*path}", get(plugin_ui_handler))
        // Plugin HTTP endpoint dispatch
        .route("/plugin/{plugin_id}/{*path}", get(plugin_handler).post(plugin_handler).put(plugin_handler).delete(plugin_handler).patch(plugin_handler))
        // SPA fallback — serve embedded frontend if present
        .fallback(get(frontend_spa_fallback))
        .with_state(state)
}

// -- Request types --

#[derive(Debug, Deserialize)]
struct CreateNodeRequest {
    pub space_id: Option<Uuid>,
    pub fields: Option<HashMap<String, FieldValue>>,
    /// Schemas this node claims to conform to (validated on create)
    #[serde(default)]
    pub schemas: Vec<panorama_core::types::SchemaRef>,
}

#[derive(Debug, Deserialize)]
struct UpdateNodeRequest {
    pub fields: HashMap<String, FieldValue>,
    /// Schemas to add/update (merged with existing, validated on update)
    #[serde(default)]
    pub schemas: Option<Vec<panorama_core::types::SchemaRef>>,
}

#[derive(Debug, Deserialize)]
struct InitiateUploadRequest {
    pub bucket: String,
    pub key: String,
    pub mime_type: Option<String>,
    pub total_size: u64,
}

#[derive(Debug, Deserialize)]
struct UploadChunkRequest {
    pub chunk_index: u32,
    pub data: String,
}

// -- Node handlers --

async fn create_node(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateNodeRequest>,
) -> Result<Json<Node>, (StatusCode, Json<ApiError>)> {
    let mut node = Node::new(req.space_id.unwrap_or_else(Uuid::nil));
    if let Some(fields) = req.fields {
        for (key, value) in fields {
            node.set_field(&key, value);
        }
    }
    node.preferred_schemas = req.schemas;

    // Enforce required schemas before persisting
    if let Err(errors) = state.schema_registry.validate_required(&node.fields, &node.preferred_schemas)
    {
        return Err(ApiError::bad_request(&errors.join("; ")));
    }

    state.storage.create(node).map(Json).map_err(|e| ApiError::internal(e))
}

async fn get_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Node>, (StatusCode, Json<ApiError>)> {
    let node = state.storage.get(id)
        .map_err(|e| ApiError::internal(e))?
        .ok_or_else(|| ApiError::not_found("Node not found"))?;
    Ok(Json(node))
}

async fn update_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateNodeRequest>,
) -> Result<Json<Node>, (StatusCode, Json<ApiError>)> {
    // Fetch existing node to merge schemas and validate
    let existing = state.storage.get(id)
        .map_err(|e| ApiError::internal(e))?
        .ok_or_else(|| ApiError::not_found("Node not found"))?;

    let schemas = if let Some(new_schemas) = req.schemas {
        new_schemas
    } else {
        existing.preferred_schemas.clone()
    };

    // Build the merged fields for validation
    let mut merged = existing.fields.clone();
    for (key, value) in &req.fields {
        merged.insert(key.clone(), value.clone());
    }

    // Enforce required schemas with merged fields
    if let Err(errors) = state.schema_registry.validate_required(&merged, &schemas) {
        return Err(ApiError::bad_request(&errors.join("; ")));
    }

    state.storage.update(id, req.fields).map(Json).map_err(|e| ApiError::internal(e))
}

async fn delete_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    state.storage.delete(id).map(|_| StatusCode::NO_CONTENT).map_err(|e| ApiError::internal(e))
}

async fn query_nodes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    // Build a Panorama Query Language query from URL params
    let mut qs = String::from("MATCH (n) IN space(\"default\")");

    // Field filters
    let mut preds: Vec<String> = Vec::new();
    for (key, _value) in &params {
        if let Some(field_key) = key.strip_prefix("filter.") {
            if let Some((ns, f)) = field_key.split_once(':') {
                preds.push(format!("HAS_FIELD(n, \"{}\", \"{}\")", ns, f));
            } else {
                preds.push(format!("HAS_FIELD(n, \"*\", \"{}\")", field_key));
            }
        }
    }
    if !preds.is_empty() {
        qs.push_str(" WHERE ");
        qs.push_str(&preds.join(" AND "));
    }

    qs.push_str(" RETURN n");

    // Sort
    if let Some(sort) = params.get("sort_by") {
        let descending = sort.starts_with('-');
        let field = if descending { &sort[1..] } else { sort.as_str() };
        let dir = if descending { "DESC" } else { "ASC" };
        if let Some((ns, f)) = field.split_once(':') {
            qs.push_str(&format!(" ORDER BY n.{}.{} {}", ns, f, dir));
        } else {
            qs.push_str(&format!(" ORDER BY n.{} {}", field, dir));
        }
    }

    // Limit / offset
    if let Some(l) = params.get("limit") {
        qs.push_str(&format!(" LIMIT {}", l));
    }
    if let Some(o) = params.get("offset") {
        qs.push_str(&format!(" SKIP {}", o));
    }

    execute_query(&state, &qs).map(Json)
}

// -- Schema handlers --

async fn list_schemas(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let v: Vec<_> = state.schema_registry.list_all().iter()
        .map(|s| serde_json::to_value(s).unwrap_or_default()).collect();
    Json(v)
}

async fn get_schema(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let s = state.schema_registry.get(&id).ok_or_else(|| ApiError::not_found("Schema not found"))?;
    Ok(Json(serde_json::to_value(s).unwrap_or_default()))
}

// -- Object storage handlers --

async fn put_object(
    State(state): State<Arc<AppState>>,
    Path((bucket, key)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let obj = state.object_storage.put(&bucket, &key, &body, "application/octet-stream")
        .map_err(|e| ApiError::internal(e))?;
    Ok(Json(serde_json::to_value(obj).unwrap_or_default()))
}

async fn get_object(
    State(state): State<Arc<AppState>>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    let (data, mime_type) = state.object_storage.get(&bucket, &key)
        .map_err(|e| ApiError::internal(e))?
        .ok_or_else(|| ApiError::not_found("Object not found"))?;
    let mime: mime::Mime = mime_type.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .body(axum::body::Body::from(data))
        .unwrap())
}

async fn delete_object(
    State(state): State<Arc<AppState>>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    state.object_storage.delete(&bucket, &key).map(|_| StatusCode::NO_CONTENT).map_err(|e| ApiError::internal(e))
}

async fn list_objects(
    State(state): State<Arc<AppState>>,
    Path(bucket): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let prefix = params.get("prefix").map(|s| s.as_str());
    let objects = state.object_storage.list(&bucket, prefix).map_err(|e| ApiError::internal(e))?;
    let v: Vec<_> = objects.iter().map(|o| serde_json::to_value(o).unwrap_or_default()).collect();
    Ok(Json(v))
}

async fn initiate_upload(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InitiateUploadRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let upload_id = state.object_storage.initiate_upload(
        &req.bucket, &req.key,
        req.mime_type.as_deref().unwrap_or("application/octet-stream"),
        req.total_size,
    ).map_err(|e| ApiError::internal(e))?;
    Ok(Json(serde_json::json!({"upload_id": upload_id.to_string()})))
}

async fn upload_chunk(
    State(state): State<Arc<AppState>>,
    Path(upload_id): Path<Uuid>,
    Json(req): Json<UploadChunkRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(&req.data)
        .map_err(|e| ApiError::bad_request(&format!("Invalid base64: {}", e)))?;
    state.object_storage.upload_chunk(&upload_id, req.chunk_index, data)
        .map(|_| StatusCode::OK).map_err(|e| ApiError::internal(e))
}

async fn complete_upload(
    State(state): State<Arc<AppState>>,
    Path(upload_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let obj = state.object_storage.complete_upload(&upload_id).map_err(|e| ApiError::internal(e))?;
    Ok(Json(serde_json::to_value(obj).unwrap_or_default()))
}

// -- Plugin metadata handlers --

async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let v: Vec<_> = state.plugin_loader.list_all().await.iter().map(|p| serde_json::json!({
        "id": p.info.id, "name": p.info.name, "version": p.info.version,
        "description": p.info.description, "endpoints": p.endpoints,
        "ui_components": p.ui_components,
    })).collect();
    Json(v)
}

async fn get_plugin(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let p = state.plugin_loader.get(&id).await
        .ok_or_else(|| ApiError::not_found("Plugin not found"))?;
    let static_files = state.plugin_loader.list_ui_files(&id).await;
    Ok(Json(serde_json::json!({
        "id": p.info.id, "name": p.info.name, "version": p.info.version,
        "description": p.info.description, "schemas": p.schemas,
        "endpoints": p.endpoints, "ui_components": p.ui_components,
        "static_files": static_files,
    })))
}

async fn get_plugin_static_files(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let files = state
        .plugin_loader
        .list_ui_files(&id)
        .await
        .ok_or_else(|| ApiError::not_found("Plugin static files not found"))?;
    Ok(Json(serde_json::json!({
        "plugin_id": id,
        "files": files,
    })))
}

// -- Plugin HTTP dispatch --

#[axum::debug_handler]
async fn plugin_handler(
    State(state): State<Arc<AppState>>,
    Path((plugin_id, path)): Path<(String, String)>,
    method: axum::http::Method,
    Query(query_params): Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    // Build the HttpRequest
    let header_map: HashMap<String, String> = headers.iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let req = HttpRequest {
        method: method.to_string(),
        path: path.clone(),
        query_params,
        headers: header_map,
        body: if body.is_empty() { None } else { Some(body) },
    };

    // The endpoint is the path (strip leading slash if present)
    let endpoint = path.trim_start_matches('/');

    match state.plugin_loader.dispatch_http(&plugin_id, endpoint, req).await {
        Ok(response) => {
            let mut builder = Response::builder().status(StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK));
            for (k, v) in &response.headers {
                builder = builder.header(k.as_str(), v.as_str());
            }
            Ok(builder.body(axum::body::Body::from(response.body)).unwrap())
        }
        Err(e) => {
            let status = StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Err((status, Json(ApiError { error: e.message, code: e.code })))
        }
    }
}

// -- Plugin UI asset handler --

async fn plugin_ui_handler(
    State(state): State<Arc<AppState>>,
    Path((plugin_id, path)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    match state.plugin_loader.get_ui_file(&plugin_id, &path).await {
        Some((data, mime)) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .body(axum::body::Body::from(data))
            .unwrap()),
        None => Err(ApiError::not_found("UI asset not found")),
    }
}

// -- Frontend SPA fallback (embedded via rust-embed) --

/// Serves the production frontend build embedded in the binary.
/// Tries to serve the exact static file first; falls back to `index.html`
/// for client-side (SPA) routing.  When the frontend isn't embedded (dev
/// builds without a pre-built `frontend/dist/`), returns 404.
async fn frontend_spa_fallback(
    uri: Uri,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    let path = uri.path().trim_start_matches('/');

    if let Some(resp) = crate::frontend::try_serve(path) {
        return Ok(resp);
    }
    if let Some(resp) = crate::frontend::serve_index() {
        return Ok(resp);
    }
    Err(ApiError::not_found("Not found"))
}

// -- Query language handler --

#[derive(Debug, Deserialize)]
struct QueryRequest {
    pub query: String,
}

fn execute_query(
    state: &AppState,
    query_string: &str,
) -> Result<serde_json::Value, (StatusCode, Json<ApiError>)> {
    let rows = state.storage.query_lang(query_string)
        .map_err(|e| ApiError::bad_request(&format!("Query error: {}", e)))?;
    Ok(serde_json::json!({
        "rows": rows,
        "count": rows.len(),
    }))
}

async fn query_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    execute_query(&state, &req.query).map(Json)
}
