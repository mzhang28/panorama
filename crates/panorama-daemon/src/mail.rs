use std::{collections::HashMap, default, time::Duration};

use axum::{extract::State, routing::head, Json};
use cozo::{DataValue, DbInstance, JsonData, ScriptMutability};
use futures::TryStreamExt;
use miette::IntoDiagnostic;
use serde_json::Value;
use tokio::{net::TcpStream, time::sleep};
use uuid::Uuid;

use crate::{error::AppResult, AppState};

pub async fn get_mail_config(
  State(state): State<AppState>,
) -> AppResult<Json<Value>> {
  let configs = fetch_mail_configs(&state.db)?;
  Ok(Json(json!({ "configs": configs })))
}

pub async fn get_mail(State(state): State<AppState>) -> AppResult<Json<Value>> {
  let mailboxes = state.db.run_script("
    ?[node_id, account_node_id, mailbox_name] := *mailbox {node_id, account_node_id, mailbox_name}
  ", Default::default(), ScriptMutability::Immutable)?;

  let mailboxes = mailboxes
    .rows
    .iter()
    .map(|mb| {
      json!({
        "node_id": mb[0].get_str().unwrap(),
        "account_node_id": mb[1].get_str().unwrap(),
        "mailbox_name": mb[2].get_str().unwrap(),
      })
    })
    .collect::<Vec<_>>();

  let messages = state.db.run_script("
    ?[node_id, subject, body, internal_date] := *message {node_id, subject, body, internal_date}
    :limit 10
  ", Default::default(), ScriptMutability::Immutable)?;

  let messages = messages
    .rows
    .iter()
    .map(|m| {
      json!({
        "node_id": m[0].get_str().unwrap(),
        "subject": m[1].get_str().unwrap(),
        "body": m[2].get_str(),
        "internal_date": m[3].get_str().unwrap(),
      })
    })
    .collect::<Vec<_>>();

  Ok(Json(json!({
    "mailboxes": mailboxes,
    "messages": messages,
  })))
}

pub async fn mail_loop(db: DbInstance) {
  loop {
    match mail_loop_inner(&db).await {
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

async fn mail_loop_inner(db: &DbInstance) -> AppResult<()> {
  // Fetch the mail configs
  let configs = fetch_mail_configs(&db)?;
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
    let result = db.run_script("
      ?[node_id] :=
        *mailbox{node_id, account_node_id, mailbox_name},
        account_node_id = $account_node_id,
        mailbox_name = 'INBOX'
      ", btmap! {"account_node_id".to_owned()=>DataValue::from(config.node_id.to_owned())}, ScriptMutability::Immutable)?;

    if result.rows.len() == 0 {
      let new_node_id = Uuid::now_v7();
      let new_node_id = new_node_id.to_string();
      db.run_script("
        ?[node_id, account_node_id, mailbox_name] <-
          [[$new_node_id, $account_node_id, 'INBOX']]
        :put mailbox { node_id, account_node_id, mailbox_name }
  ", 
  btmap! {
    "new_node_id".to_owned() => DataValue::from(new_node_id.clone()),
    "account_node_id".to_owned() => DataValue::from(config.node_id.to_owned()),
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
    messages
      .iter()
      .map(|f| f.internal_date())
      .collect::<Vec<_>>()
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
          DataValue::from(config.node_id.clone()),
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

  db.run_script(
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

#[derive(Debug, Serialize)]
struct MailConfig {
  node_id: String,
  imap_hostname: String,
  imap_port: u16,
  imap_username: String,
  imap_password: String,
}

fn fetch_mail_configs(db: &DbInstance) -> AppResult<Vec<MailConfig>> {
  let result = db.run_script(
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
      node_id: row[0].get_str().unwrap().to_owned(),
      imap_hostname: row[1].get_str().unwrap().to_owned(),
      imap_port: row[2].get_int().unwrap() as u16,
      imap_username: row[3].get_str().unwrap().to_owned(),
      imap_password: row[4].get_str().unwrap().to_owned(),
    })
    .collect::<Vec<_>>();

  Ok(result)
}
