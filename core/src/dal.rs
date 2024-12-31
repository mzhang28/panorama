use anyhow::Result;
use sqlx::SqlitePool;

use crate::entity::mail_account;
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

#[derive(Default)]
pub struct MailAccountOptions {
  pub id: Option<Uuid>,
  pub email: Option<String>,
  pub display_name: Option<String>,
  pub imap_host: Option<String>,
  pub imap_port: Option<i32>,
  pub smtp_host: Option<String>,
  pub smtp_port: Option<i32>,
  pub username: Option<String>,
  pub password: Option<String>,
}

pub struct PanoramaDatabase {
  connection: DatabaseConnection,
}

impl PanoramaDatabase {
  pub async fn new(connection: DatabaseConnection) -> Self {
    Self { connection }
  }

  async fn ensure_mail_account(
    &self,
    options: MailAccountOptions,
  ) -> Result<mail_account::Model, sea_orm::DbErr> {
    let account = mail_account::ActiveModel {
      id: Set(options.id.unwrap_or_else(Uuid::new_v4)),
      email: Set(options.email.unwrap_or_default()),
      display_name: Set(options.display_name.unwrap_or_default()),
      imap_host: Set(options.imap_host.unwrap_or_default()),
      imap_port: Set(options.imap_port.unwrap_or(993)),
      smtp_host: Set(options.smtp_host.unwrap_or_default()),
      smtp_port: Set(options.smtp_port.unwrap_or(587)),
      username: Set(options.username.unwrap_or_default()),
      password: Set(options.password.unwrap_or_default()),
    };

    mail_account::Entity::insert(account).exec(&self.connection)
  }
}

// pub struct PanoramaDatabase(SqlitePool);

// pub struct EnsureMailAccount {
//   imap_server_host: String,
//   imap_server_port: u16,
// }

// impl PanoramaDatabase {
//   pub async fn ensure_mail_account(&self) -> Result<()> {
//     Ok(())
//   }
// }
