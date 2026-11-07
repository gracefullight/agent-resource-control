"""Write this attempt's handoff; leave earlier receipts and reports unchanged."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
import record

ROOT, OUT = record.ROOT, record.OUT
OWNED = ["crates/arc-daemon/tests/resident.rs", "crates/arc-daemon/tests/support/mod.rs"]


def read(name):
    return json.loads((OUT / name).read_text())


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


fingerprint = record.record.fingerprint()
save("final-source.json", fingerprint)
before = {item["path"]: item for item in read("before-boundary.json")["files"]}
changes = []
for path, item in before.items():
    current = ROOT / path
    if sha(current) != item["sha256"]:
        changes.append(path)
    assert oct(current.stat().st_mode & 0o777) == item["mode"], path
assert set(changes) == set(OWNED), changes
save("boundary-check.json", {"trackedFilesInspected": len(before), "changedPaths": changes,
     "outsideOwnedPathsUnchanged": True, "finalProductFingerprint": fingerprint["sha256"]})
diff = subprocess.check_output(["git", "diff", "HEAD", "--", *OWNED], cwd=ROOT)
(OUT / "fixture-change.patch").write_bytes(diff)
save("source-evidence.json", {**read("provenance.json"), "productFingerprint": fingerprint,
     "observedLocalHead": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
     "scope": "Input snapshot 5aa081fb with two uncommitted fixture edits; all other product bytes unchanged. Not the committed accepted baseline. Missing local refs remain missing. Verify collected file hashes in the review checkout.",
     "ownedChanges": OWNED, "postCollectionVerified": False})

raw = OUT / "workspace-final-raw"
performance = json.loads((raw / "native-measurements.json").read_text())
save("performance.json", {**performance, "sourceFingerprint": fingerprint["sha256"],
     "sourceEvidence": "source-evidence.json", "rawSamples": "workspace-final-raw/small-message-samples.json",
     "rawNativeObservations": "workspace-final-raw/native-observations.json"})
observations = json.loads((raw / "native-observations.json").read_text())
span = (observations["idle"][-1]["elapsed_us"] - observations["idle"][0]["elapsed_us"]) / 1_000_000
p95 = performance["small_message"]["p95_ns"] / 1_000_000
performance_text = f"""# Task-5 attempt-3 native measurements

Actual daemon measurements on Apple M1 Pro, 8 logical CPUs, 17,179,869,184 bytes RAM;
{performance['host']['uname']}. Rust/Cargo 1.95.0, debug profile. Current Actor HEAD
is 5aa081fb2f73f3d606ef009e30872d32aa94ce6b plus two owned uncommitted fixture
changes. Source fingerprint: `{fingerprint['sha256']}`. This is local snapshot/worktree
evidence, not code attributed to accepted baseline 42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0.

| Measurement | Actual | SRS comparison |
| --- | ---: | --- |
| Maximum sampled idle RSS | {performance['idle']['max_rss_bytes']:,} bytes ({performance['idle']['rss_mb']:.6f} MB; {performance['idle']['rss_mib']:.6f} MiB) | NFR-002 <50,000,000 bytes: pass |
| Native lifetime peak RSS | {performance['lifetime_peak_rss_bytes']:,} bytes | No SRS load-peak threshold |
| Sampled OS thread peak | {performance['sampled_native_thread_peak']} | No SRS thread threshold |
| Retained handler/task high water | {performance['report']['metrics']['tracked_tasks']['high_water']} | Instrumented task gauge; supervisor separately 1 |
| 10,000 small-message P95 | {p95:.6f} ms | NFR-004 <=20 ms: pass |
| P50 / max | {performance['small_message']['p50_ns']/1e6:.6f} / {performance['small_message']['max_ns']/1e6:.6f} ms | Zero failures |
| After-load 200-message P95 | {performance['after_load_p95_ns']/1e6:.6f} ms | Recovery observation |
| Four-client 100,000-attempt load | {performance['load_elapsed_us']/1e6:.6f} seconds | All 100,000 completed; no rejections, timeouts or cancellation |

