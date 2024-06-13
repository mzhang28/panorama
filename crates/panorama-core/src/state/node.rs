use std::{
  collections::{BTreeMap, HashMap},
  str::FromStr,
};

use chrono::{DateTime, Utc};
use cozo::{DataValue, MultiTransaction, NamedRows};
use itertools::Itertools;
use miette::{bail, IntoDiagnostic, Result};
use serde_json::Value;
use tantivy::{
  collector::TopDocs,
  query::QueryParser,
  schema::{OwnedValue, Value as _},
  Document, TantivyDocument,
};
use uuid::Uuid;

use crate::{AppState, NodeId};

use super::utils::{data_value_to_json_value, owned_value_to_json_value};

pub type ExtraData = BTreeMap<String, Value>;

#[derive(Debug)]
pub struct NodeInfo {
  pub node_id: NodeId,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub fields: Option<HashMap<String, Value>>,
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
    let tx = self.db.multi_transaction(false);

    let result = tx.run_script(
      "
        ?[key, relation, field_name, type, is_fts_enabled] :=
          *node_has_key { key, id },
          *fqkey_to_dbkey { key, relation, field_name, type, is_fts_enabled },
          id = $node_id
      ",
      btmap! {"node_id".to_owned() => node_id.to_string().into()},
    )?;

    let field_mapping = AppState::rows_to_field_mapping(result)?;

    // Group the keys by which relation they're in
    let result_by_relation = field_mapping
      .iter()
      .into_group_map_by(|(_, FieldInfo { relation_name, .. })| relation_name);

    let mut all_relation_queries = vec![];
    let mut all_relation_constraints = vec![];
    let mut all_fields = vec![];
    let mut field_counter = 0;
    for (i, (relation, fields)) in result_by_relation.iter().enumerate() {
      let constraint_name = format!("c{i}");

      let mut keys = vec![];
      let mut constraints = vec![];
      for (key, field_info) in fields.iter() {
        let counted_field_name = format!("f{field_counter}");
        field_counter += 1;

        keys.push(counted_field_name.clone());
        constraints.push(format!(
          "{}: {}",
          field_info.relation_field.to_owned(),
          counted_field_name,
        ));
        all_fields.push((
          counted_field_name,
          field_info.relation_field.to_owned(),
          key,
        ))
      }

      let keys = keys.join(", ");
      let constraints = constraints.join(", ");
      all_relation_queries.push(format!(
        "
        {constraint_name}[{keys}] :=
          *{relation}{{ node_id, {constraints} }},
          node_id = $node_id
        "
      ));
      all_relation_constraints.push(format!("{constraint_name}[{keys}],"))
    }

    let all_relation_constraints = all_relation_constraints.join("\n");
    let all_relation_queries = all_relation_queries.join("\n\n");
    let all_field_names = all_fields
      .iter()
      .map(|(field_name, _, _)| field_name)
      .join(", ");
    let query = format!(
      "
      {all_relation_queries}

      ?[type, extra_data, created_at, updated_at, {all_field_names}] :=
        *node {{ id, type, created_at, updated_at, extra_data }},
        {all_relation_constraints}
        id = $node_id
      "
    );

    let result = tx.run_script(
      &query,
      btmap! { "node_id".to_owned() => node_id.to_string().into(), },
    )?;

    if result.rows.is_empty() {
      bail!("Not found")
    }

    let created_at = DateTime::from_timestamp_millis(
      (result.rows[0][2].get_float().unwrap() * 1000.0) as i64,
    )
    .unwrap();

    let updated_at = DateTime::from_timestamp_millis(
      (result.rows[0][3].get_float().unwrap() * 1000.0) as i64,
    )
    .unwrap();

    let mut fields = HashMap::new();

    for row in result
      .rows
      .into_iter()
      .map(|row| row.into_iter().skip(4).zip(all_fields.iter()))
    {
      for (value, (_, _, field_name)) in row {
        fields.insert(field_name.to_string(), data_value_to_json_value(&value));
      }
    }

    Ok(NodeInfo {
      node_id: NodeId(Uuid::from_str(&node_id).unwrap()),
      created_at,
      updated_at,
      fields: Some(fields),
    })
  }
}

