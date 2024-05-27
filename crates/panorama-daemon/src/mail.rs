use axum::{extract::State, Json};
use cozo::{DbInstance, ScriptMutability};
use serde_json::Value;

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
  // Fetch the mail configs
}

#[derive(Serialize)]
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
