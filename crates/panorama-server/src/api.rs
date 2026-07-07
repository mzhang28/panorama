use std::collections::HashMap;
use std::sync::Arc;

use axum::{
  extract::{Path, Query, State},
  http::{header, StatusCode, Uri},
  response::{Json, Response},
  routing::{get, post, put},
  Router,
};
use bytes::Bytes;
use panorama_core::plugin::HttpRequest;
use panorama_core::reactor::HookPoint;
use panorama_core::types::{FieldValue, Node};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::object_store::ObjectStorage;
use crate::plugin_loader::{PluginLoadState, PluginLoader};
use crate::reactor::deferred::DeferredReactorEngine;
use crate::reactor::eager::{EagerReactorPipeline, HookContext, HookResult};
use crate::reactor::op_stream::OpStream;
use crate::reactor::registry::ReactorRegistry;
use crate::schema_registry::SchemaRegistry;
use crate::storage::NodeStorage;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
  pub storage: NodeStorage,
  pub schema_registry: SchemaRegistry,
  pub object_storage: ObjectStorage,
  pub plugin_loader: Arc<PluginLoader>,
  pub load_state: Arc<tokio::sync::RwLock<PluginLoadState>>,
  pub reactor_registry: Arc<ReactorRegistry>,
  pub eager_pipeline: Arc<EagerReactorPipeline>,
  pub op_stream: Arc<OpStream>,
  pub deferred_engine: Arc<DeferredReactorEngine>,
}

// -- Error types --

#[derive(Debug, Serialize)]
pub struct ApiError {
  pub error: String,
  pub code: String,
}

impl ApiError {
  fn response(status: StatusCode, code: &str, msg: &str) -> (StatusCode, Json<Self>) {
    (
      status,
      Json(Self {
        error: msg.to_string(),
        code: code.into(),
      }),
    )
  }
  fn not_found(msg: &str) -> (StatusCode, Json<Self>) {
    Self::response(StatusCode::NOT_FOUND, "NOT_FOUND", msg)
  }
  fn internal(msg: String) -> (StatusCode, Json<Self>) {
    tracing::error!(error = %msg, "Internal server error");
    Self::response(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &msg)
  }
  fn bad_request(msg: &str) -> (StatusCode, Json<Self>) {
    Self::response(StatusCode::BAD_REQUEST, "BAD_REQUEST", msg)
  }
}

// -- Router --

