use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use anyhow::{Context as _, Result};
use async_imap::Session;
use backoff::{exponential::ExponentialBackoff, SystemClock};
use futures::TryStreamExt;
use itertools::Itertools;
use tokio::{net::TcpStream, time::sleep};
use uuid::Uuid;

use crate::{mail, AppState};

pub struct MailWorker {
  state: AppState,
}

impl MailWorker {
  pub fn new(state: AppState) -> MailWorker {
    MailWorker { state }
  }

  pub async fn mail_loop(self) -> Result<()> {
    loop {
      let mut policy = ExponentialBackoff::<SystemClock>::default();
      policy.current_interval = Duration::from_secs(5);
      policy.initial_interval = Duration::from_secs(5);

      backoff::future::retry(policy, || async {
        match self.mail_loop_inner().await {
          Ok(_) => {}
          Err(err) => {
            eprintln!("Mail error: {:?}", err);
            Err(err)?;
          }
        }
        // For now, just sleep 30 seconds and then fetch again
        // TODO: Run a bunch of connections at once and do IDLE over them (if possible)
        sleep(Duration::from_secs(30)).await;

        Ok(())
      })
      .await?;
    }

    Ok(())
  }

  async fn mail_loop_inner(&self) -> Result<()> {
    // Fetch the mail configs
    let configs = self.state.fetch_mail_configs()?;
    if configs.is_empty() {
      return Ok(());
    }

    // TODO: Do all configs instead of just the first
    let config = &configs[0];

    let stream =
      TcpStream::connect((config.imap_hostname.as_str(), config.imap_port))
        .await?;

    let client = async_imap::Client::new(stream);
    let mut session = client
      .login(&config.imap_username, &config.imap_password)
      .await
      .map_err(|(err, _)| err)?;

    let all_mailbox_ids = self
      .fetch_and_store_all_mailboxes(config.node_id.to_string(), &mut session)
      .await
      .context("Could not fetch mailboxes")?;

    self
      .fetch_all_mail_from_single_mailbox(
        &mut session,
        &all_mailbox_ids,
        config.node_id.to_string(),
        "INBOX",
      )
      .await
      .context("Could not fetch mail from INBOX")?;

    session.logout().await.into_diagnostic()?;

    Ok(())
  }

  async fn fetch_and_store_all_mailboxes(
    &self,
    config_node_id: String,
    session: &mut Session<TcpStream>,
  ) -> Result<HashMap<String, String>> {
    // println!("Session: {:?}", session);
    let mailboxes = session
      .list(None, Some("*"))
      .await?
      .try_collect::<Vec<_>>()
      .await?;

    let mut all_mailboxes = HashMap::new();

    // TODO: Make this more efficient by using bulk in query

    for mailbox in mailboxes {
      let tx = self.state.db.multi_transaction(true);

      let result = tx.run_script(
        "
        ?[node_id] :=
          *mailbox{node_id, account_node_id, mailbox_name},
          account_node_id = $account_node_id,
          mailbox_name = $mailbox_name,
        ",
        btmap! {
          "account_node_id".to_owned()=>DataValue::from(config_node_id.clone()),
          "mailbox_name".to_owned()=>DataValue::from(mailbox.name().to_string()),
        },
      )?;

      let node_id = if result.rows.len() == 0 {
        let new_node_id = Uuid::now_v7();
        let new_node_id = new_node_id.to_string();
        let extra_data = json!({
          "name": mailbox.name(),
        });
        tx.run_script("
            ?[node_id, account_node_id, mailbox_name, extra_data] <-
              [[$new_node_id, $account_node_id, $mailbox_name, $extra_data]]
            :put mailbox { node_id, account_node_id, mailbox_name, extra_data }
          ", 
          btmap! {
            "new_node_id".to_owned() => DataValue::from(new_node_id.clone()),
            "account_node_id".to_owned() => DataValue::from(config_node_id.clone()),
            "mailbox_name".to_owned()=>DataValue::from(mailbox.name().to_string()),
            "extra_data".to_owned()=>DataValue::Json(JsonData(extra_data)),
          },
        )?;
        new_node_id
      } else {
        result.rows[0][0].get_str().unwrap().to_owned()
      };

      tx.commit()?;

      all_mailboxes.insert(mailbox.name().to_owned(), node_id);
    }

    // println!("All mailboxes: {:?}", all_mailboxes);

    Ok(all_mailboxes)
  }

