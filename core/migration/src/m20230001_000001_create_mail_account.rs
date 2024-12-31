use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(MailAccount::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(MailAccount::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(MailAccount::Email).string().not_null())
					.col(ColumnDef::new(MailAccount::DisplayName).string().not_null())
					.col(ColumnDef::new(MailAccount::ImapHost).string().not_null())
					.col(ColumnDef::new(MailAccount::ImapPort).integer().not_null())
					.col(ColumnDef::new(MailAccount::SmtpHost).string().not_null())
					.col(ColumnDef::new(MailAccount::SmtpPort).integer().not_null())
					.col(ColumnDef::new(MailAccount::Username).string().not_null())
					.col(ColumnDef::new(MailAccount::Password).string().not_null())
					.col(
						ColumnDef::new(MailAccount::CreatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(MailAccount::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(MailAccount::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum MailAccount {
	Table,
	Id,
	Email,
	DisplayName,
	ImapHost,
	ImapPort,
	SmtpHost,
	SmtpPort,
	Username,
	Password,
	CreatedAt,
	UpdatedAt,
}