pub fn build_router(state: AppState) -> Router {
  let state = Arc::new(state);
  Router::new()
    // Node CRUD
    .route("/api/nodes", post(create_node).get(query_nodes))
    .route(
      "/api/nodes/{id}",
      get(get_node).put(update_node).delete(delete_node),
    )
    // Query language
    .route("/api/query", post(query_handler))
    // Schema API
    .route("/api/schemas", get(list_schemas))
    .route("/api/schemas/{id}", get(get_schema))
    // Object storage
    .route(
      "/api/objects/{bucket}/{*key}",
      get(get_object).put(put_object).delete(delete_object),
    )
    .route("/api/objects/{bucket}", get(list_objects))
    .route("/api/uploads", post(initiate_upload))
    .route("/api/uploads/{id}/chunks", post(upload_chunk))
    .route("/api/uploads/{id}/complete", post(complete_upload))
    // Plugin metadata
    .route("/api/plugins", get(list_plugins))
    .route("/api/plugins/status", get(get_plugin_load_status))
    .route("/api/plugins/{id}", get(get_plugin))
    .route("/api/plugins/{id}/static", get(get_plugin_static_files))
    .route("/api/plugins/{id}/files", get(get_plugin_static_files))
    // Reactor management endpoints
    .route("/api/reactors", get(list_reactors).post(create_reactor))
    .route(
      "/api/reactors/{id}",
      get(get_reactor).delete(delete_reactor),
    )
    .route("/api/reactors/{id}/status", put(update_reactor_status))
    // Plugin UI asset serving (must come before catch-all dispatch)
    .route("/plugin/{plugin_id}/ui/{*path}", get(plugin_ui_handler))
    // Plugin HTTP endpoint dispatch
    .route(
      "/plugin/{plugin_id}/{*path}",
      get(plugin_handler)
        .post(plugin_handler)
        .put(plugin_handler)
        .delete(plugin_handler)
        .patch(plugin_handler),
    )
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
  if let Err(errors) = state
    .schema_registry
    .validate_required(&node.fields, &node.preferred_schemas)
  {
    return Err(ApiError::bad_request(&errors.join("; ")));
  }

  // ── Eager reactor hooks: before_node_create ──────────────────────────
  let hook_ctx = HookContext {
    hook_point: HookPoint::BeforeNodeCreate {
      scope_schema_id: node.preferred_schemas.first().map(|s| s.schema_node_id),
    },
    node: Some(node.clone()),
    node_id: None,
    field_path: None,
    current_value: None,
    previous_value: None,
    schema_id: node.preferred_schemas.first().map(|s| s.schema_node_id),
    space_id: Some(node.space_id),
    authorized_by: None,
  };

  match state.eager_pipeline.execute_hook(&hook_ctx).await {
    HookResult::Rejected { reason, .. } => {
      return Err(ApiError::bad_request(&format!(
        "Reactor rejected: {}",
        reason
      )));
    }
    HookResult::Approved {
      computed_fields, ..
    } => {
      for (key, value) in &computed_fields {
        node.set_field(key, value.clone());
      }
    }
  }

  // ── Eager reactor hooks: per-field before_field_write ──────────────────
  // Fire BeforeFieldWrite for each initial field value concurrently.
  // Field-scoped validate/transform reactors are independent — no hook
  // depends on another field's transformation result, so running them
  // in parallel eliminates the N×latency penalty (§1.2).
  let field_keys: Vec<String> = node.fields.keys().cloned().collect();
  if !field_keys.is_empty() {
    let pipeline = state.eager_pipeline.clone();
    let futures: Vec<_> = field_keys
      .iter()
      .map(|field_path| {
        let current_value = node.fields.get(field_path).cloned();
        let field_ctx = HookContext {
          hook_point: HookPoint::BeforeFieldWrite {
            field_path: field_path.clone(),
            scope_schema_id: None,
          },
          node: Some(node.clone()),
          node_id: None,
          field_path: Some(field_path.clone()),
          current_value,
          previous_value: None,
          schema_id: None,
          space_id: Some(node.space_id),
          authorized_by: None,
        };
        let pipeline = pipeline.clone();
        async move { (field_path.clone(), pipeline.execute_hook(&field_ctx).await) }
      })
      .collect();

    let results = futures::future::join_all(futures).await;
    for (field_path, result) in results {
      match result {
        HookResult::Rejected { reason, .. } => {
          return Err(ApiError::bad_request(&format!(
            "Reactor rejected field '{}': {}",
            field_path, reason
          )));
        }
        HookResult::Approved {
          transformed_value,
          computed_fields,
          ..
        } => {
          if let Some(tv) = transformed_value {
            node.set_field(&field_path, tv);
          }
          for (key, value) in &computed_fields {
            node.set_field(key, value.clone());
          }
        }
      }
    }
  }

  let created = state
    .storage
    .create(node)
    .map_err(|e| ApiError::internal(e))?;

  // ── Op stream: record the creation ───────────────────────────────────
  let _ = state.op_stream.append_sync(
    panorama_core::reactor::OpType::NodeCreated,
    Some(created.id),
    created.preferred_schemas.first().map(|s| s.schema_node_id),
    created.space_id,
    None,
    None,
    None,
    None,
    None,
  );

  Ok(Json(created))
}

async fn get_node(
  State(state): State<Arc<AppState>>,
  Path(id): Path<Uuid>,
) -> Result<Json<Node>, (StatusCode, Json<ApiError>)> {
  let node = state
    .storage
    .get(id)
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
  let existing = state
    .storage
    .get(id)
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

  // ── Eager reactor hooks: before_field_write per field (concurrent) ───
  let mut final_fields = req.fields.clone();
  if !req.fields.is_empty() {
    let pipeline = state.eager_pipeline.clone();
    let scope_schema_id = schemas.first().map(|s| s.schema_node_id);
    let futures: Vec<_> = req
      .fields
      .iter()
      .map(|(field_path, new_value)| {
        let prev = existing.fields.get(field_path).cloned();
        let hook_ctx = HookContext {
          hook_point: HookPoint::BeforeFieldWrite {
            field_path: field_path.clone(),
            scope_schema_id,
          },
          node: Some(existing.clone()),
          node_id: Some(id),
          field_path: Some(field_path.clone()),
          current_value: Some(new_value.clone()),
          previous_value: prev,
          schema_id: scope_schema_id,
          space_id: Some(existing.space_id),
          authorized_by: None,
        };
        let pipeline = pipeline.clone();
        async move { (field_path.clone(), pipeline.execute_hook(&hook_ctx).await) }
      })
      .collect();

    let results = futures::future::join_all(futures).await;
    for (field_path, result) in results {
      match result {
        HookResult::Rejected { reason, .. } => {
          return Err(ApiError::bad_request(&format!(
            "Reactor rejected field '{}': {}",
            field_path, reason
          )));
        }
        HookResult::Approved {
          transformed_value,
          computed_fields,
        } => {
          if let Some(tv) = transformed_value {
            final_fields.insert(field_path.clone(), tv);
          }
          for (key, value) in &computed_fields {
            final_fields.insert(key.clone(), value.clone());
          }
        }
      }
    }
  }

  let updated = state
    .storage
    .update(id, final_fields)
    .map_err(|e| ApiError::internal(e))?;

  // ── Op stream: record the update ────────────────────────────────────
  let _ = state.op_stream.append_sync(
    panorama_core::reactor::OpType::NodeUpdated,
    Some(updated.id),
    updated.preferred_schemas.first().map(|s| s.schema_node_id),
    updated.space_id,
    None,
    None,
    None,
    None,
    None,
  );

  Ok(Json(updated))
}

