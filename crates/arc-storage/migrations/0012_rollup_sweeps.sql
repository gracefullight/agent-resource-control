-- Preserve partial RSS sweeps left by older daemons which created these
-- objects on first use. Migration and history are committed atomically.
CREATE TABLE IF NOT EXISTS resource_rollup_pending_sweeps (
  ts TEXT PRIMARY KEY NOT NULL,
  rss INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_resource_samples_rollup_next
  ON resource_samples(job_id, process_identity, timestamp_ns);
