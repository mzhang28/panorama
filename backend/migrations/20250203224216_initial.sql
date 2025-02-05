CREATE TABLE "node" (
  "id" PRIMARY KEY DEFAULT (UUIDV7_NOW()),
  "created_at" TEXT NOT NULL DEFAULT (NOW_ISO8601()),
  "last_updated_at" TEXT NOT NULL DEFAULT (NOW_ISO8601()),

  -- General stuff
  "title" TEXT,
  "label" TEXT,

  -- Calendar
  "cal_date" TEXT,
  "cal_date_with_time" BOOLEAN,
  "cal_date_end" TEXT,
  "cal_date_end_with_time" BOOLEAN,

  -- wakatime heartbeat
  "wakatime_language" TEXT,
  "wakatime_project" TEXT,

  -- extra json shit
  "json" TEXT
);

CREATE INDEX idx_node_cal_date ON node(cal_date);
CREATE INDEX idx_node_cal_date_with_time ON node(cal_date_with_time);
CREATE INDEX idx_node_cal_date_end ON node(cal_date_end);
CREATE INDEX idx_node_cal_date_end_with_time ON node(cal_date_end_with_time);

CREATE INDEX idx_node_wakatime_language ON node(wakatime_language);
CREATE INDEX idx_node_wakatime_project ON node(wakatime_project);