  async fn fetch_all_mail_from_single_mailbox(
    &self,
    session: &mut Session<TcpStream>,
    all_mailbox_ids: &HashMap<String, String>,
    config_node_id: String,
    mailbox_name: impl AsRef<str>,
  ) -> Result<()> {
    let mailbox_name = mailbox_name.as_ref();
    let mailbox = session.select(mailbox_name).await.into_diagnostic()?;
    let mailbox_node_id = all_mailbox_ids.get(mailbox_name).unwrap();

    let extra_data = json!({
      "uid_validity": mailbox.uid_validity,
      "last_seen": mailbox.unseen,
    });

    // TODO: Validate uid validity here

    let all_uids = session
      .uid_search("ALL")
      .await
      .context("Could not fetch all UIDs")?;

    println!("All UIDs ({}): {:?}", all_uids.len(), all_uids);

    let messages = session
      .uid_fetch(
        all_uids.iter().join(","),
        "(FLAGS ENVELOPE BODY[HEADER] BODY[TEXT] INTERNALDATE)",
      )
      .await
      .into_diagnostic()?
      .try_collect::<Vec<_>>()
      .await
      .into_diagnostic()
      .context("Could not fetch messages")?;
    println!(
      "messages {:?}",
      messages.iter().map(|f| f.body()).collect::<Vec<_>>()
    );

    let mut unique_message_ids = HashSet::new();
    let data: Vec<_> = messages
      .iter()
      .map(|msg| {
        let message_node_id = Uuid::now_v7();
        let headers =
          String::from_utf8(msg.header().unwrap().to_vec()).unwrap();
        let headers = headers
          .split("\r\n")
          .filter_map(|s| {
            // This is really bad lmao
            let p = s.split(": ").collect::<Vec<_>>();
            if p.len() < 2 {
              None
            } else {
              Some((p[0], p[1..].join(": ")))
            }
          })
          .collect::<HashMap<_, _>>();

        let message_id = headers
          .get("Message-ID")
          .map(|s| (*s).to_owned())
          .unwrap_or(message_node_id.to_string());
        unique_message_ids.insert(message_id.clone());

        DataValue::List(vec![
          DataValue::from(message_node_id.to_string()),
          DataValue::from(config_node_id.to_string()),
          DataValue::from(mailbox_node_id.clone()),
          DataValue::from(
            headers
              .get("Subject")
              .map(|s| (*s).to_owned())
              .unwrap_or("Subject".to_owned()),
          ),
          DataValue::Json(JsonData(serde_json::to_value(&headers).unwrap())),
          DataValue::Bytes(msg.text().unwrap().to_vec()),
          DataValue::from(msg.internal_date().unwrap().to_rfc3339()),
          DataValue::from(message_id),
        ])
      })
      .collect();

    println!("Adding {} messages to database...", data.len());
    let input_data = DataValue::List(data);

    // TODO: Can this be one query?
    let tx = self.state.db.multi_transaction(true);

    let unique_message_ids_data_value = DataValue::List(
      unique_message_ids
        .into_iter()
        .map(|s| DataValue::from(s))
        .collect_vec(),
    );

    let existing_ids = tx.run_script(
      "
      ?[node_id] := *message { node_id, message_id },
        is_in(message_id, $message_ids)
      ",
      btmap! { "message_ids".to_owned() => unique_message_ids_data_value },
    )?;
    println!("Existing ids: {:?}", existing_ids);

    self
      .state
      .db
      .run_script(
        "
        ?[
          node_id, account_node_id, mailbox_node_id, subject, headers, body,
          internal_date, message_id
        ] <- $input_data
        :put message {
          node_id, account_node_id, mailbox_node_id, subject, headers, body,
          internal_date, message_id
        }
      ",
        btmap! {
          "input_data".to_owned() => input_data,
        },
        ScriptMutability::Mutable,
      )
      .context("Could not add message to database")?;

    Ok(())
  }
}
