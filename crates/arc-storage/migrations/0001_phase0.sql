CREATE TABLE vendors (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  adapter_version TEXT,
  discovered_at INTEGER NOT NULL
);

CREATE TABLE agent_instances (
  id TEXT PRIMARY KEY,
  vendor_id TEXT NOT NULL REFERENCES vendors(id),
  executable_path_hash TEXT,
  os_process_id INTEGER,
  process_start_ns TEXT,
  state TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_seen_at INTEGER
);

CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  root_path TEXT NOT NULL,
  canonical_file_identity TEXT,
  repo_identity TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE agent_sessions (
  id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL REFERENCES agent_instances(id),
  project_id TEXT REFERENCES projects(id),
  vendor_session_id TEXT,
  started_at INTEGER NOT NULL,
  last_seen_at INTEGER,
  ended_at INTEGER,
  state TEXT NOT NULL,
  owner_verified INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  session_id TEXT REFERENCES agent_sessions(id),
  project_id TEXT REFERENCES projects(id),
  worktree_id TEXT,
  workload_signature TEXT,
  job_class TEXT NOT NULL,
  locality TEXT NOT NULL,
  enforcement TEXT NOT NULL,
  state TEXT NOT NULL,
  queue_at INTEGER,
  reserved_at INTEGER,
  started_at INTEGER,
  finished_at INTEGER,
  exit_code INTEGER,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_jobs_state_class ON jobs(state, job_class);
CREATE INDEX idx_jobs_project_time ON jobs(project_id, created_at DESC);

CREATE TABLE permits (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL UNIQUE REFERENCES jobs(id),
  state TEXT NOT NULL,
  cpu_cores_est REAL,
  memory_bytes_est INTEGER,
  reserved_at INTEGER NOT NULL,
  released_at INTEGER
);

CREATE TABLE resource_objects (
  id TEXT PRIMARY KEY,
  resource_type TEXT NOT NULL,
  resource_identity TEXT NOT NULL,
  canonical_path TEXT,
  canonical_file_identity TEXT,
  owner_vendor_id TEXT,
  owner_session_id TEXT,
  ownership_level TEXT NOT NULL,
  created_at INTEGER,
  last_active_at INTEGER,
  protected INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_resource_identity
  ON resource_objects(resource_type, resource_identity);

CREATE TABLE resource_leases (
  id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resource_objects(id),
  session_id TEXT REFERENCES agent_sessions(id),
  process_identity TEXT,
  acquired_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER,
  expires_at INTEGER,
  state TEXT NOT NULL
);
CREATE INDEX idx_lease_resource_state ON resource_leases(resource_id, state);

CREATE TABLE gc_operations (
  id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resource_objects(id),
  gc_reason TEXT NOT NULL,
  ownership_evidence TEXT NOT NULL,
  state TEXT NOT NULL,
  started_at INTEGER,
  ended_at INTEGER,
  reclaimed_allocated_bytes INTEGER,
  error_code TEXT
);
CREATE INDEX idx_gc_state ON gc_operations(state);

CREATE TABLE resource_samples (
  id INTEGER PRIMARY KEY,
  timestamp_ns TEXT NOT NULL,
  job_id TEXT REFERENCES jobs(id),
  agent_session_id TEXT REFERENCES agent_sessions(id),
  process_identity TEXT,
  cpu_time_ns TEXT,
  rss_bytes INTEGER,
  disk_read_bytes INTEGER,
  disk_write_bytes INTEGER,
  attribution_confidence TEXT NOT NULL
);

CREATE TABLE policy_states (
  key TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL,
  policy_revision INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE policy_decisions (
  id TEXT PRIMARY KEY,
  job_id TEXT REFERENCES jobs(id),
  policy_version TEXT NOT NULL,
  decision TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  prediction_confidence REAL,
  created_at INTEGER NOT NULL
);

CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  severity TEXT NOT NULL,
  event_type TEXT NOT NULL,
  entity_id TEXT,
  payload_redacted_json TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE schema_migrations (
  id TEXT PRIMARY KEY,
  applied_at INTEGER NOT NULL,
  checksum TEXT NOT NULL
);
