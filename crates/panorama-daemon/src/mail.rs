

// pub async fn get_mail_config(
//   State(state): State<AppState>,
// ) -> AppResult<Json<Value>> {
//   let configs = state.fetch_mail_configs()?;
//   Ok(Json(json!({ "configs": configs })))
// }

// pub async fn get_mail(State(state): State<AppState>) -> AppResult<Json<Value>> {
//   let mailboxes = state.db.run_script("
//     ?[node_id, account_node_id, mailbox_name] := *mailbox {node_id, account_node_id, mailbox_name}
//   ", Default::default(), ScriptMutability::Immutable)?;

//   let mailboxes = mailboxes
//     .rows
//     .iter()
//     .map(|mb| {
//       json!({
//         "node_id": mb[0].get_str().unwrap(),
//         "account_node_id": mb[1].get_str().unwrap(),
//         "mailbox_name": mb[2].get_str().unwrap(),
//       })
//     })
//     .collect::<Vec<_>>();

//   let messages = state.db.run_script("
//     ?[node_id, subject, body, internal_date] := *message {node_id, subject, body, internal_date}
//     :limit 10
//   ", Default::default(), ScriptMutability::Immutable)?;

//   let messages = messages
//     .rows
//     .iter()
//     .map(|m| {
//       json!({
//         "node_id": m[0].get_str().unwrap(),
//         "subject": m[1].get_str().unwrap(),
//         "body": m[2].get_str(),
//         "internal_date": m[3].get_str().unwrap(),
//       })
//     })
//     .collect::<Vec<_>>();

//   Ok(Json(json!({
//     "mailboxes": mailboxes,
//     "messages": messages,
//   })))
// }
