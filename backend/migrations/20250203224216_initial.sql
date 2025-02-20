CREATE TABLE "node" (
  "id" PRIMARY KEY DEFAULT (UUIDV7_NOW()),
  "created_at" TEXT NOT NULL DEFAULT (NOW_ISO8601()),
  "last_updated_at" TEXT NOT NULL DEFAULT (NOW_ISO8601()),

  -- System stuff
  "panorama_seed_id" TEXT,
  "panorama_config_key" TEXT,
  "panorama_config_value" TEXT,

  -- General stuff
  "title" TEXT,
  "label" TEXT,
  "content" TEXT, -- generic content

  -- Journal
  -- NOTE: We are using a YYYY-MM-DD string date rather than a timestamp
  -- We want journals to be dated, but this distinction is rather arbitrary
  "journal_date" TEXT,

  -- File
  "blob_hash" TEXT,

  -- Automate stuff
  "automate_flow_json" TEXT,
  "automate_trigger_json" TEXT,
  "automate_node_json" TEXT,
  "automate_run_json" TEXT,

  -- Data viz stuff
  "dataviz_graph_type" TEXT,
  "dataviz_query" TEXT,
  "dataviz_options_json" TEXT,

  -- Calendar
  "cal_date" INTEGER,
  "cal_date_json" TEXT,
  "cal_date_end" INTEGER,
  "cal_date_end_json" TEXT,
  "cal_location" TEXT,

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

CREATE INDEX idx_node_panorama_seed_id ON node(panorama_seed_id);
CREATE UNIQUE INDEX idx_node_panorama_config_key ON node(panorama_config_key);

CREATE UNIQUE INDEX idx_node_journal_date ON node(journal_date);

CREATE INDEX idx_node_cal_date ON node(cal_date);
CREATE INDEX idx_node_cal_date_end ON node(cal_date_end);

CREATE INDEX idx_node_wakatime_language ON node(wakatime_language);
CREATE INDEX idx_node_wakatime_project ON node(wakatime_project);

CREATE TABLE "tags" (
  "node_id" TEXT,
  "tag" TEXT,

  PRIMARY KEY (node_id, tag)
);
