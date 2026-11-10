-- Additive Traffic metadata. Existing lifecycle and permit rows are untouched.
-- Sequence values are never reused, including after a future archival deletion.
ALTER TABLE permits ADD COLUMN cpu_millis_est INTEGER
  CHECK(cpu_millis_est IS NULL OR cpu_millis_est BETWEEN 1 AND 9223372036854775807);
ALTER TABLE permits ADD COLUMN generation INTEGER
  CHECK(generation IS NULL OR generation BETWEEN 1 AND 9223372036854775807);

CREATE TABLE traffic_entries (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  job_id TEXT NOT NULL UNIQUE REFERENCES jobs(id),
  state TEXT NOT NULL CHECK(state IN ('QUEUED','RESERVED','RUNNING','RETAINED','COMPLETED','CANCELLED','ABANDONED')),
  queued_at_ms INTEGER NOT NULL CHECK(queued_at_ms BETWEEN 0 AND 9007199254740991),
  granted_at_ms INTEGER CHECK(granted_at_ms BETWEEN queued_at_ms AND 9007199254740991),
  generation INTEGER CHECK(generation BETWEEN 1 AND 9223372036854775807),
  root_job_id TEXT REFERENCES jobs(id),
  context TEXT UNIQUE CHECK(context IS NULL OR (length(context)=64 AND context NOT GLOB '*[^0-9a-f]*')),
  inherited_context TEXT CHECK(inherited_context IS NULL OR (length(inherited_context)=64 AND inherited_context NOT GLOB '*[^0-9a-f]*')),
  CHECK((granted_at_ms IS NULL AND generation IS NULL AND root_job_id IS NULL AND context IS NULL)
     OR (granted_at_ms IS NOT NULL AND generation IS NOT NULL AND
         ((root_job_id IS NULL AND context IS NOT NULL) OR (root_job_id IS NOT NULL AND context IS NULL))))
);
CREATE INDEX idx_traffic_fifo ON traffic_entries(state, sequence);
CREATE INDEX idx_traffic_root_state ON traffic_entries(root_job_id, state);

-- Cancellation or fallback may arrive before enqueue commits. Preserve the
-- owner-bound terminal state without fabricating classification or lifecycle facts.
CREATE TABLE traffic_tombstones (
  job_id TEXT PRIMARY KEY NOT NULL,
  observer_pid INTEGER NOT NULL CHECK(observer_pid BETWEEN 1 AND 2147483647),
  observer_start_token TEXT NOT NULL,
  observer_boot_id TEXT NOT NULL,
  terminal_state TEXT NOT NULL CHECK(terminal_state IN ('CANCELLED','ABANDONED')),
  recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms BETWEEN 0 AND 9007199254740991)
);
