use std::{
  collections::{BTreeMap, HashMap},
  str::FromStr,
};

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use itertools::Itertools;
use miette::{bail, Context, Error, IntoDiagnostic, Report, Result};
use serde_json::Value;
use sqlx::{
  query::Query, sqlite::SqliteArguments, Acquire, Connection, Executor,
  FromRow, QueryBuilder, Sqlite,
};
use tantivy::{
  schema::{OwnedValue, Value as _},
  time::Date,
  Term,
};
use uuid::Uuid;

use crate::{state::node_raw::FieldMappingRow, AppState, NodeId};

// use super::utils::owned_value_to_json_value;

pub type ExtraData = BTreeMap<String, Value>;
pub type FieldsByTable<'a> =
  HashMap<(&'a i64, &'a String), Vec<&'a FieldMappingRow>>;

#[derive(Debug)]
pub struct NodeInfo {
  pub node_id: NodeId,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub fields: Option<HashMap<String, Value>>,
}

impl AppState {
  /// Get all properties of a node
  pub async fn get_node(&self, node_id: impl AsRef<str>) -> Result<NodeInfo> {
    let node_id = node_id.as_ref().to_owned();
    let mut conn = self.conn().await?;

    conn
      .transaction::<_, _, sqlx::Error>(|tx| {
        Box::pin(async move {
          let node_id = node_id.clone();
          let field_mapping =
            AppState::get_related_field_list_for_node_id(&mut **tx, &node_id)
              .await?;

          // Group the keys by which relation they're in
          let fields_by_table = field_mapping.iter().into_group_map_by(
            |FieldMappingRow {
               app_id,
               app_table_name,
               ..
             }| (app_id, app_table_name),
          );

          // Run the query that grabs all of the relevant fields, and coalesce
          // the fields back
          let related_fields =
            AppState::query_related_fields(&mut **tx, &fields_by_table).await?;

          println!("Related fields: {:?}", related_fields);

          // let created_at = DateTime::from_timestamp_millis(
          //   (result.rows[0][2].get_float().unwrap() * 1000.0) as i64,
          // )
          // .unwrap();

          // let updated_at = DateTime::from_timestamp_millis(
          //   (result.rows[0][3].get_float().unwrap() * 1000.0) as i64,
          // )
          // .unwrap();

          // let mut fields = HashMap::new();

          // for row in result
          //   .rows
          //   .into_iter()
          //   .map(|row| row.into_iter().skip(4).zip(all_fields.iter()))
          // {
          //   for (value, (_, _, field_name)) in row {
          //     fields.insert(
          //       field_name.to_string(),
          //       data_value_to_json_value(&value),
          //     );
          //   }
          // }

          todo!()

          // Ok(NodeInfo {
          //   node_id: NodeId(Uuid::from_str(&node_id).unwrap()),
          //   created_at,
          //   updated_at,
          //   fields: Some(fields),
          // })
        })
      })
      .await
      .into_diagnostic()?;

    todo!()
    // Ok(())
  }

  async fn query_related_fields<'e, 'c: 'e, X>(
    x: X,
    fields_by_table: &FieldsByTable<'_>,
  ) -> sqlx::Result<HashMap<String, Value>>
  where
    X: 'e + Executor<'c, Database = Sqlite>,
  {
    let mut query = QueryBuilder::new("");
    let mut mapping = HashMap::new();
    let mut ctr = 0;

    let mut selected_fields = vec![];
    for ((app_id, app_table_name), fields) in fields_by_table.iter() {
      let table_gen_name = format!("c{ctr}");
      ctr += 1;

      let mut keys = vec![];
      for field_info in fields.iter() {
        let field_gen_name = format!("f{ctr}");
        ctr += 1;
        mapping.insert(&field_info.full_key, field_gen_name.clone());

        keys.push(field_gen_name.clone());

        selected_fields.push(format!(
          "{}.{} as {}",
          table_gen_name, field_info.app_table_field, field_gen_name
        ));

        // constraints.push(format!(
        //   "{}: {}",
        //   field_info.relation_field.to_owned(),
        //   field_gen_name,
        // ));
        // all_fields.push((
        //   field_gen_name,
        //   field_info.relation_field.to_owned(),
        //   key,
        // ))
      }

      // let keys = keys.join(", ");
      // let constraints = constraints.join(", ");
      // all_relation_queries.push(format!(
      //   "
      //   {table_gen_name}[{keys}] :=
      //     *{relation}{{ node_id, {constraints} }},
      //     node_id = $node_id
      //   "
      // ));
      // all_relation_constraints.push(format!("{table_gen_name}[{keys}],"))
    }

    query.push("SELECT");
    query.push(selected_fields.join(", "));
    query.push("FROM");
    println!("Query: {:?}", query.sql());

    // let all_relation_constraints = all_relation_constraints.join("\n");
    // let all_relation_queries = all_relation_queries.join("\n\n");
    // let all_field_names = all_fields
    //   .iter()
    //   .map(|(field_name, _, _)| field_name)
    //   .join(", ");
    // let _query = format!(
    //   "
    //   {all_relation_queries}

    //   ?[type, extra_data, created_at, updated_at, {all_field_names}] :=
    //     *node {{ id, type, created_at, updated_at, extra_data }},
    //     {all_relation_constraints}
    //     id = $node_id
    //   "
    // );

    let rows = query.build().fetch_all(x).await.into_diagnostic();

    todo!()
  }
}

