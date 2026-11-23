-- Expand only. A session digest and allocated-footprint readings on job rows,
-- durable job membership of rolled-up buckets, and native process-attribution
-- aggregates. Past sessions, footprints and attribution are not backfilled;
-- historical rollup counts are retained with unknown completeness.
--
-- The client-side domain-separated SHA-256 session digest; a raw vendor
-- session id never reaches this column.
ALTER TABLE jobs ADD COLUMN session_key TEXT
  CHECK(session_key IS NULL OR (length(session_key)=64 AND session_key NOT GLOB '*[^0-9a-f]*'));
-- Allocated bytes (st_blocks * 512) under the job's working directory at job
-- start and end. Bytes are NULL exactly when the reading is unavailable or
-- was never taken; a partial walk keeps its lower-bound bytes.
ALTER TABLE jobs ADD COLUMN footprint_start_bytes INTEGER
  CHECK(footprint_start_bytes IS NULL OR
    (typeof(footprint_start_bytes)='integer' AND footprint_start_bytes >= 0));
ALTER TABLE jobs ADD COLUMN footprint_start_completeness TEXT
  CHECK(footprint_start_completeness IS NULL OR
    (footprint_start_completeness IN ('complete','partial') AND footprint_start_bytes IS NOT NULL) OR
    (footprint_start_completeness='unavailable' AND footprint_start_bytes IS NULL));
ALTER TABLE jobs ADD COLUMN footprint_end_bytes INTEGER
  CHECK(footprint_end_bytes IS NULL OR
    (typeof(footprint_end_bytes)='integer' AND footprint_end_bytes >= 0));
ALTER TABLE jobs ADD COLUMN footprint_end_completeness TEXT
  CHECK(footprint_end_completeness IS NULL OR
    (footprint_end_completeness IN ('complete','partial') AND footprint_end_bytes IS NOT NULL) OR
    (footprint_end_completeness='unavailable' AND footprint_end_bytes IS NULL));
-- One row per job with a raw sample in a rolled-up bucket, so job_count is
-- the count of distinct jobs however many passes add to that bucket. Deleted
-- with its bucket.
-- Existing buckets predate membership tracking; their count may already
-- include duplicates. Preserve it but never present it as an exact count.
ALTER TABLE resource_rollups ADD COLUMN job_count_completeness TEXT NOT NULL
  DEFAULT 'unknown' CHECK(job_count_completeness IN ('complete','unknown'));
CREATE TABLE resource_rollup_jobs (
  granularity TEXT NOT NULL CHECK(granularity IN ('minute','hour','day')),
  bucket_start_ms INTEGER NOT NULL CHECK(bucket_start_ms BETWEEN 0 AND 9007199254740991),
  job_id TEXT NOT NULL CHECK(length(job_id) BETWEEN 1 AND 64),
  PRIMARY KEY(granularity, bucket_start_ms, job_id)
);
-- Native process-attribution CPU per whole UTC minute of the scan that
-- observed it. Aggregates only: no PID, argv, path or cwd of any process.
-- Unattributed CPU has the empty vendor and is never stored under one;
-- a Proven managed job without a vendor is stored under 'none'.
CREATE TABLE process_attribution (
  bucket_start_ms INTEGER NOT NULL
    CHECK(bucket_start_ms BETWEEN 0 AND 9007199254740991 AND bucket_start_ms % 60000 = 0),
  confidence TEXT NOT NULL CHECK(confidence IN ('proven','estimated','unattributed')),
  vendor TEXT NOT NULL CHECK(
    (confidence='unattributed' AND vendor='') OR
    (confidence='proven' AND vendor IN ('claude-code','cursor','none')) OR
    (confidence='estimated' AND vendor IN ('claude-code','cursor'))),
  cpu_core_seconds REAL NOT NULL CHECK(cpu_core_seconds >= 0),
  PRIMARY KEY(bucket_start_ms, confidence, vendor)
);
-- The scans that completed in each minute; a minute without a row was not
-- observed, so its attribution is unavailable rather than zero.
CREATE TABLE process_attribution_scans (
  bucket_start_ms INTEGER PRIMARY KEY NOT NULL
    CHECK(bucket_start_ms BETWEEN 0 AND 9007199254740991 AND bucket_start_ms % 60000 = 0),
  scans INTEGER NOT NULL CHECK(scans >= 1),
  first_scan_ms INTEGER NOT NULL
    CHECK(first_scan_ms BETWEEN bucket_start_ms AND bucket_start_ms + 59999),
  last_scan_ms INTEGER NOT NULL
    CHECK(last_scan_ms BETWEEN first_scan_ms AND bucket_start_ms + 59999),
  -- Processes whose identity changed between scans or could not be read;
  -- they contributed nothing.
  skipped_processes INTEGER NOT NULL DEFAULT 0 CHECK(skipped_processes >= 0)
);
