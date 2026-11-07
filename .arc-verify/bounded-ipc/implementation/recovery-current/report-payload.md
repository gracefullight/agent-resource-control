# Task-5 attempt-3 handoff

Status: assigned implementation and verification complete; ready for Chief collection
and independent review. Used oma-backend and the installed backend-engineer definition.

## Changes and provenance

The input snapshot already contains the bounded/runtime/measurement tests and prior
receipts. Initial read-only historical verification passed and all 50 product files
matched the previous final source fingerprint. Current local HEAD/snapshot is
5aa081fb2f73f3d606ef009e30872d32aa94ce6b, tree 87d951cecbcdf7236abf9af5da192e5e897096c0.
The accepted committed baseline remains 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0,
tree c802f71ac4d91ed6b8a602d5c83bf9cf57116b76. These are distinct evidence scopes.
Both supplied local ref names fail resolution and are retained as missing; no revision
was substituted. `source-evidence.json` and `final-source.json` bind actual delivered
bytes. Final fingerprint: `b3610549adfebffdd626a760741a200c848ec7ee26a88482bcf7e38e563383c4`. Review must compare collected
bytes in its own checkout; Actor-local tracking does not establish baseline tracking.

Only two test files changed in this attempt: `crates/arc-daemon/tests/resident.rs` and
`crates/arc-daemon/tests/support/mod.rs`. The coordination probe now releases its file
lock while retaining the local mutex until the child exits. Previously dropping the
whole guard allowed sibling fixture threads to compete with the child during the
handoff. All original child-acquisition assertions and the ten-second exit timeout
remain. Other fixtures still retain both locks through child reaping. A method-level
dead-code annotation covers binaries that import support without this resident-only
handoff. No production module, manifest, lockfile, database interface, shared registry,
saved assignment or mission state changed. `fixture-change.patch` and `boundary-check.json`
record the two-file boundary across 120 inspected tracked files.

All other additions are within this `resume-attempt-3/` evidence directory and the
supplemental `.agents/results/` report. Prior evidence was not overwritten. The
historical top-level implementation report remains historical and does not describe
these two uncommitted changes. The new read-only verifier accepts differing review
HEADs but requires exact delivered source/artifact hashes.

## Acceptance evidence

- [x] Three independent 100,000-attempt production admission tests reach their caps:
  connections 8 admitted/99,992 rejected; requests 4/99,996; database 9/99,991 with
  queue 8 plus executing 1. Every attempt has a disjoint terminal outcome. Cancelled
  DB tickets retain queue/outstanding accounting until worker disposal. These units
  are separate from network throughput and do not claim 100,000 client threads.
- [x] Actual default-profile daemon handles 100,000 exchanges over four synchronized
  persistent clients, all completed, no rejection/timeout/cancellation. Snapshot
  high waters and final zero gauges cover connections, requests, tracked/unreaped
  tasks, queue, executing/outstanding DB work, body/response bytes and rejection work.
- [x] Active paused-time transport units cover trickled headers/bodies, absolute
  deadlines, malformed frames and socket backpressure. Real subprocesses cover
  stalled headers/partial bodies, malformed/version/length recovery and a genuinely
  non-reading peer: write-timeout counter 1, closure 107310 us.
  Synchronized connection/request saturation reached 8/4 and recovered actual status
  and doctor in the same process after 303692 us.
- [x] Full DB queue returns protocol DB_BUSY and actual CLI IPC_ERROR without new
  stdout. Admitted status remains responsive while a bounded test-only SQLite
  authorizer pauses preparation of the real integrity SQL: 6048 us.
  Explicit release completes diagnostics and normal status/doctor. Production has
  no SQL-control environment hook.
- [x] SIGINT/SIGTERM with full connection/request/DB work exit in
  7300/4588 us,
  both <=5,000,000 us. Each reports 11 spawned/11 reaped handlers (active high water 8),
  one worker started/joined, zero final gauges, database closed and socket removed.
  Database reopen/integrity=ok and new-process admission/actual CLI checks succeed.
- [x] Native peak RSS, sampled OS threads, application task high waters and 10,000
  measured small-message P95 are retained separately; see `performance.md/json`
  and raw samples. Idle RSS 5,586,944 bytes and P95
  0.089916 ms pass the located SRS targets on this macOS/debug host.
- [x] Existing active resident/startup/CLI/core/protocol/storage suites preserve
  same-user policy, TMPDIR-independent identity, single-daemon admission, filesystem
  protections and protocol-1/CLI behavior. Frozen migration and harness match accepted
  baseline bytes, hashes 046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384
  and 93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33.

## Commands, failures and retained limits

| Exact command | Outcome |
| --- | --- |
| cargo fmt --all -- --check | Final exit 0 |
| cargo clippy --workspace --all-targets -- -D warnings | Final exit 0 |
| cargo test --workspace | Final exit 0; 173 passing invocations across 17 suites; zero failed/ignored/filtered |
| cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture | Exit 0; 51 passed, zero ignored/filtered |
| cargo test -p arc-daemon --test resident -- --nocapture | Exit 0; 11 passed after handoff repair |
| ./scripts/harness/validate.sh | Exit 1; unchanged; all three failures retained |

The first workspace attempt failed the coordination test at resident.rs:185 after
ten seconds. Its isolated filtered rerun passed; this was not a deterministic isolated
reproduction. The source exposed the local handoff race above; the full resident suite
and final workspace then passed after its repair. `workspace.json/txt` retain the failed
run, never promoted as passing evidence. `clippy-fixture.json/txt` retains the intervening
unused-helper compilation failure, corrected with a narrow resident-only annotation.
Final fmt/Clippy/workspace/focused/harness receipts all bind the final fingerprint.
Earlier successful receipts retain their earlier fingerprints and scopes.
The first evidence-collection invocation failed because the new checkout lacked
`.agents/results/`; its following verifier could not open the unfinished index.
The collector now creates that generated report directory before writing. This
repair touched evidence tooling only; `collection-first-failure.json` retains it.

The unchanged harness still reports resident.rs 901 lines, IPC lib.rs 551 lines and
missing test:coverage. No threshold, verification command, skip flag, coverage tooling
or acceptance assertion changed. No dependency download/install, standalone build,
browser or GUI check occurred. Required Cargo tests compile their normal prerequisites
offline in the owned worktree target cache. The saved deadline, concurrency/run/retry
limits were retained in `preflight.json`; no new mission or run state was created.
No OMA claim path/runId was injected; none was invented.

## Chief handoff and remaining obligations

Run `python3 .arc-verify/bounded-ipc/implementation/resume-attempt-3/verify.py` after
collection. It checks receipts/logs, source/boundary hashes, caps, disjoint accounting,
raw percentile recomputation, signal joins/zero gauges, integrity and restart receipts.
`performance.json` here is the current promotable task-5 measurement input; shared
mission performance/QA artifacts and final independent oma-qa remain their writers' work.
The production ARC_RUNTIME_REPORT interface is unchanged; no ownership handoff is needed.

Remaining: the three harness failures; independent collection/review; Linux qualification;
Windows Named Pipe/runtime/signal/native measurements and two-OS tray/desktop integration;
Phase 0 exec/Managed Job/hook/locality features and vendor/classification/Meter/Traffic/Sweeper;
long-run idle CPU and broader crash-recovery qualification. Measurements are host/profile
specific; sampled thread peaks may miss brief spikes and instrumentation overhead is
unquantified. No baseline performance delta or during-saturation P95 is claimed.
`av.yaml` remains denied in supported Git inspection, so configured external target
applicability is unverified. No external delivery or service-health proof is invented.
