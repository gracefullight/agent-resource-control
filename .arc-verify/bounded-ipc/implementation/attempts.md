Task-5 first attempts retained.

1. Focused cargo test --offline -p arc-daemon --test bounded --test worker_runtime -- --nocapture: exit 101; fresh isolated target had no arcctl prerequisite. Native 200-request probe succeeded; all three bounded tests reached missing CLI assertion. Worker target was not run after bounded failure.
2. Copied collector initially resolved parent directory rather than this worktree. workspace-first receipt records exact wrong cwd and Cargo permission failure; it is not acceptance evidence. Corrected collector ROOT to parents[3] and actor identity.
3. Optional ps host inventory was denied with Operation not permitted. No workaround was attempted; required native sampler uses proc_pidinfo for owned child PIDs as designed.
4. Corrected a new doctor-output assertion against actual existing CLI text before acceptance verification; no prior assertion was changed.

5. workspace-check: native/load and nonreader passed; stalled-frame observation failed because macOS rejected SO_RCVTIMEO after peer disconnection (EINVAL). Repaired only the fixture to use nonblocking reads under one absolute observation budget. focused-repair then passed (3 bounded + 46 worker-runtime tests). Tightened cancellation ordering so the active worker remains gated while canceled queued accounting is asserted.
6. Added a separate synchronized SQLite authorizer delay within preparation of real PRAGMA integrity_check; sql-health focused test passed. This is a controlled native SQL-preparation delay, not a claim of measured SQL compute time.
