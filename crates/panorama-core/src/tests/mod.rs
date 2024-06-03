use cozo::DbInstance;
use miette::Result;
use tantivy::Index;

use crate::{state::tantivy_schema, AppState};

pub fn test_state() -> Result<AppState> {
  let db = DbInstance::new("mem", "", "")?;
  let schema = tantivy_schema();
  let tantivy_index = Index::create_in_ram(schema);
  Ok(AppState { db, tantivy_index })
}

#[test]
pub fn test_create_node() -> Result<()> {
  let state = test_state()?;

  Ok(())
}