#[derive(Debug)]
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

    let action = match opts {
      CreateOrUpdate::Create { .. } => "put",
      CreateOrUpdate::Update { .. } => "update",
    };

    println!("Request: {opts:?} {extra_data:?}");

    let mut conn = self.conn().await?;

    conn
      .transaction::<_, _, sqlx::Error>(|tx| {
        Box::pin(async move {
          let node_info = match opts {
            CreateOrUpdate::Create { r#type } => {
              AppState::create_node_raw(&mut **tx, &r#type).await?
            }
            CreateOrUpdate::Update { node_id } => todo!(),
          };

          if let Some(extra_data) = extra_data {
            if !extra_data.is_empty() {
              let node_id_str = node_id.to_string();
              let field_mapping = AppState::get_related_field_list_for_node_id(
                &mut **tx,
                &node_id_str,
              )
              .await?;

              // Group the keys by which relation they're in
              let fields_by_table = field_mapping.iter().into_group_map_by(
                |FieldMappingRow {
                   app_id,
                   app_table_name,
                   ..
                 }| (app_id, app_table_name),
              );

              AppState::write_extra_data(
                &mut **tx,
                &node_id_str,
                &fields_by_table,
                extra_data,
              )
              .await?;
            }
          }

          Ok(node_info)
        })
      })
      .await
      .into_diagnostic()
  }

  async fn create_node_raw<'e, 'c: 'e, X>(
    x: X,
    r#type: &str,
  ) -> sqlx::Result<NodeInfo>
  where
    X: 'e + Executor<'c, Database = Sqlite>,
  {
    let node_id = Uuid::now_v7();
    let node_id_str = node_id.to_string();

    #[derive(FromRow)]
    struct Result {
      updated_at: i64,
    }

    let result = sqlx::query_as!(
      Result,
      r#"
      INSERT INTO node (node_id, node_type, extra_data)
      VALUES (?, ?, "{}")
      RETURNING updated_at
      "#,
      node_id_str,
      r#type,
    )
    .fetch_one(x)
    .await?;

    let updated_at =
      DateTime::from_timestamp_millis(result.updated_at * 1000).unwrap();
    let created_at = DateTime::from_timestamp_millis(
      node_id.get_timestamp().unwrap().to_unix().0 as i64 * 1000,
    )
    .unwrap();

    Ok(NodeInfo {
      node_id: NodeId(node_id),
      created_at,
      updated_at,
      fields: None,
    })
  }

  async fn write_extra_data<'e, 'c: 'e, X>(
    x: X,
    node_id: &str,
    fields_by_table: &FieldsByTable<'_>,
    extra_data: ExtraData,
  ) -> sqlx::Result<()>
  where
    X: 'e + Executor<'c, Database = Sqlite>,
  {
    // Update Tantivy indexes
    // for ((app_id, app_table_name), fields) in fields_by_table.iter() {
    //   let mut writer =
    //     self.tantivy_index.writer(15_000_000).into_diagnostic()?;

    //   let delete_term = Term::from_field_text(node_id_field.clone(), &node_id);
    //   writer.delete_term(delete_term);

    //   writer.add_document(doc).into_diagnostic()?;
    //   writer.commit().into_diagnostic()?;
    //   drop(writer);
    // }

    // Update database
    let mut node_has_keys = Vec::new();
    for ((app_id, app_table_name), fields) in fields_by_table.iter() {
      for field_info in fields {
        node_has_keys.push(&field_info.full_key);
      }

      // let mut doc =
      //   btmap! { node_id_field.clone() => OwnedValue::Str(node_id.to_owned()) };
      // let fields_mapping = fields
      //   .into_iter()
      //   .map(
      //     |(
      //       key,
      //       FieldInfo {
      //         relation_field,
      //         r#type,
      //         is_fts_enabled,
      //         ..
      //       },
      //     )| {
      //       let new_value = extra_data.get(*key).unwrap();

      //       // TODO: Make this more generic
      //       let new_value = match r#type.as_str() {
      //         "int" => DataValue::from(new_value.as_i64().unwrap()),
      //         _ => DataValue::from(new_value.as_str().unwrap()),
      //       };

      //       if *is_fts_enabled {
      //         if let Some(field) = self.tantivy_field_map.get_by_left(*key) {
      //           doc.insert(
      //             field.clone(),
      //             OwnedValue::Str(new_value.get_str().unwrap().to_owned()),
      //           );
      //         }
      //       }

      //       (relation_field.to_owned(), new_value)
      //     },
      //   )
      //   .collect::<BTreeMap<_, _>>();

      // let keys = fields_mapping.keys().collect::<Vec<_>>();
      // let keys_joined = keys.iter().join(", ");

      // if !keys.is_empty() {
      //   let query = format!(
      //     "
      //       ?[ node_id, {keys_joined} ] <- [$input_data]
      //       :{action} {relation} {{ node_id, {keys_joined} }}
      //       "
      //   );

      //   let mut params = vec![];
      //   params.push(DataValue::from(node_id.clone()));
      //   for key in keys {
      //     params.push(fields_mapping[key].clone());
      //   }

      //   let result = tx.run_script(
      //     &query,
      //     btmap! {
      //       "input_data".to_owned() => DataValue::List(params),
      //     },
      //   );
      // }
    }

    let mut query =
      QueryBuilder::new("INSERT INTO node_has_key (node_id, full_key) VALUES ");
    query.push_values(node_has_keys, |mut b, key| {
      b.push_bind(node_id).push_bind(key);
    });
    println!("Query: {:?}", query.sql());
    query.build().execute(x).await?;

    Ok(())
  }
}

