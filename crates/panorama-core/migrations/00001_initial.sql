CREATE TABLE "node" (
  id TEXT PRIMARY KEY,
  type TEXT,
  updated_at DATETIME DEFAULT NOW(),
  extra_data JSON
);