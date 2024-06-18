CREATE TABLE node (
  node_id TEXT PRIMARY KEY,
  node_type TEXT NOT NULL,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  extra_data JSON
);

CREATE TABLE node_has_key (
  node_id TEXT NOT NULL,
  full_key TEXT NOT NULL,
  PRIMARY KEY (node_id, full_key)
);
CREATE INDEX node_has_key_idx_node_id ON node_has_key(node_id);
CREATE INDEX node_has_key_idx_full_key ON node_has_key(full_key);

-- App-related tables
CREATE TABLE app (
  app_id INTEGER PRIMARY KEY AUTOINCREMENT,
  app_name TEXT NOT NULL,
  app_version TEXT NOT NULL,
  app_version_hash TEXT,
  app_description TEXT,
  app_homepage TEXT,
  app_repository TEXT,
  app_license TEXT
);

CREATE TABLE app_table_mapping (
  app_id INTEGER NOT NULL,
  app_table_name TEXT NOT NULL,
  db_table_name TEXT NOT NULL
);

CREATE TABLE key_mapping (
  full_key TEXT NOT NULL,
  app_id INTEGER NOT NULL,
  app_table_name TEXT NOT NULL,
  app_table_field TEXT NOT NULL,
  is_fts_enabled BOOLEAN NOT NULL DEFAULT FALSE
);