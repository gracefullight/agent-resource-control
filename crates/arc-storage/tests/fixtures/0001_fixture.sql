CREATE TABLE vendors (id TEXT PRIMARY KEY, display_name TEXT NOT NULL);
CREATE TABLE schema_migrations (
  id TEXT PRIMARY KEY,
  applied_at INTEGER NOT NULL,
  checksum TEXT NOT NULL
);
