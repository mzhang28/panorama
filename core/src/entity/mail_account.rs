use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "mail_accounts")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub email: String,
	pub display_name: String,
	pub imap_host: String,
	pub imap_port: i32,
	pub smtp_host: String,
	pub smtp_port: i32,
	pub username: String,
	pub password: String, // Note: In production, consider encryption
	pub created_at: DateTimeWithTimeZone,
	pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}