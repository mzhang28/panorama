use std::collections::BTreeMap;

use cozo::{DataValue, DbInstance, ScriptMutability};
use miette::Result;

pub fn ensure_schema(db: &DbInstance) -> Result<()> {
  let existing_relations = db.run_script(
    "::relations",
    Default::default(),
    ScriptMutability::Immutable,
  )?;

  println!("Result: {:?}", existing_relations);

  db.run_script(
    "%ignore_error {:create node {id: String, content: String}}",
    Default::default(),
    ScriptMutability::Mutable,
  )?;

  let mut create_rule_params = BTreeMap::new();
  create_rule_params
    .insert(format!("input_data"), DataValue::from(format!("root")));
  db.run_script(
    "
    %if { len[count(x)] := *node[x, y]; ?[x] := len[z], x = z == 0 }
      { ?[node, content] := $input_data; :put node {id, content} }
    %end
  ",
    create_rule_params,
    ScriptMutability::Mutable,
  )?;

  Ok(())
}
