-- One metadata binding per job. NULL report_id/report_json is a placeholder,
-- never an accepted measurement. The validated immutable JSON retains explicit
-- null CPU/run values, identity, attribution, scope and completeness together.
CREATE TABLE job_meter_summaries (
  job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs(id),
  measurement_aware INTEGER NOT NULL CHECK(measurement_aware IN (0,1)),
  relation TEXT NOT NULL DEFAULT 'unverified'
    CHECK(relation IN ('unverified','canonical_root','included_in_root')),
  root_job_id TEXT REFERENCES jobs(id),
  child_bound INTEGER NOT NULL DEFAULT 0 CHECK(child_bound IN (0,1)),
  terminal_at_ms INTEGER CHECK(terminal_at_ms BETWEEN 0 AND 9007199254740991),
  expired_at_ms INTEGER CHECK(expired_at_ms BETWEEN 0 AND 9007199254740991),
  report_id TEXT UNIQUE CHECK(report_id IS NULL OR
    (length(report_id)=32 AND report_id NOT GLOB '*[^0-9a-f]*')),
  report_json TEXT CHECK(report_json IS NULL OR
    (length(CAST(report_json AS BLOB))<=16384 AND json_valid(report_json))),
  CHECK((relation='unverified' AND root_job_id IS NULL) OR
    (relation='canonical_root' AND root_job_id IS NOT NULL AND root_job_id=job_id) OR
    (relation='included_in_root' AND root_job_id IS NOT NULL AND root_job_id!=job_id)),
  CHECK((report_id IS NULL AND report_json IS NULL) OR
    (report_id IS NOT NULL AND report_json IS NOT NULL AND terminal_at_ms IS NOT NULL)),
  CHECK(expired_at_ms IS NULL OR
    (terminal_at_ms IS NOT NULL AND report_id IS NULL AND report_json IS NULL))
);
CREATE INDEX idx_job_meter_expiry ON job_meter_summaries(terminal_at_ms,job_id)
  WHERE expired_at_ms IS NULL AND terminal_at_ms IS NOT NULL;
CREATE INDEX idx_job_meter_root ON job_meter_summaries(root_job_id);
CREATE INDEX idx_lifecycle_observer_identity
  ON job_lifecycle(observer_pid,observer_start_token,observer_boot_id);
CREATE INDEX idx_lifecycle_child_identity
  ON job_lifecycle(child_pid,child_start_token,child_boot_id);

-- Preserve historical facts and their first known terminal timestamp. No old
-- lifecycle row establishes a newly measured lifecycle-only queue zero.
INSERT INTO job_meter_summaries(job_id,measurement_aware,terminal_at_ms)
SELECT j.id,0,CASE
  WHEN j.state IN ('SUCCEEDED','FAILED','SIGNALED','SPAWN_FAILED','COMPLETED')
    THEN CASE WHEN j.finished_at BETWEEN 0 AND 9007199254740991 THEN j.finished_at END
  WHEN j.state='OUTCOME_UNKNOWN' AND l.observation_status IN ('MISSING','MISMATCH')
    THEN l.updated_at_ms END
FROM jobs j LEFT JOIN job_lifecycle l ON l.job_id=j.id;
