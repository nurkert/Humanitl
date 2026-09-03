CREATE TABLE rules_snapshot (
  id         TEXT PRIMARY KEY,
  yaml       TEXT NOT NULL,
  first_seen INTEGER NOT NULL,
  deleted_at INTEGER
);
