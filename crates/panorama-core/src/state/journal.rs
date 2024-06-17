use std::str::FromStr;

use chrono::Local;
// use cozo::ScriptMutability;
use miette::{IntoDiagnostic, Result};
use uuid::Uuid;

use crate::{AppState, NodeId};

use super::node::CreateOrUpdate;

impl AppState {
  pub async fn get_todays_journal_id(&self) -> Result<NodeId> {
    let today = todays_date();

    let result = self.db.run_script(
      "
      ?[node_id] := *journal_day{day, node_id}, day = $day
    ",
      btmap! {
        "day".to_owned() => today.clone().into(),
      },
      ScriptMutability::Immutable,
    )?;

    // TODO: Do this check on the server side
    if result.rows.len() == 0 {
      // Insert a new one
      // let uuid = Uuid::now_v7();
      // let node_id = uuid.to_string();

      let node_info = self
        .create_or_update_node(
          CreateOrUpdate::Create {
            r#type: "panorama/journal/page".to_owned(),
          },
          Some(btmap! {
            "panorama/journal/page/day".to_owned() => today.clone().into(),
            "panorama/journal/page/content".to_owned() => "".to_owned().into(),
            "panorama/journal/page/title".to_owned() => today.clone().into(),
          }),
        )
        .await?;

      // self.db.run_script(
      //   "
      //     {
      //       ?[id, type] <- [[$node_id, 'panorama/journal/page']]
      //       :put node { id, type }
      //     }
      //     {
      //       ?[node_id, title, content] <- [[$node_id, $title, '']]
      //       :put journal { node_id => title, content }
      //     }
      //     {
      //       ?[day, node_id] <- [[$day, $node_id]]
      //       :put journal_day { day => node_id }
      //     }
      //   ",
      //   btmap! {
      //     "node_id".to_owned() => node_id.clone().into(),
      //     "day".to_owned() => today.clone().into(),
      //     "title".to_owned() => today.clone().into(),
      //   },
      //   ScriptMutability::Mutable,
      // )?;

      return Ok(node_info.node_id);
    }

    let node_id = result.rows[0][0].get_str().unwrap();
    Ok(NodeId(Uuid::from_str(node_id).into_diagnostic()?))
  }
}

fn todays_date() -> String {
  let now = Local::now();
  let date = now.date_naive();
  date.format("%Y-%m-%d").to_string()
}