A separate 200-request subprocess probe ran before full load and verified native
sampling, successful shutdown, accounting and database integrity. The measured
child used default C=64/R=16/Q=8, two Tokio worker threads and one SQLite thread.
After five seconds settling, 100 idle samples used nominal 100 ms cadence; actual
first-to-last span was {span:.6f} seconds. Load snapshots used nominal 10 ms cadence.
`proc_pidinfo(PROC_PIDTASKINFO)` measures daemon RSS bytes and native thread count.
Owned-child `wait4` ru_maxrss records lifetime peak bytes on macOS; Linux fixture
code converts KiB to bytes but was not executed. Sampling happens in the parent;
sampled thread peaks can miss short spikes. Instrumentation overhead is unquantified.
Task gauges count application JoinSet entries, including completed entries before
reaping, separately from native threads and the supervisor; they are not RSS data.

After 200 process-warming hello/health exchanges on a separate connection, the
latency sample uses one persistent same-user Unix connection and alternates
hello/health with ID `small` and null payload. Request bodies are 85/86 bytes;
responses are 124 bytes, with four-byte prefixes each way. `std::time::Instant`
spans request write through decoded response, including serialization. Connection
setup is separate ({performance['small_message']['connection_setup_ns']} ns).
Nearest-rank P95 is sorted index 9499. All 10,000 nanosecond durations and message
sizes are retained in `workspace-final-raw/small-message-samples.json`; no failed
exchange was discarded. No during-saturation latency claim is made.

The SRS was read in place at
`/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md`,
SHA-256 a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e.
Section 18 lines 2426/2428 supplies NFR-002/NFR-004. Decimal MB is the disclosed
comparison convention. Its draft version labels differ; hash and requirement IDs
identify the source. Both comparisons are complete and passed on this host/profile.
This does not establish an accepted-baseline improvement, release-profile behavior,
long-run idle CPU, Linux/Windows qualification or the remaining Phase 0 features.

Reproduce using a new output directory:
`ARC_TEST_EVIDENCE_DIR=<new-directory> CARGO_NET_OFFLINE=true cargo test --workspace`.
The required workspace suite supplies the actual CLI binary prerequisite. Focused
tests afterward: `cargo test -p arc-daemon --test bounded --test worker_runtime -- --nocapture`.
The active native fixture is `bounded::native_probe_then_full_100000_attempt_workload`.
`record.py` records exact argv, cwd, bounded 120-second process ownership, output
hashes and source hashes. Earlier and focused measurements remain separate raw runs;
this report uses only the successful final workspace run.
"""
(OUT / "performance.md").write_text(performance_text)

experiments = []
for name in ["connection", "request", "db"]:
    data = json.loads((raw / (name + "-100000.json")).read_text())
    experiments.append({"artifact": "workspace-final-raw/" + name + "-100000.json",
                        **{key: value for key, value in data.items() if key != "final"}})
experiments.append({"artifact": "workspace-final-raw/native-measurements.json",
                    "scope": "Actual daemon, four persistent same-user clients",
                    **performance["attempts"]})
save("accounting.json", {"note": "Four independent experiments, not 400000 network requests. Admission is intermediate; terminal outcomes are disjoint. Rejected work was never admitted.", "experiments": experiments})

results = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", (OUT / "workspace-final.txt").read_text())
passed = sum(int(r[0]) for r in results)
assert passed == 173 and all(all(int(x) == 0 for x in r[1:]) for r in results)
signals = {name: json.loads((raw / (name + ".json")).read_text()) for name in ["SIGINT", "SIGTERM"]}
health = json.loads((raw / "native-sql-health.json").read_text())
deadlines = json.loads((raw / "deadline-saturation.json").read_text())
nonreader = json.loads((raw / "nonreader.json").read_text())
failures = ["resident.rs 901 lines exceeds 500", "arc-ipc/src/lib.rs 551 lines exceeds 500", "missing test:coverage"]
save("acceptance.json", {"status": "assigned task complete; independent review pending",
    "skillsUsed": ["oma-backend"], "workspacePassingInvocations": passed, "focusedPassingInvocations": 51,
    "sourceFingerprint": fingerprint["sha256"], "ownedSourceChanges": OWNED,
    "harnessExit": read("harness.json")["exitCode"], "unresolvedHarnessFailures": failures,
    "signalToExitUs": {name: data["signal_to_exit_us"] for name, data in signals.items()},
    "interfaceCompatibility": "Production modules/manifests unchanged. ARC_RUNTIME_REPORT consumed as specified. Existing assertions, command timeouts and CLI output retained.",
    "remaining": ["Independent collection/review and mission evidence promotion", "Linux/Windows and remaining Phase 0 obligations", "av.yaml configured-target applicability inaccessible"]})

report = f"""# Task-5 attempt-3 handoff

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
bytes. Final fingerprint: `{fingerprint['sha256']}`. Review must compare collected
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
record the two-file boundary across {len(before)} inspected tracked files.

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
  non-reading peer: write-timeout counter 1, closure {nonreader['closure_us']} us.
  Synchronized connection/request saturation reached 8/4 and recovered actual status
  and doctor in the same process after {deadlines['recovery_us']} us.
