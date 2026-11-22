-- Expand only. One nullable opaque project identity per Traffic queue entry.
-- The value is 64 lowercase hexadecimal digits of a client-side digest; it is
-- never a path. Existing rows and entries of protocol-1 clients stay NULL and
-- keep sequence order. No backfill: a past entry's project is unknown.
ALTER TABLE traffic_entries ADD COLUMN project_key TEXT
  CHECK(project_key IS NULL OR (length(project_key)=64 AND project_key NOT GLOB '*[^0-9a-f]*'));
-- Serves the per-project last-grant lookup of fair queue-head selection.
CREATE INDEX idx_traffic_project_grant ON traffic_entries(project_key, granted_at_ms)
  WHERE project_key IS NOT NULL;
