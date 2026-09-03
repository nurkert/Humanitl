CREATE TABLE sessions (
  id              TEXT PRIMARY KEY,
  started_at      INTEGER NOT NULL,          -- Unix-Millisekunden UTC
  ended_at        INTEGER,
  sandbox_profile TEXT NOT NULL,
  llm_endpoint    TEXT,
  work_dir        TEXT NOT NULL,
  agent           TEXT NOT NULL
);

CREATE TABLE flows (
  id             TEXT PRIMARY KEY,
  session_id     TEXT NOT NULL REFERENCES sessions(id),
  seq            INTEGER NOT NULL,           -- laufende Nummer pro Session, 1-basiert
  ts             INTEGER NOT NULL,           -- Received, Unix-ms
  method         TEXT NOT NULL,
  scheme         TEXT NOT NULL,              -- http | https
  host           TEXT NOT NULL,              -- A-Label, lowercase
  host_display   TEXT NOT NULL,              -- U-Label
  port           INTEGER NOT NULL,
  path           TEXT NOT NULL,              -- path_and_query
  upgrade        TEXT,                       -- websocket | NULL
  state          TEXT NOT NULL,              -- received|analyzed|held|decided|forwarded|responded|recorded
  decision       TEXT,                       -- allow|allow_edited|block|timed_out
  block_reason   TEXT,                       -- user|rule|timeout|body_cap|authority_mismatch|no_route
  rule_id        TEXT,
  passthrough    INTEGER NOT NULL DEFAULT 0, -- 1 = LLM-Passthrough
  status         INTEGER,                    -- HTTP-Status der Response
  duration_ms    INTEGER,                    -- Received bis Responded
  held_ms        INTEGER,                    -- Held bis Decided
  edited         INTEGER NOT NULL DEFAULT 0,
  findings_count INTEGER NOT NULL DEFAULT 0,
  request_size   INTEGER NOT NULL DEFAULT 0,
  response_size  INTEGER,
  apex           TEXT,                       -- PSL-Apex
  catalog_id     TEXT,
  UNIQUE (session_id, seq)
);
CREATE INDEX flows_ts        ON flows(ts DESC, id);
CREATE INDEX flows_session   ON flows(session_id, ts DESC);
CREATE INDEX flows_host      ON flows(host);
CREATE INDEX flows_state     ON flows(state);
CREATE INDEX flows_decision  ON flows(decision);

CREATE TABLE messages (
  flow_id          TEXT NOT NULL REFERENCES flows(id),
  dir              TEXT NOT NULL,            -- request | request_edited | response
  headers_json     TEXT NOT NULL,            -- [["name","value"],...] in Originalreihenfolge
  content_type     TEXT,
  content_encoding TEXT,
  body_inline      BLOB,                     -- wenn size <= recorder.inline_max_bytes
  blob_sha256      BLOB,                     -- sonst Referenz in den Blob-Store (32 Bytes)
  size             INTEGER NOT NULL,         -- Bytes wie gesendet (roh, nicht dekomprimiert)
  truncated        INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (flow_id, dir)
);

CREATE TABLE findings (
  flow_id        TEXT NOT NULL REFERENCES flows(id),
  idx            INTEGER NOT NULL,
  kind           TEXT NOT NULL,              -- z. B. api_key:github, email, iban
  location       TEXT NOT NULL,              -- header:<name> | query | body
  span_start     INTEGER NOT NULL,
  span_end       INTEGER NOT NULL,
  tier           TEXT NOT NULL,              -- checksum | regex | user_term
  value_hash     BLOB NOT NULL,
  display_prefix TEXT NOT NULL,
  resolved       TEXT,                       -- NULL | replaced | ignored
  PRIMARY KEY (flow_id, idx)
);
CREATE INDEX findings_hash ON findings(value_hash);
