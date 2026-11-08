# Independent lifecycle implementation review

## Review Result: PASS

Status: completed scoped source review. No open CRITICAL, HIGH or MEDIUM code findings remain in final snapshot `66f24fe65f54862539fba92bed779d86820a2b4ccc6bcac859485b2f529009ec`. The confirmed closed-descriptor finding is corrected and independently rechecked. Final mission acceptance and collection/coverage gates remain downstream QA responsibilities. A repeated, persisted terminal-generated SIGTSTP probe passed as detailed below.

Review owner: `/root/lifecycle_review_fresh`, using `oma-qa`. Files changed: `.arc-verify/managed-exec/implementation/code-review.md` and the new ignored diagnostic artifacts listed in the retained-artifact tables below. These were generated for explicitly requested verification follow-ups. Product code, manifests, mission state and prior evidence were not edited. No run/claim identity was injected into this native review dispatch; the parent owns its task-7 handoff. The assigned report path takes precedence over the generic run-report location.

## Summary

The CLI has one spawn site after durable register/claim acknowledgement. Metadata retries retain one job key and cannot spawn. Native argv and inherited process streams remain local; typed IPC/storage fields exclude argv/environment/cwd/output. The daemon has no managed-child launch or wait ownership. Status reconciles identities without replay or an invented outcome; completion preserves immutable identical outcomes and rejects conflicts. Short worker transactions retain existing admission, cancellation and shutdown accounting. Recent-job discovery uses a bounded indexed query instead of per-record native probing. No web UI accessibility claims apply; native terminal behavior is covered by the tests listed below.

## Findings

### CRITICAL

None open.

### HIGH

None open.

### MEDIUM

None open.

### LOW

None confirmed.

## Corrections reviewed

These were raised during review and corrected by the implementation owner; they are not findings against the final snapshot.

| Severity | Location in final source | Problem and correction |
| --- | --- | --- |
| HIGH | `crates/arc-cli/src/exec.rs:97`, `exec/signals.rs:10`, `exec/terminal.rs:48` | Darwin signal-origin heuristics alternately duplicated terminal Ctrl-C or suppressed wrapper-only SIGTERM. The child now has its own process group; terminal ownership moves to that group and returns afterward. Wrapper-directed signals forward to the unreaped child. Remediation: `command.args(&argv[1..]).process_group(0); terminal.configure(&mut command);` |
| MEDIUM | `crates/arc-cli/src/exec/signals.rs:44` | Installing catches over inherited SIG_IGN changed the child's native disposition. Preserve ignored signals: `if old.sa_sigaction == libc::SIG_IGN { continue; }`. |
| MEDIUM | `crates/arc-cli/src/exec/signals.rs:91` | Stop/resume must consume a stop event without blocking and retain the child's actual outcome if its foreground group disappears. Remediation uses `waitid(..., WSTOPPED | WNOHANG)`, best-effort terminal restoration/resume, and continued native waiting. The killed-while-stopped fixture now returns the real signal result. |
| MEDIUM | `crates/arc-storage/src/jobs/transitions.rs:220` | Completion previously replaced native identity observation time with completion time. Keep `job.observation.observed_at_ms` unchanged; store the distinct event in `job.completed_at_ms = Some(now)`. |
| MEDIUM | `crates/arc-daemon/src/db_worker/jobs.rs:70` | Sampling native verification time after dequeue falsely shifted the observation into the future. Capture before probing/admission and carry it through the queue: `let observed_at_ms = now(); let evidence = StartEvidence::gather(&request); Operation::Start(request, evidence, observed_at_ms)`. |
| MEDIUM | `crates/arc-storage/src/jobs/transitions.rs:29`, `jobs/reconcile.rs:43` | A retained zombie identity could be presented as currently running. Separate exact identity evidence (`probe`) from live evidence (`probe_live`); retain verified identity metadata without claiming live Running. |
| MEDIUM | `crates/arc-ipc/src/jobs/validation.rs:8` | The generic 1..127 envelope accepted impossible Darwin signals. Remediation validates against `native_signal_max()` (31 on Darwin, 64 on Linux). |

The initial blocking-connect concern was not reproduced on Darwin: 128 held connections to a listen(0) fake endpoint still yielded static STATUS_UNAVAILABLE in about 27 ms. It was not reported as a confirmed defect. The final implementation nevertheless configures a native nonblocking socket before connect and applies the existing absolute deadline to connection establishment.

