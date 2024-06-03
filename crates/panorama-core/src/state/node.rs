use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use cozo::{DataValue, MultiTransaction, ScriptMutability};
use itertools::Itertools;
use miette::Result;
use serde_json::Value;
use uuid::Uuid;

use crate::AppState;

pub type ExtraData = BTreeMap<String, Value>;

#[derive(Debug)]
pub struct NodeInfo {
  pub node_id: String,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub fields: Option<HashMap<String, DataValue>>,
}

pub struct FieldInfo {
  pub relation_name: String,
  pub relation_field: String,
  pub r#type: String,
  pub is_fts_enabled: bool,
}

pub type FieldMapping = HashMap<String, FieldInfo>;

impl AppState {
  /// Get all properties of a node
  pub async fn get_node(&self, node_id: impl AsRef<str>) -> Result<NodeInfo> {
    let node_id = node_id.as_ref().to_owned();

    let result = self.db.run_script(
      "
        ?[relation, field_name, type, is_fts_enabled] :=
          *node_has_key { key, id },
          *fqkey_to_dbkey { key, relation, field_name, type, is_fts_enabled },
          id = $node_id
      ",
      btmap! {"node_id".to_owned() => node_id.clone().into()},
      ScriptMutability::Immutable,
    )?;

    println!("FIELDS: {:?}", result);

    todo!()
  }

  pub async fn create_node(
    &self,
    r#type: impl AsRef<str>,
    extra_data: Option<ExtraData>,
  ) -> Result<NodeInfo> {
    let ty = r#type.as_ref();

    let node_id = Uuid::now_v7();
    let node_id = node_id.to_string();

    let tx = self.db.multi_transaction(true);

    let node_result = tx.run_script(
      "
        ?[id, type] <- [[$node_id, $type]]
        :put node { id, type }
        :returning
      ",
      btmap! {
        "node_id".to_owned() => DataValue::from(node_id.clone()),
        "type".to_owned() => DataValue::from(ty),
      },
    )?;

    if let Some(extra_data) = extra_data {
      if !extra_data.is_empty() {
        let keys = extra_data.keys().map(|s| s.to_owned()).collect::<Vec<_>>();
        let field_mapping =
          self.get_rows_for_extra_keys(&tx, keys.as_slice())?;

        // Group the keys by which relation they're in
        let result_by_relation = field_mapping.iter().into_group_map_by(
          |(key, FieldInfo { relation_name, .. })| relation_name,
        );

        for (relation, fields) in result_by_relation.iter() {
          let fields_mapping = fields
            .into_iter()
            .map(
              |(
                key,
                FieldInfo {
                  relation_field,
                  r#type,
                  ..
                },
              )| {
                let new_value = extra_data.get(*key).unwrap();
                // TODO: Make this more generic
                let new_value = match r#type.as_str() {
                  "int" => DataValue::from(new_value.as_i64().unwrap()),
                  _ => DataValue::from(new_value.as_str().unwrap()),
                };
                (relation_field.to_owned(), new_value)
              },
            )
            .collect::<BTreeMap<_, _>>();

          let keys = fields_mapping.keys().collect::<Vec<_>>();
          let keys_joined = keys.iter().join(", ");

          let query = format!(
            "
            ?[ node_id, {keys_joined} ] <- [$input_data]
            :insert {relation} {{ node_id, {keys_joined} }}
          "
          );

          let mut params = vec![];
          params.push(DataValue::from(node_id.clone()));
          for key in keys {
            params.push(fields_mapping[key].clone());
          }

          let result = tx.run_script(
            &query,
            btmap! {
              "input_data".to_owned() => DataValue::List(params),
            },
          )?;
        }

        let input = DataValue::List(
          keys
            .iter()
            .map(|s| {
              DataValue::List(vec![
                DataValue::from(s.to_owned()),
                DataValue::from(node_id.clone()),
              ])
            })
            .collect_vec(),
        );
        tx.run_script(
          "
          ?[key, id] <- $input_data
          :put node_has_key { key, id }
        ",
          btmap! {
            "input_data".to_owned() => input
          },
        )?;
      }
    }

    tx.commit()?;

    let created_at = DateTime::from_timestamp_millis(
      (node_result.rows[0][4].get_float().unwrap() * 1000.0) as i64,
    )
    .unwrap();
    let updated_at = DateTime::from_timestamp_millis(
      (node_result.rows[0][5].get_float().unwrap() * 1000.0) as i64,
    )
    .unwrap();

    Ok(NodeInfo {
      node_id,
      created_at,
      updated_at,
      fields: None,
    })
  }

  pub fn get_rows_for_extra_keys(
    &self,
    tx: &MultiTransaction,
    keys: &[String],
  ) -> Result<FieldMapping> {
    let result = tx.run_script(
      "
        ?[key, relation, field_name, type, is_fts_enabled] :=
          *fqkey_to_dbkey{key, relation, field_name, type, is_fts_enabled},
          is_in(key, $keys)
      ",
      btmap! {
        "keys".to_owned() => DataValue::List(
          keys.into_iter()
            .map(|s| DataValue::from(s.as_str()))
            .collect::<Vec<_>>()
        ),
      },
    )?;

    let s = |s: &DataValue| s.get_str().unwrap().to_owned();

    Ok(
      result
        .rows
        .into_iter()
        .map(|row| {
          (
            s(&row[0]),
            FieldInfo {
              relation_name: s(&row[1]),
              relation_field: s(&row[2]),
              r#type: s(&row[3]),
              is_fts_enabled: row[4].get_bool().unwrap(),
            },
          )
        })
        .collect::<HashMap<_, _>>(),
    )
  }
}
