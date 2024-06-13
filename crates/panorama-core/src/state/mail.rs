use std::{collections::HashMap, str::FromStr, time::Duration};

use cozo::{DataValue, JsonData, ScriptMutability};
use futures::TryStreamExt;
use miette::{IntoDiagnostic, Result};
use tokio::{net::TcpStream, time::sleep};
use uuid::Uuid;

use crate::{AppState, NodeId};

#[derive(Debug, Serialize)]
pub struct MailConfig {
  pub node_id: NodeId,
  pub imap_hostname: String,
  pub imap_port: u16,
  pub imap_username: String,
  pub imap_password: String,
}

impl AppState {
  /// Fetch the list of mail configs in the database
  pub fn fetch_mail_configs(&self) -> Result<Vec<MailConfig>> {
    let result = self.db.run_script(
      "
        ?[node_id, imap_hostname, imap_port, imap_username, imap_password] :=
          *node{ id: node_id },
          *mail_config{ node_id, imap_hostname, imap_port, imap_username, imap_password }
      ",
      Default::default(),
      ScriptMutability::Immutable,
    )?;

    let result = result
      .rows
      .into_iter()
      .map(|row| MailConfig {
        node_id: NodeId(Uuid::from_str(row[0].get_str().unwrap()).unwrap()),
        imap_hostname: row[1].get_str().unwrap().to_owned(),
        imap_port: row[2].get_int().unwrap() as u16,
        imap_username: row[3].get_str().unwrap().to_owned(),
        imap_password: row[4].get_str().unwrap().to_owned(),
      })
      .collect::<Vec<_>>();

    Ok(result)
  }
}
