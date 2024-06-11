use std::str::FromStr;

use chrono::Local;
use cozo::ScriptMutability;
use miette::{IntoDiagnostic, Result};
use uuid::Uuid;

use crate::{AppState, NodeId};

impl AppState {
  pub async fn get_todays_journal_id(&self) -> Result<NodeId> {
    let today = todays_date();

    let result = self.db.run_script(
      "
      ?[node_id] := *journal_day[day, node_id], day = $day
    ",
      btmap! {
        "day".to_owned() => today.clone().into(),
      },
      ScriptMutability::Immutable,
    )?;

    // TODO: Do this check on the server side
    if result.rows.len() == 0 {
      // Insert a new one
      let uuid = Uuid::now_v7();
      let node_id = uuid.to_string();

      self.db.run_script(
        "
          {
            ?[id, title, type] <- [[$node_id, $title, 'panorama/journal/page']]
            :put node { id, title, type }
          }
          {
            ?[node_id, content] <- [[$node_id, '']]
            :put journal { node_id => content }
          }
          {
            ?[day, node_id] <- [[$day, $node_id]]
            :put journal_day { day => node_id }
          }
        ",
        btmap! {
          "node_id".to_owned() => node_id.clone().into(),
          "day".to_owned() => today.clone().into(),
          "title".to_owned() => today.clone().into(),
        },
        ScriptMutability::Mutable,
      )?;

      return Ok(NodeId(uuid));
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