async fn delete_node(
  State(state): State<Arc<AppState>>,
  Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
  let existing = state.storage.get(id).map_err(|e| ApiError::internal(e))?;

  // ── Eager reactor hooks: before_node_delete ──────────────────────────
  let hook_ctx = HookContext {
    hook_point: HookPoint::BeforeNodeDelete {
      scope_schema_id: existing
        .as_ref()
        .and_then(|n| n.preferred_schemas.first().map(|s| s.schema_node_id)),
    },
    node: existing.clone(),
    node_id: Some(id),
    field_path: None,
    current_value: None,
    previous_value: None,
    schema_id: existing
      .as_ref()
      .and_then(|n| n.preferred_schemas.first().map(|s| s.schema_node_id)),
    space_id: existing.as_ref().map(|n| n.space_id),
    authorized_by: None,
  };

  match state.eager_pipeline.execute_hook(&hook_ctx).await {
    HookResult::Rejected { reason, .. } => {
      return Err(ApiError::bad_request(&format!(
        "Reactor rejected delete: {}",
        reason
      )));
    }
    HookResult::Approved { .. } => {}
  }

  let space_id = existing
    .as_ref()
    .map(|n| n.space_id)
    .unwrap_or_else(Uuid::nil);

  state
    .storage
    .delete(id)
    .map(|_| {
      let _ = state.op_stream.append_sync(
        panorama_core::reactor::OpType::NodeDeleted,
        Some(id),
        None,
        space_id,
        None,
        None,
        None,
        None,
        None,
      );
      StatusCode::NO_CONTENT
    })
    .map_err(|e| ApiError::internal(e))
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
    let field = if descending {
      &sort[1..]
    } else {
      sort.as_str()
    };
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
  let v: Vec<_> = state
    .schema_registry
    .list_all()
    .iter()
    .map(|s| serde_json::to_value(s).unwrap_or_default())
    .collect();
  Json(v)
}

async fn get_schema(
  State(state): State<Arc<AppState>>,
  Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  let s = state
    .schema_registry
    .get(&id)
    .ok_or_else(|| ApiError::not_found("Schema not found"))?;
  Ok(Json(serde_json::to_value(s).unwrap_or_default()))
}

// -- Object storage handlers --

async fn put_object(
  State(state): State<Arc<AppState>>,
  Path((bucket, key)): Path<(String, String)>,
  body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  let obj = state
    .object_storage
    .put(&bucket, &key, &body, "application/octet-stream")
    .map_err(|e| ApiError::internal(e))?;
  Ok(Json(serde_json::to_value(obj).unwrap_or_default()))
}

async fn get_object(
  State(state): State<Arc<AppState>>,
  Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
  let (data, mime_type) = state
    .object_storage
    .get(&bucket, &key)
    .map_err(|e| ApiError::internal(e))?
    .ok_or_else(|| ApiError::not_found("Object not found"))?;
  let mime: mime::Mime = mime_type.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);
  Ok(
    Response::builder()
      .status(StatusCode::OK)
      .header(header::CONTENT_TYPE, mime.to_string())
      .body(axum::body::Body::from(data))
      .unwrap(),
  )
}