// impl AppState {

//   pub async fn update_node() {}

//   pub async fn search_nodes(
//     &self,
//     query: impl AsRef<str>,
//   ) -> Result<Vec<(NodeId, Value)>> {
//     let query = query.as_ref();

//     let reader = self.tantivy_index.reader().into_diagnostic()?;
//     let searcher = reader.searcher();

//     let node_id_field = self
//       .tantivy_field_map
//       .get_by_left("node_id")
//       .unwrap()
//       .clone();
//     let journal_page_field = self
//       .tantivy_field_map
//       .get_by_left("panorama/journal/page/content")
//       .unwrap()
//       .clone();
//     let mut query_parser =
//       QueryParser::for_index(&self.tantivy_index, vec![journal_page_field]);
//     query_parser.set_field_fuzzy(journal_page_field, true, 2, true);
//     let query = query_parser.parse_query(query).into_diagnostic()?;

//     let top_docs = searcher
//       .search(&query, &TopDocs::with_limit(10))
//       .into_diagnostic()?;

//     Ok(
//       top_docs
//         .into_iter()
//         .map(|(score, doc_address)| {
//           let retrieved_doc =
//             searcher.doc::<TantivyDocument>(doc_address).unwrap();
//           let node_id = retrieved_doc
//             .get_first(node_id_field.clone())
//             .unwrap()
//             .as_str()
//             .unwrap();
//           let all_fields = retrieved_doc.get_sorted_field_values();
//           let node_id = NodeId(Uuid::from_str(node_id).unwrap());
//           let fields = all_fields
//             .into_iter()
//             .map(|(field, values)| {
//               (
//                 self.tantivy_field_map.get_by_right(&field).unwrap(),
//                 if values.len() == 1 {
//                   owned_value_to_json_value(values[0])
//                 } else {
//                   Value::Array(
//                     values
//                       .into_iter()
//                       .map(owned_value_to_json_value)
//                       .collect_vec(),
//                   )
//                 },
//               )
//             })
//             .collect::<HashMap<_, _>>();
//           (
//             node_id,
//             json!({
//               "score": score,
//               "fields": fields,
//             }),
//           )
//         })
//         .collect::<Vec<_>>(),
//     )
//   }

//   fn get_rows_for_extra_keys(
//     &self,
//     tx: &MultiTransaction,
//     keys: &[String],
//   ) -> Result<FieldMapping> {
//     let result = tx.run_script(
//       "
//         ?[key, relation, field_name, type, is_fts_enabled] :=
//           *fqkey_to_dbkey{key, relation, field_name, type, is_fts_enabled},
//           is_in(key, $keys)
//       ",
//       btmap! {
//         "keys".to_owned() => DataValue::List(
//           keys.into_iter()
//             .map(|s| DataValue::from(s.as_str()))
//             .collect::<Vec<_>>()
//         ),
//       },
//     )?;

//     AppState::rows_to_field_mapping(result)
//   }

//   fn rows_to_field_mapping(result: NamedRows) -> Result<FieldMapping> {
//     let s = |s: &DataValue| s.get_str().unwrap().to_owned();

//     Ok(
//       result
//         .rows
//         .into_iter()
//         .map(|row| {
//           (
//             s(&row[0]),
//             FieldInfo {
//               relation_name: s(&row[1]),
//               relation_field: s(&row[2]),
//               r#type: s(&row[3]),
//               is_fts_enabled: row[4].get_bool().unwrap(),
//             },
//           )
//         })
//         .collect::<HashMap<_, _>>(),
//     )
//   }
// }
