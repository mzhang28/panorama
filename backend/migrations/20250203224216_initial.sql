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
  "blob_mime" TEXT,
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
  "cal_date" TEXT,
  "cal_date_json" TEXT,
  "cal_date_end" TEXT,
  "cal_date_end_json" TEXT,
  "cal_location" TEXT,

  -- Contacts
  "contact_name" TEXT,
  "contact_email" TEXT,

  -- wakatime heartbeat
  "wakatime_language" TEXT,
  "wakatime_project" TEXT,

  -- Zotero shit
  "zotero_parent_collection" TEXT,
  "zotero_json" TEXT,

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

CREATE TRIGGER update_last_updated
AFTER UPDATE ON node
FOR EACH ROW BEGIN
  UPDATE node
  SET last_updated_at = NOW_ISO8601()
  WHERE id = OLD.id;
END;