async fn delete_object(
  State(state): State<Arc<AppState>>,
  Path((bucket, key)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
  state
    .object_storage
    .delete(&bucket, &key)
    .map(|_| StatusCode::NO_CONTENT)
    .map_err(|e| ApiError::internal(e))
}

async fn list_objects(
  State(state): State<Arc<AppState>>,
  Path(bucket): Path<String>,
  Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
  let prefix = params.get("prefix").map(|s| s.as_str());
  let objects = state
    .object_storage
    .list(&bucket, prefix)
    .map_err(|e| ApiError::internal(e))?;
  let v: Vec<_> = objects
    .iter()
    .map(|o| serde_json::to_value(o).unwrap_or_default())
    .collect();
  Ok(Json(v))
}

async fn initiate_upload(
  State(state): State<Arc<AppState>>,
  Json(req): Json<InitiateUploadRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  let upload_id = state
    .object_storage
    .initiate_upload(
      &req.bucket,
      &req.key,
      req
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream"),
      req.total_size,
    )
    .map_err(|e| ApiError::internal(e))?;
  Ok(Json(
    serde_json::json!({"upload_id": upload_id.to_string()}),
  ))
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
  state
    .object_storage
    .upload_chunk(&upload_id, req.chunk_index, data)
    .map(|_| StatusCode::OK)
    .map_err(|e| ApiError::internal(e))
}

async fn complete_upload(
  State(state): State<Arc<AppState>>,
  Path(upload_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  let obj = state
    .object_storage
    .complete_upload(&upload_id)
    .map_err(|e| ApiError::internal(e))?;
  Ok(Json(serde_json::to_value(obj).unwrap_or_default()))
}

// -- Plugin load status (for frontend long-polling during startup) --

async fn get_plugin_load_status(State(state): State<Arc<AppState>>) -> Json<PluginLoadState> {
  let status = state.load_state.read().await.clone();
  Json(status)
}

// -- Plugin metadata handlers --

async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
  let v: Vec<_> = state
    .plugin_loader
    .list_all()
    .await
    .iter()
    .map(|p| {
      serde_json::json!({
          "id": p.info.id, "name": p.info.name, "version": p.info.version,
          "description": p.info.description, "endpoints": p.endpoints,
          "ui_components": p.ui_components,
      })
    })
    .collect();
  Json(v)
}

async fn get_plugin(
  State(state): State<Arc<AppState>>,
  Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  let p = state
    .plugin_loader
    .get(&id)
    .await
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
  let header_map: HashMap<String, String> = headers
    .iter()
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

  match state
    .plugin_loader
    .dispatch_http(&plugin_id, endpoint, req)
    .await
  {
    Ok(response) => {
      let mut builder =
        Response::builder().status(StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK));
      for (k, v) in &response.headers {
        builder = builder.header(k.as_str(), v.as_str());
      }
      Ok(builder.body(axum::body::Body::from(response.body)).unwrap())
    }
    Err(e) => {
      let status = StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
      Err((
        status,
        Json(ApiError {
          error: e.message,
          code: e.code,
        }),
      ))
    }
  }
}

// -- Plugin UI asset handler --

