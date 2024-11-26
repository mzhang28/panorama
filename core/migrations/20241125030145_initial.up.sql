CREATE TABLE "node" (
  "id" TEXT NOT NULL PRIMARY KEY,
  "name" TEXT,

  "created_at" DATETIME NOT NULL,
  "updated_at" DATETIME NOT NULL
);

CREATE TABLE "mail_account" (
  "node_id" TEXT NOT NULL PRIMARY KEY,

  "imap_server_host" TEXT,
  "imap_server_port" INT,
  "imap_server_auth_method" TEXT,
  "imap_server_auth_username" TEXT,
  "imap_server_auth_password" TEXT,

  "smtp_server_host" TEXT,
  "smtp_server_port" INT,

  FOREIGN KEY ("node_id") REFERENCES "node"("id")
);

CREATE TABLE "mail_mailbox" (
  "node_id" TEXT NOT NULL PRIMARY KEY,
  "mail_account_id" TEXT NOT NULL,
  "name" TEXT NOT NULL,

  FOREIGN KEY ("node_id") REFERENCES "node"("id"),
  FOREIGN KEY ("mail_account_id") REFERENCES "mail_account"("node_id")
);

CREATE TABLE "maiL_message" (
  "node_id" TEXT NOT NULL PRIMARY KEY,
  "mail_account_id" TEXT NOT NULL,
  "mail_mailbox_id" TEXT NOT NULL,

  FOREIGN KEY ("node_id") REFERENCES "node"("id"),
  FOREIGN KEY ("mail_account_id") REFERENCES "mail_account"("node_id")
  FOREIGN KEY ("mail_mailbox_id") REFERENCES "mail_mailbox"("node_id")
);