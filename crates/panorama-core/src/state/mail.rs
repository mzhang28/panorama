use std::{collections::HashMap, str::FromStr, time::Duration};

use cozo::{DataValue, JsonData, ScriptMutability};
use futures::TryStreamExt;
use miette::{IntoDiagnostic, Result};
use tokio::{net::TcpStream, time::sleep};
use uuid::Uuid;

use crate::{AppState, NodeId};

#[derive(Debug, Serialize)]
pub struct MailConfig {
  node_id: NodeId,
  imap_hostname: String,
  imap_port: u16,
  imap_username: String,
  imap_password: String,
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

  pub async fn mail_loop(&self) {
    loop {
      match self.mail_loop_inner().await {
        Ok(_) => {
          // For now, just sleep 30 seconds and then fetch again
          // TODO: Run a bunch of connections at once and do IDLE over them (if possible)
          sleep(Duration::from_secs(30)).await;
        }
        Err(err) => {
          eprintln!("Fetch config error: {err:?}");
          // Back off, retry
          // TODO: Exponential backoff
          sleep(Duration::from_secs(5)).await;
          continue;
        }
      }
    }
  }

  async fn mail_loop_inner(&self) -> Result<()> {
    // Fetch the mail configs
    let configs = self.fetch_mail_configs()?;
    if configs.len() == 0 {
      return Ok(());
    }

    // TODO: Do all configs instead of just the first
    let config = &configs[0];

    let stream =
      TcpStream::connect((config.imap_hostname.as_str(), config.imap_port))
        .await
        .into_diagnostic()?;

    let client = async_imap::Client::new(stream);
    let mut session = client
      .login(&config.imap_username, &config.imap_password)
      .await
      .map_err(|(err, _)| err)
      .into_diagnostic()?;

    // println!("Session: {:?}", session);
    let mailboxes = session
      .list(None, Some("*"))
      .await
      .into_diagnostic()?
      .try_collect::<Vec<_>>()
      .await
      .into_diagnostic()?;
    let mailbox_names =
      mailboxes.iter().map(|name| name.name()).collect::<Vec<_>>();
    println!("mailboxes: {mailbox_names:?}");

    // Get the mailbox with INBOX
    let inbox_node_id = {
      let result = self.db.run_script("
      ?[node_id] :=
        *mailbox{node_id, account_node_id, mailbox_name},
        account_node_id = $account_node_id,
        mailbox_name = 'INBOX'
      ", btmap! {"account_node_id".to_owned()=>DataValue::from(config.node_id.to_string())}, ScriptMutability::Immutable)?;

      if result.rows.len() == 0 {
        let new_node_id = Uuid::now_v7();
        let new_node_id = new_node_id.to_string();
        self.db.run_script("
        ?[node_id, account_node_id, mailbox_name] <-
          [[$new_node_id, $account_node_id, 'INBOX']]
        :put mailbox { node_id, account_node_id, mailbox_name }
  ", 
  btmap! {
    "new_node_id".to_owned() => DataValue::from(new_node_id.clone()),
    "account_node_id".to_owned() => DataValue::from(config.node_id.to_string()),
  },
 ScriptMutability::Mutable)?;
        new_node_id
      } else {
        result.rows[0][0].get_str().unwrap().to_owned()
      }
    };
    println!("INBOX: {:?}", inbox_node_id);

    let inbox = session.select("INBOX").await.into_diagnostic()?;
    println!("last unseen: {:?}", inbox.unseen);

    let messages = session
      .fetch(
        "1:4",
        "(FLAGS ENVELOPE BODY[HEADER] BODY[TEXT] INTERNALDATE)",
      )
      .await
      .into_diagnostic()?
      .try_collect::<Vec<_>>()
      .await
      .into_diagnostic()?;
    println!(
      "messages {:?}",
      messages.iter().map(|f| f.body()).collect::<Vec<_>>()
    );

    let input_data = DataValue::List(
      messages
        .iter()
        .map(|msg| {
          let message_id = Uuid::now_v7();
          let headers =
            String::from_utf8(msg.header().unwrap().to_vec()).unwrap();
          let headers = headers
            .split("\r\n")
            .filter_map(|s| {
              let p = s.split(": ").collect::<Vec<_>>();
              if p.len() < 2 {
                None
              } else {
                Some((p[0], p[1]))
              }
            })
            .collect::<HashMap<_, _>>();
          DataValue::List(vec![
            DataValue::from(message_id.to_string()),
            DataValue::from(config.node_id.to_string()),
            DataValue::from(inbox_node_id.clone()),
            DataValue::from(
              headers
                .get("Subject")
                .map(|s| (*s).to_owned())
                .unwrap_or("Subject".to_owned()),
            ),
            DataValue::Json(JsonData(serde_json::to_value(headers).unwrap())),
            DataValue::Bytes(msg.text().unwrap().to_vec()),
            DataValue::from(msg.internal_date().unwrap().to_rfc3339()),
          ])
        })
        .collect(),
    );

    self.db.run_script(
      "
        ?[node_id, account_node_id, mailbox_node_id, subject, headers, body, internal_date] <- $input_data
        :put message { node_id, account_node_id, mailbox_node_id, subject, headers, body, internal_date }
      ",
      btmap! {
        "input_data".to_owned() => input_data,
      },
      ScriptMutability::Mutable,
    )?;

    session.logout().await.into_diagnostic()?;

    Ok(())
  }
}
