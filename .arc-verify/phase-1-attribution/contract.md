# Phase 1 attribution: contract

Base `6f63a3a` (tree `a679952f`); paths under `crates/`. Wave 1 is a seam.

## Decisions

- **D1 v1 bytes.** v1 bytes are unchanged; new data uses advertised operations or optional fields (D3).
- **D2 migration `0010_attribution`**, expand-only, schema 10: `jobs.session_key` (64 hex); `jobs.footprint_{start,end}_bytes` (>= 0) and `_completeness` (complete|partial|unavailable; bytes NULL iff unavailable); `resource_rollup_jobs(granularity,bucket_start_ms,job_id)` PK all three (new `job_count` = its rows; pre-migration counts have unknown completeness); `process_attribution(bucket_start_ms%60000=0, confidence proven|estimated|unattributed, vendor, cpu_core_seconds)`: unattributed only under vendor `''`, proven under a vendor or `none`, estimated under a vendor; `process_attribution_scans(bucket_start_ms, scans>=1, first/last_scan_ms, skipped_processes)`. Rowid tables.
- **D3 capabilities** (files beside the socket): `meter.usage-summary.v2` (also gates `UsageFields.session_key`), `meter.attribution.v1`, `meter.job-footprint.v1`, `meter.job-tree-quality.v1`. `capability_advertisement.rs` sets `usage_summary::ATTRIBUTION_ADVERTISED` while the attribution file is held.
- **D4 summary v2** (arc-ipc `src/meter/usage.rs`): `meter.usage_summary.v2` with `UsageQueryV2{by vendor|project|period|session, from_ms, to_ms, bucket}` (v1 bytes for the first three); reply `UsageSummaryV2{schema_version 1, query, generated_at_ms, groups, attribution?}`, additive: v1 keys kept. `arcctl meter summary --by` prints v2 from an advertising daemon, else v1; `--by session` then fails `UNSUPPORTED_CAPABILITY`. With no explicit bounds or bucket, the CLI uses the preceding 24 hours and hourly buckets; explicit requests require all three.
- **D5 quality** (`src/meter/quality.rs`): `Flagged<T>{value, availability complete|partial|unavailable, basis, reason?}`; `None` iff unavailable, with a reason; never a silent zero. B flags v2 CPU, RSS, I/O, disk-written, footprint and per-vendor totals; `meter.job_tree_quality` flags each returned tree value. Sampled CPU, RSS and I/O are `partial`. F3 uses sampled tree CPU, else waited rusage; F4 retains partial coverage.
- **D6 attribution** (`src/meter/attribution.rs`).
  - **Proven (A)**: in a RUNNING job's tree. Managed spawn recorded the root identity with verified ownership, the job holds its lease, and in the same scan the member's identity is revalidated and its ppid chain reaches the root. Grouped by `jobs.vendor` or `none`.
  - **Estimated (C)**: not Proven; revalidated ancestry reaches an exact, case-sensitive OS process name in `AGENT_EXECUTABLES` (`claude`→claude-code, `cursor-agent`/`Cursor`→cursor; not `node`).
  - **Unattributed (D)**: everything else, never under a vendor or `none`.
  - **Scans:** per-identity CPU deltas; first sightings, changed identities and unreadable processes do not add a delta. A window includes measured scan envelopes wholly inside `[from,to)`; coverage uses their union. Ratio `100*U/(P+E+U)` is unavailable without a scan, with zero total, or when coverage is unresolved or failed.
  - Job-tree C describes sampled descendants; Proven describes agent ownership. Lease reads bracket the scan, not publication. Never sum these reports. Aggregates only; no foreign PID, argv, path or cwd.
