use std::time::Duration;

use axum::{extract::State, Json};
use cozo::{DbInstance, ScriptMutability};
use futures::TryStreamExt;
use miette::IntoDiagnostic;
use serde_json::Value;
use tokio::{net::TcpStream, time::sleep};

use crate::{error::AppResult, AppState};

pub async fn get_mail_config(
  State(state): State<AppState>,
) -> AppResult<Json<Value>> {
  let configs = fetch_mail_configs(&state.db)?;
  Ok(Json(json!({
    "configs": configs,
  })))
}

pub async fn mail_loop(db: DbInstance) {
  loop {
    match mail_loop_inner(&db).await {
      Ok(_) => sleep(Duration::from_secs(30)).await,
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

  let inbox = session.select("INBOX").await.into_diagnostic()?;
  println!("last unseen: {:?}", inbox.unseen);

  let messages = session
    .fetch("1", "RFC822")
    .await
    .into_diagnostic()?
    .try_collect::<Vec<_>>()
    .await
    .into_diagnostic()?;
  println!(
    "messages {:?}",
    messages
      .iter()
      .map(|f| f.body().and_then(|t| String::from_utf8(t.to_vec()).ok()))
      .collect::<Vec<_>>()
  );

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