async fn plugin_ui_handler(
  State(state): State<Arc<AppState>>,
  Path((plugin_id, path)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
  match state.plugin_loader.get_ui_file(&plugin_id, &path).await {
    Some((data, mime)) => Ok(
      Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(axum::body::Body::from(data))
        .unwrap(),
    ),
    None => Err(ApiError::not_found("UI asset not found")),
  }
}

// -- Frontend SPA fallback (embedded via rust-embed) --

/// Serves the production frontend build embedded in the binary.
/// Tries to serve the exact static file first; falls back to `index.html`
/// for client-side (SPA) routing.  When the frontend isn't embedded (dev
/// builds without a pre-built `frontend/dist/`), returns 404.
async fn frontend_spa_fallback(uri: Uri) -> Result<Response, (StatusCode, Json<ApiError>)> {
  let path = uri.path().trim_start_matches('/');
  tracing::debug!(path = %path, "SPA fallback requested");

  #[cfg(frontend_embedded)]
  tracing::debug!("frontend_embedded is active in cfg");
  #[cfg(not(frontend_embedded))]
  tracing::warn!("frontend_embedded is NOT active in cfg!");

  if let Some(resp) = crate::frontend::try_serve(path) {
    tracing::debug!(path = %path, "Served static frontend asset");
    return Ok(resp);
  }
  if let Some(resp) = crate::frontend::serve_index() {
    tracing::debug!("Served index.html fallback");
    return Ok(resp);
  }
  tracing::error!(path = %path, "Frontend asset/index.html not found, returning NOT_FOUND");
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
  let rows = state
    .storage
    .query_lang(query_string)
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

// ── Reactor management handlers ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateReactorRequest {
  pub defined_by_app: Option<Uuid>,
  pub registered_by_plugin: Option<String>,
  pub owner_schema_id: Option<Uuid>,
  pub mode: String,
  pub trigger: serde_json::Value,
  pub filter: Option<serde_json::Value>,
  pub action_kind: String,
  pub action_target: Option<String>,
  pub action_ref: serde_json::Value,
  #[serde(default)]
  pub priority: i32,
  #[serde(default)]
  pub capabilities: Vec<serde_json::Value>,
  #[serde(default = "default_status")]
  pub status: String,
  pub retry_policy: Option<serde_json::Value>,
  pub authorized_by: Option<String>,
}

fn default_status() -> String {
  "active".into()
}

#[derive(Debug, Deserialize)]
struct UpdateReactorStatusRequest {
  pub status: String,
}

async fn list_reactors(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
  let reactors: Vec<serde_json::Value> = state
    .reactor_registry
    .list_all()
    .iter()
    .map(|r| serde_json::to_value(r).unwrap_or_default())
    .collect();
  Json(reactors)
}

async fn get_reactor(
  State(state): State<Arc<AppState>>,
  Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  let reactor = state
    .reactor_registry
    .get(&id)
    .ok_or_else(|| ApiError::not_found("Reactor not found"))?;
  Ok(Json(serde_json::to_value(reactor).unwrap_or_default()))
}

async fn create_reactor(
  State(state): State<Arc<AppState>>,
  Json(req): Json<CreateReactorRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
  use panorama_core::reactor::{
    ActionKind, CapRef, Reactor, ReactorMode, ReactorStatus, ReactorTrigger, RetryPolicy, WasmRef,
  };

  let mode = match req.mode.as_str() {
    "eager" => ReactorMode::Eager,
    "deferred" => ReactorMode::Deferred,
    _ => return Err(ApiError::bad_request("mode must be 'eager' or 'deferred'")),
  };
  let trigger: ReactorTrigger = serde_json::from_value(req.trigger)
    .map_err(|e| ApiError::bad_request(&format!("Invalid trigger: {}", e)))?;
  let action_kind = match req.action_kind.as_str() {
    "validate" => ActionKind::Validate,
    "transform" => ActionKind::Transform,
    "compute_field" => ActionKind::ComputeField,
    "side_effect" => ActionKind::SideEffect,
    "internal_write" => ActionKind::InternalWrite,
    _ => return Err(ApiError::bad_request("Invalid action_kind")),
  };
  let action_ref: WasmRef = serde_json::from_value(req.action_ref)
    .map_err(|e| ApiError::bad_request(&format!("Invalid action_ref: {}", e)))?;
  let capabilities: Vec<CapRef> = req
    .capabilities
    .iter()
    .filter_map(|c| serde_json::from_value(c.clone()).ok())
    .collect();
  let status = match req.status.as_str() {
    "active" => ReactorStatus::Active,
    "disabled" => ReactorStatus::Disabled,
    "error_quarantined" => ReactorStatus::ErrorQuarantined,
    _ => return Err(ApiError::bad_request("Invalid status")),
  };
  let retry_policy: Option<RetryPolicy> = req
    .retry_policy
    .and_then(|rp| serde_json::from_value(rp).ok());
  let filter = req.filter.and_then(|f| serde_json::from_value(f).ok());

  let reactor = Reactor {
    id: Uuid::new_v4(),
    defined_by_app: req.defined_by_app,
    registered_by_plugin: req.registered_by_plugin,
    owner_schema_id: req.owner_schema_id,
    mode,
    trigger,
    filter,
    action_kind,
    action_target: req.action_target,
    action_ref,
    priority: req.priority,
    capabilities,
    status,
    retry_policy,
    authorized_by: req.authorized_by,
    created_at: chrono::Utc::now().to_rfc3339(),
  };

  let registered = state
    .reactor_registry
    .register(reactor)
    .await
    .map_err(|e| ApiError::bad_request(e.as_str()))?;
  Ok(Json(serde_json::to_value(registered).unwrap_or_default()))
}

async fn delete_reactor(
  State(state): State<Arc<AppState>>,
  Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
  state
    .reactor_registry
    .delete(id)
    .await
    .map(|_| StatusCode::NO_CONTENT)
    .map_err(|e| ApiError::internal(e))
}

async fn update_reactor_status(
  State(state): State<Arc<AppState>>,
  Path(id): Path<Uuid>,
  Json(req): Json<UpdateReactorStatusRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
  use panorama_core::reactor::ReactorStatus;
  let status = match req.status.as_str() {
    "active" => ReactorStatus::Active,
    "disabled" => ReactorStatus::Disabled,
    "error_quarantined" => ReactorStatus::ErrorQuarantined,
    _ => return Err(ApiError::bad_request("Invalid status")),
  };
  state
    .reactor_registry
    .update_status(id, status)
    .await
    .map(|_| StatusCode::OK)
    .map_err(|e| ApiError::internal(e))
}