- **D7 footprint** (`src/meter/footprint.rs`): bounded allocated-byte walks (`st_blocks*512`), one device and no symlinks. Registration and completion walks precede their commit; grant and release walks precede the reply. Recovery binds an unavailable end in the same transaction that abandons the job and releases its permit; earlier readings win and late walks cannot replace them. `JobFootprint{start_bytes,end_bytes,delta_bytes}` uses basis `allocated`; end is Current Footprint and delta requires both walks complete. Neither derives from Disk Written. `meter.job_footprint {job_id}` is separately negotiated.
- **D8 session.** With v1 and v2 advertised, the hook adds `--arc-session <64 hex>` after `--arc-hook-overhead-ms`; `hook-exec` parses it into `SessionKey`, dropped for a daemon without v2. B: SHA-256 of a domain tag, vendor id and raw id, length-framed; the raw id never leaves the hook.
- **D9 workspace.** The resolver reads declared glob members and classifies script leaves. For differing local classes, `detector.workspace-compound.v1` probes the peer on the same connection before carrying members and localities; the daemon keeps up to 64 jobs in memory. Admission remains UNKNOWN/OBSERVE without a reservation: separate class windows cannot be reduced to one. Legacy job bytes keep that projection; local-plus-remote remains MIXED, and unresolved graphs are not compounds.
- **D10 cache.** Supported scripts share bounded project analysis. `--json --diagnostics` adds cache hit|miss|none, members and optional compound; ordinary output keeps four fields. Reads validate each file's mtime, inode and digest and re-evaluate glob membership. The owner-only state location is independent of `ARC_DB_PATH` and never under /tmp or TMPDIR; `detector.workspace-cache.v1` supplies bounded daemon memory when the location is unavailable, else older peers use no cache.
- **D11 cadence.** `meter_cadence::bind` installs the one hook: `job_process_meter::sampler`, then `process_attribution::sampler`, each with its own state and budget.

## Call sites (frozen)
- `usage_summary_dispatch::read`: `summarize_v2` (B) then `arc_storage::attribution::section` (A) in one read transaction; `footprint::read` (B).
- `db_worker/jobs.rs`: lifecycle, Traffic and Governor calls attach through `Operation::Watched` with `footprint::Bound` readings (B).
- `hooks/usage_fields.rs`: `session_key::digest` (B). `classify.rs`: `workspace::refine` (C).

## Seam tests (passing)
`process_attribution::seam_composed_meter_cadence_still_samples_the_managed_job_tree`, `meter_quality::seam_direct_exec_job_is_summarized_under_vendor_none`, `detector_workspace::seam_single_package_bun_script_is_resolved_on_the_exec_path`.

Lane tests: A `process_attribution::native_process_attribution_separates_proven_estimated_and_unattributed`; B `meter_quality::meter_quality_footprint_sessions_and_rollups_use_production_paths`; C `detector_workspace::workspace_fanout_and_cache_follow_production_paths`; G-01 item 4 `job_process_meter::mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job`.

## Wave 2 ownership (one writer per file)
A lane also owns new files in a directory named after one of its files.
- **A:** arc-platform `src/process_scan.rs`; arc-meter `src/attribution.rs`; arc-storage `src/attribution.rs`; arc-daemon `src/process_attribution.rs`, `src/db_worker/attribution.rs`, `tests/process_attribution.rs`.
- **B:** arc-platform `src/footprint.rs`; arc-storage `src/usage_summary.rs` (+`usage_summary/tests.rs`), `src/footprint.rs`, `src/jobs/usage_fields.rs`, `src/rollup.rs`; arc-daemon `src/usage_summary_dispatch.rs`, `src/db_worker/resource_samples.rs` (+tests), `src/db_worker/footprint.rs`, `tests/meter_quality.rs`, and the F3-superseded assertions of `tests/usage_summary.rs`; arc-cli `src/hooks/session_key.rs`, `src/meter/footprint.rs`.
- **C:** arc-detector `src/scripts/resolver.rs`, `src/scripts/workspace.rs`, `src/scripts/cache.rs`; arc-cli `src/classify/workspace.rs`, `src/exec/metadata.rs` (+tests); arc-daemon `tests/detector_workspace.rs`.
- **Frozen:** migrations, schema pins, manifests, `Cargo.lock`, `lib.rs`/`main.rs`, arc-ipc, `tests/support/**`, arc-daemon `lifecycle.rs`, `meter_cadence.rs`, `capability_advertisement.rs`, `job_process_meter.rs`, `db_worker.rs`, `db_worker/jobs.rs`, `policy_dispatch.rs`; arc-cli `meter.rs`, `meter/usage.rs`, `hooks.rs`, `hooks/usage_fields.rs`, `exec/usage_fields.rs`, `exec/entry.rs`, `classify.rs`; arc-detector `scripts.rs`. Other changes go to the Chief. Wave 4 may edit all but `qa.md`.

## Wave 1 notes
- Schema pins 9→10 (newer fixtures 10→11); `schema_step_back.rs` undoes 0010.
