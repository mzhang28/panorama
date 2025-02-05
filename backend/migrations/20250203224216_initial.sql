CREATE TABLE "node" (
  "id" PRIMARY KEY DEFAULT (UUIDV7_NOW()),
  "created_at" TEXT NOT NULL DEFAULT (NOW_ISO8601()),
  "last_updated_at" TEXT NOT NULL DEFAULT (NOW_ISO8601()),

  -- System stuff
  "panorama_config_key" TEXT,
  "panorama_config_value" TEXT,

  -- General stuff
  "title" TEXT,
  "label" TEXT,

  -- Calendar
  "cal_date" TEXT,
  "cal_date_with_time" BOOLEAN,
  "cal_date_end" TEXT,
  "cal_date_end_with_time" BOOLEAN,

  -- Contacts
  "contact_name" TEXT,
  "contact_email" TEXT,

  -- Mail
  "mail_account_imap_host" TEXT,
  "mail_account_imap_port" TEXT,
  "mail_account_imap_username" TEXT,
  "mail_account_imap_password" TEXT,
  "mailbox_account_id" TEXT,
  "mailbox_name" TEXT,
  "mailbox_uid_validity" TEXT,
  "message_mailbox_id" TEXT,
  "message_uid" TEXT,
  "message_from" TEXT,
  "message_from_contact_id" TEXT,
  "message_body" TEXT,

  -- wakatime heartbeat
  "wakatime_language" TEXT,
  "wakatime_project" TEXT,

  -- extra json shit
  "json" TEXT
);

CREATE INDEX idx_node_panorama_config_key ON node(panorama_config_key);

CREATE INDEX idx_node_cal_date ON node(cal_date);
CREATE INDEX idx_node_cal_date_with_time ON node(cal_date_with_time);
CREATE INDEX idx_node_cal_date_end ON node(cal_date_end);
CREATE INDEX idx_node_cal_date_end_with_time ON node(cal_date_end_with_time);

CREATE INDEX idx_node_wakatime_language ON node(wakatime_language);
CREATE INDEX idx_node_wakatime_project ON node(wakatime_project);
