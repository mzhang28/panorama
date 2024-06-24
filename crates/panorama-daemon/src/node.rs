use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

/// Node API
#[derive(OpenApi)]
#[openapi(paths(), components(schemas()))]
pub(super) struct NodeApi;

pub(super) fn router() -> Router<AppState> {
  Router::new()
  // .route("/", put(create_node))
  // .route("/:id", get(get_node))
  // .route("/:id", post(update_node))
  // .route("/search", get(search_nodes))
}

// #[derive(Serialize, Deserialize, ToSchema, Clone)]
// struct GetNodeResult {
//   node_id: String,
//   fields: HashMap<String, Value>,
//   created_at: DateTime<Utc>,
//   updated_at: DateTime<Utc>,
// }

// /// Get node info
// ///
// /// This endpoint retrieves all the fields for a particular node
// #[utoipa::path(
//   get,
//   path = "/{id}",
//   responses(
//     (status = 200, body = [GetNodeResult]),
//     (status = 404, description = "the node ID provided was not found")
//   ),
//   params(
//     ("id" = String, Path, description = "Node ID"),
//   ),
// )]
// pub async fn get_node(
//   State(state): State<AppState>,
//   Path(node_id): Path<String>,
// ) -> AppResult<(StatusCode, Json<Value>)> {
//   let node_info = state.get_node(&node_id).await?;

//   Ok((
//     StatusCode::OK,
//     Json(json!({
//       "node_id": node_id,
//       "fields": node_info.fields,
//       "created_at": node_info.created_at,
//       "updated_at": node_info.updated_at,
//     })),
//   ))
// }

// #[derive(Deserialize, Debug)]
// pub struct UpdateData {
//   extra_data: Option<ExtraData>,
// }

// /// Update node info
// #[utoipa::path(
//   post,
//   path = "/{id}",
//   responses(
//     (status = 200)
//   ),
//   params(
//     ("id" = String, Path, description = "Node ID"),
//   )
// )]
// pub async fn update_node(
//   State(state): State<AppState>,
//   Path(node_id): Path<String>,
//   Json(opts): Json<UpdateData>,
// ) -> AppResult<Json<Value>> {
//   let node_id = NodeId(Uuid::from_str(&node_id).into_diagnostic()?);
//   let node_info = state
//     .create_or_update_node(CreateOrUpdate::Update { node_id }, opts.extra_data)
//     .await?;

//   Ok(Json(json!({
//     "node_id": node_info.node_id.to_string(),
//   })))
// }

// #[derive(Debug, Deserialize)]
// pub struct CreateNodeOpts {
//   // TODO: Allow submitting a string
//   // id: Option<String>,
//   #[serde(rename = "type")]
//   ty: String,
//   extra_data: Option<ExtraData>,
// }

// #[utoipa::path(
//   put,
//   path = "/",
//   responses((status = 200)),
// )]
// pub async fn create_node(
//   State(state): State<AppState>,
//   Json(opts): Json<CreateNodeOpts>,
// ) -> AppResult<Json<Value>> {
//   let node_info = state
//     .create_or_update_node(
//       CreateOrUpdate::Create { r#type: opts.ty },
//       opts.extra_data,
//     )
//     .await?;

//   Ok(Json(json!({
//     "node_id": node_info.node_id.to_string(),
//   })))
// }

// #[derive(Deserialize)]
// pub struct SearchQuery {
//   query: String,
// }

// #[utoipa::path(
//   get,
//   path = "/search",
//   responses((status = 200)),
// )]
// pub async fn search_nodes(
//   State(state): State<AppState>,
//   Query(query): Query<SearchQuery>,
// ) -> AppResult<Json<Value>> {
//   let search_result = state.search_nodes(query.query).await?;
//   let search_result = search_result
//     .into_iter()
//     .map(|(id, value)| value["fields"].clone())
//     .collect_vec();

//   Ok(Json(json!({
//     "results": search_result,
//   })))
// }

// fn get_rows_for_extra_keys(
//   tx: &MultiTransaction,
//   extra_data: &ExtraData,
// ) -> AppResult<HashMap<String, (String, String, String)>> {
//   let result = tx.run_script(
//     "
//       ?[key, relation, field_name, type] :=
//         *fqkey_to_dbkey{key, relation, field_name, type},
//         is_in(key, $keys)
//     ",
//     btmap! {
//       "keys".to_owned() => DataValue::List(
//         extra_data
//           .keys()
//           .map(|s| DataValue::from(s.as_str()))
//           .collect::<Vec<_>>()
//       ),
//     },
//   )?;

//   let s = |s: &DataValue| s.get_str().unwrap().to_owned();

//   Ok(
//     result
//       .rows
//       .into_iter()
//       .map(|row| (s(&row[0]), (s(&row[1]), s(&row[2]), s(&row[3]))))
//       .collect::<HashMap<_, _>>(),
//   )
// }
