use std::collections::HashMap;

use chrono::{DateTime, Utc};
use cozo::{DataValue, ScriptMutability};
use miette::Result;

use crate::AppState;

pub struct NodeInfo {
  pub node_id: String,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub fields: HashMap<String, DataValue>,
}

impl AppState {
  /// Get all properties of a node
  pub async fn get_node(&self, node_id: impl AsRef<str>) -> Result<NodeInfo> {
    let node_id = node_id.as_ref().to_owned();

    let result = self.db.run_script(
      "
        ?[relation, field_name, type, fts_enabled] :=
          *node_has_key { key, id },
          *fqkey_to_dbkey { key, relation, field_name, type, fts_enabled },
          id = $node_id
      ",
      btmap! {"node_id".to_owned() => node_id.clone().into()},
      ScriptMutability::Immutable,
    )?;

    todo!()
  }
}