## Verification

Final managed-input SHA-256, independently recomputed through `managedInputs`:

`66f24fe65f54862539fba92bed779d86820a2b4ccc6bcac859485b2f529009ec`

Native host in all final command receipts: Darwin 27.2.0, arm64. The reviewer recomputed each receipt's stdout/stderr digest and matched the retained bytes. Commands were run serially by the implementation owner before final source review; the reviewer did not duplicate Cargo or collection work.

| Command / evidence | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS; final-source receipt `implementation/fmt.json`, SHA-256 `5be1372a2778c8c53f73378f7ed76ca5d3ddfb2d24ba126d057686383d0318f8` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS; final-source receipt `implementation/clippy.json`, SHA-256 `166cb77b7c25fedc51af21e19cca2963bc549569525d7ddcd6e5281c5f949c9a` |
| `cargo test --workspace` | PASS; final-source receipt `implementation/workspace.json`, SHA-256 `f10622f46cb30226e6e2d5b3c6c0c0e95622a9fd4d27c08affacc9c0d785a184`; 329 passed, 0 failed, 0 ignored, 32 harnesses |
| `target/debug/deps/managed_exec-4f76c78332cff03c --test-threads=1` | Independent native execution: 14 passed, 0 failed, 0 ignored, 4.63 s, on earlier hash `784326315075d6e317ac19f207f0c625abfdb34f61cc4c258952c008f1156eda`; final workspace reruns and extends those cases after the timestamp, descriptor and terminal corrections |
| `git diff --check -- <reviewed implementation paths>` | PASS |
| Reviewed production file line counts | All at most 500; counts and hashes below |
| Migration `0001_phase0.sql` | No diff; SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`, matching the immutable contract |
| Initial inline Python terminal Ctrl-Z orchestration | Inconclusive: stop observed; resume/cleanup wait timed out. Retained below as failed verification history. |
| Persisted Python terminal Ctrl-Z retry with continuous PTY drain | PASS, exit 0; VSUSP stopped the chain, SIGCONT resumed it, input was preserved, exit 17 and foreground restoration were verified; all four recorded native identities absent afterward. |

The final workspace output is `.arc-verify/managed-exec/implementation/runs/2026-11-08T12-29-46-849Z-workspace/stdout.txt`. Earlier failures remain retained by the implementation owner; they are not substituted for final passing receipts. One earlier unchanged-source workspace attempt had a BrokenPipe in the existing classification privacy test; the final source-bound full run passed that regression.

## Acceptance criteria checklist

- [x] Read the assigned CLI, daemon, storage/IPC contracts and focused tests; review in security, performance, terminal/accessibility and correctness order.
- [x] Verify launch ownership, one stable key, durable prelaunch operations, no spawn in retry/daemon paths, no resource reservations and metadata privacy.
- [x] Review exact argv/streams, exit 17, native signal status, daemon SIGTERM/SIGKILL survival, idempotence and identity reconciliation against focused fixtures.
- [x] Verify final formatting, Clippy and workspace receipts bind the recomputed source hash and retained output digests.
- [x] Independently run the existing native integration binary; identify its exact earlier source snapshot rather than mislabel it as the later timestamp snapshot.
- [x] Preserve source and all other agents' outputs; write only the assigned report.
- [x] Record unverified acceptance and manual-probe termination limits explicitly.
- [x] Review native pre-runtime descriptor capture, final child re-close ordering and actual non-Rust regression; independently recheck all four closed-descriptor masks.
- [x] Review final background terminal ownership/resume, static job parse errors, exact exec help and canonical-root guard repair.
- [x] Verify refreshed fmt, Clippy, 329-case workspace and canonical-root regression receipts against the final input hash.
- [ ] Downstream mission QA: confirm each crate's 80% floor, current-index clean-checkout/collector/harness gates and final acceptance artifacts. These are outside this source-review dispatch.
- [x] Independently establish terminal-generated SIGTSTP/Ctrl-Z stop/resume, preserved input, native exit 17, foreground restoration and absence of the four recorded retry identities with a continuously drained native PTY probe.

## Reviewed source binding

| Path | Lines | SHA-256 |
| --- | ---: | --- |
| `crates/arc-cli/src/exec.rs` | 213 | `4fa2f5a2ebd8be69f76af2347d727de53913317a0466d4d7b6344cd0362fbfed` |
| `crates/arc-cli/src/exec/signals.rs` | 155 | `af1ad552fd30ee98dbdacf1517d65757e7c098cca00d65ccb55b82da9b77a6c9` |
| `crates/arc-cli/src/exec/terminal.rs` | 103 | `20ab64860571badd26f522ac2174f37b563cab7e6112c1f2c5023f68221bb4fd` |
| `crates/arc-cli/src/exec/transport.rs` | 148 | `e127b2c16eb3a9f1ae02593252ecc6d12ce5934cafe92417a96945dad8e5e9df` |
| `crates/arc-cli/src/job.rs` | 114 | `47bfb5fb337928ced9bb933b99912567e0d33204e3f197e06f028dae8105e033` |
| `crates/arc-cli/src/main.rs` | 225 | `22c10fbcb888a29930d9ff3dcc37bebb8e02ff45c7ced4447fdd6cadcb1107ed` |
| `crates/arc-daemon/src/connection.rs` | 212 | `0a3cedf6b9b1103e2aa2d79d02302a1c2eee754c8b44c37169431ce82d330289` |
| `crates/arc-daemon/src/db_worker.rs` | 362 | `9956e9b3b28f8ea1d58f9a08b42c2df0ada9b7dfc8d61710b06cf0cb7aaa2b69` |
| `crates/arc-daemon/src/db_worker/execution.rs` | 353 | `aa2d4c01fbb7b2b1a5ca9220eca4315fefab5a1ea0817a6af6cb2fd79500861b` |
| `crates/arc-daemon/src/db_worker/jobs.rs` | 129 | `37423dee5cd9827f79e69d505e4f7f50745c39d571f6421fa8051f702e36cc6d` |
| `crates/arc-daemon/src/lifecycle.rs` | 305 | `70692d247d8245e075e961dd20fe72485eed4eda1b0884f088a3ad688c2c4bb2` |
| `crates/arc-daemon/tests/managed_exec/cli.rs` | 308 | `de832f22bac5c5fa8a99ea4fa933a87a3f543999871e914323dad5f1e57756fe` |
| `crates/arc-daemon/tests/managed_exec/protocol.rs` | 267 | `957693d38491c0842f6a6ab6731ef15d089ad6a998d94e9cbbcb3488289064d7` |
| `crates/arc-daemon/tests/managed_exec/retries.rs` | 189 | `de8f6e37059c94dd479c3b6be357e4a4954abc50f260a26f2686edeb715a29ae` |
| `crates/arc-daemon/tests/managed_exec/terminal.rs` | 355 | `0a886ab35d8e167da16527176c8781e184edbf4197735ecf1ee2d8b99c74ad58` |
| `crates/arc-daemon/tests/managed_exec/scenarios.rs` | 198 | `6aef15458e4745372b58652cc09ec47f0db7c5cd8268fb532f7d673c0c708801` |
| `crates/arc-daemon/src/db_worker/jobs_tests.rs` | 146 | `023ca7117c76443d568d324acc7c2a86e57099620f5e87373a638de5feb1cfcd` |
| `crates/arc-cli/tests/fixtures/child.rs` | 193 | `a0025a46825d0e971f506714f1d64b178bb9023510249c6265f0aebc4247c586` |
| `crates/arc-ipc/src/jobs.rs` | 216 | `c75464c6cd8239c862ba100a501975b2189ae37aa6202a0931f7875432619a3e` |
| `crates/arc-ipc/src/jobs/types.rs` | 162 | `9459b9a22f0e3cd2ca2c921e5039117007adc928354587612759ad840380ff7b` |
| `crates/arc-ipc/src/jobs/validation.rs` | 100 | `ac9eda72ed28c82d28f646e00b0b66327f516e39769160b297b787eab4ebc11a` |
| `crates/arc-storage/src/jobs/transitions.rs` | 229 | `e746d99e944903392a688aedf7ab9efd6621d879f169c1fa8065abb7477e8193` |
| `crates/arc-storage/src/jobs/reconcile.rs` | 155 | `2232fc199f0e1dac573017d4e92ef7fcde02689268111bfcd218a47a38186d00` |
| `crates/arc-storage/src/jobs/codec.rs` | 171 | `cab00f289829e63019d1d8c86e2cdd986060317cc3b1050b08c6a22955c2796f` |
| `crates/arc-core/src/process_identity.rs` | 213 | `dae7a4ec3df23948205f7d22bceea5320fdc84dbbe2ecdce7c996e669088053d` |
| `crates/arc-core/src/process_identity/macos.rs` | 117 | `37b322088d396306fc6583e6739d410b85e4fe179f7a6e099f345d68990cc57d` |
| `crates/arc-core/src/process_identity/linux.rs` | 29 | `772b31f1b8c7e023aebf90f349861d0f4be1b867aec4a567dd94bb5744180ccf` |
| `crates/arc-cli/src/exec/descriptors.rs` | 40 | `059437b7c7fa2cbe19eeb2d70d286043e59e2d5aaca6c1b481eb7b06e1d1d668` |
| `scripts/coverage/current-preservation.ts` | 295 | `2ec0a7cf1b48bb565492286c7856c728e3e3f3fc24f1fbd8c2e08d4f3bb47f72` |
| `.arc-verify/managed-exec/implementation/canonical-root.test.ts` | 16 | `0765708b9d4e97b1d747d06e5efc1f16898963a26ee86f928f04045b75611b42` |

## Manual Ctrl-Z attempt: inconclusive

Command: `python3 -u -` with an inline Python script, executed in tool session `12658`. No separate script or output file was created. The exact ownership IDs were retained only in the failed Python process's local variables and were not printed before its error. This is an evidence limitation; no child/wrapper/supervisor tuple is claimed. The isolated daemon's stale lock identified PID `88135`, start token `1794139453690694`; a subsequent `os.kill(88135, 0)` raised `ProcessLookupError`.

The script used a temporary `/tmp/az-*` directory, started existing `target/debug/arc-daemon` with isolated ARC_DB_PATH and ARC_SOCKET_PATH, and waited for its ready line. It opened a Unix control listener and `pty.openpty()`, then launched this existing native fixture chain with slave stdin/stdout/stderr:

```text
target/debug/arc-exec-fixture target/debug/arcctl exec -- target/debug/arc-exec-fixture
ARC_FIXTURE_LAUNCH=tty
ARC_FIXTURE_CONTROL=<isolated child.sock>
ARC_FIXTURE_RESTORED=<isolated restored file>
PRIVATE_ENV=qa-native
```

Relevant exact orchestration, after accepting the control connection:

```python
msg = json.loads(read_exact(struct.unpack('>I', read_exact(4))[0]))
child_pid = msg['pid']
wrapper_pid = msg['ppid']
assert msg['tty'] and msg['foreground'] and msg['pgid'] == child_pid
conn.sendall(b'x')
os.write(master, b'\x1a')
deadline = time.monotonic() + 8
stopped = False
while time.monotonic() < deadline:
    pid, status = os.waitpid(supervisor.pid, os.WUNTRACED | os.WNOHANG)
    if pid:
        assert os.WIFSTOPPED(status)
        assert os.WSTOPSIG(status) == signal.SIGSTOP
        stopped = True
        break
    time.sleep(.005)
