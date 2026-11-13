-- Expand only. The policy_states row 'governor.enabled' is the sole switch.
-- Receipts are pruned/capped by the existing application writer, not triggers.
-- A mutation ID embeds its immutable expected revision, preventing an evicted
-- receipt from being reapplied at a newer revision. Never prune the current base.
CREATE TABLE governor_mutation_receipts (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  mutation_id TEXT NOT NULL UNIQUE,
  expected_revision INTEGER NOT NULL CHECK(typeof(expected_revision)='integer' AND expected_revision BETWEEN 0 AND 9007199254740991),
  desired_enabled INTEGER NOT NULL CHECK(desired_enabled IN (0,1)),
  result_enabled INTEGER NOT NULL CHECK(result_enabled IN (0,1)),
  result_revision INTEGER NOT NULL CHECK(typeof(result_revision)='integer' AND result_revision BETWEEN 0 AND 9007199254740991),
  result_updated_at_ms INTEGER CHECK(result_updated_at_ms IS NULL OR (typeof(result_updated_at_ms)='integer' AND result_updated_at_ms BETWEEN 0 AND 9007199254740991)),
  changed INTEGER NOT NULL CHECK(changed IN (0,1)),
  committed_at_ms INTEGER NOT NULL CHECK(typeof(committed_at_ms)='integer' AND committed_at_ms BETWEEN 0 AND 9007199254740991),
  CHECK(result_enabled=desired_enabled AND result_revision=expected_revision+changed),
  CHECK(result_updated_at_ms IS NOT NULL OR (result_enabled=1 AND result_revision=0)),
  CHECK(length(mutation_id)=length(CAST(expected_revision AS TEXT))+33
    AND substr(mutation_id,1,length(CAST(expected_revision AS TEXT))+1)=CAST(expected_revision AS TEXT)||'-'
    AND substr(mutation_id,-32) NOT GLOB '*[^0-9a-f]*')
);
CREATE INDEX idx_governor_receipts_revision ON governor_mutation_receipts(expected_revision,sequence);

-- One negotiated execution disposition per EXISTING lifecycle job. No second
-- queue, child lifecycle or permit store. Observer/child/outcome stay there.
-- A row in SCHEDULED mode identifies an opted-in queued/reserved current client;
-- legacy clients never get these rows. Only the application owns transitions.
CREATE TABLE governor_jobs (
  job_id TEXT PRIMARY KEY NOT NULL REFERENCES job_lifecycle(job_id),
  mode TEXT NOT NULL CHECK(mode IN ('SCHEDULED','UNRESERVED')),
  switch_revision INTEGER NOT NULL CHECK(typeof(switch_revision)='integer' AND switch_revision BETWEEN 0 AND 9007199254740991),
  unreserved_at_ms INTEGER CHECK(unreserved_at_ms IS NULL OR (typeof(unreserved_at_ms)='integer' AND unreserved_at_ms BETWEEN 0 AND 9007199254740991)),
  previous_generation INTEGER CHECK(previous_generation IS NULL OR (typeof(previous_generation)='integer' AND previous_generation BETWEEN 1 AND 9007199254740991)),
  release_id TEXT UNIQUE CHECK(release_id IS NULL OR (length(release_id)=32 AND release_id NOT GLOB '*[^0-9a-f]*')),
  release_claimed_at_ms INTEGER CHECK(release_claimed_at_ms IS NULL OR (typeof(release_claimed_at_ms)='integer' AND release_claimed_at_ms BETWEEN 0 AND 9007199254740991)),
  CHECK((mode='SCHEDULED' AND unreserved_at_ms IS NULL AND previous_generation IS NULL AND release_id IS NULL AND release_claimed_at_ms IS NULL)
     OR (mode='UNRESERVED' AND unreserved_at_ms IS NOT NULL)),
  CHECK((release_id IS NULL AND release_claimed_at_ms IS NULL)
     OR (release_id IS NOT NULL AND release_claimed_at_ms IS NOT NULL AND release_claimed_at_ms>=unreserved_at_ms))
);
CREATE INDEX idx_governor_jobs_recovery ON governor_jobs(mode,job_id);
