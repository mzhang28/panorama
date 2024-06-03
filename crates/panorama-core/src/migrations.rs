use cozo::DbInstance;
use miette::{IntoDiagnostic, Result};

use serde_json::Value;

use crate::ensure_ok;

pub async fn run_migrations(db: &DbInstance) -> Result<()> {
  let migration_status = check_migration_status(db).await?;
  println!("migration status: {:?}", migration_status);

  let migrations: Vec<Box<dyn for<'a> Fn(&'a DbInstance) -> Result<()>>> =
    vec![Box::new(no_op), Box::new(migration_01)];

  if let MigrationStatus::NoMigrations = migration_status {
    let result = db.run_script_str(
      "
        { :create migrations { yeah: Int default 0 => version: Int default 0 } }
        {
          ?[yeah, version] <- [[0, 0]]
          :put migrations { yeah, version }
        }
      ",
      "",
      false,
    );
    ensure_ok(&result)?;
  }

  let start_at_migration = match migration_status {
    MigrationStatus::NoMigrations => 0,
    MigrationStatus::HasVersion(n) => n,
  };
  let migrations_to_run = migrations
    .iter()
    .enumerate()
    .skip(start_at_migration as usize + 1);
  // println!("running {} migrations...", migrations_to_run.len());

  //TODO: This should all be done in a transaction
  for (idx, migration) in migrations_to_run {
    println!("running migration {idx}...");

    migration(db)?;

    let result = db.run_script_str(
      "
        ?[yeah, version] <- [[0, $version]]
        :put migrations { yeah => version }
      ",
      &format!("{{\"version\":{}}}", idx),
      false,
    );

    ensure_ok(&result)?;

    println!("succeeded migration {idx}!");
  }

  Ok(())
}

#[derive(Debug)]
enum MigrationStatus {
  NoMigrations,
  HasVersion(u64),
}

async fn check_migration_status(db: &DbInstance) -> Result<MigrationStatus> {
  let status = db.run_script_str(
    "
      ?[yeah, version] := *migrations[yeah, version]
    ",
    "",
    true,
  );
  println!("Status: {}", status);

  let status: Value = serde_json::from_str(&status).into_diagnostic()?;
  let status = status.as_object().unwrap();
  let ok = status.get("ok").unwrap().as_bool().unwrap_or(false);
  if !ok {
    let status_code = status.get("code").unwrap().as_str().unwrap();
    if status_code == "query::relation_not_found" {
      return Ok(MigrationStatus::NoMigrations);
    }
  }

  let rows = status.get("rows").unwrap().as_array().unwrap();
  let row = rows[0].as_array().unwrap();
  let version = row[1].as_number().unwrap().as_u64().unwrap();
  println!("row: {row:?}");

  Ok(MigrationStatus::HasVersion(version))
}

fn no_op(_: &DbInstance) -> Result<()> {
  Ok(())
}

fn migration_01(db: &DbInstance) -> Result<()> {
  let result = db.run_script_str(
    "
      # Primary node type
      {
        :create node {
          id: String
          =>
          type: String,
          title: String? default null,
          created_at: Float default now(),
          updated_at: Float default now(),
          extra_data: Json default {},
        }
      }

      # Inverse mappings for easy querying
      { :create node_has_key { key: String => id: String } }
      { :create node_managed_by_app { node_id: String => app: String } }
      { :create node_refers_to { node_id: String => other_node_id: String } }
      {
        :create fqkey_to_dbkey {
          key: String
          =>
          relation: String,
          field_name: String,
          type: String,
          fts_enabled: Bool,
        }
      }
      {
        ?[key, relation, field_name, type, fts_enabled] <- [
          ['panorama/journal/page/content', 'journal', 'content', 'string', true],
          ['panorama/mail/config/imap_hostname', 'mail_config', 'imap_hostname', 'string', false],
          ['panorama/mail/config/imap_port', 'mail_config', 'imap_port', 'int', false],
          ['panorama/mail/config/imap_username', 'mail_config', 'imap_username', 'string', false],
          ['panorama/mail/config/imap_password', 'mail_config', 'imap_password', 'string', false],
        ]
        :put fqkey_to_dbkey { key, relation, field_name, type, fts_enabled }
      }

      # Create journal type
      { :create journal { node_id: String => content: String } }
      { :create journal_day { day: String => node_id: String } }
      {
        ::fts create journal:text_index {
          extractor: content,
          extract_filter: !is_null(content),
          tokenizer: Simple,
          filters: [Lowercase, Stemmer('english'), Stopwords('en')],
        }
      }

      # Mail
      {
        :create mail_config {
          node_id: String
          =>
          imap_hostname: String,
          imap_port: Int,
          imap_username: String,
          imap_password: String,
        }
      }
      {
        :create mailbox {
          node_id: String
          =>
          account_node_id: String,
          mailbox_name: String,
        }
      }
      { ::index create mailbox:by_account_id_and_name { account_node_id, mailbox_name } }
      {
        :create message {
          node_id: String
          =>
          account_node_id: String,
          mailbox_node_id: String,
          subject: String,
          headers: Json?,
          body: Bytes,
          internal_date: String,
        }
      }

      # Calendar
    ",
    "",
    false,
  );
  ensure_ok(&result)?;

  Ok(())
}
