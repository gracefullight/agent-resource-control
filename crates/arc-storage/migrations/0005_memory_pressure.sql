-- Additive policy metadata. Existing tables/rows and migration bytes remain.
-- Policy and retention logic is application-owned on the single SQLite writer.
ALTER TABLE policy_states ADD COLUMN policy_version TEXT;
ALTER TABLE policy_states ADD COLUMN state_json TEXT
  CHECK(state_json IS NULL OR length(CAST(state_json AS BLOB)) <= 16384);

ALTER TABLE policy_decisions ADD COLUMN observation_availability TEXT NOT NULL DEFAULT 'missing'
  CHECK(observation_availability IN ('available','missing','stale','failed','unsupported','denied','invalid'));
ALTER TABLE policy_decisions ADD COLUMN observation_age_ms INTEGER
  CHECK(observation_age_ms IS NULL OR observation_age_ms BETWEEN 0 AND 9007199254740991);
ALTER TABLE policy_decisions ADD COLUMN observation_json TEXT
  CHECK(observation_json IS NULL OR length(CAST(observation_json AS BLOB)) <= 8192);
ALTER TABLE policy_decisions ADD COLUMN transition_key TEXT
  CHECK(transition_key IS NULL OR length(CAST(transition_key AS BLOB)) <= 512);
ALTER TABLE policy_decisions ADD COLUMN permit_generation INTEGER
  CHECK(permit_generation IS NULL OR permit_generation BETWEEN 1 AND 9007199254740991);
ALTER TABLE policy_decisions ADD COLUMN recorded_at_ms INTEGER
  CHECK(recorded_at_ms IS NULL OR recorded_at_ms BETWEEN 0 AND 9007199254740991);
ALTER TABLE policy_decisions ADD COLUMN expires_at_ms INTEGER
  CHECK(expires_at_ms IS NULL OR expires_at_ms BETWEEN 0 AND 9007199254740991);

CREATE INDEX idx_policy_decisions_job_time
  ON policy_decisions(job_id, recorded_at_ms DESC, id DESC);
CREATE INDEX idx_policy_decisions_expiry
  ON policy_decisions(expires_at_ms, id) WHERE expires_at_ms IS NOT NULL;
