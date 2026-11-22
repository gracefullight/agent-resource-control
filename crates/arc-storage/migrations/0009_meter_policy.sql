-- Expand only. Usage attribution on job rows, a per-process CPU rate on raw
-- samples, a bounded process-tree report beside the job Meter summary, and
-- rolled-up resource buckets. Existing rows stay NULL; nothing is backfilled,
-- because the vendor, hook overhead, project and tree of a past job are unknown.
-- resource_samples.timestamp_ns and cpu_time_ns stay decimal TEXT and are read
-- with CAST(... AS INTEGER).
ALTER TABLE jobs ADD COLUMN vendor TEXT
  CHECK(vendor IS NULL OR vendor IN ('claude-code','cursor'));
ALTER TABLE jobs ADD COLUMN hook_overhead_ms INTEGER
  CHECK(hook_overhead_ms IS NULL OR
    (typeof(hook_overhead_ms)='integer' AND hook_overhead_ms >= 0));
-- The client-side project digest; jobs.project_id keeps its projects FK.
ALTER TABLE jobs ADD COLUMN project_key TEXT
  CHECK(project_key IS NULL OR (length(project_key)=64 AND project_key NOT GLOB '*[^0-9a-f]*'));
-- 100 = one core, from consecutive cpu_time_ns deltas of one revalidated identity.
ALTER TABLE resource_samples ADD COLUMN cpu_core_percent REAL
  CHECK(cpu_core_percent IS NULL OR cpu_core_percent >= 0);
CREATE INDEX idx_resource_samples_time ON resource_samples(timestamp_ns);
CREATE INDEX idx_resource_samples_job_time ON resource_samples(job_id, timestamp_ns);
-- Cleared together with report_json when the summary expires (writer-enforced).
ALTER TABLE job_meter_summaries ADD COLUMN process_tree_json TEXT
  CHECK(process_tree_json IS NULL OR
    (length(CAST(process_tree_json AS BLOB))<=16384 AND json_valid(process_tree_json)));
-- reclaimed_bytes stays NULL until a Sweeper reclaims storage.
CREATE TABLE resource_rollups (
  granularity TEXT NOT NULL CHECK(granularity IN ('minute','hour','day')),
  bucket_start_ms INTEGER NOT NULL CHECK(bucket_start_ms BETWEEN 0 AND 9007199254740991),
  cpu_core_seconds REAL CHECK(cpu_core_seconds IS NULL OR cpu_core_seconds >= 0),
  peak_rss_bytes INTEGER CHECK(peak_rss_bytes IS NULL OR peak_rss_bytes >= 0),
  disk_write_bytes INTEGER CHECK(disk_write_bytes IS NULL OR disk_write_bytes >= 0),
  job_count INTEGER NOT NULL CHECK(job_count >= 0),
  reclaimed_bytes INTEGER CHECK(reclaimed_bytes IS NULL OR reclaimed_bytes >= 0),
  PRIMARY KEY(granularity, bucket_start_ms)
);
