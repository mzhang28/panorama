use anyhow::Result;
use sqlx::SqlitePool;
use tantivy::Index;

use crate::{
  migrations::MIGRATOR,
  state::{node::CreateOrUpdate, tantivy_schema},
  AppState,
};

pub async fn test_state() -> Result<AppState> {
  let db = SqlitePool::connect(":memory:").await?;
  let (schema, tantivy_field_map) = tantivy_schema();
  let tantivy_index = Index::create_in_ram(schema);
  MIGRATOR.run(&db).await?;

  let state = AppState {
    db,
    tantivy_index,
    tantivy_field_map,
  };

  Ok(state)
}

#[tokio::test]
pub async fn test_create_node() -> Result<()> {
  let state = test_state().await?;

  let node_info = state
    .create_or_update_node(
      CreateOrUpdate::Create {
        r#type: "panorama/journal/page".to_string(),
      },
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
      CreateOrUpdate::Create {
        r#type: "panorama/journal/page".to_string(),
      },
      Some(btmap! {
        "panorama/journal/page/content".to_owned() => json!("Hello, world!"),
      }),
    )
    .await?;

  todo!();
  // let results = state.search_nodes("world").await?;

  // assert!(results
  //   .into_iter()
  //   .map(|entry| entry.0)
  //   .contains(&node_info.node_id));

  Ok(())
}

#[tokio::test]
pub async fn test_install_apps() -> Result<()> {
  let state = test_state().await?;

  state.install_apps_from_search_paths().await?;

  todo!();

  Ok(())
}
