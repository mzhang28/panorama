use std::collections::HashMap;

use cozo::ScriptMutability;
use miette::Result;
use serde_json::Value;

use crate::AppState;

use super::utils::data_value_to_json_value;

impl AppState {
  pub async fn export(&self) -> Result<Value> {
    let result = self.db.run_script(
      "::relations",
      Default::default(),
      ScriptMutability::Immutable,
    )?;

    let name_index = result.headers.iter().position(|x| x == "name").unwrap();
    let relation_names = result
      .rows
      .into_iter()
      .map(|row| row[name_index].get_str().unwrap().to_owned())
      .collect::<Vec<_>>();

    let mut relation_columns = HashMap::new();

    for relation_name in relation_names.iter() {
      let result = self.db.run_script(
        &format!("::columns {relation_name}"),
        Default::default(),
        ScriptMutability::Immutable,
      )?;

      let column_index =
        result.headers.iter().position(|x| x == "column").unwrap();
      let columns = result
        .rows
        .into_iter()
        .map(|row| row[column_index].get_str().unwrap().to_owned())
        .collect::<Vec<_>>();

      relation_columns.insert(relation_name.clone(), columns);
    }

    let tx = self.db.multi_transaction(false);

    let mut all_relations = hmap! {};
    for relation_name in relation_names.iter() {
      if relation_name.contains(":") {
        continue;
      }

      let mut relation_info = vec![];

      let columns = relation_columns.get(relation_name.as_str()).unwrap();
      let columns_str = columns.join(", ");

      let query =
        format!("?[{columns_str}] := *{relation_name} {{ {columns_str} }}");
      let result = tx.run_script(&query, Default::default())?;

      for row in result.rows.into_iter() {
        let mut object = hmap! {};
        row.into_iter().enumerate().for_each(|(idx, col)| {
          object
            .insert(columns[idx].to_owned(), data_value_to_json_value(&col));
        });
        relation_info.push(object);
      }

      all_relations.insert(relation_name.to_owned(), relation_info);
    }

    Ok(json!({"relations": all_relations}))
  }
}
