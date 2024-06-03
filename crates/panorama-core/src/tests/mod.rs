use cozo::DbInstance;
use miette::Result;
use tantivy::Index;

use crate::{migrations::run_migrations, state::tantivy_schema, AppState};

pub async fn test_state() -> Result<AppState> {
  let db = DbInstance::new("mem", "", "")?;
  let schema = tantivy_schema();
  let tantivy_index = Index::create_in_ram(schema);

  let state = AppState { db, tantivy_index };
  run_migrations(&state.db).await?;

  Ok(state)
}

#[tokio::test]
pub async fn test_create_node() -> Result<()> {
  let state = test_state().await?;

  let node_info = state
    .create_node(
      "panorama/journal/page",
      Some(btmap! {
        "panorama/journal/page/content".to_owned() => json!("helloge"),
      }),
    )
    .await?;

  println!(
    "{}",
    serde_json::to_string_pretty(&state.export().await.unwrap()).unwrap()
  );

  let node = state.get_node(node_info.node_id).await?;

  println!("node: {:?}", node);

  Ok(())
}