assert stopped
os.kill(supervisor.pid, signal.SIGCONT)
os.write(master, b'hello\n')
assert supervisor.wait(timeout=8) == 17
```

The stop assertions passed. `supervisor.wait(timeout=8)` raised `subprocess.TimeoutExpired`. Cleanup then sent SIGKILL to the captured `child_pid`, `wrapper_pid` and `supervisor.pid`, but its `supervisor.wait(timeout=5)` also raised TimeoutExpired. PTY descriptors were scheduled to close after that wait, so the failing wait prevented explicit close; they closed when Python exited. The daemon shutdown code was also skipped by that exception; the daemon was subsequently confirmed absent as above. No stdout success JSON was produced.

The script did **not** continuously drain the PTY during its waits. The implementation owner identified this as a host-sensitive orchestration difference from the passing native test fixture, which uses a continuous background reader. This failed manual attempt is not counted as a pass or a reproduced product defect. Exact terminal-generated SIGTSTP acceptance remains unverified by this attempt. Native integration tests separately verify self-SIGSTOP, continuation, child death while stopped, inherited ignored SIGINT, terminal Ctrl-C count once, terminal SIGQUIT, exact native wait results and foreground restoration.

A later read-only inventory found no retained `/tmp/az-*` probe directory, so there was no surviving local SQLite/marker evidence from which to recover the three helper identities. SIGKILL was attempted, but termination/reaping of those helper tuples was not independently proven. This report does not claim they were all reaped.

## Persisted Ctrl-Z retry: PASS

The implementation owner explicitly requested one retry with persisted script/output, continuous PTY draining and recorded native identities before signaling. The command was `python3 -u target/task7-review-ctrlz.py`; it exited 0. The probe completed on earlier Rust behavior snapshot `85bb0518105bd601b89c890d7d6af04e6f520976b94e7654f859c26f4d40eb2c`, with no concurrent Cargo/collector/daemon work. It is retained with its actual source identity, not relabeled as execution against later snapshots.

The script starts the existing native daemon and fixture chain in an isolated temporary directory, runs a continuous PTY reader through normal execution and cleanup, persists each PID/start token before signaling, sends terminal VSUSP byte `0x1a`, observes the supervisor stopped by SIGSTOP, sends it SIGCONT, writes `hello\n`, and requires native exit 17, `input=hello` output and the `foreground-restored` sentinel. The daemon exits 0.

Recorded native identities (start tokens are native macOS microseconds):

| Role | PID | Start token | Final native observation |
| --- | ---: | --- | --- |
| daemon | 19653 | `1794139812690425` | absent |
| supervisor | 19654 | `1794139812731302` | absent |
| child | 19656 | `1794139812763804` | absent |
| wrapper | 19655 | `1794139812742550` | absent |

Final absence was checked with native `proc_pidinfo` plus `sysctl(KERN_PROC_PID)` fallback for retained process records; every recorded identity returned absent. The retry's daemon and supervisor were waited successfully. Child/wrapper absence is recorded separately from parent reaping; this report does not claim to have waited those nondirect children itself.

The exact-workspace-fixture executable inventory found zero matching active fixture paths before and after the retry among 768 enumerated PIDs; two process paths were unreadable. That inventory is supporting evidence, not a proof about unreadable processes or recovery of the first attempt's lost tuples. The first attempt's historical reaping evidence limitation remains recorded above.

| Local retained artifact | SHA-256 |
| --- | --- |
| `target/task7-review-ctrlz.py` | `1e17ecd41cbefe7e4d4d67cd1566ed7644497928c9cd038aeb6f6fa1f3a3b1bc` |
| `target/task7-review-ctrlz.json` | `72f04a895fa4986d146481f9ca9a84d5758ddadcee125395be159caf50b71df2` |
| `target/task7-review-ctrlz.log` | `3f7ff327118d09a5eb8e66f593d2280bdcabb23b65f6dfcfad2024d86e787ed3` |

These artifacts are ignored local diagnostics. They supplement this report and are not substitutes for final mission receipts.

## Closed-descriptor follow-up: pre-correction failure retained

A direct native Python probe, using `os.fstat` before opening its sidecar report file, confirmed a contract violation on source hash `01801d5dc6ee09e91a6b3a8036b6ef0fd62d6ce995d76023e10e77c8d7865247`:

| Originally closed | Direct child fd0/fd1/fd2 open | Wrapped child open |
| --- | --- | --- |
| fd0 | false / true / true | true / true / true |
| fd1 | true / false / true | true / true / true |
| fd2 | true / true / false | true / true / true |
| all | false / false / false | true / true / true |

All eight child executions returned 17. The owned daemon PID 60225 exited 0. Compact local evidence: `target/task7-closed-fd-probe.json`. The prior passing Rust fixture matrix is retained as evidence of that fixture's behavior; it is not proof of transparent closed-descriptor inheritance for a non-Rust child. The correction and independent passing recheck are documented below; no product source was edited by this reviewer.

## Final bounded follow-up corrections

### HIGH closed-descriptor finding: corrected

Final locations: `crates/arc-cli/src/exec/descriptors.rs:10` and `crates/arc-cli/src/exec.rs:103`. A native loader constructor records originally absent standard descriptors with allocation-free `fcntl(F_GETFD)` calls before Rust runtime sanitization. `descriptors::restore_in_child(&mut command)` registers an async-signal-safe `close` hook after terminal handoff and before the one child exec. It changes neither environment nor stream bytes, and closes only descriptors originally absent. The constructor runs for diagnostic invocations but only observes descriptors; restoration is scoped to exec children.

`scenarios::native_non_rust_child_observes_literal_closed_standard_descriptors` complements the original Rust fixture parity test. It observes fd0/fd1/fd2 with native Python `fcntl` before opening its sidecar report and asserts the exact expected state for each mask, for direct and wrapped launches. The reviewer independently reran the actual Python `os.fstat` probe on descriptor-repair hash `d61071851e156dc0e2676c1e3d6d28ff48f1341c59732723bb800fced447724d`, with source unchanged during execution:

| Originally closed | Direct and wrapped descriptor states | Result |
| --- | --- | --- |
| fd0 | false / true / true | PASS |
| fd1 | true / false / true | PASS |
| fd2 | true / true / false | PASS |
| all | false / false / false | PASS |

All eight child executions returned exactly 17 and emitted no stdout/stderr. The owned daemon PID 61094 exited 0 and was waited. The later background/diagnostic changes retain this descriptor module and final hook ordering; the final source-bound workspace run executes the non-Rust regression again.

### Terminal and diagnostic follow-ups

`crates/arc-cli/src/exec/terminal.rs:20` now retains a controlling-terminal handle for background invocations without moving the foreground at initial spawn. The guard tracks ARC-made ownership with `Cell<bool>`. Resume hands the terminal to the child only after the caller has foregrounded the wrapper's group; restoration occurs only when ARC owns that change. `terminal::native_background_terminal_read_can_be_foregrounded_and_resumed` exercises actual background terminal read/stop and caller foreground/continuation/input/exit 17 for direct and managed children. The final 16 native integration cases pass inside the source-bound workspace run.

`crates/arc-cli/src/main.rs:44` gives Exec exact static `override_usage = "arcctl exec -- <PROGRAM> [ARG...]"` and a valid PROGRAM placeholder; the regression checks that usage and rejects the malformed earlier bracket text. `main.rs:125` routes malformed new job command errors to `job::invalid`, so supplied identifiers/options are not reflected. The branch remains scoped to job; existing diagnostic/classification paths retain their prior handling. The existing status scenario now checks static errors for unknown subcommand, unknown option and extra argument.

### Verification-root repair

`scripts/coverage/current-preservation.ts:244` canonicalizes the checkout root with `realpathSync` before any guards, cache access or source inventory. This fixes native `/var` versus `/private/var` identities seen by real clean-checkout execution. Every existing harness/admission check, protected byte/index check, historical baseline reference, assertion/reachability check and line limit remains active. No accounting denominator, exclusion or bypass changed.

The owned temporary symlink regression runs real preservation checks through alias and canonical paths, requires equal results and requires pass. Successful exact command: `bun test ./.arc-verify/managed-exec/implementation/canonical-root.test.ts`. Its earlier invocation without `./` matched no files; that failed attempt is retained and is not a passing regression run. Its content hash is recorded in the binding table separately because the test is a verification artifact outside `managedInputs`.

Final source-bound canonical-root receipt SHA-256: `d3e464e1c16aab9b1f51034139d0685e9d7d70f9ac290e399d9d47967c10f1d3`. Its output is retained in `.arc-verify/managed-exec/implementation/runs/2026-11-08T12-30-34-364Z-canonical-root`.

The final fmt/Clippy/workspace/canonical-root receipts and their retained stdout/stderr digests were independently checked. The full workspace run has 329 passes, zero failures and zero ignored tests across 32 harnesses. Earlier independent native execution remains attributed to its actual earlier snapshot: the 14-case run on `01801d5d...` is not relabeled as a run against the later descriptor/terminal source.

Additional local review artifacts:

| Artifact | SHA-256 |
| --- | --- |
| `target/task7-review-final-native.json` | `f4cc6b8e87aaec8fbbee12b2bf9a5b682d3188839c78bcae2651e0e9bfc6a80c` |
| `target/task7-review-final-native.log` | `8555a1a4ade766a36fef31be4b360277fbe398dd1de202d7e624b51d432b2ff1` |
| `target/task7-closed-fd-probe.json` | `70e5e27885957dd390bcd1653d88391184d0402db2f0388fd7b1b25b1f52b3f4` |
| `target/task7-closed-fd-recheck.py` | `0ea2294fe4c468c29f0ec9771961e28d613ca76e83d637581be3cb2d1d5753fc` |
| `target/task7-closed-fd-recheck.json` | `4c8a0bad11ed0388f35ee33ac913354042f826d037b112b9e3c893e3bc4e578f` |

These are ignored local diagnostics, supplemental to the report. Final per-crate coverage, current-index clean-checkout/collector/harness gates and mission delivery remain with Chief QA. No further native or Cargo work was performed by this reviewer after the independent descriptor recheck.
