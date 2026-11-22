-- Expand only. One nullable opaque vendor hook event identity per job record.
-- The value is 64 lowercase hexadecimal digits of a client-side digest of the
-- event tuple; the raw session and tool call ids are never stored. Existing
-- rows and jobs of clients that send no key stay NULL. No backfill: the event
-- of a past job is unknown. The key lives and is deleted with its job row.
ALTER TABLE job_lifecycle ADD COLUMN event_key TEXT
  CHECK(event_key IS NULL OR (length(event_key)=64 AND event_key NOT GLOB '*[^0-9a-f]*'));
-- At most one job per event: a second registration of a recorded key cannot
-- insert a job row inside the admission transaction.
CREATE UNIQUE INDEX idx_job_lifecycle_event_key ON job_lifecycle(event_key)
  WHERE event_key IS NOT NULL;
