use std::{
  collections::HashMap,
  fs::{self, File},
  io::{BufWriter, Write},
  path::PathBuf,
};

use axum::extract::State;
use cozo::ScriptMutability;

use crate::{error::AppResult, AppState};

pub async fn export(State(state): State<AppState>) -> AppResult<()> {
  let result = state.db.run_script(
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
    let result = state.db.run_script(
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

  println!("columns: {relation_columns:?}");

  let base_dir = PathBuf::from("export");
  fs::create_dir_all(&base_dir);

  let tx = state.db.multi_transaction(false);

  for relation_name in relation_names.iter() {
    let relation_path = base_dir.join(format!("{relation_name}.ndjson"));
    let mut file = File::create(&relation_path).unwrap();

    let columns = relation_columns
      .get(relation_name.as_str())
      .unwrap()
      .join(", ");

    let query = format!("?[{columns}] := *{relation_name} {{ {columns} }}");
    println!("Query: {query}");
    let result = tx.run_script(&query, Default::default())?;

    for row in result.rows.into_iter() {
      let mut object = HashMap::new();

      for (idx, col) in row.into_iter().enumerate() {
        let row_name = result.headers[idx].clone();
        object.insert(row_name, col);
      }

      let serialized = serde_json::to_string(&object).unwrap();
      file.write(serialized.as_bytes());
      file.write(b"\n");
    }
  }

  Ok(())
}
