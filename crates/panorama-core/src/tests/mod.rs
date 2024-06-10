use cozo::DbInstance;
use itertools::Itertools;
use miette::Result;
use tantivy::Index;

use crate::{migrations::run_migrations, state::tantivy_schema, AppState};

pub async fn test_state() -> Result<AppState> {
  let db = DbInstance::new("mem", "", "")?;
  let (schema, tantivy_field_map) = tantivy_schema();
  let tantivy_index = Index::create_in_ram(schema);

  let state = AppState {
    db,
    tantivy_index,
    tantivy_field_map,
  };
  run_migrations(&state.db).await?;

  Ok(state)
}

#[tokio::test]
pub async fn test_create_node() -> Result<()> {
  let state = test_state().await?;

  let node_info = state
    .create_or_update_node(
      "panorama/journal/page",
      Some(btmap! {
        "panorama/journal/page/content".to_owned() => json!("helloge"),
      }),
    )
    .await?;

  let mut node = state.get_node(node_info.node_id.to_string()).await?;
  assert!(node.fields.is_some());

  let fields = node.fields.take().unwrap();
  assert!(fields.contains_key("panorama/journal/page/content"));

  Ok(())
}

#[tokio::test]
pub async fn test_full_text_search() -> Result<()> {
  let state = test_state().await?;

  let node_info = state
    .create_or_update_node(
      "panorama/journal/page",
      Some(btmap! {
        "panorama/journal/page/content".to_owned() => json!("Hello, world!"),
      }),
    )
    .await?;

  let results = state.search_nodes("world").await?;

  assert!(results
    .into_iter()
    .map(|entry| entry.0)
    .contains(&node_info.node_id));

  Ok(())
}