pub enum CreateOrUpdate {
  Create { r#type: String },
  Update { node_id: NodeId },
}

impl AppState {
  // TODO: Split this out into create and update
  pub async fn create_or_update_node(
    &self,
    opts: CreateOrUpdate,
    extra_data: Option<ExtraData>,
  ) -> Result<NodeInfo> {
    let node_id = match opts {
      CreateOrUpdate::Create { .. } => NodeId(Uuid::now_v7()),
      CreateOrUpdate::Update { ref node_id } => node_id.clone(),
    };
    let node_id = node_id.to_string();

    let tx = self.db.multi_transaction(true);

    let (created_at, updated_at) = match opts {
      CreateOrUpdate::Create { r#type } => {
        let node_result = tx.run_script(
          "
        ?[id, type] <- [[$node_id, $type]]
        :put node { id, type }
        :returning
      ",
          btmap! {
            "node_id".to_owned() => DataValue::from(node_id.clone()),
            "type".to_owned() => DataValue::from(r#type),
          },
        )?;
        println!("ROWS(1): {:?}", node_result);
        let created_at = DateTime::from_timestamp_millis(
          (node_result.rows[0][4].get_float().unwrap() * 1000.0) as i64,
        )
        .unwrap();
        let updated_at = DateTime::from_timestamp_millis(
          (node_result.rows[0][5].get_float().unwrap() * 1000.0) as i64,
        )
        .unwrap();
        (created_at, updated_at)
      }
      CreateOrUpdate::Update { .. } => {
        let node_result = tx.run_script(
        "
        ?[id, type, created_at, updated_at] := *node { id, type, created_at, updated_at },
          id = $node_id
      ",
        btmap! {
          "node_id".to_owned() => DataValue::from(node_id.clone()),
        },
      )?;
        println!("ROWS(2): {:?}", node_result);
        let created_at = DateTime::from_timestamp_millis(
          (node_result.rows[0][2].get_float().unwrap() * 1000.0) as i64,
        )
        .unwrap();
        let updated_at = DateTime::from_timestamp_millis(
          (node_result.rows[0][3].get_float().unwrap() * 1000.0) as i64,
        )
        .unwrap();
        (created_at, updated_at)
      }
    };

    if let Some(extra_data) = extra_data {
      let node_id_field = self
        .tantivy_field_map
        .get_by_left("node_id")
        .unwrap()
        .clone();
      if !extra_data.is_empty() {
        let keys = extra_data.keys().map(|s| s.to_owned()).collect::<Vec<_>>();
        let field_mapping =
          self.get_rows_for_extra_keys(&tx, keys.as_slice())?;

        // Group the keys by which relation they're in
        let result_by_relation = field_mapping.iter().into_group_map_by(
          |(_, FieldInfo { relation_name, .. })| relation_name,
        );

        for (relation, fields) in result_by_relation.iter() {
          let mut doc = btmap! { node_id_field.clone() => OwnedValue::Str(node_id.to_owned()) };
          let fields_mapping = fields
            .into_iter()
            .map(
              |(
                key,
                FieldInfo {
                  relation_field,
                  r#type,
                  is_fts_enabled,
                  ..
                },
              )| {
                let new_value = extra_data.get(*key).unwrap();

                // TODO: Make this more generic
                let new_value = match r#type.as_str() {
                  "int" => DataValue::from(new_value.as_i64().unwrap()),
                  _ => DataValue::from(new_value.as_str().unwrap()),
                };

                if *is_fts_enabled {
                  if let Some(field) = self.tantivy_field_map.get_by_left(*key)
                  {
                    doc.insert(
                      field.clone(),
                      OwnedValue::Str(new_value.get_str().unwrap().to_owned()),
                    );
                  }
                }

                (relation_field.to_owned(), new_value)
              },
            )
            .collect::<BTreeMap<_, _>>();

          let mut writer =
            self.tantivy_index.writer(15_000_000).into_diagnostic()?;
          writer.add_document(doc).into_diagnostic()?;
          writer.commit().into_diagnostic()?;
          drop(writer);

          let keys = fields_mapping.keys().collect::<Vec<_>>();
          let keys_joined = keys.iter().join(", ");

          let query = format!(
            "
            ?[ node_id, {keys_joined} ] <- [$input_data]
            :put {relation} {{ node_id, {keys_joined} }}
            "
          );

          let mut params = vec![];
          params.push(DataValue::from(node_id.clone()));
          for key in keys {
            params.push(fields_mapping[key].clone());
          }

          println!("Query: {:?} \n {:?}", query, params);

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

    Ok(NodeInfo {
      node_id: NodeId(Uuid::from_str(&node_id).unwrap()),
      created_at,
      updated_at,
      fields: None,
    })
  }

  pub async fn update_node() {}

  pub async fn search_nodes(
    &self,
    query: impl AsRef<str>,
  ) -> Result<Vec<(NodeId, Value)>> {
    let query = query.as_ref();

    let reader = self.tantivy_index.reader().into_diagnostic()?;
    let searcher = reader.searcher();

    let node_id_field = self
      .tantivy_field_map
      .get_by_left("node_id")
      .unwrap()
      .clone();
    let journal_page_field = self
      .tantivy_field_map
      .get_by_left("panorama/journal/page/content")
      .unwrap()
      .clone();
    let query_parser =
      QueryParser::for_index(&self.tantivy_index, vec![journal_page_field]);
    let query = query_parser.parse_query(query).into_diagnostic()?;

    let top_docs = searcher
      .search(&query, &TopDocs::with_limit(10))
      .into_diagnostic()?;

    Ok(
      top_docs
        .into_iter()
        .map(|(score, doc_address)| {
          let retrieved_doc =
            searcher.doc::<TantivyDocument>(doc_address).unwrap();
          let node_id = retrieved_doc
            .get_first(node_id_field.clone())
            .unwrap()
            .as_str()
            .unwrap();
          let all_fields = retrieved_doc.get_sorted_field_values();
          let node_id = NodeId(Uuid::from_str(node_id).unwrap());
          let fields = all_fields
            .into_iter()
            .map(|(field, values)| {
              (
                self.tantivy_field_map.get_by_right(&field).unwrap(),
                if values.len() == 1 {
                  owned_value_to_json_value(values[0])
                } else {
                  Value::Array(
                    values
                      .into_iter()
                      .map(owned_value_to_json_value)
                      .collect_vec(),
                  )
                },
              )
            })
            .collect::<HashMap<_, _>>();
          (
            node_id,
            json!({
              "score": score,
              "fields": fields,
            }),
          )
        })
        .collect::<Vec<_>>(),
    )
  }

  fn get_rows_for_extra_keys(
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

    AppState::rows_to_field_mapping(result)
  }

  fn rows_to_field_mapping(result: NamedRows) -> Result<FieldMapping> {
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
