-- Allowlisted lifecycle facts only. No command, path, environment or payload columns.
CREATE TABLE job_lifecycle (
  job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs(id),
  registered_at_ms INTEGER NOT NULL CHECK(registered_at_ms BETWEEN 0 AND 9007199254740991),
  observer_pid INTEGER NOT NULL CHECK(observer_pid BETWEEN 1 AND 2147483647),
  observer_start_token TEXT NOT NULL CHECK(length(observer_start_token) BETWEEN 1 AND 20 AND observer_start_token NOT GLOB '*[^0-9]*' AND substr(observer_start_token,1,1) != '0' AND (length(observer_start_token) < 20 OR observer_start_token <= '18446744073709551615')),
  observer_boot_id TEXT NOT NULL CHECK(length(observer_boot_id)=36),
  observer_verified_at_ms INTEGER NOT NULL CHECK(observer_verified_at_ms BETWEEN 0 AND 9007199254740991),
  child_pid INTEGER CHECK(child_pid BETWEEN 1 AND 2147483647),
  child_start_token TEXT CHECK(length(child_start_token) BETWEEN 1 AND 20 AND child_start_token NOT GLOB '*[^0-9]*' AND substr(child_start_token,1,1) != '0' AND (length(child_start_token) < 20 OR child_start_token <= '18446744073709551615')),
  child_boot_id TEXT CHECK(length(child_boot_id)=36),
  child_verified_at_ms INTEGER CHECK(child_verified_at_ms BETWEEN 0 AND 9007199254740991),
  classification_reason TEXT NOT NULL,
  classification_uncertainty TEXT NOT NULL CHECK(classification_uncertainty IN ('NONE','OBSERVE')),
  launch_claimed_at_ms INTEGER CHECK(launch_claimed_at_ms BETWEEN 0 AND 9007199254740991),
  observation_target TEXT NOT NULL CHECK(observation_target IN ('OBSERVER','CHILD','NONE')),
  observation_status TEXT NOT NULL CHECK(observation_status IN ('VERIFIED_MATCH','MISSING','MISMATCH','INDETERMINATE','NOT_ATTACHED')),
  observation_at_ms INTEGER NOT NULL CHECK(observation_at_ms BETWEEN 0 AND 9007199254740991),
  observation_reason TEXT NOT NULL,
  outcome_kind TEXT CHECK(outcome_kind IN ('EXITED','SIGNALED','SPAWN_FAILED')),
  signal INTEGER CHECK(signal BETWEEN 1 AND 127),
  core_dumped INTEGER CHECK(core_dumped IN (0,1)),
  spawn_reason TEXT CHECK(spawn_reason IN ('NOT_FOUND','PERMISSION_DENIED','INVALID_INPUT','RESOURCE_UNAVAILABLE','OTHER')),
  os_code INTEGER CHECK(os_code BETWEEN -2147483648 AND 2147483647),
  revision INTEGER NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
  updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms BETWEEN 0 AND 9007199254740991),
  reason TEXT NOT NULL,
  CHECK((child_pid IS NULL AND child_start_token IS NULL AND child_boot_id IS NULL AND child_verified_at_ms IS NULL) OR (child_pid IS NOT NULL AND child_start_token IS NOT NULL AND child_boot_id IS NOT NULL)),
  CHECK((outcome_kind IS NULL AND signal IS NULL AND core_dumped IS NULL AND spawn_reason IS NULL AND os_code IS NULL) OR
        (outcome_kind IS 'EXITED' AND signal IS NULL AND core_dumped IS NULL AND spawn_reason IS NULL AND os_code IS NULL) OR
        (outcome_kind IS 'SIGNALED' AND signal IS NOT NULL AND core_dumped IS NOT NULL AND spawn_reason IS NULL AND os_code IS NULL) OR
        (outcome_kind IS 'SPAWN_FAILED' AND child_pid IS NULL AND signal IS NULL AND core_dumped IS NULL AND spawn_reason IS NOT NULL))
);
-- The immutable registration time is copied in the same transaction as jobs.
-- This index contains lifecycle candidates only; legacy jobs never enter a scan.
CREATE INDEX idx_jobs_lifecycle_recent ON job_lifecycle(registered_at_ms DESC, job_id DESC);