- [x] Full DB queue returns protocol DB_BUSY and actual CLI IPC_ERROR without new
  stdout. Admitted status remains responsive while a bounded test-only SQLite
  authorizer pauses preparation of the real integrity SQL: {health['health_cli_us']} us.
  Explicit release completes diagnostics and normal status/doctor. Production has
  no SQL-control environment hook.
- [x] SIGINT/SIGTERM with full connection/request/DB work exit in
  {signals['SIGINT']['signal_to_exit_us']}/{signals['SIGTERM']['signal_to_exit_us']} us,
  both <=5,000,000 us. Each reports 11 spawned/11 reaped handlers (active high water 8),
  one worker started/joined, zero final gauges, database closed and socket removed.
  Database reopen/integrity=ok and new-process admission/actual CLI checks succeed.
- [x] Native peak RSS, sampled OS threads, application task high waters and 10,000
  measured small-message P95 are retained separately; see `performance.md/json`
  and raw samples. Idle RSS {performance['idle']['max_rss_bytes']:,} bytes and P95
  {p95:.6f} ms pass the located SRS targets on this macOS/debug host.
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
"""
(OUT / "delivery.md").write_text(report)
result = ROOT / ".agents/results/result-test-engineer-task-5-uninjected-attempt-3-b137e253-def6-46ef-a75d-25ab414784ea.md"
result.parent.mkdir(parents=True, exist_ok=True)
result.write_text(report)
save("claim.json", {"status": "completed", "scope": "assigned task-5; mission completion not claimed",
     "changedFiles": OWNED + [str(OUT.relative_to(ROOT)) + "/", str(result.relative_to(ROOT))],
     "artifacts": [str((OUT / name).relative_to(ROOT)) for name in ["delivery.md", "performance.md", "performance.json", "source-evidence.json", "acceptance.json"]],
     "unresolved": failures + ["Independent collection/review and wider platform/Phase 0 obligations remain", "Configured target applicability inaccessible"],
     "omaRunId": None, "injectedClaimPath": None})
artifacts = []
paths = list(OUT.rglob("*")) + [OUT.parent / "record.py", OUT.parent / "evidence.json", OUT.parent / "source-evidence.json", result]
for path in sorted(paths):
    if path.is_file() and path.name not in ["verification.json", "verification.txt"] and path != OUT / "evidence.json" and "__pycache__" not in path.parts:
        artifacts.append({"path": str(path.relative_to(ROOT)), "bytes": path.stat().st_size, "sha256": sha(path)})
save("evidence.json", {"sourceFingerprint": fingerprint["sha256"], "artifacts": artifacts})
print(json.dumps({"status": "ready-for-collection", "sourceFiles": len(fingerprint["files"]),
      "changedSourcePaths": changes, "artifacts": len(artifacts), "fingerprint": fingerprint["sha256"]}))
