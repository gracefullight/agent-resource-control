# Independent QA — task-24 attempt 2

**Review result: PASS for the assigned independent QA scope.** No reproducible CRITICAL, HIGH or MEDIUM product defect was found. The prior task-24 review did not reproduce credential failure through spawn; the explicitly simulated observation below closes that review gap. This is not final mission acceptance: the existing final and terminal decision remains stale and belongs to the Chief.

Reviewer `reviewer`; skill `oma-qa`. Accepted HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`; current source fingerprint `bd81c863144d1dd2b149122fa47299c60f503064eda68262fa1d464c08212fdb` over 242 inputs before and after every recorded command. Current gate SHA-256 `4493bb1b91d970a5ee77b86e2c436b020f22220e039e59bd726ad6fde736bbcd`. Review recorded 2026-11-10T01:24:40.586080+00:00.

Workspace and cwd: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Host: `Eunkwangui-MacBookPro.local`, Darwin 27.2.0 arm64, Rust host `aarch64-apple-darwin`; Cargo cargo 1.95.0 (f2d3ce0bd 2026-03-21), rustc rustc 1.95.0 (59807616e 2026-04-14), Bun 1.4.4. At admission the workspace and HEAD matched the assignment. At workspace-test start the retained absolute deadline was 2026-11-10T20:40:08.584Z, with 69,841.199 seconds remaining. Each wrapper allowed 7,200,000 ms, bounded by the remaining absolute deadline. No cancellation notification was received; controller cancellation state was not independently exposed. The host had 8 logical CPUs, 16 GiB RAM and 28 GiB available disk. `ps` was denied with `operation not permitted`; no GUI or other workaround was used. This review started no coverage collector and cannot independently rule out other host processes.

Before changes, the complete preceding QA files were saved as `target/exec-fail-open/task24-attempt2-independent/prior-review.md` (SHA-256 `182d5b3900db3a04095da42c8596101a7439d1d9fa60e5e275b5d678ee859cc9`, 176642 bytes) and `target/exec-fail-open/task24-attempt2-independent/prior-review.json` (SHA-256 `025c2295f62ca91b7a427b795361d06ff675f1dca7e30e04507fcb2d64a56381`, 323359 bytes). The entire preceding Markdown, including the task-12 preliminary section, is appended verbatim below. Its conclusions are historical; the current review adds the credential-failure spawn observation.

## Credential failure: simulated native execution

The corrected `crates/arc-cli/src/exec/transport.rs:281` authenticates a connected peer before `arc_ipc::exchange` at line 148 can send request bytes. `exec/transport/peer.rs:15` calls macOS `getpeereid`; `peer.rs:5` compares its UID with the effective UID. Authentication errors map to retryable endpoint failure. `exec/transport/repair_tests.rs:277` independently injects credential errors in the transport and confirms no bytes reach the untrusted connections; `repair_tests.rs:209` checks a real same-user socket pair and an injected unequal UID. Those unit tests alone stop before spawn.

To exercise the missing path, `credential-fault-output.py` copied the existing uninstrumented QA CLI (SHA-256 `1c8fae62a0d2094f8560dedf88984e76945f8414cbe97d1665b3ad12051fb9b7`) to ignored `target/`, replaced the arm64 call to `_getpeereid` at virtual address `0x10002d228` / file offset 184872 with `mov w0, #-1`, and ad-hoc signed the copy. `otool -Iv` and the original and altered disassemblies bind the call and replacement; the script verified the text section differs by exactly four bytes and all other 14 file-backed sections match. The modified copy SHA-256 is `e12f90d8de66b2e9d3ff03f37ca6ac71330dcb7738e1aa7668022fd9dd92746d`. No product source, implementation test or original binary was modified. This forces a simulated credential-query failure on a same-user fixture; it is not a real foreign-user observation, and the injected instruction does not reproduce a specific errno.

The instrumented CLI entered the actual preparation and sole spawn path: three peer connections received **[0, 0, 0] bytes**, the child ran **1** time, exited **17**, and remained at **1** invocation after a real daemon started at the same endpoint. Parent-before-wrapper-spawn to child handshake was **355.948 ms**, unadjusted. Its diagnostic said `TRACKING_UNAVAILABLE` and did not disclose the synthetic secret. The recovered database contained 0 jobs, 0 permits and 0 GC operations; the seeded resource and sentinel remained unchanged. Child stdout and stderr hashes are `719c38c0154e40c342ac58fb3f7a817a8b06c9776848eae642afcaf951c8af67` and `aae8f7014249a39e008328c502d3e1796528dffc88d639c6d3326fae8956fabc`; their raw bytes are retained beside the probe. A first independent run also passed; its receipt remains in the same directory.

A native same-user peer sending a protocol-1 authenticated rejection returned exit 1, `NOT_STARTED` and zero child invocations in `decision-probe.py`. The other decision cases checked protocol incompatibility, complete malformed replies, initial ENOTSOCK and ENOTDIR configurations, and invalid arguments with no child. The operator classification of initial ENOTSOCK/ENOTDIR as invalid endpoint configuration is retained.

## Execution, recovery and native compatibility

`crates/arc-cli/src/exec.rs:66` sets one 1,800 ms absolute deadline before metadata preparation. `exec/preparation.rs:37` passes that same deadline to REGISTER and CLAIM; `exec/transport.rs:109` retains uncertainty across retries and bounds each attempt to the remaining shared budget. A confirmed REGISTER followed by an outage becomes uncertain; prior sent request bytes preserve uncertainty across endpoint replacement. Cancellation, local setup failures, invalid complete replies and authenticated rejections remain non-launching. `exec.rs:100`–`exec.rs:110` funnels managed and fallback decisions to one `Command::spawn`; cancellation is checked at admission and in the signal handoff. Fallback sends neither ATTACH nor COMPLETE and diagnostics avoid command arguments and environment values.

The unfiltered `cargo test --workspace` run used the separate QA Cargo target and passed **412 tests in 32 harnesses**, with zero failed, ignored or filtered. Its native test receipts used the unmodified CLI `1c8fae62a0d2094f8560dedf88984e76945f8414cbe97d1665b3ad12051fb9b7` and covered byte-preserved argv/cwd/environment, descriptors and streams, exit 17, directed/terminal signals, foreground pipelines, SIGPIPE, full and broken stderr, cancellation, managed lifecycle tracking, protocol-1 diagnostics, idempotency and daemon-loss survival. The LLVM runtime marker was absent in this uninstrumented run. The earlier coverage-instrumented environment observation remains historical below.

The fresh native matrix recorded 23 outage cases, all exit 17, one invocation and one total invocation after daemon recovery. The maximum unadjusted parent-before-wrapper-spawn to child handshake was **1817.616 ms**. The operating system's scheduling is included; startup, metadata and handoff subdivisions were not separately measured. Phase 0 has no separate scheduler stage.

| Case | Parent to child (ms) | Invocations | After recovery |
| --- | ---: | ---: | ---: |
| `absent` | 9.229 | 1 | 1 |
| `busy-claim` | 15.623 | 1 | 1 |
| `busy-register` | 11.634 | 1 | 1 |
| `claim-file` | 13.245 | 1 | 1 |
| `claim-parent` | 14.444 | 1 | 1 |
| `committed-claim` | 16.620 | 1 | 1 |
| `committed-register` | 14.399 | 1 | 1 |
| `connect-access-before-claim` | 14.579 | 1 | 1 |
| `connect-access-before-register` | 10.822 | 1 | 1 |
| `connect-access-claim-ack-loss` | 30.335 | 1 | 1 |
| `connect-access-register-ack-loss` | 17.596 | 1 | 1 |
| `disconnect-before-read` | 12.930 | 1 | 1 |
| `disconnect-claim` | 15.299 | 1 | 1 |
| `disconnect-register` | 11.831 | 1 | 1 |
| `partial-register` | 1817.616 | 1 | 1 |
| `refused` | 9.212 | 1 | 1 |
| `register-file` | 10.560 | 1 | 1 |
| `register-parent` | 10.623 | 1 | 1 |
| `registered-file` | 11.792 | 1 | 1 |
| `registered-parent` | 11.885 | 1 | 1 |
| `shared-budget` | 1813.110 | 1 | 1 |
| `stall-claim` | 1812.142 | 1 | 1 |
| `stall-register` | 1817.270 | 1 | 1 |

An independent proxy delivered requests to a real daemon, observed its successful commit response, then replaced its endpoint with a regular file. It withheld the REGISTER or CLAIM acknowledgement, or forwarded REGISTER before CLAIM:

| Case | Parent to child (ms) | Lifecycle row | Exit | Invocations before / after recovery |
| --- | ---: | --- | ---: | --- |
| `register-ack-lost` | 65.544 | `REGISTERED`; no child identity | 17 | 1 / 1 |
| `claim-ack-lost` | 37.423 | `LAUNCHING`; no child identity | 17 | 1 / 1 |
| `registered-before-claim` | 40.157 | `REGISTERED`; no child identity | 17 | 1 / 1 |

Each replacement case reported unconfirmed tracking, preserved child streams, kept a synthetic secret out of diagnostics, and left permits, GC operations and the sentinel unchanged. A second 13-case cleanup matrix checked one invocation, exit 17, zero decoded cleanup requests, zero permits and GC operations, unchanged resource/sentinel state and no replay after recovery. Its slowest observation, 2,080.324 ms for stalled CLAIM, measures parent-to-wrapper **exit including child runtime**, not preparation or child start; it does not contradict or establish the separate parent-to-child measurement above.

## Gate, coverage and preservation

The independent gate audit validated the current source-bound gate, all 11 selected prerequisite receipts and the SHA-256 of their 22 retained stdout/stderr streams. These were backend task-23 executions, not new QA executions. The recorded format, warning-denying Clippy, workspace, coverage, full harness, preservation, coverage CLI/harness, live coverage and docs commands all exited 0. The live-coverage command followed the final recorded collector; this reviewer reran the live coverage test without collecting coverage.

| Retained prerequisite command | Exit | Elapsed ms |
| --- | ---: | ---: |
| `cargo fmt --all -- --check` | 0 | 666.991 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 607.937 |
| `cargo test --workspace` | 0 | 115085.927 |
| `bun run test:coverage` | 0 | 156169.002 |
| `./scripts/harness/validate.sh` | 0 | 161864.557 |
| `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 0 | 212640.965 |
| `bun test scripts/coverage/clean-checkout.test.ts` | 0 | 176650.580 |
| `bun test scripts/coverage/coverage-cli.test.ts` | 0 | 156585.738 |
| `bun test scripts/coverage/harness.test.ts` | 0 | 165031.378 |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 1401.844 |
| `bun test scripts/harness/exec-fail-open-docs.test.ts` | 0 | 754.644 |

The raw coverage audit parsed LCOV, recomputed 47 production-file unions and checked all 1494 profile hashes and the merge inventory. Coverage run `2026-11-10T00-42-53-732Z-1afbcf42-661c-45e5-8e60-a5651852ed20` has its own source fingerprint `571b999cda9908beda13517f466cd2595e23af3444ffae4bc902a034e8508fce`; this differs from the 242-input gate fingerprint.

| Crate | Covered / total production lines | 80% integer floor |
| --- | ---: | --- |
| arc-core | 247 / 256 | pass |
| arc-detector | 609 / 610 | pass |
| arc-storage | 653 / 674 | pass |
| arc-ipc | 387 / 393 | pass |
| arc-daemon | 1628 / 1746 | pass |
| arc-cli | 968 / 1100 | pass |

The compact container is 1,273,952 bytes, SHA-256 `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`, with 61 selections (49 current, 12 archived). The audit reconstructed all 12 archived selections and required predecessor containers. The indexed evidence count is 149, below 150. Migration `0001_phase0.sql` matches accepted HEAD, index and worktree at SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The independent source audit checked 222 scoped files; the maximum was 483 lines in `scripts/harness/exec-fail-open-compact.test.ts`. The gate reevaluated the preservation/accounting checks and authenticated the retained tamper rejections; this attempt did not rerun the full mutation suite.

The workspace test reran the two fixed-baseline historical routes and the live fallback negative control. `crates/arc-daemon/tests/managed_exec/historical.rs` restricts those routes to the original before-register and after-shutdown assertions; the current live fallback cases use the corrected binary. Gate-bound historical route records remained present after this test and a current snapshot is retained under `target/`.

## Reviewer command receipts

All commands below ran in the workspace above. Each receipt records its executable, PID, host, actual argv, timestamps, source fingerprints, timeout allowance and raw output paths. Hashes are SHA-256; all raw output is under the ignored `target/exec-fail-open/task24-attempt2-independent/` directory. The fault-injection drivers and binaries are review fixtures, not deliverable product code.

| Label and actual command | PID | UTC start | Elapsed ms | Exit | stdout SHA-256 | stderr SHA-256 |
| --- | ---: | --- | ---: | ---: | --- | --- |
| `workspace: cargo test --workspace` | 58645 | 2026-11-10T01:16:07.384708+00:00 | 116065.018 | 0 | `81adbc398c37d87aef72df8c6dfe6b1e20d88227d00086b70731b210cf7da478` | `8da11ff01f83ae90832d004b26139d6d34a321f06e875b21370b52f0370d5753` |
| `lldb-smoke: lldb --batch -o run -- target/exec-fail-open/task24-independent/cargo-target/debug/arcctl --help` | 58820 | 2026-11-10T01:16:13.548355+00:00 | 5779.561 | 1 | `82392acc1b63270269d6ea34f2abca86d241c3b71ee8218a4c9df0e6dababff3` | `55e530fb5ef62edbc2894b0ebed035d4e7ed81eff43205b3373fc845cd5feec8` |
| `replacement-probe: python3 target/exec-fail-open/task24-independent/replacement-probe.py` | 64728 | 2026-11-10T01:18:15.575635+00:00 | 449.991 | 0 | `5d19fcadd0ed22f75553f9821c5c280d52bc81365cef778ea654be91b721fd11` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `decision-probe: python3 target/exec-fail-open/task24-independent/decision-probe.py` | 64750 | 2026-11-10T01:18:16.286850+00:00 | 131.252 | 0 | `c84f6d86b72016a13cde3033659e9419f71efa770e0af9e7170f69ec7fcc754c` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cleanup-matrix: python3 target/exec-fail-open/task24-independent/cleanup-matrix.py` | 64763 | 2026-11-10T01:18:16.718143+00:00 | 9489.206 | 0 | `1266ed9e48ad98f9ea8740169f6477d2239287fab85aef2e8984620ef93e7a92` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `parse-workspace: bun -e 'import {parseCargoExecution} from "./scripts/harness/managed-exec-test-output"; import {readFileSync} from "node:fs"; console.log(JSON.stringify(parseCargoExecution(readFileSync("target/exec-fail-open/task24-attempt2-independent/workspace.stdout","utf8"))));'` | 66587 | 2026-11-10T01:18:46.175722+00:00 | 40.522 | 0 | `1cfa027d8401a8e644683e01c8f5c8a996e0f006d37c2d32683bd55dbd8ce3ab` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `native-audit: python3 target/exec-fail-open/task24-attempt2-independent/native-audit.py` | 66593 | 2026-11-10T01:18:46.509157+00:00 | 125.159 | 0 | `48502279516b82d569c037c8550a5cfee2d5979219715ca287c0e8eb5ad7452d` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `gate-audit: bun target/exec-fail-open/task17-attempt2/gate-audit.ts` | 66599 | 2026-11-10T01:18:46.869556+00:00 | 26097.194 | 0 | `c1709c249162bda79822a2c530bf96b22dbe2ef67882404a28670f7a55ca72e6` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `coverage-audit: python3 target/exec-fail-open/task24-independent/coverage-audit.py` | 67563 | 2026-11-10T01:19:13.369936+00:00 | 554.346 | 0 | `0010c1117d5f42bb6d870271c1e2c0fb017c8e133f538afc7df85a8c74ab4dbd` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `archive-audit: bun target/exec-fail-open/task24-independent/archive-audit.ts` | 67569 | 2026-11-10T01:19:14.135952+00:00 | 46810.064 | 0 | `1bd6d9f8f28b9928edae2afe24db2ed14440f7f84e14634f97403dad056cfc05` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `source-audit: python3 target/exec-fail-open/task24-independent/source-audit.py` | 68488 | 2026-11-10T01:20:01.270883+00:00 | 130.977 | 0 | `e8f61f94cea3edcaec9ac3d586ed502b50abac63d1d7e4a46f62b1fa1dabc52c` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `live-coverage: bun test scripts/coverage/evidence.test.ts` | 68498 | 2026-11-10T01:20:01.613105+00:00 | 4228.236 | 0 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `033b2b383080220a96c74e4ed3c4a2bdbc22efc5b73be4547b9df2dc5327335e` |
| `docs: bun test scripts/harness/exec-fail-open-docs.test.ts` | 68657 | 2026-11-10T01:20:06.094901+00:00 | 510.007 | 0 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `2577b3ce40113387cbdf422979e153d77349b1a6446230dd2499241f809c21c9` |
| `gate-tests: bun test scripts/harness/exec-fail-open-gates.test.ts` | 68666 | 2026-11-10T01:20:06.855570+00:00 | 42233.148 | 0 | `4d0b680e45cc79ce5f482e0ca700ef1afcdd35a32c1b877283c6b5c6405c478e` | `6ce309bcff87ee352d42e6841edb8b46c83d19efcee3fb2dde1c14b6287e4735` |
| `context: bun -e 'import {context,root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(context(root)));'` | 71030 | 2026-11-10T01:20:53.424430+00:00 | 292.106 | 0 | `acf4dd494c56958268b7aa4b7dbe68028abcc4e43c82778fd022389d259f1f81` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `terminal-precheck: bun -e 'import {validateEvidence} from "./scripts/harness/exec-fail-open-evidence"; import {root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(validateEvidence(root)));'` | 71047 | 2026-11-10T01:20:53.970300+00:00 | 21526.415 | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `616bbe0cea04348a66aea47d42b144b5741d161939514463f2874d88f01b4244` |
| `credential-fault: python3 target/exec-fail-open/task24-attempt2-independent/credential-fault.py` | 72784 | 2026-11-10T01:22:36.722418+00:00 | 668.508 | 0 | `289c3a3a89f10cdadb0cc5f9e28238d46addc3cde6937607a7b9f3d057fda683` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `credential-fault-output: python3 target/exec-fail-open/task24-attempt2-independent/credential-fault-output.py` | 72934 | 2026-11-10T01:22:51.737575+00:00 | 562.113 | 0 | `a28c86837bd2a001be2ee57da94811b7fe0c12d42b9620b3962fc049bf78dcd1` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `binary-disassembly: xcrun llvm-objdump --disassemble --start-address=0x10002d21c --stop-address=0x10002d234 target/exec-fail-open/task24-independent/cargo-target/debug/arcctl` | 73067 | 2026-11-10T01:23:05.634074+00:00 | 42.438 | 0 | `9271467519d4b173729e707a583f3c9b97dc3cc5b1808ea3685de24f18bccbc2` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `binary-stubs: otool -Iv target/exec-fail-open/task24-independent/cargo-target/debug/arcctl` | 73073 | 2026-11-10T01:23:05.871910+00:00 | 21.048 | 0 | `6c2e3303570e2a36c31693c2503bb2de48e72fabdd4c8ffc30cd74b6479f698f` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `fault-disassembly: xcrun llvm-objdump --disassemble --start-address=0x10002d21c --stop-address=0x10002d234 target/exec-fail-open/task24-attempt2-independent/arcctl-credential-fault-output` | 73080 | 2026-11-10T01:23:06.091076+00:00 | 22.502 | 0 | `abcf810302d972aa611f4ec8fd1ae750003bbbdbf4c76b514ec4f8f5c32897c6` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `final-context: bun -e 'import {context,root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(context(root)));'` | 73085 | 2026-11-10T01:23:06.308866+00:00 | 456.523 | 0 | `acf4dd494c56958268b7aa4b7dbe68028abcc4e43c82778fd022389d259f1f81` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `review-audit: bun -e 'import {readFileSync} from "node:fs"; import {root,context,hash} from "./scripts/harness/exec-fail-open-inputs"; import {productionRecipe} from "./scripts/harness/exec-fail-open-recipe"; import {validateReviewOutcome} from "./scripts/harness/exec-fail-open-evidence"; const q=JSON.parse(readFileSync(".arc-verify/exec-fail-open/qa/review.json","utf8")); const ctx=context(root); validateReviewOutcome(q); if(q.sourceHash!==ctx.sourceHash || hash(readFileSync(q.report.path))!==q.report.sha256) throw Error("review source or report differs"); const r=productionRecipe(root); const commands=q.commands.map(c=>r.command(c,ctx,"reviewer")); console.log(JSON.stringify({passed:true,sourceHash:ctx.sourceHash,commands:commands.length,workspace:commands[0].execution.passed,reportSha256:q.report.sha256}));'` | 73456 | 2026-11-10T01:24:25.334527+00:00 | 1161.007 | 0 | `6705eedb9969e810de7c0f683cf6dbd10a462418bd2ebe4db1c6e4ee9c05b65d` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

- `target/exec-fail-open/task24-attempt2-independent/run.py` SHA-256 `683ddbd8454098f3cd9180c0a7e8918c3f51ef6b49ce7856bdfc860ab2cd2802`.
- `target/exec-fail-open/task24-attempt2-independent/native-audit.py` SHA-256 `d267111a2e5e991c84fcb497ed08d3c76e9c627189f1a8bf82d73eda608f8b19`.
- `target/exec-fail-open/task24-attempt2-independent/credential-fault.py` SHA-256 `61556ac950efc0148c1240db69dc5b880e6a76473e31449b18f43b7d92936527`.
- `target/exec-fail-open/task24-attempt2-independent/credential-fault-output.py` SHA-256 `12a6c4f6e87fd2c0ec38fa73107fa459edbf3ca8b06e1dc74363b4c68bff0e6d`.
- `target/exec-fail-open/task24-independent/replacement-probe.py` SHA-256 `316f71bfe71adffde1fa98391b827bbb78bd10068cb2b0860c73563ccc009964`.
- `target/exec-fail-open/task24-independent/decision-probe.py` SHA-256 `0048f160750f40a755504c35821892b0c50077107c0e23af7c4bdda272787d1c`.
- `target/exec-fail-open/task24-independent/cleanup-matrix.py` SHA-256 `44160c397a608e23256858da37f543c5c433f97c4bedc242f31285708c6dabcb`.
- `target/exec-fail-open/task17-attempt2/gate-audit.ts` SHA-256 `aeb6db3492737f807bf9f3b2acbcfead7493bef5d55fec85e97fa43623a4a031`.
- `target/exec-fail-open/task24-independent/coverage-audit.py` SHA-256 `7bbb8273d63dcd188baa504e044897a46ef2b9149db17e6e4492f94efa898b22`.
- `target/exec-fail-open/task24-independent/archive-audit.ts` SHA-256 `e9e7bb76fdc27e6dca2596683a5cef881fa156d94a129df8834361f27ae6a7e6`.
- `target/exec-fail-open/task24-independent/source-audit.py` SHA-256 `f28153b9f3d407a4b32a5444646839ea742723450e787d982bd2c7715abdb76c`.

The `review-audit` command validated the provisional report binding and its four canonical reviewer command receipts before this final report added the audit receipt. The final Markdown/JSON binding was checked again after writing.

The `lldb-smoke` exit 1 was a tooling limitation: macOS denied debugger attachment. The later fault-injected binary copy provided the requested simulated execution without debugger attachment. A one-off stdout summarizer initially raised `KeyError: passed` because the cleanup matrix has per-case results instead of a top-level `passed` field; a corrected summarizer and explicit per-case assertions succeeded. Neither failure is a product defect.

The `terminal-precheck` exit 1 reports `EXEC_FAIL_OPEN: stale receipt inputs/git` because `final.json` still binds the earlier source and records the task-21 blocked decision. That decision is historical, not a current finding against the corrected implementation. The Chief owns updating final and terminal evidence, executing the unchanged terminal/framework checks, and determining mission acceptance. This review did not edit those artifacts.

## Findings and limits

- CRITICAL / HIGH / MEDIUM: none reproduced within this review scope.
- A real foreign-user socket was not exercised. UID mismatch and credential errors were injected; the full spawn-path observation used a same-user peer and an altered binary copy, as described above.
- Desktop integration, native Windows acceptance, vendor hooks and scheduling remain outstanding. Adoption, revenue and user outcomes are unmeasured.
- Serena code-intelligence tools were not available; scoped reads of already identified paths and source-bound Git/test evidence were used. No browser or GUI was used.

## Historical QA retained verbatim

The following is the preceding QA report, preserved byte for byte. Its conclusions and timestamps are historical; the current result is above.

# Independent QA — task-24 corrected source

**Review result: PASS for this independent QA scope; verification complete.** No reproducible CRITICAL, HIGH or MEDIUM product finding was found. This is not final mission acceptance: the Chief must update the stale final decision and terminal evidence, then run the unchanged framework verification and final review.

Reviewer: `reviewer`; skill: `oma-qa`. Accepted HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`; source fingerprint `bd81c863144d1dd2b149122fa47299c60f503064eda68262fa1d464c08212fdb` over 242 inputs before and after each recorded command. Gate SHA-256 `4493bb1b91d970a5ee77b86e2c436b020f22220e039e59bd726ad6fde736bbcd`. Review completed at 2026-11-10T01:12:09.756413+00:00.

Workspace and cwd: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Host: `Eunkwangui-MacBookPro.local`, Darwin 27.2.0 arm64; Rust host `aarch64-apple-darwin`. Cargo cargo 1.95.0 (f2d3ce0bd 2026-03-21); rustc rustc 1.95.0 (59807616e 2026-04-14); Bun 1.4.4. At admission, HEAD and workspace matched the assignment; no unstaged tracked change was observed outside `av.yaml`, which Git reported `Operation not permitted`. The previous QA receipt recorded an absolute deadline of 2026-11-10T20:40:08.584Z; at 01:04:23Z approximately 70,545.584 seconds remained. No cancellation notification was received; live controller cancellation state was not independently available. The shell denied `ps` with `operation not permitted`; no workaround was used. The attempt budget was 7,200 seconds, the configured maximum parallelism was three, and 28 GiB was available. This review did not start a coverage collector; other process activity could not be inspected after the shell denied `ps`.

Prior QA files were copied before modification, without changing their bytes:
- `target/exec-fail-open/task24-independent/prior-review.md`: SHA-256 `74b799f9bd892262d06d32eaff6e31247077a4b0507b02ba7b35883162a6f75b` (156621 bytes).
- `target/exec-fail-open/task24-independent/prior-review.json`: SHA-256 `74584b6628cb308750d48d202b2414389f7e99ea8d40a211462dd3ffa1eba996` (312000 bytes).
The complete prior Markdown follows this review verbatim, including the task-12 preliminary section and task-17/task-20 findings. Those earlier current-source claims refer to their then-current fingerprints, not this corrected source.

## Review of execution decisions

- `crates/arc-cli/src/exec.rs:66` validates argv, sets one 1,800 ms absolute preparation deadline and passes it through REGISTER and CLAIM; `exec/preparation.rs:37` maps unavailable, uncertain and DB_BUSY exhaustion to fallback. `exec/transport.rs:109` retains sent-request uncertainty across retries and uses bounded 600 ms attempts with the same outer deadline; `exec/transport.rs:286` polls in at most 25 ms slices. No scheduler stage exists before spawn. The measured boundary is parent-before-wrapper-spawn to child handshake, with no subtraction; startup, metadata and handoff subdivisions are unmeasured.
- `exec.rs:100`–`exec.rs:110` has one `Command::spawn` expression shared by managed and fallback states. `exec/preparation.rs:168` checks cancellation at admission; `signals.rs` also handles the admission-to-fork interval. Invalid arguments, initial invalid socket paths, local setup failure, invalid complete replies, explicit same-user authenticated rejection and cancellation remain non-launching. Initial ENOTSOCK/ENOTDIR are invalid configuration under the operator decision; replacement after a sent request or confirmed registration remains uncertain and launches once.
- `exec/transport.rs:274`–`exec/transport.rs:283` obtains and verifies peer credentials before exchange transmits lifecycle bytes; `exec/transport/peer.rs:15` uses macOS `getpeereid` and compares effective UIDs. The corrected-source unit tests observed same-user credentials on a real socket pair and injected PermissionDenied, Other and Unsupported credential failures before transmission. They observed zero bytes sent to an untrusted connection and classified it unavailable, or uncertain after a prior sent request. This is simulated credential rejection, not a real foreign-user socket test. A separate native same-user socket probe returned explicit authenticated rejection and launched no child.
- Fallback diagnostics in `exec/preparation.rs:98`–`exec/preparation.rs:106` report unavailable or unconfirmed tracking only after spawn, without printing command or environment secrets. Fallback sends no ATTACH or COMPLETE; native tests checked zero permits, zero GC operations, unchanged sentinel resources and one invocation after daemon restart. A full or closed stderr cannot guarantee delivery of a diagnostic; the tests show it does not block supervision or change the child outcome.

## Independent native observations

The independent workspace run executed 412 tests in 32 harnesses, with zero failures, ignored or filtered tests. Its ordinary uninstrumented candidate SHA-256 is `1c8fae62a0d2094f8560dedf88984e76945f8414cbe97d1665b3ad12051fb9b7`. Its 23 native timing records are under `target/exec-fail-open/native/50875/`; the audit hashed each record, checked the candidate binary, source-window timestamps, exit 17, one invocation and one invocation after recovery. Maximum unadjusted parent-to-child handshake: **1820.073 ms**. Scheduling overhead is not a separate Phase 0 stage; the individual timing subdivisions were not measured.

| Native case | Parent to child, ms | Invocations | After recovery |
| --- | ---: | ---: | ---: |
| `absent` | 8.171 | 1 | 1 |
| `busy-claim` | 15.128 | 1 | 1 |
| `busy-register` | 14.053 | 1 | 1 |
| `claim-file` | 10.814 | 1 | 1 |
| `claim-parent` | 12.593 | 1 | 1 |
| `committed-claim` | 18.652 | 1 | 1 |
| `committed-register` | 16.224 | 1 | 1 |
| `connect-access-before-claim` | 14.947 | 1 | 1 |
| `connect-access-before-register` | 11.411 | 1 | 1 |
| `connect-access-claim-ack-loss` | 15.990 | 1 | 1 |
| `connect-access-register-ack-loss` | 12.618 | 1 | 1 |
| `disconnect-before-read` | 11.714 | 1 | 1 |
| `disconnect-claim` | 17.020 | 1 | 1 |
| `disconnect-register` | 14.122 | 1 | 1 |
| `partial-register` | 1820.045 | 1 | 1 |
| `refused` | 9.004 | 1 | 1 |
| `register-file` | 10.872 | 1 | 1 |
| `register-parent` | 10.935 | 1 | 1 |
| `registered-file` | 10.629 | 1 | 1 |
| `registered-parent` | 10.969 | 1 | 1 |
| `shared-budget` | 1817.804 | 1 | 1 |
| `stall-claim` | 1817.885 | 1 | 1 |
| `stall-register` | 1820.073 | 1 | 1 |

The committed REGISTER and CLAIM cases in the test matrix each withheld three successful real-daemon replies, inspected the resulting SQLite lifecycle row and checked no child identity was attached. The independent replacement probe used a separate Python driver and the QA Cargo target:

| Real daemon commit and replacement | Parent to child, ms | Exit | Calls before / after recovery |
| --- | ---: | ---: | --- |
| `register-ack-lost` | 41.515 | 17 | 1 / 1 |
| `claim-ack-lost` | 37.271 | 17 | 1 / 1 |
| `registered-before-claim` | 34.905 | 17 | 1 / 1 |

For these three cases the proxy observed an `ok` response from the real daemon, replaced the socket with a regular file, then either withheld REGISTER/CLAIM acknowledgement or forwarded REGISTER before the attempted CLAIM. SQLite showed REGISTERED or LAUNCHING as appropriate, no child identity, no permits and no GC operations. The sentinel remained unchanged, stderr identified uncertain tracking, the synthetic secret did not appear in diagnostics, and restart/status did not replay the command. These endpoint replacement checks are separate from the initial invalid-socket checks.

The separate 13-case cleanup matrix covered absent, refused, disconnected, stalled, partial, committed, shared-budget and DB_BUSY endpoints. Each exited 17 with one invocation before and after recovery and no reservation, cleanup operation or sentinel change. Its times are parent-to-wrapper-exit, not child-start measurements; the largest was 1856.337 ms. REGISTER/CLAIM committed cases each observed three real successful replies withheld. `replacement-probe.py`, `cleanup-matrix.py` and `decision-probe.py` are retained in the ignored task-24 directory with the raw output and the actual command receipts below.

The unfiltered native suite also passed byte-for-byte direct-versus-wrapped argv, cwd, synthetic environment, stdin/stdout/stderr and inherited descriptor checks; exit 17; terminal and directed signals; macOS foreground pipelines, stop/continue and SIGPIPE; full and broken stderr; cancellation near the deadline; managed status/completion, daemon shutdown/crash survival, protocol-1 diagnostics and the fixed-baseline negative control. `fallback_environment.rs` covers 13 outage classes and tests that added, removed and changed environment bytes fail comparison. The ordinary current run reported the LLVM runtime marker absent. The older coverage-instrumented unseeded-runtime observation remains in the historical section and is not a fresh corrected-source result.

## Gate, preservation and coverage review

The maintained current-source gate and an independent audit authenticated all 11 prerequisite command receipts and their 22 raw output hashes, the current timing, historical, preservation and coverage dependencies. Those are backend task-23 executions, not reviewer reruns. Their actual cwd, executable identities, timestamps, commands and hashes are listed in `target/exec-fail-open/task24-independent/gate-audit.stdout`. The recorded format, warning-denying Clippy, workspace test, `bun run test:coverage`, full `./scripts/harness/validate.sh` and subsequent required checks exited 0; the last recorded collector ended before the recorded live-coverage check. The reviewer independently reran the live-coverage, gate and docs tests as listed below.

The reviewer parsed the selected raw LCOV and independently recomputed 47 production-file unions and six integer floors, then hashed all 1494 raw profiles and checked the merge inventory, LCOV and LLVM exports. Coverage run `2026-11-10T00-42-53-732Z-1afbcf42-661c-45e5-8e60-a5651852ed20` has its own coverage-source fingerprint `571b999cda9908beda13517f466cd2595e23af3444ffae4bc902a034e8508fce`, distinct from the 242-input gate fingerprint:

| Crate | Covered / total production lines | Integer floor |
| --- | ---: | --- |
| arc-core | 247 / 256 | pass |
| arc-detector | 609 / 610 | pass |
| arc-storage | 653 / 674 | pass |
| arc-ipc | 387 / 393 | pass |
| arc-daemon | 1628 / 1746 | pass |
| arc-cli | 968 / 1100 | pass |

LCOV SHA-256 `263aeb40f29484b9472a924fc76317e89a52f48479f100307889cece8a2bb54a`; LLVM export SHA-256 `8aabab065224764ee377eee3d85c296193c68ab59b3f20c3856b42eb019afa81`. The subsequent independent `bun test scripts/coverage/evidence.test.ts` passed without running a collector.

The current compact container is 1,273,952 bytes, SHA-256 `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`, with 61 authenticated selections (49 current, 12 archived) and all 11 selected command receipts. The archive audit reconstructed and hashed all 12 archive selections and nested containers, including the required persistent4 and attempt13 predecessors. The indexed evidence count is 149, below 150. The migration's worktree, index and accepted HEAD bytes all have SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. Independent line counting covered 222 scoped source files; the maximum was 483 lines in `scripts/harness/exec-fail-open-compact.test.ts`. The maintained preservation check authenticated accounting, migration and historical test protections. The accepted task-23 preservation receipt records the mutation tests; this reviewer did not rerun that full protection suite.

The two incompatible historical assertion bodies ran against the fixed baseline binary, with routes restricted to before-register and after-shutdown. The current workspace run observed both routes and the historical binary failed the live fallback negative control as expected. The route log is append-only; both gate-bound route observations remained present after the review run and a current snapshot is retained under `target/exec-fail-open/task24-independent/routes-after-workspace.jsonl`.

## Reviewer command receipts

All commands ran from the workspace above with a 7,200,000 ms per-check allowance and the recorded absolute deadline. Stdout and stderr hashes are SHA-256; raw streams and complete per-command metadata are in the ignored task-24 directory. `cargo test` used only the separate QA Cargo target. Audit and probe scripts are retained there with their hashes below.

| Command / receipt label | PID | UTC start | Elapsed ms | Exit | stdout SHA-256 | stderr SHA-256 |
| --- | ---: | --- | ---: | ---: | --- | --- |
| `cargo test --workspace` (workspace) | 48558 | 2026-11-10T01:05:04.900830+00:00 | 137246.391 | 0 | `c054e62e0a1d173a08bc0e778f4174b1bebcc0a04c56ca35b648b8173dc5f6a0` | `a9887339e713ea046df71e6e3abf97be27059d543eff61ecfab257d998ecdce7` |
| `bun target/exec-fail-open/task17-attempt2/gate-audit.ts` (gate-audit) | 52544 | 2026-11-10T01:07:41.017698+00:00 | 20121.898 | 0 | `c1709c249162bda79822a2c530bf96b22dbe2ef67882404a28670f7a55ca72e6` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task24-independent/coverage-audit.py` (coverage-audit) | 52543 | 2026-11-10T01:07:41.017465+00:00 | 1114.698 | 0 | `0010c1117d5f42bb6d870271c1e2c0fb017c8e133f538afc7df85a8c74ab4dbd` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun test scripts/coverage/evidence.test.ts` (live-coverage) | 53131 | 2026-11-10T01:07:59.965737+00:00 | 2030.431 | 0 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `e35001331b99c719d9c985e2d89d5708098f5261235eec2aa9af37675ed8f910` |
| `python3 target/exec-fail-open/task24-independent/replacement-probe.py` (replacement-probe) | 53174 | 2026-11-10T01:08:07.292464+00:00 | 380.428 | 0 | `4a0528e02783afb6465db220bc74cf2012e8e9717bb7430f851da05a7d0cb2b1` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task24-independent/cleanup-matrix.py` (cleanup-matrix) | 53197 | 2026-11-10T01:08:09.386158+00:00 | 8983.452 | 0 | `ec60e9f557ec83a7b8d290ee452eb7101071ca28584488998c94357d550a564f` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task24-independent/decision-probe.py` (decision-probe) | 55702 | 2026-11-10T01:09:26.471894+00:00 | 198.309 | 0 | `ddcf425563cf0fe8eb7d79fd686eebc44c6bacfc79496cd1a720d33f0e23f061` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun test scripts/harness/exec-fail-open-gates.test.ts` (gate-tests) | 53316 | 2026-11-10T01:08:35.377726+00:00 | 49058.030 | 0 | `4d0b680e45cc79ce5f482e0ca700ef1afcdd35a32c1b877283c6b5c6405c478e` | `27efdbbb5a14db472e3129bb5030e243c2237d6b6ac7f4c632b22ad81ce69fa1` |
| `bun test scripts/harness/exec-fail-open-docs.test.ts` (docs) | 53317 | 2026-11-10T01:08:35.377822+00:00 | 814.476 | 0 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `4bf3cab2ca5964df636bfa93f500813f056bc61852978e696c29f04365cc1724` |
| `bun target/exec-fail-open/task24-independent/archive-audit.ts` (archive-audit) | 53897 | 2026-11-10T01:08:53.714450+00:00 | 40308.597 | 0 | `1bd6d9f8f28b9928edae2afe24db2ed14440f7f84e14634f97403dad056cfc05` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task24-independent/source-audit.py` (source-audit) | 54050 | 2026-11-10T01:08:59.464474+00:00 | 147.154 | 0 | `e8f61f94cea3edcaec9ac3d586ed502b50abac63d1d7e4a46f62b1fa1dabc52c` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task24-independent/native-audit.py` (native-audit) | 55732 | 2026-11-10T01:09:47.659478+00:00 | 90.530 | 0 | `061e40049808b346bb4a83d5f679a578617e0b8dcbabeb46b9be22d5c7e87748` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun -e import {context,root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(context(root)));` (context) | 55810 | 2026-11-10T01:10:17.527514+00:00 | 307.382 | 0 | `acf4dd494c56958268b7aa4b7dbe68028abcc4e43c82778fd022389d259f1f81` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun -e import {parseCargoExecution} from "./scripts/harness/managed-exec-test-output"; import {readFileSync} from "node:fs"; console.log(JSON.stringify(parseCargoExecution(readFileSync("target/exec-fail-open/task24-independent/workspace.stdout","utf8"))));` (parse-workspace) | 55873 | 2026-11-10T01:10:20.734769+00:00 | 21.906 | 0 | `0d67919454f830454d32ba849dea8e962aff0d34283fad67c6fce80ec9fbdc16` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun -e import {validateEvidence} from "./scripts/harness/exec-fail-open-evidence"; import {root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(validateEvidence(root)));` (preqa-terminal-check) | 55787 | 2026-11-10T01:10:12.847462+00:00 | 19806.670 | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `616bbe0cea04348a66aea47d42b144b5741d161939514463f2874d88f01b4244` |

- `target/exec-fail-open/task24-independent/run.py` SHA-256 `c4f4ad8b59964b01a0c5f8a1388086c537a6b573d56d2dda391edbd2364a5125`.
- `target/exec-fail-open/task24-independent/replacement-probe.py` SHA-256 `316f71bfe71adffde1fa98391b827bbb78bd10068cb2b0860c73563ccc009964`.
- `target/exec-fail-open/task24-independent/cleanup-matrix.py` SHA-256 `44160c397a608e23256858da37f543c5c433f97c4bedc242f31285708c6dabcb`.
- `target/exec-fail-open/task24-independent/decision-probe.py` SHA-256 `0048f160750f40a755504c35821892b0c50077107c0e23af7c4bdda272787d1c`.
- `target/exec-fail-open/task24-independent/archive-audit.ts` SHA-256 `e9e7bb76fdc27e6dca2596683a5cef881fa156d94a129df8834361f27ae6a7e6`.
- `target/exec-fail-open/task24-independent/coverage-audit.py` SHA-256 `7bbb8273d63dcd188baa504e044897a46ef2b9149db17e6e4492f94efa898b22`.
- `target/exec-fail-open/task24-independent/source-audit.py` SHA-256 `f28153b9f3d407a4b32a5444646839ea742723450e787d982bd2c7715abdb76c`.
- `target/exec-fail-open/task24-independent/native-audit.py` SHA-256 `5bb6df0e7acb4be09c5d0c788ea17e97c33e6c18c558be81d4100bdb7fc40e16`.

The `preqa-terminal-check` exit 1 is a downstream evidence prerequisite, not a product defect: the existing `final.json` is still bound to the earlier `ec4e...` source and says `blocked` with two historical blockers. The validator stopped at `EXEC_FAIL_OPEN: stale receipt inputs/git` (`scripts/harness/exec-fail-open-evidence.ts:101`). The corrected source and fresh reproductions above address the old peer-authentication and endpoint-replacement findings. The Chief owns rebinding the final/terminal artifacts to this fresh QA, running the terminal evidence check and framework verification, and determining final acceptance. This reviewer did not change those files.

## Findings and limits

- CRITICAL / HIGH / MEDIUM: none reproduced in this review.
- Unmeasured: an actual foreign-user socket was not created; peer credential errors and foreign UID comparison were injected. The endpoint behavior in that environment remains a residual test limit, not a claimed observation.
- Desktop, native Windows, vendor hooks and scheduling remain outstanding. Adoption, revenue and user outcomes are unmeasured.
- Serena code-intelligence tools were not available in this session; the review used scoped reads of known paths and the maintained source-bound Git and test evidence, without a broad native project-code search.

## Historical QA retained verbatim

The following is the previous report, preserved byte for byte. Its dates, findings and claims are historical and are not a fresh review of the corrected source.

# Independent QA — task-20 follow-up

**Direct current-source QA: PASS; verification completeness: completed. QA17-1: resolved.** No new product defect was found in this focused reassessment. The Chief's final decision, terminal evidence, framework verification and final review remain pending; this QA result is not mission acceptance. The current `final.json` is still blocked on the prior QA state. The Chief owns rebinding it and rerunning the unchanged terminal checks.

Reviewer: `reviewer`; skill: `oma-qa`. Baseline HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`. The 235-input source fingerprint before and after both independent reruns is `ec4e258a4ad8f42f59fd67bd92e7733a907e7afadfc386ac5a3e90bc9bc9a663`. Gate SHA-256 is `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7`. Host: Darwin 27.2.0 arm64, `Eunkwangui-MacBookPro.local`; Bun 1.4.4 at `/Users/gracefullight/.local/share/mise/installs/bun/1.4.4/bin/bun`, SHA-256 `0a2237cdcfa0d16ef501d9acca9f0026309ef348bea7393a64e8d1e3609565cc`. Working directory: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`.

## QA17-1 resolution

The operator recorded the shadow move at 2026-11-09T21:49:53.679Z. The former `target/exec-fail-open/task19/shadow-workspace` was absent at admission and after the checks. The Chief's separate post-move runs passed: docs PID 9387 at 2026-11-09T21:50:32.053039+00:00, 769.524 ms; live coverage PID 9386 at 2026-11-09T21:50:32.053120+00:00, 2219.402 ms. Both exited 0. Their actual argv, cwd, timestamps, stdout/stderr hashes, Bun executable identity and before/after source and Git bindings were checked against `target/exec-fail-open/task18-attempt4/docs.json` and `live-coverage.json`; they are Chief runs, not reviewer runs. Audit: `target/exec-fail-open/task20-followup/retained-audit.json`, SHA-256 `6c1e2ff86cbf36e4c12b085a46785d6121ae0ae2c4792db9d83ec62d142378e2`.

The reviewer then ran these unfiltered commands **serially**, with a 7,200,000 ms allowance per check and no coverage collector:

| Actual argv | PID | UTC start – end | Elapsed ms | Result | stderr SHA-256 |
| --- | ---: | --- | ---: | --- | --- |
| `bun test scripts/harness/exec-fail-open-docs.test.ts` | 63582 | 2026-11-09T22:14:12.251879+00:00 – 2026-11-09T22:14:14.783920+00:00 | 2532.021 | exit 0; 2 pass, 0 fail | `4ef8bd39bce6a31c91d6bfa83a31832a938a70ff837bc7f4d07886da763afa7b` |
| `bun test scripts/coverage/evidence.test.ts` | 63801 | 2026-11-09T22:14:17.944062+00:00 – 2026-11-09T22:14:19.969178+00:00 | 2025.096 | exit 0; 1 pass, 0 fail | `ce0d7a4720b83221b3f79e3d984a8b419aad29d67e8cc81627c1684402c64cc5` |

Each stdout SHA-256 is `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47`. Complete process receipts, logs, the runner and source/Git bindings are under `target/exec-fail-open/task20-followup/` as `docs.json`, `live-coverage.json` and matching `.stdout`/`.stderr` files. The receipts are also in the schema-validated `commands` array; the prior independent workspace command remains unchanged in that array.

The earlier task-17 exact docs and live-coverage commands did fail while Bun discovered shadow copies; their original outputs, hashes, and finding remain verbatim below as historical observations. The separate absolute-path diagnostic and targeted fixture run do not substitute for the unchanged commands above.

## Binding and remaining work

The maintained gate validator and an independent follow-up audit passed on the same source: all 15 QA predecessor references, all 11 selected prerequisite receipts and their 22 raw stream hashes, current input hashes and modes, six coverage floors, and 17 timing cases were checked. Candidate `target/debug/arcctl` SHA-256 `c857ff1cc184f534d4a85e49a85ccde5d9eb071bca43dfdb2ee707b4d0272c87` matches the native timing cases and the retained task-17 native drivers. The task-17 independent `cargo test --workspace` receipt validates in its original scope: 395 passed, 0 failed or ignored, with no filtered tests. The retained native, recovery and environment observation outputs and their recorded driver hashes were checked. The unseeded instrumented binary's added `__LLVM_PROFILE_RT_INIT_ONCE` marker remains an instrumentation-environment observation; the ordinary binary preserved the tested environment. Gate audit: `target/exec-fail-open/task20-followup/gate-audit.stdout`, SHA-256 `2138ef8c1d07a8e5705025fca7c7567b6e56fbbc8345e8747c6cb413a919c3e2`. The separate retained-reproduction audit checked 28 process metadata records and 56 raw output stream hashes (`target/exec-fail-open/task20-followup/all-reproductions-audit.json`, SHA-256 `51902b66e525db0c7a6d87b7dcd069c233b5bf6e5c9b9c5ad6cd63bd19d4b4d3`). Its first attempt exited 1 because older metadata does not contain `stdoutPath`; the corrected audit used the established label-based log filenames and passed. The 52 direct retained QA path/hash pairs also matched; the staged evidence inventory is 149 files, below 150.

The first immediate post-update inventory audit exited 1 because the staged QA JSON had not yet caught up with the worktree. After index synchronization, the same audit passed for all 149 files; no non-owned indexed evidence changed. A transcription of the initial process output is retained at `target/exec-fail-open/task20-followup/post-audit-initial-failure.txt`.

The Chief's later `bun test scripts/harness/exec-fail-open-evidence.test.ts` exited 1 with 21 passing tests and one failure, `EXEC_FAIL_OPEN: invalid final decision or gate binding`; its receipt and stderr remain at `target/exec-fail-open/task18-attempt4/terminal-evidence.json` and `.stderr` (stderr SHA-256 `3ab2ba5a418067d533dc0d54e109a6784e2baef81d8aaa093d06c56864c99da8`). It ran against the blocked final decision and is not represented as passing. After the independent review accepts this handoff, the Chief must rebind final/terminal evidence and run the required terminal and framework verification. Desktop, native Windows, vendor hooks and scheduling remain outstanding; usability and commercial outcomes remain unmeasured.

No cancellation notification was received. The original mission deadline is 2026-11-10T20:40:08.584000+00:00; each new check was admitted with its full allowance before that deadline. QA changed no product code, implementation tests, validators, contracts, gate evidence, migration, accounting, compact collection or final artifacts. This update owns only the existing QA report and JSON plus ignored observations.

## Preserved task-17 and earlier records

The following report bytes are preserved verbatim as historical context, including the task-12 preliminary section and earlier failed observations. The pre-update Markdown SHA-256 was `345abc0f33681ec0ebbbf3a5ef78ed75bf16da2b0710d232b31876aaad3036c6` and JSON SHA-256 was `bab7dc894325840e0ccb6391922f35670de3f0de200ce6550e92694e9b949f46`; exact copies are under `target/exec-fail-open/task20-followup/prior-review.md` and `prior-review.json`.

---

# Independent QA - task-17 (corrected source)

## Task-17 attempt 2: independent correction review

**QA result: verification blocked; report complete with a reproducible harness blocker.** This is not final mission acceptance. The current required Bun docs and live-coverage commands fail because they also discover an incomplete copied shadow workspace. The production docs tests and production live-coverage test passed inside those commands, but their overall exit codes were 1. Task-19/backend evidence or the harness owner must repair the discovery issue and the Chief must coordinate the required reruns and terminal rebinding.

The baseline is `00773c69a8e5e7fd417fd5cc78d03e222364f609`; current 235-input product-source fingerprint is `ec4e258a4ad8f42f59fd67bd92e7733a907e7afadfc386ac5a3e90bc9bc9a663` before and after the checks, and selected gate SHA-256 is `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7`. Host: Darwin 27.2 arm64. All commands ran from `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace` with a 7,200,000 ms per-check allowance subject to the mission deadline. The supplied current 172800-second mission duration, applied to the previously recorded start, yields 2026-11-10T20:40:08.584Z; the earlier attempt used the then-recorded 24-hour limit. No cancellation signal was observed. Run and round budgets are framework-controlled per the operator note. Skill: `oma-qa`. Only the existing QA Markdown and JSON are modified; the detailed reproduction files and a separate Cargo target are ignored under `target/exec-fail-open/task17-attempt2/`. No tracked evidence path was added.

After an earlier report edit, both QA files showed worktree changes relative to staged versions (`AM`); a later inspection showed `A`, indicating the index had synchronized at that point. Integration must bind the delivered bytes before terminal verification.

### HIGH — shadow tests break required verification

`target/exec-fail-open/task19/shadow-proof.ts:4-7` copies `scripts/harness` and `scripts/coverage` including tests into an incomplete shadow checkout. `bun test scripts/harness/exec-fail-open-docs.test.ts` produced 2 intended passes and 2 shadow failures because the shadow lacks `crates/arc-cli/src/main.rs`. `bun test scripts/coverage/evidence.test.ts` produced 1 intended pass and 1 shadow failure because the shadow lacks `.arc-verify/rust-coverage/baseline/latest.json`. Both exact commands exited 1. See the `docs-exact` and `live-coverage-exact` receipts below. This is an integrated-workspace verification blocker, not proof of a product failure.

Owner: task-19/backend evidence or harness owner. For future shadow copies, one concrete correction is `cpSync(source, destination, { recursive: true, filter: path => !path.endsWith(".test.ts") })`; preserve the existing shadow test bytes outside Bun discovery, rebind any affected references, then rerun the original commands and downstream verification without changing them. QA did not edit the shadow, implementation, tests, validator, collection or contract.

### Reproduced behavior and evidence

- The fresh unfiltered `cargo test --workspace`, with offline mode and an isolated Cargo target, passed **395 tests across 32 harnesses**, with zero failed, ignored or filtered, in 122723.1 ms. Gate candidate and daemon hashes were unchanged across the run. It exercises live outage, cancellation, malformed reply, explicit rejection, native stream/descriptor/argv/cwd, signal and foreground-pipeline cases, both fixed-baseline routes and the historical negative control. The earlier task-17 run remains separate evidence, not the basis of this fresh count.
- The independent native probe matched direct execution for default SIGPIPE (signal 13) and ignored SIGPIPE (exit 17), each with one invocation. For unavailable and real committed-REGISTER uncertainty, the full stderr reader stayed open and undrained through exit; child `F_GETFL` and descriptor flags matched direct execution. The committed case observed two lifecycle rows and six withheld successful daemon acknowledgements across its ordinary and full-stderr invocations. Ordinary readable diagnostics were truthful; broken stderr preserved exit 17 and one invocation without claiming unreadable output. Its driver, proxy helper and native binary hashes are in `native-independent-meta.json` and its case records in `native-independent.json`.
- The 13-mode recovery probe observed one child before and after daemon recovery, exit 17, zero permits and GC operations, unchanged seeded resource/file state and no cleanup request. The environment matrix had 26 direct-versus-wrapped complete synthetic byte comparisons across ordinary and instrumented binaries, with 16 negative controls rejected. **Every instrumented matrix input was pre-seeded with `__LLVM_PROFILE_RT_INIT_ONCE`; those matches do not establish unseeded instrumented equivalence.**
- A fresh unseeded comparison produced equal raw bytes for the ordinary binary. For the coverage-instrumented binary it produced a real mismatch: the wrapped child received the extra `__LLVM_PROFILE_RT_INIT_ONCE` entry; the driver exited 3 to record that mismatch. Seeding that same key in both inputs restored equality. Each direct and wrapped command exited 17 and ran once. A separate retained standalone probe with no ARC code also added the same key only when instrumented, supporting attribution to the profiling runtime. This is an instrumented-build limitation; the result is not described as a mismatch in the ordinary binary. The initial unseeded probe exited 1 because its Unix socket pathname was too long; the corrected v2 used a short temporary socket path. Both attempts and bound drivers are retained.
- The gate audit revalidated the current gate, eleven prerequisite receipts and all 22 selected raw output hashes; its validator also reconciled LLVM against LCOV and verifies live coverage ordering after the last recorded collector. The independent LCOV audit reconciled 159 records, 46 production files, all 1,457 profile hashes and merge entries. Integer 80% floors: core 247/256, detector 609/610, storage 653/674, IPC 387/393, daemon 1628/1746 and CLI 933/1065. The current exact live test nevertheless exits 1 as described above.
- The compact archive is 1243746 bytes with 43 current and 12 archived selections; the predecessor and original archived bytes were reauthenticated. Migration 0001 matches SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in HEAD, index and worktree. Across 212 tracked crate/script source files the maximum is 483 lines. The staged evidence inventory has 149 files, below 150.
- Correction of the earlier timing claim: the first scoped fixture run ended at 20:36:44 UTC, before the preceding live QA files were written at 20:39:12 UTC. It did not prove post-update behavior. After updating the live QA files in this attempt, the absolute-path scoped run passed all three transition, repeated-rotation and nested-archive tests. A second run explicitly bound both changed live QA files before and after and also passed; it did not substitute for an unfiltered verification command. At that run the bound Markdown and JSON hashes were `43ba66fb10bdd837c53743d4cef5179bcb6aa4b6783185055a8317199e0d574b` and `c8f1bfcb83aae763c69ba2512defba9fae9cdafa4d07201a5393a837722f02ac`. The report was subsequently updated to record the results; those hashes describe the inputs actually seen by the fixture, not the final report bytes.
- The old task-6 Markdown and JSON remain archived in `target/exec-fail-open/task17-independent-vs3qytnv/prior-review.md` and `prior-review.json` with SHA-256 `ded9cd8ef940dc46081ecedf6f3cc2659926d7e7a9391b812db4839d93d4b1a7` and `c0b7cfb81f1de855436cab07ea32817bb2cc41e919a2c282451e013c9bc17dd4`. The historical report section below, including the task-12 preliminary section, is retained byte-for-byte; those reports do not test this corrected source.
- Contract, design and claims artifacts were consumed within their original scopes. The earlier claims wording concern was addressed in current help/docs; neither usability nor commercial results are measured. Desktop, native Windows, vendor hooks and scheduling remain outstanding.

### Attempt-2 command receipts

All logs, exact argv, UTC timestamps, process IDs, cwd, elapsed time, exits, stdout/stderr hashes, and bound driver/binary hashes before and after execution are in `target/exec-fail-open/task17-attempt2/<label>-meta.json` and the matching logs. The runner itself is hash-bound in each receipt. The full independent procedures are retained as `coverage-audit.py`, `gate-audit.ts`, `archive-audit.ts`, `preservation-audit.py` and the named native/environment drivers in that directory.

| Label | Scope | Exit | Elapsed ms | stderr SHA-256 |
| --- | --- | ---: | ---: | --- |
| `fixture-after-live-qa` | scoped archive and transition fixture after live QA update | 0 | 47357.131 | `aab508f44f38f50544f1b46f54257e433e62f7946b3e51dd69cbc33329ae1929` |
| `fixture-bound-live-qa` | same scoped fixture with live QA files hash-bound | 0 | 48187.826 | `7a60d051d0bd7009097d41cee290c414822cb68ba72686aba606cae7d4f08300` |
| `environment-unseeded` | unseeded probe first attempt (local socket path too long) | 1 | 299.792 | `df14e8175d0f89be61a88af7d3eda2011afd6dd18a1cd9d7dd259010638279a9` |
| `environment-unseeded-v2` | ordinary and instrumented unseeded environment comparison | 3 | 758.98 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `runtime-attribution` | standalone ordinary and instrumented runtime attribution | 0 | 105.845 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `docs-exact` | required docs verification | 1 | 2297.333 | `bf729eb60a8e8e47aa2032e5122e24790a597f3483444118df00108c5c01d93c` |
| `live-coverage-exact` | required live coverage verification | 1 | 1904.221 | `bff790ef66af8164d2ac80c3b812c89f357223157d96414bd1e85f8966eeb68e` |
| `coverage-audit` | independent raw LCOV and profile audit | 0 | 1032.343 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `gate-audit` | current gate and eleven command receipt audit | 0 | 18598.662 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `archive-audit` | compact selection and archive audit | 0 | 7382.075 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `preservation-audit` | migration, source length and inventory audit | 0 | 146.971 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `native-independent` | SIGPIPE, full and broken stderr reproduction | 0 | 570.53 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `environment-matrix` | ordinary and instrumented environment outage matrix | 0 | 20161.148 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `environment-negative` | environment negative controls | 0 | 1960.368 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `recovery-matrix` | no replay, reservation or cleanup matrix | 0 | 9009.132 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `workspace` | unfiltered independent Cargo workspace test | 0 | 122723.099 | `eb3b1febd7862be7197f6804bb2de465f2969d5491db165c787216bb76671159` |
| `source-after` | post-workspace source fingerprint | 0 | 98.583 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `source-final` | final source fingerprint | 0 | 105.515 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

**Result: verification blocked.** The intended product-source checks observed no reproducible execution defect, but the exact required Bun docs command fails in the integrated workspace because it also discovers a retained task-19 shadow copy. This report records an independent review; it is not a final mission decision. Reviewed at 2026-11-09T20:39:12.023235+00:00. Skill: `oma-qa`.

## HIGH - required verification command discovers incomplete shadow tests

- `target/exec-fail-open/task19/shadow-proof.ts:4-7` copies `scripts/harness` and `scripts/coverage`, including `*.test.ts`, to `target/exec-fail-open/task19/shadow-workspace`, without copying product sources. `bun test scripts/harness/exec-fail-open-docs.test.ts` returned 1: the two intended tests passed, but two additional copies failed with `ENOENT` for the shadow workspace's `crates/arc-cli/src/main.rs`. Full stderr: `target/exec-fail-open/task17-independent-vs3qytnv/docs.stderr`, SHA-256 `bc7eb5c93285d7122d5c3d7d7fc27cbc8c136bbd224e5e3ad955c8fc98eca441`. The command and outcome are retained below. The same discovery affected the targeted fixture check: its three intended tests passed, then the copied shadow suite failed loading its missing `.arc-verify/exec-fail-open/contract.json`.
- Owner: task-19/backend evidence or harness owner, coordinated by the Chief. Preserve the shadow evidence while moving it out of Bun test discovery or changing how the shadow fixture stores copied test files; rebind any changed evidence paths. Then rerun the **unchanged** required verification commands and downstream gate. An absolute-path diagnostic passed the two real docs tests, but does not satisfy the original command. No product or test file was edited by QA.

## Scope and source binding

Base HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`; Darwin arm64, release 27.2.0; cwd `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. The fresh review binds the 235 current source inputs directly to SHA-256 `ec4e258a4ad8f42f59fd67bd92e7733a907e7afadfc386ac5a3e90bc9bc9a663` and gate `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7`. Every listed current input was rehashed before writing, with no mismatch. Candidate `target/debug/arcctl` SHA-256 `c857ff1cc184f534d4a85e49a85ccde5d9eb071bca43dfdb2ee707b4d0272c87` matches the gate's native candidate. The old task-6 review and receipt are historical, not evidence that task-6 ran the corrected code: exact copies are `target/exec-fail-open/task17-independent-vs3qytnv/prior-review.md` SHA-256 `ded9cd8ef940dc46081ecedf6f3cc2659926d7e7a9391b812db4839d93d4b1a7` and `prior-review.json` SHA-256 `c0b7cfb81f1de855436cab07ea32817bb2cc41e919a2c282451e013c9bc17dd4`. All original task-12 preliminary and task-6 Markdown bytes follow this section verbatim. Their earlier source and conclusions remain scoped as recorded there.

No cancellation signal was observed. The last supplied absolute deadline was 2026-11-09 20:40:08 UTC; checks used a 7,200,000 ms nominal allowance subject to that deadline and a shutdown reserve. Run/round limits are framework-controlled and unavailable to the Actor; the operator note declares the missing budget record satisfied. Host had 41 GiB free and an 8-core arm64 CPU. No external delivery, GUI, browser, installation or extra tracked evidence file was used.

## Fresh execution and design review

- `crates/arc-cli/src/exec/descriptors.rs:11-32` captures inherited standard-descriptor state and SIGPIPE before Rust runtime initialization; `:34-70` restores them in the child. `exec.rs:66-108` has one `Command::spawn` admission site; `preparation.rs:37-77` sends REGISTER and CLAIM the same absolute deadline and maps outage uncertainty separately from rejection/protocol/cancellation. `transport.rs:101-182,259-270` bounds retries and polls cancellation. `signals.rs:123-180` checks cancellation at admission and the pre-exec handoff, then attaches and forwards signals. `exec.rs:142-205` waits for and reports the same child outcome; fallback does not call ATTACH or COMPLETE. Diagnostics use static text, generated references and exit outcomes rather than command secrets (`preparation.rs:80-105`).
- The fresh unfiltered `cargo test --workspace` exited 0 after 116.044 s, PID 78766, 395 passed across 32 harnesses, zero failed, ignored or filtered. Its stdout SHA-256 `fc81ebd023d650877b615896c35d9e9ffee22216c12200a2c9d57dc176a73fa5`, stderr SHA-256 `61463bbccc575772b4ece419762d45df0b25b67e905740ade15dbddbf4a30b73`; log paths and full receipt are in the JSON report. It ran managed behavior, the outage matrix, cancellation/rejection, historical routes and negative control, native descriptors, streams, exit 17, signals and foreground pipelines. This is independent of the earlier QA run.
- The separate direct-versus-wrapped native probe (`native-corrected`) observed default SIGPIPE terminate both with signal 13 and ignored SIGPIPE return 17 in both; each marker had one invocation. Both absent and committed-REGISTER cases preserved exit 17, stdin, argv, cwd, environment, F_GETFL and FD flags while the full stderr reader stayed open and undrained through exit. The committed case queried two real lifecycle rows and observed six real daemon acknowledgements withheld (three each for ordinary and full-pipe wrapped invocations); permits and GC operations were zero. Ordinary diagnostics reported unavailable or uncertain tracking without the synthetic environment sentinel. Broken stderr returned 17 and one invocation in each mode; no diagnostic delivery is claimed when the reader is closed. Separate first probe attempts and their failure logs are retained: the first hit the per-user daemon lock while Cargo ran; the next probe initially assumed macOS F_GETFL was unchanged by its own pipe setup. Neither is treated as a product defect.
- Independent complete-environment comparison covered 13 outage modes for each of ordinary and instrumented binaries, with byte-equal synthetic environment and one invocation each. For every instrumented comparison the driver pre-seeded `__LLVM_PROFILE_RT_INIT_ONCE` in both inputs; that run does not establish unseeded instrumented behavior. Committed REGISTER and CLAIM each withheld three successful acknowledgements. Eight added/removed/changed controls per binary, including non-UTF-8, empty and multiline values, all failed the byte equality check as intended. The native workspace tests and retained gate additionally check no replay after recovery and no reservations, cleanup or secret persistence. Logs are under `target/exec-fail-open/task17-independent-vs3qytnv`.
- The separate `recovery-matrix` exercised 13 real-daemon outage modes with a seeded resource sentinel and database checks before execution, after execution, and after daemon restart. Each case observed one child invocation before and after recovery, exit 17, zero permits and GC operations, unchanged resource row and sentinel, and no decoded cleanup request. Its output records proxy request events and commit acknowledgements.
- Native gate has 17 parent-before-wrapper-spawn-to-child-handshake measurements, each below 2,000 ms; maximum `partial-register` 1816.063 ms. This total includes startup and observation; subdivisions were not instrumented. Phase 0 has no scheduler stage. The stated two-second target is an engineering observation, not a real-time guarantee.

## Gates, preservation and accepted inputs

- The direct `validateGates` call passed against current source and authenticated 11 prerequisite command selections. The independent gate audit authenticated the 22 retained stdout/stderr hashes, command ordering, no skipped flags and the 395-test backend workspace receipt. The gate includes prior successful formatting, warning-denying Clippy, full workspace, coverage collection and full harness. The exact docs command now fails for the separate shadow-discovery reason above; earlier gate success cannot establish that the current framework verification will pass.
- Independent raw LCOV/LLVM reconciliation checked 159 records, 46 production files and all 1,457 retained profile hashes/merge entries, including instrumented arcctl/daemon child proof. Integer 80% floors: arc-core 247/256; arc-detector 609/610; arc-storage 653/674; arc-ipc 387/393; arc-daemon 1628/1746; arc-cli 933/1065. Selected run `2026-11-09T20-03-49-259Z-913026a3-d819-4010-8ed6-f20947b957cb`; raw LCOV SHA-256 `50b3bcde0a48e9b6bd17b1a3b6129d9cf83ca0abcfffbb2d7554bbec5002d230`, LLVM SHA-256 `847f50fb8574472a630a3e67b59351e4a3003eb4fc3a059ac1985a18fd0df1a0`. Gate receipts place live coverage verification after the last listed collector.
- Migration 0001 has SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in worktree, index and accepted HEAD. The retained preservation guard checks unchanged accounting, historical test bodies and two narrow adapters: before-register and after-shutdown use the accepted historical binary, while other cases use the current candidate. The fresh workspace test executes both routes and the historical negative control. Source line inventory found the longest tracked product/harness source at 483 lines; pre-existing `.agents/` definitions are outside this product scope and were not changed.
- The compact `collection-next.json` is 1,243,746 bytes with 43 current and 12 archived selections. Independent reconstruction authenticated nested archive hashes and external predecessor bytes; the staged evidence inventory is 149 files. Three intended task-19 frozen historical transition and nested predecessor tests passed, but the parent command exited 1 after Bun discovered the incomplete shadow copy. This run ended at 20:36:44 UTC before the live QA files were written at 20:39:12 UTC; it does not prove operation after that change. A separate full compact-suite attempt was terminated by the reviewer after 180 s to preserve the absolute deadline; 10 production tests had passed at that point, and it is not reported as a completed suite. Retained task-19 receipts record prior unfiltered compact and flow passes on this source.
- `contract.md`, `contract.json`, `design-contract.md` and `claims-review.md` were consumed in their accepted scope. Delivered help and docs distinguish outage fallback, uncertainty and protocol/local exclusions. The claims report predates later wording correction and its recorded source references remain historical. Usability and commercial outcomes are unmeasured; desktop, native Windows, vendor hooks and scheduling remain outstanding.

## Fresh command receipts

Each `*-meta.json` in `target/exec-fail-open/task17-independent-vs3qytnv` contains cwd, actual argv, start/end, PID, elapsed time, exit, and stdout/stderr digests. The first workspace attempt (exit 101) set `LLVM_PROFILE_FILE` while running uninstrumented binaries, causing `crates/arc-daemon/tests/resident/coverage.rs:36` to expect nonexistent profiles. The successful retry inherited the normal environment. The targeted fixture command intentionally selected only three tests; it is not an unfiltered gate.

| Label | Actual argv | Elapsed | Exit | Stderr SHA-256 |
| --- | --- | ---: | ---: | --- |
| `docs` | `["bun", "test", "scripts/harness/exec-fail-open-docs.test.ts"]` | 578.8 ms | 1 | `bc7eb5c93285d7122d5c3d7d7fc27cbc8c136bbd224e5e3ad955c8fc98eca441` |
| `docs-absolute` | `["bun", "test", "/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace/scripts/harness/exec-fail-open-docs.test.ts"]` | 41.5 ms | 0 | `00b0d21a7c592422e44261863b88c55cf30d809183b10d37ce274014ff2474a7` |
| `fixture-targeted` | `["bun", "test", "scripts/harness/exec-fail-open-compact.test.ts", "--test-name-pattern", "correction rotation preserves the previously rotated predecessor|accepted QA transition binds prior bytes|production gates require the current combined archive"]` | 58315.3 ms | 1 | `9010631097c4028946d9cb2fbaa7f24795bce62456c8a32737876a5d1e7347ad` |
| `compact` | `["bun", "test", "scripts/harness/exec-fail-open-compact.test.ts"]` | 180115.6 ms | -15 | `03e11b63e666c9439eac8b581301220d77c2c2ade2ef82b9f493c99494e8752f` |
| `native-final` | `["python3", "target/exec-fail-open/task17-independent-vs3qytnv/native-independent.py"]` | 196.5 ms | 1 | `60f5abb7d91ead7181f575c448c144986d81eae4ab349e7688f581feb47060c0` |
| `native-corrected` | `["python3", "target/exec-fail-open/task17-independent-vs3qytnv/native-independent.py"]` | 548.8 ms | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `environment-matrix` | `["python3", "target/exec-fail-open/task17-independent-vs3qytnv/raw-env-matrix.py"]` | 20431.8 ms | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `environment-negative` | `["python3", "target/exec-fail-open/task17-independent-vs3qytnv/raw-env-negative-timed.py"]` | 1934.3 ms | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `gate-validator` | `["bun", "-e", "import {validateGates} from \"./scripts/harness/exec-fail-open-gates\"; import {root} from \"./scripts/harness/exec-fail-open-inputs\"; const r=validateGates(root); console.log(JSON.stringify({passed:r.passed,sourceHash:r.sourceHash,reference:r.reference,commands:r.commands.length}));"]` | 22185.3 ms | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `recovery-matrix` | `["python3", "target/exec-fail-open/task6-attempt4-independent/cleanup-matrix.py"]` | 9022.4 ms | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The workspace retry began `2026-11-09T20:34:58.808146+00:00` and ended `2026-11-09T20:36:54.853489+00:00`. Full source-bound records, actual outputs and failures are retained in the ignored directory, not copied into tracked evidence. The Chief owns final review and `final.json`/terminal rebinding; the existing final decision still refers to the older QA and is not current acceptance.

---

# Historical QA records (preserved verbatim)

# Independent QA — task-6 attempt 4

**Review result: PASS. Verification completeness: completed for task-6.** No critical, high, medium, or open low-severity defect was reproduced. The Chief Director's final decision and task-7's two terminal checks remain outstanding; this report is not the mission's final verdict.

Reviewer `reviewer`, skill `oma-qa`. Accepted baseline `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. Current 229-input fingerprint before and after review: `6fd5018b3e56e6e1fbc9190fbe74ff3c990b89bbd8b73e9778d354f97d96d00e`. Host: Darwin 27.2 arm64, cargo/rustc 1.95.0 and Bun 1.4.4. Tests used `CARGO_NET_OFFLINE=true`; independent Cargo execution used a separate target at `target/exec-fail-open/task6-attempt4-independent/cargo-target`. The selected gate and QA binaries have different hashes and were produced in different targets; each is bound to the same source fingerprint. Every new command wrapper allowed 7,200,000 ms. No product source, implementation test, contract, migration, validator, or mission graph was edited.

## Findings and earlier observations

- **C1, LOW, resolved; owner backend.** Earlier short help implied that all unconfirmed tracking leads to an attempt. Current `crates/arc-cli/src/main.rs:53–57` and the actual `arcctl exec -h` output limit fallback to outages and list invalid replies and local preparation failures as non-launching. A real mismatched protocol-1 request ID exited 1 with `IPC_ERROR`, `NOT_STARTED`, empty stdout and zero invocations. `docs/managed-exec.md:11,29` agrees. The earlier task-6 and task-5 text below describes the older state.
- **G1, prior collection gap, resolved for the current gate; owner task-4 backend.** The disposed attempt-13 logs were not restored. The persistent collection selected by the current gate has all 22 prerequisite stdout/stderr files and its selected coverage data available in this integrated workspace. The maintained gate validator and an independent selected-entry, log-hash and exact-argv audit passed. This is a new source-bound collection, not a relabeling of the missing original files.

The unrelated monitoring probe `ps -o ...` was denied with `operation not permitted`; the command runner captured its own child PIDs and timings. A shell glob for a nonexistent `target/exec-fail-open/task4*` and a lookup for an old helper in the wrong attempt directory also failed; the retained paths were then found by supported filesystem reads. These were not verification commands and did not block review. No GUI or browser workaround was used.

## Execution and security review

`crates/arc-cli/src/exec.rs:67–108` validates arguments, creates one 1,800 ms absolute pre-spawn deadline and reaches one `Command::spawn`. `exec/preparation.rs:44–75` passes that deadline unchanged through REGISTER and CLAIM and makes a post-REGISTER outage uncertain. `exec/transport.rs:14–25,101–182` distinguishes endpoint loss, sent-request uncertainty, exhausted DB_BUSY, explicit errors, invalid replies, protocol mismatch, local failures and cancellation; frame waits use the remaining deadline and at most 25 ms polling (`transport.rs:259–320`). Fallback reaches the same native spawn path and makes no ATTACH or COMPLETE request. `exec/preparation.rs:78–124` prints static, secret-free tracking copy only after a successful spawn and makes diagnostic writes best effort.

The existing daemon authenticates accepted Unix peers by same-user credentials (`crates/arc-daemon/src/admission.rs:36–47`, `platform.rs:9–55`). In an additional local test, that real daemon admitted a same-user proxy which submitted invalid REGISTER or CLAIM payloads and forwarded its actual `INVALID_REQUEST` response. Both cases exited 1, wrote `NOT_STARTED`, and had zero child invocations. This test does not assert authentication in the opposite direction beyond the existing socket design. `exec/signals.rs:104–162,240–245` and `exec/signals/handoff.rs:8–70` cover the pre-fork admission and post-fork signal handoff. `crates/arc-storage/src/jobs/transitions.rs:46–47,107–128,176–179` retains idempotent registration/claim transitions and transaction commit. The real-daemon fault proxy at `crates/arc-daemon/tests/managed_exec/fault.rs:194–205` observes successful operations before withholding acknowledgements.

The independent unfiltered `cargo test --workspace` passed **386 tests across 32 harnesses**, with zero failures, ignored tests or filtered tests. It exercised the four access-denial tests, both invalid paths, outage, committed-ack loss, cancellation, malformed replies, managed lifecycle and idempotency, daemon-loss survival, protocol-1 diagnostics, broken stderr, terminal and pipeline cases. Focused access runs observed errno 13 four times. The invalid parent and endpoint tests observed errno 20 and 38 respectively, exit 1 and no child. Both original fail-closed assertion files match baseline byte-for-byte except for their single historical routing expression; both routes executed against the fixed-baseline binary, and its failure of the live fallback assertion passed as a negative control.

## Native observations

Each row records the parent monotonic instant immediately before wrapper spawn through the child's control handshake. The numbers include startup and observation overhead; the metadata, startup and handoff subdivisions were not separately measured. They are observations on this host, not a hard real-time guarantee. All 17 cases attempted the child once, observed exit 17 and remained at one invocation after daemon recovery. The maximum was **1812.754 ms**, below the 2,000 ms engineering target.

| Scenario | Unadjusted handshake ms | Invocations / exit / after recovery |
| --- | ---: | --- |
| `absent` | 9.902 | 1 / 17 / 1 |
| `refused` | 9.694 | 1 / 17 / 1 |
| `disconnect-before-read` | 12.680 | 1 / 17 / 1 |
| `disconnect-register` | 14.161 | 1 / 17 / 1 |
| `stall-register` | 1812.754 | 1 / 17 / 1 |
| `partial-register` | 1811.961 | 1 / 17 / 1 |
| `committed-register` | 17.155 | 1 / 17 / 1 |
| `disconnect-claim` | 15.788 | 1 / 17 / 1 |
| `stall-claim` | 1811.805 | 1 / 17 / 1 |
| `committed-claim` | 19.433 | 1 / 17 / 1 |
| `shared-budget` | 1812.167 | 1 / 17 / 1 |
| `busy-register` | 12.808 | 1 / 17 / 1 |
| `busy-claim` | 17.096 | 1 / 17 / 1 |
| `connect-access-before-register` | 9.685 | 1 / 17 / 1 |
| `connect-access-register-ack-loss` | 12.146 | 1 / 17 / 1 |
| `connect-access-before-claim` | 12.645 | 1 / 17 / 1 |
| `connect-access-claim-ack-loss` | 17.234 | 1 / 17 / 1 |

The committed REGISTER and CLAIM rows each recorded three successful real-daemon responses withheld after commit. A separate 13-case resource probe observed zero permits, zero GC-operation rows and zero decoded cleanup requests, with its seeded resource row and file unchanged before, after and after recovery. That observation is scoped to the controlled fixtures; the literal `arcCleanup: false` in the original timing fixture was not treated as independent proof.

Direct-versus-wrapped execution also matched raw argv, cwd, complete synthetic environment, stdin and stdout bytes, a separate inherited descriptor and exit 17; child stderr and the static fallback diagnostic were both present without the synthetic secret sentinels in the diagnostic. Closed standard and extra descriptors, broken stderr and terminal modes were covered by the workspace tests. Two directed signals reached one child once, and the wrapper reported the corresponding signal. Immediate and near-deadline cancellation during a stalled REGISTER produced `NOT_STARTED` and zero invocations. Fifteen direct and wrapped foreground-pipeline observations passed, including input, signals, stop/continue and `[17, 0]` outcomes.

For all 13 outage modes, independent ordinary and actual instrumented CLI runs produced **26 unsorted, unfiltered complete-environment byte matches**, with one invocation and exit 17 per run. Sixteen added, removed and changed-entry negative controls were rejected. A separate unseeded attribution check observed `__LLVM_PROFILE_RT_INIT_ONCE` only in the instrumented run; the instrumented equality checks supplied that same literal input to both sides and discarded no observed keys. The ordinary run used the same unseeded input on both sides. The detailed synthetic inputs and outputs are retained in the JSON and target logs; no host environment values were recorded.

## Gate, coverage and preservation

The maintained `validateGates(root)` passed on the current source and its final rerun is in the command receipts. An independent audit verified the backend / `oma-backend` identity, the exact 11 prerequisite commands in contract order, their selected container entries, both raw stream hashes, source bindings, exits and serial ordering. The accepted backend gate had 386 workspace tests and no failed, ignored or filtered tests. Its live coverage check followed all five recorded collectors.

| Prerequisite command | Elapsed seconds | Exit |
| --- | ---: | ---: |
| `cargo fmt --all -- --check` | 0.668 | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 1.272 | 0 |
| `cargo test --workspace` | 111.264 | 0 |
| `bun run test:coverage` | 158.694 | 0 |
| `./scripts/harness/validate.sh` | 178.043 | 0 |
| `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 227.487 | 0 |
| `bun test scripts/coverage/clean-checkout.test.ts` | 210.375 | 0 |
| `bun test scripts/coverage/coverage-cli.test.ts` | 184.154 | 0 |
| `bun test scripts/coverage/harness.test.ts` | 214.579 | 0 |
| `bun test scripts/coverage/evidence.test.ts` | 3.042 | 0 |
| `bun test scripts/harness/exec-fail-open-docs.test.ts` | 0.478 | 0 |

The selected coverage run `2026-11-09T12-08-51-259Z-bd0245f8-4b1e-4021-a674-071b6f0198cf` was recomputed independently from raw LCOV and the matching LLVM export, including every file's mapped and omitted lines. All 1442 raw profile bytes matched their recorded size and SHA-256, the complete merge list matched those profiles, and both recorded child proofs matched the actual collector receipts. The CLI proof was `--help` with exit 0; the daemon proof deliberately used an invalid socket path and exited 1, so it proves profile collection rather than successful daemon startup. Coverage uses its narrower fingerprint `36a77838606546d542152242341a012d5e1fe758a0c7a99ff5173c0a34494321`. Each floor was checked using integer arithmetic `covered * 100 >= total * 80`. The earlier coverage and harness runs were also recomputed from their own raw data. In the retained clean checkout, all six floors passed independently; its daemon count was 1628 / 1746, while the other five counts matched the table. The clean checkout's paths and exports were read in place, not copied into the integrated workspace.

| Crate | Covered / production lines | 80% floor |
| --- | ---: | --- |
| arc-core | 247 / 256 | pass |
| arc-detector | 609 / 610 | pass |
| arc-storage | 653 / 674 | pass |
| arc-ipc | 387 / 393 | pass |
| arc-daemon | 1629 / 1746 | pass |
| arc-cli | 877 / 991 | pass |

The four-file preservation group passed 19 tests and 301 expectations, including fixture and accounting tamper controls. It produced three temporary changes under `.arc-verify/harness-speed/`; their exact pre-run worktree bytes were restored and the index subsequently matched. Test-produced bytes and hashes remain under `target/exec-fail-open/task6-attempt4-independent/harness-speed-generated/`. The historical route log was similarly restored after extracting this run's two route records. `0001_phase0.sql` has the same worktree, index and baseline SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The tracked `crates/` and `scripts/` source inventory has 206 files, maximum 476 lines.

The original 44-entry collection and all current container selections were hash-checked. The 52-entry attempt-13 container and all four immediately preceding canonical artifacts matched the retained pre-rotation copies byte for byte. A changed selected entry was rejected. The independent successor guard probe rejected 20 normalized artifact aliases and six structured terminal-command aliases; it accepted an acyclic graph, rejected a cycle and checked sibling-root isolation with final artifacts present. The unfiltered compact and compact-flow suites completed with their fixture-only terminal validations. A post-write check verified 146 staged regular evidence files with matching index and worktree bytes; task-7's three reserved final files project 149, below the 150-file limit.

## Independent command receipts

The exact argv, host, PID, UTC start/end, monotonic elapsed time, exit, environment overrides and stdout/stderr paths and hashes are inline in `qa/review.json`. Full logs and fixture observations are under `target/exec-fail-open/task6-attempt4-independent/` and the corresponding unique `target/exec-fail-open/native/` process directory. The production collector's raw logs and profiles remain in the integrated workspace `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`.

| Receipt | Elapsed seconds | Exit |
| --- | ---: | ---: |
| `workspace` | 156.053 | 0 |
| `fmt` | 0.457 | 0 |
| `clippy` | 6.830 | 0 |
| `access-tests` | 1.139 | 0 |
| `invalid-socket-tests` | 0.151 | 0 |
| `protections` | 276.146 | 0 |
| `compact` | 288.481 | 0 |
| `compact-flow` | 374.447 | 0 |
| `docs` | 0.809 | 0 |
| `short-help` | 0.014 | 0 |
| `long-help` | 0.023 | 0 |
| `live-coverage-final` | 2.702 | 0 |
| `invalid-reply` | 0.087 | 0 |
| `native-bytes` | 0.727 | 0 |
| `native-cases` | 1.987 | 0 |
| `pipeline-cases` | 1.039 | 0 |
| `real-rejections` | 0.153 | 0 |
| `environment-matrix` | 20.345 | 0 |
| `environment-negative` | 1.902 | 0 |
| `runtime-marker` | 0.318 | 0 |
| `cleanup-matrix` | 9.011 | 0 |
| `historical-audit` | 0.129 | 0 |
| `archive-audit` | 1.906 | 0 |
| `coverage-independent` | 1.136 | 0 |
| `coverage-earlier-final` | 1.821 | 0 |
| `coverage-harness-final` | 1.801 | 0 |
| `coverage-clean` | 1.803 | 0 |
| `guards` | 0.050 | 0 |
| `gate-audit-fixed` | 3.777 | 0 |
| `gate-after` | 141.637 | 0 |
| `source-after` | 0.370 | 0 |

Desktop integration, native Windows acceptance, vendor hooks and scheduling remain outstanding. User comprehension, adoption, retention and financial outcomes remain unmeasured. Serena code-intelligence tools were unavailable, so the review used scoped reads of known paths and Git observations.

## Preserved earlier task-6 and task-12 observations

Everything below this sentence is the previous review, retained verbatim as historical evidence. Its old open findings and partial verdict apply to the earlier source and collection, not this attempt.

---

# Independent QA — task-6 attempt 3

**Quality verdict: NOT_ASSESSED. Completeness: partial.** One LOW short-help finding (C1, backend) and one production-evidence gap (G1, task-4 collection owner) remain open. The task-7 terminal checks and mission decision remain pending. This review changes no product code or implementation tests.

Reviewer: `reviewer`, skill `oma-qa`. Accepted HEAD: `00773c69a8e5e7fd417fd5cc78d03e222364f609`. The 228-input fingerprint before and after the new checks was `1bee897be5a934308987ea49650f139ab704833196ce8756e13ebfeb8485a4b9`. Host: macOS 27.2, Darwin arm64; commands ran in the writable integrated workspace with `CARGO_NET_OFFLINE=true`. Each new wrapper allowed 7,200,000 ms. Prior independent test and coverage logs were rechecked by digest against the same source fingerprint.

The earlier timeout blocker is withdrawn. The task requires at least 3,600,000 ms, which the eleven task-4 receipt headers record; the operator's two-hour attempt allowance does not change that minimum. The collecting checkout and its raw timing records are absent here, so its actual process timers were not independently revalidated.

## Independent cleanup observations for 13 outage cases

`crates/arc-daemon/tests/managed_exec/fail_open.rs:221` emits `arcCleanup: false` as a literal: that field alone does not prove the 13 earlier cases checked cleanup. The four access tests separately inspect `gc_operations`, permits and an untouched sentinel before and after recovery at `connect_access.rs:360` and `:390`. To cover the other cases, I ran `python3 target/exec-fail-open/task6-attempt3-independent/cleanup-matrix.py` against the current native binaries and a new local daemon/database for every case. The script SHA-256 is `e27b98fbdc92a4b57e60409a1da38e56a07b26d70c8b10e9dbcd14f9ac338d72`; its full command receipt, binary hashes, request events, database observations and monotonic boundaries are inline in `review.json`. Raw stdout SHA-256 is `f23cc715b447af90150484a29de4d8093621da6d8a5a2d4d00c4b176ced78812`; stderr was empty. It exited 0 in 9067.4 ms.

Each case produced one invocation and exit 17; two read-only status requests after daemon restart left one total invocation. Each observed zero `permits` and zero `gc_operations` rows before execution, after execution and after recovery. A seeded resource-object row and file sentinel had identical contents and SHA-256 at all three observations. The fault proxy decoded only `job.start` REGISTER/CLAIM requests; there were zero decoded cleanup requests. In the absent/refused cases no connection was accepted; in the before-read case the full request frame was peeked without consuming it before disconnect. These are scoped observations in the local fixtures, not proof about every possible resource type.

| Outage | Observed proxy actions | Parent-to-wrapper-exit ms |
| --- | --- | ---: |
| absent | none accepted | 43.9 |
| refused | none accepted | 37.3 |
| disconnect-before-read | REGISTER × 3 | 38.1 |
| disconnect-register | REGISTER × 3 | 36.4 |
| stall-register | REGISTER × 3 | 1842.5 |
| partial-register | REGISTER × 3 | 1837.2 |
| committed-register | REGISTER × 3 | 40.8 |
| disconnect-claim | REGISTER × 1, CLAIM × 3 | 36.5 |
| stall-claim | REGISTER × 1, CLAIM × 3 | 1838.7 |
| committed-claim | REGISTER × 1, CLAIM × 3 | 38.9 |
| shared-budget | REGISTER × 3, CLAIM × 1 | 1843.2 |
| busy-register | REGISTER × 3 | 41.7 |
| busy-claim | REGISTER × 1, CLAIM × 3 | 41.0 |

The supplemental time includes wrapper startup, the Python child and exit; it is not the metadata-only interval. The two committed cases each forwarded three successful real-daemon operations and withheld their acknowledgements. The prior native parent-to-child-handshake measurements below remain the relevant unadjusted timing observations.

## Production gate recheck and handoff

`validateGates(root)` was rerun and exited 1 because `target/exec-fail-open/commands/a13-fmt-50d75e58-3417-48b4-bc5b-15bc48b14045/stdout.log` is absent. Its stderr SHA-256 is `a6939bb3d4c93cca58705db5d7540cb7060dfd7dba999dc81d867d48cca8faae`. This confirms G1 remains a verification gap, not a product defect; task-4 must retain or recollect and assemble its eleven unchanged prerequisite receipts, native logs and checkout-specific coverage in the final-validation checkout. Do not substitute the fresh independent coverage for the missing selected collection. C1 remains the independently reproduced short-help defect described below. No timeout-wrapper change is requested.

The first local cleanup probe could not start its daemon because the newly created socket directory had mode 0755; after using mode 0700, the next two attempts failed to restart because the fixture's runtime report had not been rotated. Both fixture setup errors were corrected locally and are recorded in the inline receipts. They are not product findings. Immediately after the review JSON was updated, `validateIndex` reported that its staged bytes differed; the operator index synchronizer caught up and a retry verified all 146 staged files. No manual index change was made. The successful run did not change product files. The preliminary task-12 section remains byte-for-byte unchanged.

---

# Independent QA — task-6 attempt 2

**Quality verdict: NOT_ASSESSED. Completeness: partial.** The current source passed the independent checks below. One evidence gap and the short-help finding remain open; a passing gate decision is not supported. This report is an independent review, not a product or test change.

Reviewer: `reviewer`; skill: `oma-qa`. Accepted HEAD: `00773c69a8e5e7fd417fd5cc78d03e222364f609` (tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`). Current 228-input source fingerprint before and after checks: `1bee897be5a934308987ea49650f139ab704833196ce8756e13ebfeb8485a4b9`. Host: macOS 27.2, Darwin arm64, Rust/Cargo 1.95.0, Bun 1.4.4. Checks ran in the writable integrated workspace with `CARGO_NET_OFFLINE=true`; every new outer check wrapper allowed 7,200,000 ms. Raw logs and case records are local under `target/exec-fail-open/task6-attempt2-independent/`; their hashes and the actual input/output observations are inline in `review.json`.

## Open finding

**LOW — C1; owner: backend.** `crates/arc-cli/src/main.rs:53` says the command is attempted when tracking cannot be confirmed. The short `arcctl exec -h` output does not state that an invalid protocol reply or local preparation failure stops execution. A local protocol-1 peer returned a mismatched request ID: the CLI exited 1 with `IPC_ERROR`, `NOT_STARTED`, no stdout and zero invocations. `docs/managed-exec.md:11` already limits fallback to outages, and the task-5 claims review identified this copy issue. Change the short-help promise to “tracking is unavailable or unconfirmed because of an outage” and list invalid replies and local preparation failures among the non-launching exclusions; coordinate the exact-string help check with the backend harness owner. The long help's additional explanation does not appear in short help.

## Open verification gaps

**G1 — task-4 collection owner.** `implementation/gates.json` selects eleven backend / `oma-backend` receipts with the current source fingerprint. Their 22 stdout/stderr files are absent here, as is selected coverage run `2026-11-09T09-28-48-252Z-8f665ec2-d0e7-400f-8200-cc3f11c26edb`; selected timing and historical binary hashes also differ from the binaries currently at their recorded paths. `validateGates(root)` exited 1 at `target/exec-fail-open/commands/a13-fmt-50d75e58-3417-48b4-bc5b-15bc48b14045/stdout.log` with ENOENT. The raw bytes must be retained and the original gate assembled and validated in the final-validation checkout. The independently verified coverage collections below do not authenticate this missing collection.

## Execution decisions and native observations

The source still has one CLI-owned `Command::spawn` at `crates/arc-cli/src/exec.rs:100–108`, after argument validation and a single 1,800 ms absolute pre-spawn deadline at lines 67–98. `exec/preparation.rs:44–75` passes it to REGISTER and CLAIM and retains uncertainty after a confirmed REGISTER. `exec/transport.rs:101–182` separates endpoint outages and possibly sent requests from cancellation, local failures, malformed replies, protocol mismatch and explicit rejection; retries do not spawn. Daemon-side admission checks the peer in `crates/arc-daemon/src/admission.rs:36–47`. Signal admission and child handoff are in `exec/signals.rs:104–162`, with a barrier-controlled post-admission cancellation test in `exec/signals/handoff.rs:8–70`. Storage commits and idempotent transitions are in `arc-storage/src/jobs/transitions.rs`; the tests forward actual operations to the real daemon before withholding committed replies.

The new unfiltered `cargo test --workspace` run passed **385 tests across 32 harnesses, with zero failed, ignored or filtered**. It included all four task-10 access-denied cases, both invalid socket-path cases, loss of REGISTER/CLAIM acknowledgements, invalid arguments and replies, cancellation, successful managed tracking, daemon-loss survival, native environment/descriptors/streams, signals and terminal pipelines. Repeated targeted access tests observed native errno 13 four times. The invalid path tests observed ENOTDIR (20) and ENOTSOCK (38), exit 1 and no child in each case. The deterministic spawn-admission and shared-job pre-fork signal tests each passed with zero admitted child executions.

The following measurements are unadjusted parent-before-wrapper-spawn to child-control-handshake intervals, including startup and observer overhead. Each case recorded one invocation, exit 17, zero permits, and one total invocation after daemon recovery. The 13 outage-case cleanup field was a literal; see the separate attempt-3 observations above. Internal metadata and handoff subdivisions were not independently observed. These are measurements, not a hard real-time guarantee.

| Scenario | Elapsed ms | Invocations | Exit | After recovery |
| --- | ---: | ---: | ---: | ---: |
| absent | 9.387 | 1 | 17 | 1 |
| refused | 9.795 | 1 | 17 | 1 |
| disconnect-before-read | 13.246 | 1 | 17 | 1 |
| disconnect-register | 11.964 | 1 | 17 | 1 |
| stall-register | 1811.412 | 1 | 17 | 1 |
| partial-register | 1813.754 | 1 | 17 | 1 |
| committed-register | 15.897 | 1 | 17 | 1 |
| disconnect-claim | 15.494 | 1 | 17 | 1 |
| stall-claim | 1812.720 | 1 | 17 | 1 |
| committed-claim | 19.476 | 1 | 17 | 1 |
| shared-budget | 1812.114 | 1 | 17 | 1 |
| busy-register | 13.194 | 1 | 17 | 1 |
| busy-claim | 16.522 | 1 | 17 | 1 |
| connect-access-before-register | 9.422 | 1 | 17 | 1 |
| connect-access-register-ack-loss | 14.430 | 1 | 17 | 1 |
| connect-access-before-claim | 32.550 | 1 | 17 | 1 |
| connect-access-claim-ack-loss | 18.033 | 1 | 17 | 1 |

The maximum was **1813.754 ms**, below the 2,000 ms target in this run. Separate observations recorded exact monotonic boundaries, commands and case inputs: SIGTERM and SIGHUP sent to the wrapper were delivered once to one child and the wrapper terminated by the same signal. A stalled REGISTER cancelled immediately and at 1,664.396 ms after wrapper launch exited 1 with `NOT_STARTED` and zero invocations. Fifteen direct and wrapped foreground-pipeline scenarios retained `[17, 0]` producer/peer outcomes, one producer invocation, correct signal counts, terminal input, stop/continue behavior and no surviving recorded process identities. The complete individual observations are inline in `review.json`, including signal counts and timestamps; timing covers each whole case rather than attributing hidden subintervals.

The ordinary and actual instrumented CLI binaries were each compared against direct execution across 13 outage modes. All **26 unfiltered, unsorted complete-environment byte comparisons matched**, including non-UTF-8, empty and multiline entries; both programs exited 17 with one invocation. The instrumented case used the actual coverage binary with the LLVM runtime marker present in both compared inputs. All **16** added, removed and changed-entry negative controls were rejected. The committed REGISTER and CLAIM cases each withheld three real-daemon successful acknowledgements for both binaries.

## Preservation, coverage and compact evidence

Both contradictory historical assertions remain byte-identical after reversing their one prescribed routing expression. Both historical routes executed during this workspace run against the fixed-baseline binary (SHA-256 `59dd9bbd94ed9e112cede82928d29d0065419b03aa406fc1bb697c6256e6d126`); the live-fallback negative control passed by observing the expected historical failure. The exact four-file preservation/equivalence/coverage-verification command passed 19 tests and 301 expectations, including mutation checks. Migration `0001_phase0.sql` matched baseline, index and worktree SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The tracked `crates/` and `scripts/` source inventory contained 205 files, maximum 476 lines.

The prior unfiltered compact and compact-flow runs were checked against their retained stdout/stderr digests and the same source fingerprint: 8 and 2 tests passed, respectively. The current compact audit authenticated all 44 original selections, 43 current selections, the original and combined container hashes, and the exact bytes of all four immediately preceding canonical records. Both 62-entry predecessor containers round-tripped. The fixture suites cover legacy references, entry-aware cycles and tamper rejection; the current guard probe rejected 20 successor aliases and six structured command references and confirmed root isolation with terminal files present. Before writing this review, 146 regular evidence files matched staged bytes, with 149 projected including reserved downstream files. The task-12 preliminary report remains an exact verbatim suffix.

The independent previous collection and later full harness logs were rechecked against their hashes. Their real LCOV, LLVM exports, 1,440 raw profiles and matching merge inputs per collection were revalidated in this checkout. The exact covered/total line counts for both were core 247/256, detector 609/610, storage 653/674, IPC 387/393, daemon 1628/1746 and CLI 877/991; all exceed 80%. Live coverage verification passed after the final reviewed collector. These results are distinct from task-4's unavailable original raw collection.

The exact four-file preservation command also overwrote three older `.arc-verify/harness-speed/` receipts as a test side effect. Their pre-run versions matched accepted HEAD; I restored those bytes and confirmed that both worktree and index again matched. The transient test-produced bytes and restoration hashes remain under the local QA target directory. No product files or implementation tests were edited.

A first QA-only reachability check failed because archived logical paths in this review and an earlier raw audit log looked like current artifact references. The review JSON now labels those names as archival, and the corrected QA-only traversal passed. The separately identified task-4 predecessor gaps remain open.

## New command receipts

All commands below ran in the workspace with 7,200,000 ms outer allowances. The JSON report binds stdout/stderr hashes and the raw case data.

| Command | Outcome | Elapsed ms |
| --- | --- | ---: |
| `cargo test --workspace` | exit 0 | 112700.6 |
| `python3 target/exec-fail-open/task6-attempt2-independent/native-cases.py` | exit 0 | 1957.3 |
| `python3 target/exec-fail-open/task6-attempt2-independent/pipeline-cases.py` | exit 0 | 1216.0 |
| `bun -e 'import {validateGates} from "./scripts/harness/exec-fail-open-gates"; import {root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(validateGates(root)));'` | exit 1 (missing task-4 log; G1) | 2970.5 |
| `bun target/exec-fail-open/task6-independent/audit.ts` | exit 0 | 31713.5 |
| `bun target/exec-fail-open/task6-independent/coverage-audit.ts` | exit 0 | 2728.5 |
| `python3 target/exec-fail-open/task6-attempt2-independent/raw-env-matrix.py` | exit 0 | 20017.8 |
| `python3 target/exec-fail-open/task6-attempt2-independent/raw-env-negative.py` | exit 0 | 1915.7 |
| `bun test scripts/coverage/evidence.test.ts` | exit 0 | 1054.9 |
| `bun test scripts/harness/managed-exec.test.ts scripts/harness/coverage-verification.test.ts scripts/harness/equivalence.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | exit 0 | 211688.1 |
| `cargo fmt --all -- --check` | exit 0 | 653.0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | 427.5 |
| `cargo test -p arc-daemon --test managed_exec connect_access -- --nocapture` | exit 0 | 1088.9 |
| `cargo test -p arc-daemon --test managed_exec invalid_socket -- --nocapture` | exit 0 | 145.8 |
| `cargo test -p arc-cli --bin arcctl exec::preparation::tests::cancellation_at_spawn_admission_cannot_invoke_spawn -- --exact --nocapture` | exit 0 | 138.5 |
| `cargo test -p arc-cli --bin arcctl exec::signals::tests::handoff::shared_job_terminal_signals_after_admission_before_fork_cancel_execution -- --exact --nocapture` | exit 0 | 138.2 |
| `bun test scripts/harness/exec-fail-open-docs.test.ts` | exit 0 | 368.3 |
| `python3 target/exec-fail-open/task6-attempt2-independent/historical-audit.py` | exit 0 | 89.6 |
| `python3 target/exec-fail-open/task6-independent/migration.py` | exit 0 | 85.3 |
| `python3 target/exec-fail-open/task6-independent/inventory.py` | exit 0 | 83.5 |
| `bun target/exec-fail-open/task6-attempt2-independent/guards.ts` | exit 0 | 45.2 |
| `bun target/exec-fail-open/task6-independent/reachability.ts` | exit 0 (probe reports missing or mismatched predecessors) | 4762.1 |
| `python3 target/exec-fail-open/task6-independent/invalid-reply.py` | exit 0 | 76.0 |
| `target/debug/arcctl exec -h` | exit 0 | 7.4 |
| `bun test scripts/coverage/evidence.test.ts` | exit 0 | 1344.6 |
| `python3 target/exec-fail-open/task6-attempt2-independent/prior-receipts-audit.py` | exit 0 | 49.5 |
| `bun -e 'import {currentInputs,root,validateIndex} from "./scripts/harness/exec-fail-open-inputs"; const x=currentInputs(root); console.log(JSON.stringify({sha256:x.sha256,count:Object.keys(x.files).length,stagedRegularAndIdentical:validateIndex(root,[])}));'` | exit 0 | 2064.6 |

Desktop integration, native Windows, vendor hooks and scheduling remain outstanding. Adoption, comprehension, retention and financial outcomes remain unmeasured. Serena code-intelligence tools were unavailable; review used scoped reads of known paths and Git observations. Git reported `av.yaml: Operation not permitted`; it was not read or bypassed.

---

# Independent QA review — task-6

**Quality verdict: NOT_ASSESSED. Verification completeness: partial.** The independent workspace test, native behavior matrix, fresh coverage, live coverage verification and full harness passed. I could not validate the task-4 production gate's original raw evidence: its collecting checkout and raw outputs are absent here. One low-severity help finding remains open. No reproducible critical, high or medium product defect was found. A passing mission decision must wait for the gate evidence and the unchanged task-7 terminal checks.

Reviewer: `reviewer`, skill `oma-qa`. Accepted committed baseline: `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. Current HEAD is the baseline with the accepted implementation staged; current 228-input source fingerprint is `1bee897be5a934308987ea49650f139ab704833196ce8756e13ebfeb8485a4b9`. Host: `Eunkwangui-MacBookPro.local`, `darwin 27.2.0 arm64`, Rust/Cargo 1.95.0, Bun 1.4.4 and cargo-llvm-cov 0.6.21. All runner envelopes allowed 7,200,000 ms per check, used `time.monotonic_ns`, and set `CARGO_NET_OFFLINE=true`. Raw files stay under `target/exec-fail-open/task6-independent/` and native observation directories; their hashes and actual commands are in `qa/review.json`. The tests use local processes, sockets and temporary fixtures. No browser or GUI was used. Serena was unavailable, so review used direct reads of named files and Git observations. Git reported `av.yaml: Operation not permitted`; it was not read or bypassed.

## Finding

**LOW — C1 remains open in short help — owner: backend.** `crates/arc-cli/src/main.rs:53` says that ARC still attempts the command if tracking cannot be confirmed. `arcctl exec -h` prints that line but omits the protocol and local-setup exclusions added to the long help at `main.rs:44–45`. A local protocol-1 peer returning a complete response with a mismatched request ID reproduced exit 1, `IPC_ERROR` / `NOT_STARTED`, empty child stdout and zero marker invocations. The docs intro now correctly limits fallback to outages and lists the exclusions. The task-8 disposition marked C1 clarified; that is true for `--help` and the docs, but the short-help promise remains ambiguous. The backend owner must coordinate the help wording with the existing docs checker and recollect source-bound gates after any source change. Suggested replacement in the existing help text:

```text
If tracking is unavailable or unconfirmed because of an outage, ARC still attempts the command once.
Cancellation, invalid arguments, invalid protocol replies, local preparation failures and explicit authenticated rejection do not start a child.
```

## Verification gap — task-4 / backend owner

The retained `implementation/gates.json` identifies backend / `oma-backend`, the correct source fingerprint and all 11 exact prerequisite command headers. All 43 current selections in the combined container authenticated, but metadata is not raw execution proof. The original task-4 checkout named in the handoff, `.agent-valley/parallel/workspaces/a546003e0df37906-8f9ba36d8e59dd86-13/`, is absent. All 22 referenced task-4 stdout/stderr paths are absent in this checkout; the first production-gate failure is `ENOENT` for `target/exec-fail-open/commands/a13-fmt-50d75e58-3417-48b4-bc5b-15bc48b14045/stdout.log`. The original selected coverage run `2026-11-09T09-28-48-252Z-8f665ec2-d0e7-400f-8200-cc3f11c26edb` lacks its summary, LCOV, LLVM export and profiles here. The retained timing and historical paths resolve to locally rebuilt binaries with hashes different from their original selected binaries. `validateGates(root)` exited 1 both before and after fresh local collection. The gate must not be accepted from its `passed` flag or the selected headers alone.

Task-4 should recollect and assemble the unchanged 11 prerequisite commands in the integrated checkout that will perform final validation, retaining its actual logs, profiles and native exports there. Do not transplant absolute-path coverage exports. The fresh independent runs below demonstrate current behavior, but do not authenticate the original task-4 outputs. Terminal `exec-fail-open-gates.test.ts` and `exec-fail-open-evidence.test.ts` remain task-7 work. The evidence validator rejects an open finding and an incomplete QA status; this report deliberately does not claim a passing final decision.

## Implementation and native behavior

`crates/arc-cli/src/exec.rs:67–108` validates arguments, creates one 1,800 ms deadline, prepares metadata and reaches one `Command::spawn` through the admission check. `exec/preparation.rs:44–75` passes that same deadline to REGISTER and CLAIM, maps endpoint/uncertain/DB_BUSY cases to unavailable or uncertain tracking, and retains uncertainty after confirmed REGISTER. `exec/transport.rs:101–182` retains sent-request uncertainty across endpoint retries and separates cancellation, local setup, invalid replies, protocol errors and explicit rejections. Each attempt is capped by the remaining deadline; `exec/signals.rs:104–160` handles cancellation at admission and signal handoff. `arc-daemon/src/admission.rs:36–47` authenticates accepted peers using `platform.rs:9–55`; this is the daemon-side same-user check. `arc-storage/src/jobs/transitions.rs:46–48,107–126,177–179` gives idempotent metadata transitions and a real SQLite commit. The lost-ack proxy at `managed_exec/fault.rs:194–205` withholds successful real-daemon replies; access tests additionally query the committed row before denying connections.

The independent unfiltered `cargo test --workspace` passed **385 tests, 0 failed, 0 ignored and 0 filtered** across 32 harnesses. It executed all four native EACCES access tests, both invalid-socket non-launching tests, the REGISTER/CLAIM lost-ack tests, cancellation including near-deadline and spawn admission, managed tracking and daemon-loss regressions, protocol diagnostics, native descriptor and environment comparisons, directed signals, macOS terminal tests and foreground-pipeline behavior. The fixture checked native bytes including non-UTF-8 argv, cwd, stdin/stdout/stderr, exit 17, sentinel secrets, zero permits and no replay. The four access cases also checked garbage-collection rows and an untouched sentinel; the 13 outage cases did not perform those cleanup checks in this run. The four access cases directly observed errno 13; the committed-register and committed-claim cases each withheld three successful acknowledgements and inspected the saved row.

The following are the actual unadjusted parent-before-wrapper-spawn to child-control-handshake measurements from this independent run. Each includes startup, scheduling and observation overhead. Internal metadata, handoff and startup subdivisions were not independently measured.

| Scenario | Elapsed ms | Invocations | Exit | Total after recovery |
| --- | ---: | ---: | ---: | ---: |
| absent | 9.604 | 1 | 17 | 1 |
| busy-claim | 16.980 | 1 | 17 | 1 |
| busy-register | 13.691 | 1 | 17 | 1 |
| committed-claim | 19.741 | 1 | 17 | 1 |
| committed-register | 15.657 | 1 | 17 | 1 |
| connect-access-before-claim | 12.658 | 1 | 17 | 1 |
| connect-access-before-register | 9.246 | 1 | 17 | 1 |
| connect-access-claim-ack-loss | 15.616 | 1 | 17 | 1 |
| connect-access-register-ack-loss | 12.622 | 1 | 17 | 1 |
| disconnect-before-read | 12.652 | 1 | 17 | 1 |
| disconnect-claim | 17.275 | 1 | 17 | 1 |
| disconnect-register | 14.204 | 1 | 17 | 1 |
| partial-register | 1812.477 | 1 | 17 | 1 |
| refused | 9.662 | 1 | 17 | 1 |
| shared-budget | 1809.462 | 1 | 17 | 1 |
| stall-claim | 1813.303 | 1 | 17 | 1 |
| stall-register | 1812.716 | 1 | 17 | 1 |

The maximum was **1813.303 ms**, below the 2,000 ms engineering target in these observations; it is not a real-time guarantee. The full unfiltered workspace log and each individual native record are hash-bound in `qa/review.json`.

The existing complete-environment fixture sorts key/value byte pairs before comparison (`fallback_environment.rs:18–21`); it filters no environment entry. The ordinary run had no profiling marker; the instrumented run seeded LLVM's marker in both inputs and used the actual instrumented `arcctl`. Both passed 13 modes and eight existing negative controls. In addition, my `raw-env-matrix.py` compared **unsorted, unfiltered output bytes** between direct and wrapped commands for all 13 outage modes using both binaries: 26/26 matched, with exit 17 and one invocation per command. Its proxy forwarded REGISTER/CLAIM to the real daemon and withheld each of three committed acknowledgements in the committed cases. A separate unsorted comparison rejected all 16 added/removed/changed-entry controls across ordinary and instrumented binaries. An unseeded instrumented-only comparison differed by the added `__LLVM_PROFILE_RT_INIT_ONCE` key; this is recorded, not normalized away or described as ordinary product behavior.

The original assertion bodies in `retries.rs` and `cli.rs` match accepted HEAD exactly after accounting for only the two historical-routing expressions. Both routes executed against the baseline-built binary SHA-256 `59dd9bbd94ed9e112cede82928d29d0065419b03aa406fc1bb697c6256e6d126`; the negative control failed the live-fallback assertions as expected. Unaffected live assertions passed. `0001_phase0.sql` worktree, index and baseline all hash to `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`. The preservation suites passed 16 tests and their mutation controls, including routing, disabled assertions, accounting, migration and environment comparison. Of 205 tracked Rust, script and shell source files in `crates/` and `scripts/`, the maximum is 476 lines; none exceeds 500.

## Coverage and compact evidence

A fresh `bun run test:coverage` and the later full harness each collected native evidence here. Both recomputed the exact integer floors: core 247/256, detector 609/610, storage 653/674, IPC 387/393, daemon 1628/1746 and CLI 877/991. Each had 1,440 raw profiles and 1,440 matching merge inputs. The live evidence test ran after the later harness collector and passed; `coverage-audit.ts` independently reran `nativeCollection` over both real LCOV/LLVM exports, source inventories, merge lists, profiles and child proofs. The full harness exited 0 with zero violations and coverage enabled. These runs do not authenticate the missing original task-4 collection.

The original collection remains at SHA-256 `7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529`: all 44 selections were authenticated. The current combined collection is `d23a70c9bb337a54be4d9169730fce34669c6910f9981442d14f7000510c121d`: 43 current and nine archive entries authenticated. All four archived immediately preceding canonical files matched bytes in the preceding `-12` checkout and were rejected as current; both nested 62-entry predecessor containers round-tripped. The retained task-12 logs and hashes match its preliminary report. The current unfiltered compact suite passed eight tests; the compact-flow suite passed two tests with 190 expectations. Its two fixture terminal validators passed in the 148- and 149-file flows; these are fixture results, not production gate acceptance. The guard probe rejected 20 exact/normalized successor aliases and six structured terminal-command references, enforced root isolation with final artifacts present, accepted an acyclic graph and rejected a cycle. The task-11 operator note limits this guard review to the specified heuristic scope. The staged index check authenticated 145 regular evidence files with identical worktree bytes before this review; adding this JSON and the three reserved task-7 files projects 149, below 150.

## Command receipts

Elapsed time is the wrapper's monotonic duration in milliseconds. Stdout/stderr hashes bind local files in the adjacent JSON; failures are retained separately.

| Actual command | Exit | Elapsed ms | Stdout SHA-256 | Stderr SHA-256 |
| --- | ---: | ---: | --- | --- |
| `cargo test --workspace` | 0 | 132041.148 | `58b23a8b5b311a8a791fd46ab48e7bb9db5d21c256fc90f7ca4c465d650ec2b2` | `5d9dcaf7973b96bedfa747a43ca0ac5b13bf6bba3a4ff2a1ac7ba87500a59825` |
| `cargo fmt --all -- --check` | 0 | 532.556 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 2252.781 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `714aed1a83212db14d366016c5b52986289f0237298c6018b55a4ec9aa36e8be` |
| `bun run test:coverage` | 0 | 161932.057 | `3efbc2e3839f923aee803b6967514c50157fa6f13b66bf8e26965fe9d8733812` | `bc5967d975ba228cabd93d8a140bce15cff6f86eb0c2c1aa191ea6b600dd6233` |
| `./scripts/harness/validate.sh` | 0 | 167581.646 | `63cac1ddfe8c54873e673199cef72ce32239e847978ebedaaa7b6f46c912afa5` | `bc5967d975ba228cabd93d8a140bce15cff6f86eb0c2c1aa191ea6b600dd6233` |
| `bun test scripts/coverage/evidence.test.ts` | 0 | 1282.165 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `8a50ad171037104f291defa78c5236026161b381d1ccf1957f37cf6756113360` |
| `bun test scripts/harness/exec-fail-open-docs.test.ts` | 0 | 204.410 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `35347694e4e10da59991604436f4da12f5b7e09f89fc6983fcdb5b07f0bf3cf4` |
| `bun test scripts/harness/managed-exec.test.ts scripts/harness/exec-fail-open-preservation.test.ts` | 0 | 148017.842 | `674f98d91dfbc02484a3456ac46f6ea55a4419d8ba436ae2124053270b8468f3` | `85bb5fda115a3318239cec3ab4ffa299999d59fbf53e7a34cff0f92ccf3363cf` |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | 0 | 116404.808 | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `69ddb6f1fa9652d513e4c90187c49cad4bae353b27c99f3c93e222013af99e45` |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | 0 | 359774.438 | `cd449c856d12f7f9c1d8f20399f3531cef1c48aba92566fcfe4aaaece10aca3d` | `71129d4efc09c860f8ac3b657abac2de861cfbfd1b89f2824dc380850fe2c60f` |
| `bun target/exec-fail-open/task6-independent/audit.ts` | 0 | 32633.687 | `070c5a252c6f53b23b65b2b614222917d555fdb9b041cd72c3204e6f796cb660` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun target/exec-fail-open/task6-independent/coverage-audit.ts` | 0 | 2381.271 | `01253311ce66f228d57b6314b7b333365b1d745410d428932d703404b000175e` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/environment-audit.py` | 0 | 87.575 | `51c6f12b19dfec3752b17179625ad3dd96f76cfe60598e52337abe65b3dc434f` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/raw-environment.py` | 0 | 796.825 | `b3e0027aea30051c1493e4c4b3985c1c00bd6c19acb164bb08bc210cdf8dd177` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/raw-env-negative.py` | 0 | 1924.920 | `6cbd805c8dd9da2997ec5d8bf2d670428ff1a0721a196c7848e0383a3a6e6832` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/raw-env-matrix.py` | 0 | 20183.717 | `f97c273fcb712669f2bd24c20e240200143edea35772d753ba5473b919429133` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun target/exec-fail-open/task6-independent/guards.ts` | 0 | 48.207 | `5f9fec51e8bda355b6a965cb5c4064a3fb805e37f16672dab0416880053d3166` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun -e import {validateIndex,root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify({stagedRegularAndIdentical:validateIndex(root,[])}));` | 0 | 2138.021 | `b3b2a52db77eec1e2a39147ed7fef06d76acb9d9ff232c0947f530f14e2a910c` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun target/exec-fail-open/task6-independent/reachability.ts` | 0 | 5312.698 | `5693f9545fef502d7fb67ebbc38eadc555fd7d2922c3f5f317182f1003fde1f5` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/historical-audit.py` | 0 | 88.247 | `c94411147fb1b8323192038ad0ee447a55a184efe6a57f24393c8bc64e2fa0b4` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/migration.py` | 0 | 88.749 | `2f31bf758dc818087f44d5e682ec63898a127b8b63658f9259829af7d19a3377` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/inventory.py` | 0 | 79.403 | `50ab810a0bc6430d4ddfb82223b2af8f82740a89b24fbce31436912a2b554335` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `target/debug/arcctl exec -h` | 0 | 11.234 | `b9b1c9a78c30fe91f1284b749cc372225f2da5d69dc4546932ba6d31f9b4476a` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `python3 target/exec-fail-open/task6-independent/invalid-reply.py` | 0 | 91.899 | `33a1ceccd143dcdb59a8bf4406882ca0d3b5e2ae649e5aa98b5924fcfee57135` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `bun -e import {validateGates} from "./scripts/harness/exec-fail-open-gates"; import {root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(validateGates(root)));` | 1 | 2749.465 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `a6939bb3d4c93cca58705db5d7540cb7060dfd7dba999dc81d867d48cca8faae` |

The first two `historical-audit` runs exited 1 because my source comparison selected an ambiguous repeated expression. The third used the surrounding shutdown context and passed; all attempts and scripts are retained. The first `raw-env-negative` run exited 1 on a QA-script `NameError`; the corrected run passed all 16 controls. A `ps` inspection attempt returned `operation not permitted`; no GUI workaround was used. Neither script error is a product finding.

Desktop integration, native Windows, vendor hooks and scheduling remain outstanding. Adoption, user comprehension, retention and financial outcomes are unmeasured.

---

# Preliminary storage review — task-12

## Attempt 4 — independent integrated-checkout reproduction

**Storage-review status: completed. Product verdict: NOT_ASSESSED.** The two required unfiltered suites passed on the integrated source. No compact-storage or harness defect was reproduced in this scope. Fixture subprocesses and simulated leaves do not establish production execution, coverage, or final acceptance. Task-4 must examine these results before collecting fresh gates; task-6 and task-7 remain responsible for final QA and assessment. Task-11's previously reported stale production receipts were not revalidated here.

The operator note of `2026-11-09T05:27:37.462Z` permits this single-task wave to use the writable mission workspace. It supersedes the earlier isolated-checkout restriction and resolves the historical workspace blocker below. The prior attempt text is retained as a record of what was known at that time.

**Reviewer and baseline:** `reviewer`, `oma-qa`; HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. Actual cwd: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Host: `Eunkwangui-MacBookPro.local`, Darwin `27.2.0`, arm64; Bun `1.4.4` (`38afee6c8`), Git `2.56.0`, rustc `1.95.0`, Cargo `1.95.0`, cargo-llvm-cov `0.6.21`. Serena tools were unavailable; the task-named source files were inspected directly. No UI or browser checks were part of this task.

**Source binding:** `currentInputs(root)` returned `769a813345be26d56ba2e98ae488ee872dbe1cf81e0c3d8c4167b6f2ec5a9613` before and after both suites, with all 225 ordered path/mode/SHA-256 entries identical. The 225-row **Contract input file hashes** table below was compared programmatically against the new before-map and matched exactly. The map covers the task-10 and task-11 combined source, tests, validators, manifests and contract-defined inputs; it does not itself establish the correctness of product behavior.

### Executed commands and receipts

Both test commands ran serially and unfiltered from the cwd above. Each Python subprocess wrapper allowed `3,600,000 ms` and measured elapsed time with a monotonic clock. UTC timestamps and raw logs/JSON timing receipts are under `target/exec-fail-open/task12-independent-vo382x72/`; the logs are local artifacts, not additional tracked evidence files.

| Command | Start UTC | End UTC | Elapsed ms | Exit | Result |
| --- | --- | --- | ---: | ---: | --- |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | `2026-11-09T05:29:29.819806Z` | `2026-11-09T05:29:32.142853Z` | 2323.016 | 0 | 5 passed, 0 failed, 0 reported skipped; 72 expectations |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | `2026-11-09T05:29:32.143739Z` | `2026-11-09T05:31:00.074631Z` | 87930.468 | 0 | 2 passed, 0 failed, 0 reported skipped; 114 expectations |

The logs report no skips, and all five and two declared tests respectively appear as passed. The first command's stdout SHA-256 is `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` and stderr SHA-256 is `3d4179bd6cad54722c09fc4bab667520276ea75c5b1b6285f4f8b3368c50d6f9`. The second command's stdout SHA-256 is `cd449c856d12f7f9c1d8f20399f3531cef1c48aba92566fcfe4aaaece10aca3d` and stderr SHA-256 is `98719ff70a4257c5048db2a67e81f2cab3a402a77fd5d5e1d0747fb6f67ceb8e`. The corresponding names are `compact.stdout`, `compact.stderr`, `compact-flow.stdout`, and `compact-flow.stderr` in that directory.

Fingerprint command, before and after: `bun -e 'import {currentInputs,root} from "./scripts/harness/exec-fail-open-inputs"; console.log(JSON.stringify(currentInputs(root)));'`. Before: `2026-11-09T05:29:29.694289Z`–`2026-11-09T05:29:29.818053Z`, 123.567 ms, exit 0. After: `2026-11-09T05:31:00.078969Z`–`2026-11-09T05:31:00.156401Z`, 77.434 ms, exit 0. Both stdout files have SHA-256 `55f6075bc3cf83d37a71bf87cf1e55e9baeb6ad51204bbe94b3ef70746e986b2`; both stderr files are empty with SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

The independent retained-collection audit was rerun as `bun target/exec-fail-open/task-12-review/read-only-audit.ts`, with the exact program reproduced at the end of this report. It ran `2026-11-09T05:31:16.648738Z`–`2026-11-09T05:31:18.489399Z`, 1840.462 ms, exit 0. Program SHA-256: `808d0f5433f3cc57143f8047da67266c88c2281986de425553821fa7336814f2`; `collection-audit.stdout` SHA-256: `6b14dbec0d102cbf3ea2a42497a735cf241ba49bca97603b487c8f9921c682da`; empty `collection-audit.stderr` SHA-256: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

The read-only guard probe was rerun with `bun -e` and the program in **Reproduce the guard and tamper probe** below. It ran `2026-11-09T05:31:18.491333Z`–`2026-11-09T05:31:18.567598Z`, 76.300 ms, exit 0. A copy is `target/exec-fail-open/task12-independent-vo382x72/guard-probe.js`, SHA-256 `21396e11356363922a25cf5f8415b652907cdcf563359a773adabe5da4adb16c`; run it from the cwd above as `bun -e "$(cat target/exec-fail-open/task12-independent-vo382x72/guard-probe.js)"`. `guard.stdout` SHA-256: `df0824e06564e67dd5dc1d9f160acf96bfabd533ea4b9057ac500e580ad2c015`; empty `guard.stderr` SHA-256: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

### Results and source-bound observations

| Required observation | Independent result and location |
| --- | --- |
| Retained collection | `.arc-verify/exec-fail-open/implementation/collection.json` was 1,714,669 bytes, SHA-256 `7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529`, with regular index entry `100644 34e92f34c1eb8e779cc7f6ecae2f4772b22b8483`. Index bytes equaled worktree bytes. The maintained `compactReferences` and `artifact` resolver authenticated all 44 entries against decoded bytes, size and selector digest. The existing **Selected collection entries** table below was programmatically compared to the new audit output: all 44 rows matched. |
| 142 indexed + one unindexed | In the combined fixture, the real retained collection was copied unchanged; 142 index entries plus 143 filesystem/union entries were observed, and the unindexed container was rejected. After adding it in the temporary fixture, 143 were indexed, the bytes remained equal, and all 44 selections resolved. The passing assertions are at `scripts/harness/exec-fail-open-compact-fixture.ts:49` and `scripts/harness/exec-fail-open-compact-flow.test.ts:38`. This transition is a fixture observation, not the actual mission index history. |
| Legacy and complete flows | The legacy per-observation route with 140 retained files was rejected for exceeding the below-150 inventory limit (`scripts/harness/exec-fail-open-compact.test.ts:69`). The compact fixture completed at 148 files with 140 originals; stdout reported 11 commands, 13 simulated outage cases and both terminal exits `[0,0]`. The combined fixture completed at 149 files with 143 retained originals and all downstream artifacts (`scripts/harness/exec-fail-open-compact-flow.test.ts:10`, `:38`). |
| Both terminal validators | For each completed fixture, first and later `validateGates` and `validateEvidence` subprocesses exited 0, as required by `scripts/harness/exec-fail-open-compact-fixture.ts:124`–`:143` and checked by the passing flow tests. These run with a fixture recipe and simulated observations, not a production gate recipe. |
| Exact archive bytes and separation | The combined fixture saved four prior canonical fixture byte sequences for gates, timing, historical and preservation, sealed them into the archive, and compared all four byte-for-byte through `archiveBytes`; each was rejected through current `artifact` selection. Its separate earlier four-entry fixture archive also round-tripped exactly. See `scripts/harness/exec-fail-open-compact-fixture.ts:40`, `:76`, `:104`, `:144` and `scripts/harness/exec-fail-open-compact-flow.test.ts:57`. The retained real `canonical-archive.json` was separately decoded and its four entries authenticated and rejected as current, but all four differ from today's canonical files. No independent prior-canonical source bytes were available to prove that retained real archive matched historical originals. The digest details remain in **Archive selections** below. |
| Successors and cycles | The passing compact suite rejected a logical successor before read, rejected a selected nested successor and a cyclic selected identity, while accepting a sibling (`scripts/harness/exec-fail-open-compact.test.ts:49`). The separate current-source probe rejected 20 exact/normalized successor aliases, 20 guarded reads and six structured/argv terminal references; it accepted a predecessor and acyclic graph, and rejected cyclic and duplicate graphs and a successor binding. This checks the operator-defined heuristic scope, not every shell syntax. |
| Tampering and provenance | The compact suites rejected altered container bytes, unstaged/index-mismatched evidence, malformed encoding/size/digest/path/identity, duplicate or missing entries, unsafe selectors, symlinks, archived-as-current selections, stale source, host/Actor substitutions and fixture-as-production evidence. The combined flow also rejected stale receipt/provenance, attempted successor reads and changed archive scope (`scripts/harness/exec-fail-open-compact.test.ts:20`, `:72`, `:105`; `scripts/harness/exec-fail-open-compact-flow.test.ts:62`). The guard probe separately rejected six container/selector tamper cases. |
| Evidence-file budget | The integrated checkout audit observed 144 indexed files, 144 filesystem files, and a 144-file union under `.arc-verify/exec-fail-open/`. Reserving `implementation/collection-next.json`, `qa/review.json`, `final.md`, `final.json`, and `terminal.json` adds five: projected 149. `qa/review.md` is already included. No extra tracked evidence-file slot remains within the `<150` limit. |

### Findings and handoff

- No reproducible defect was found in the reviewed compact-storage scope. No product code, tests, collection scripts, contracts, migration, coverage accounting, or retained evidence were edited; this report is the only owned deliverable.
- After the report was written, `currentInputs` still returned the same 225-file fingerprint; the inventory still reported 144 indexed, 144 filesystem and 149 projected. Both the staged and unstaged `git diff --check` for this report returned 0. The operator's index synchronization staged this report; the reviewer did not run `git add`.
- Task-4 / backend must consider these fixture results and collect fresh production-bound receipts. The fixture leaves, simulated timing and terminal results must not be used to claim native CLI behavior, coverage floors, or a production pass. Final Cargo, coverage, harness and native checks remain downstream under the original mission contract.
- Task-6 is the next sole writer of this report and must preserve this attempt-4 section verbatim when completing final QA. Desktop, native Windows, vendor hooks and scheduling remain outstanding.

## Historical attempts 1–3 — superseded status

**Quality verdict: NOT_ASSESSED. Verification completeness: blocked.** This is an independent read-only audit of the integrated mission source checkout, not completion of task-12 or final product QA. The two required unfiltered fixture suites were not run: the current working directory is the mission source workspace and no current AV-provided isolated integrated checkout was identified for this run. The task explicitly requires running them in that checkout, not in `sourceWorkspacePath`. The older AV workspaces inspected have different source fingerprints. No product or harness defect was established by this audit.

## Attempt 3 supplement — in-memory storage checks

The attempt 3 process was again started at the mission source workspace, with `AGENT_VALLEY_MANAGED_RUN=1` and no workspace-path variable. After `2026-11-09T05:21:25Z`, all 19 visible directories under `.agent-valley/parallel/workspaces` were checked using the current `currentInputs` implementation: 18 had fingerprints different from the 225-file integrated fingerprint below; one lacked the required `claims-review.md` input. None was a matching AV-provided integrated checkout. No older Actor checkout was changed. The fixture suites were therefore not run; the following read-only probes are additional observations, not a replacement for them.

The archive probe ran `bun -e` with the script in the first code block below, in the mission source cwd shown under Scope. On `Darwin 27.2.0 arm64`, Bun `1.4.4`, it started `2026-11-09T05:22:17.498Z`, ended `2026-11-09T05:22:19.703Z`, took `2205.353 ms` by `performance.now()`, and exited `0`. Its source fingerprint before and after was `769a813345be26d56ba2e98ae488ee872dbe1cf81e0c3d8c4167b6f2ec5a9613`; both ordered input maps contained 225 files. The per-file hashes and modes below remain bound to that fingerprint. The prior collection was 1,714,669 bytes with SHA-256 `7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529`; its index mode was `100644`, object `34e92f34c1eb8e779cc7f6ecae2f4772b22b8483`, and staged bytes equaled worktree bytes. All 44 selections were resolved through `compactReferences` and `artifact`, then matched against the original decoded bytes, size and selected-entry SHA-256 in the table below.

The probe also constructed a **memory-only** 48-entry combined container (the 44 original entries marked current plus the four current canonical files marked archive), SHA-256 `f04901d1031c95506af6116259db54affdad8fd59c3f90540deb80ea95c26bf3`. It decoded each archived selection through `selectedBytes(..., true)`, compared exact bytes and digest to its current canonical input, rejected each as current, and rejected a current selection as archive. This verifies the current bytes through the in-memory container codec; it does not test `seal`, staging a new combined container, `archiveBytes`, or the full assembler. Those remain part of the unrun fixture suites. The older retained `canonical-archive.json` represents different earlier canonical bytes and is not presented as an archive of these four current files.

| Current canonical path | Bytes | SHA-256 | Exact in-memory round trip | Rejected as current |
| --- | ---: | --- | --- | --- |
| `implementation/gates.json` | 43086 | `f63b72e5c35d74beac12c50093cea6bde3f9e9a35ccc0e7eb8e65297cf405dff` | yes | yes |
| `implementation/timing.json` | 91280 | `8fc38848c9e09d1623b56eca89068bc0eb6cb7cd9ec185f9b39b86bfa71f01f6` | yes | yes |
| `implementation/historical.json` | 74721 | `d4dfb56b6d6315631e51fc00a9d2db10a7c97ab01077eeeea85c31b6d41f71c3` | yes | yes |
| `implementation/preservation.json` | 36550 | `05c0165a784ae857de617f405b807cefea801b376b593e7e9df9da1fe259d986` | yes | yes |

Paths in the table are relative to `.arc-verify/exec-fail-open/`. Inventory at the probe was 144 indexed, 144 filesystem and 144 union files; the five additional reserved downstream paths project 149. This is not a completed final inventory.

The second in-memory probe below ran in the same cwd and host, started `2026-11-09T05:22:43.367Z`, ended `2026-11-09T05:22:43.450Z`, took `82.497 ms`, and exited `0`. It independently observed rejection of 20 exact/normalized successor references, the same 20 guarded reads, six terminal command references in structured-command or argv context, a cyclic graph, a duplicate graph, an added successor binding, and six container/selector tamper cases. It accepted one legitimate predecessor and an acyclic graph. The 225-file fingerprint and file map were unchanged across this probe. These probes do not establish the unrun 142-indexed-plus-one-unindexed transition, legacy and combined complete flows, both terminal validator subprocesses, or all tamper cases in the test files.

The inline commands and compact observations above are the attempt 3 receipts. Their output was returned in the execution transcript; no attempt 3 raw logs were created in a collecting checkout because no such checkout was supplied. The earlier read-only audit's on-disk logs and their digests are listed below. After writing this report, both inline probes were extracted from the report and executed with a 3,600-second subprocess timeout to check transcription: both exited `0`, and each again reported the same before/after source fingerprint. `git diff --check -- .arc-verify/exec-fail-open/qa/review.md` passed, and a scoped unstaged diff excluding this report and `av.yaml` returned 0. A fresh run in the required checkout must retain its own raw logs under that checkout's `target` directory and provide the required one-hour-or-longer wrapper timeout.

### Task acceptance status

- **Incomplete:** the two unfiltered fixture-suite commands, their subprocess observations and their isolated-checkout raw logs were not obtained.
- **Partially checked:** all 44 retained selections, staged collection mode and bytes, the 225-file input fingerprint and file hashes, four current canonical in-memory round trips, sampled separation/guard/tamper checks, and a projected 149-file inventory are recorded here.
- **No reproduced product or harness defect:** the blocking issue is the missing matching isolated checkout, owned by the Chief Director / AV workspace provisioner. Task-4 must not interpret these checks as product acceptance.

### Reproduce the archive probe

Run the following from the integrated checkout being inspected; it performs no writes. Its times and inventory will vary.

```sh
bun -e '
import { currentInputs, root, directory, canonicalBytes, reference, artifact, hash } from "./scripts/harness/exec-fail-open-inputs";
import { compactReferences } from "./scripts/harness/exec-fail-open-compact";
import { decodeContainer, retainedEntry, selectedBytes } from "./scripts/harness/exec-fail-open-container";
import { inventory } from "./scripts/harness/exec-fail-open-inventory";
import { spawnSync } from "node:child_process";
const started = new Date().toISOString(), clock = performance.now();
const must = (v, m) => { if (!v) throw Error(m); };
const before = currentInputs(root);
const priorPath = directory + "/implementation/collection.json", prior = canonicalBytes(root, priorPath), priorRef = reference(root, priorPath), priorEntries = decodeContainer(prior).entries, refs = compactReferences(root, priorRef);
must(priorRef.sha256 === "7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529" && refs.length === 44 && priorEntries.length === 44, "unexpected collection");
const staged = spawnSync("git", ["show", ":" + priorPath], {cwd: root, maxBuffer: 8 * 1024 * 1024});
const stage = spawnSync("git", ["ls-files", "--stage", "--", priorPath], {cwd: root, encoding: "utf8"});
must(staged.status === 0 && staged.stdout.equals(prior) && stage.status === 0 && stage.stdout.startsWith("100644 "), "collection index bytes/mode");
const selected = refs.map(ref => { const entry = priorEntries.find(e => e.path === ref.selector.path); const raw = artifact(root, ref); must(entry && raw.equals(Buffer.from(entry.bytes, "base64")) && hash(raw) === ref.selector.sha256, "prior selection mismatch"); return {path:entry.path,size:raw.length,sha256:hash(raw)}; });
const originals = ["gates","timing","historical","preservation"].map(name => { const path = directory + "/implementation/" + name + ".json"; const bytes = canonicalBytes(root, path); return {path, bytes, entry:retainedEntry(path, bytes)}; });
const combined = Buffer.from(JSON.stringify({schemaVersion:1,kind:"retained-evidence-container",purpose:"combined",entries:[...priorEntries.map(e => ({...e,scope:"current"})),...originals.map(o => ({...o.entry,scope:"archive"}))]}));
const decoded = decodeContainer(combined);
const archives = originals.map(o => { const selector = {path:o.path,sha256:o.entry.sha256}; const raw = selectedBytes(combined, selector, true); must(raw.equals(o.bytes) && hash(raw) === hash(o.bytes), "archive round trip"); let rejected = false; try { selectedBytes(combined, selector); } catch(e) { rejected = String(e).includes("archived bytes"); } must(rejected, "archive accepted as current"); return {path:o.path,size:o.bytes.length,sha256:hash(o.bytes),roundTripExact:true,rejectedAsCurrent:rejected}; });
const currentEntry = priorEntries[0], currentSelector = {path:currentEntry.path,sha256:currentEntry.sha256};
must(selectedBytes(combined,currentSelector).equals(Buffer.from(currentEntry.bytes,"base64")), "current mismatch");
let currentRejectedAsArchive = false; try { selectedBytes(combined,currentSelector,true); } catch(e) {currentRejectedAsArchive = String(e).includes("not archived evidence");}
must(currentRejectedAsArchive, "current accepted as archive");
const plans = ["implementation/collection-next.json","qa/review.md","qa/review.json","final.md","final.json","terminal.json"].map(p => directory + "/" + p);
const counts = inventory(root, plans), after = currentInputs(root);
must(before.sha256 === after.sha256 && JSON.stringify(before.files) === JSON.stringify(after.files), "inputs changed");
console.log(JSON.stringify({started,ended:new Date().toISOString(),elapsedMs:performance.now()-clock,cwd:root,baseline:"00773c69a8e5e7fd417fd5cc78d03e222364f609",sourceHashBefore:before.sha256,sourceHashAfter:after.sha256,inputCount:Object.keys(before.files).length,collection:{bytes:prior.length,sha256:priorRef.sha256,index:stage.stdout.trim(),indexBytesEqual:true,selected},memoryCombined:{sha256:hash(combined),entries:decoded.entries.length,archives,currentRejectedAsArchive},inventory:{filesystem:counts.filesystem,indexed:counts.indexed,retained:counts.retained,added:counts.added,projected:counts.projected}}));
'
```

### Reproduce the guard and tamper probe

```sh
bun -e '
import {resolve} from "node:path";
import {root,directory,currentInputs} from "./scripts/harness/exec-fail-open-inputs";
import {rejectFutureReferences,validateGraph,predecessorBindings} from "./scripts/harness/exec-fail-open-graph";
import {predecessorReads,checkPredecessorRead} from "./scripts/harness/exec-fail-open-reads";
import {retainedEntry,decodeContainer,selectedBytes} from "./scripts/harness/exec-fail-open-container";
const started=new Date().toISOString(), clock=performance.now(), before=currentInputs(root);
const must=(v,m)=>{if(!v)throw Error(m)}, rejects=(fn,part)=>{try{fn()}catch(e){must(String(e).includes(part),"wrong rejection: "+e);return}throw Error("accepted: "+part)};
const paths=["qa/review.md","qa/review.json","final.md","final.json","terminal.json"];
let aliases=0,reads=0;
for(const p of paths){const path=directory+"/"+p;for(const alias of [path,"./"+path,resolve(root,path),directory+"/implementation/../"+p]){rejects(()=>rejectFutureReferences({path:alias}),"successor");aliases++;predecessorReads(root,"gates",()=>rejects(()=>checkPredecessorRead(root,alias),"attempted successor read"));reads++;}}
let commands=0;
for(const file of ["exec-fail-open-gates.test.ts","exec-fail-open-evidence.test.ts"]){for(const cmd of [{command:{cwd:"scripts/harness",args:[file]}},{argv:["bun","./scripts/harness/"+file]},{command:{cwd:"scripts",args:["../scripts/harness/"+file]}}]){rejects(()=>rejectFutureReferences(cmd),"successor");commands++;}}
rejectFutureReferences({path:directory+"/implementation/commands/focused-cli-final.json"});
predecessorReads(root,"gates",()=>checkPredecessorRead(root,directory+"/implementation/commands/focused-cli-final.json"));
const good=[{id:"a",requires:[]},{id:"b",requires:["a"]},{id:"c",requires:["b"]}];validateGraph(good);
rejects(()=>validateGraph([{id:"a",requires:["b"]},{id:"b",requires:["a"]}]),"cycle");
rejects(()=>validateGraph([{id:"a",requires:[]},{id:"a",requires:[]}]),"duplicate");
rejects(()=>predecessorBindings(good,"b",{a:{sha256:"a".repeat(64)},c:{sha256:"b".repeat(64)}}),"successor");
const e=retainedEntry(directory+"/implementation/commands/unit.json",Buffer.from("{\"ok\":true}"));
const container={schemaVersion:1,kind:"retained-evidence-container",purpose:"collection",entries:[e]};
const raw=Buffer.from(JSON.stringify(container));decodeContainer(raw);must(selectedBytes(raw,{path:e.path,sha256:e.sha256}).equals(Buffer.from(e.bytes,"base64")),"selection");
let tamper=0;for(const entry of [{...e,bytes:e.bytes+"!"},{...e,sha256:"0".repeat(64)},{...e,path:"../bad"},{...e,size:0}]){rejects(()=>decodeContainer(Buffer.from(JSON.stringify({...container,entries:[entry]}))),"EXEC_FAIL_OPEN");tamper++;}
rejects(()=>decodeContainer(Buffer.from(JSON.stringify({...container,entries:[e,e]}))),"duplicate");tamper++;
rejects(()=>selectedBytes(raw,{path:e.path,sha256:"0".repeat(64)}),"altered");tamper++;
const after=currentInputs(root);must(before.sha256===after.sha256&&JSON.stringify(before.files)===JSON.stringify(after.files),"source changed");
console.log(JSON.stringify({started,ended:new Date().toISOString(),elapsedMs:performance.now()-clock,cwd:root,sourceHashBefore:before.sha256,sourceHashAfter:after.sha256,successorAliasesRejected:aliases,guardReadsRejected:reads,structuredAndArgvTerminalCommandsRejected:commands,predecessorAccepted:true,validDagAccepted:true,cyclicAndDuplicateGraphsRejected:true,successorBindingRejected:true,tamperRejections:tamper}));
'
```

### Reproduce the checkout fingerprint search

```sh
bun -e 'import {readdirSync} from "node:fs"; import {resolve} from "node:path"; import {currentInputs} from "./scripts/harness/exec-fail-open-inputs"; const base=resolve(".agent-valley/parallel/workspaces"); const expected=currentInputs(process.cwd()).sha256; console.log("mission",expected); for(const n of readdirSync(base)){try{const i=currentInputs(resolve(base,n)); console.log(n,i.sha256,Object.keys(i.files).length,i.sha256===expected?"MATCH":"different");}catch(e){console.log(n,"unavailable",String(e).split("\n")[0]);}}'
```

## Scope and source binding

- Reviewer/skill: `reviewer`, `oma-qa`. Baseline requested and observed HEAD: `00773c69a8e5e7fd417fd5cc78d03e222364f609`.
- Read-only audit cwd: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Git HEAD `00773c69a8e5e7fd417fd5cc78d03e222364f609`; HEAD tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`. This cwd is the mission source path, not an identified isolated checkout.
- Contract `currentInputs` fingerprint: before `769a813345be26d56ba2e98ae488ee872dbe1cf81e0c3d8c4167b6f2ec5a9613`; after `769a813345be26d56ba2e98ae488ee872dbe1cf81e0c3d8c4167b6f2ec5a9613`; 225 input files. These two observations bracket the read-only audit below, **not** the unrun fixture suites. Full per-file hashes and modes appear below.
- Host: `Darwin 27.2.0 arm64`, hostname `Eunkwangui-MacBookPro.local`; `bun 1.4.4`; `git version 2.56.0`. Times are UTC; elapsed times are monotonic.
- A scoped `git diff --quiet -- . ':!av.yaml'` returned 0, meaning no unstaged tracked differences in that scope at inspection. `git status --short -- av.yaml` printed `av.yaml: Operation not permitted` despite exit 0. No access workaround was attempted. Four pre-existing untracked `.profraw` files were observed in the root status; none was edited.
- Serena code-intelligence tools were not available in the tool inventory. Inspection used direct reads of the task-named files and read-only Git operations; no project-wide native code search was substituted.

## Executed read-only receipt

- Command: `bun target/exec-fail-open/task-12-review/read-only-audit.ts`
- Cwd: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`; started `2026-11-09T05:18:01.548541+00:00`; ended `2026-11-09T05:18:03.788666+00:00`; elapsed `2239.661 ms`; exit `0`. The wrapper allowed 3,600,000 ms.
- Script SHA-256 `808d0f5433f3cc57143f8047da67266c88c2281986de425553821fa7336814f2`; stdout SHA-256 `4610416d9964a5b24b1e6f322b9748abd1965a87e549cd6bd2155c91281c6091`; stderr SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (empty). Raw output, receipt, and script are under `target/exec-fail-open/task-12-review/` in the **mission source checkout**. They are local, ignored files, not captured delivery evidence from an isolated collecting checkout. The script is reproduced verbatim at the end of this report.

## Observations

- `.arc-verify/exec-fail-open/implementation/collection.json`: 1714669 bytes; SHA-256 `7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529`; index entry `100644 34e92f34c1eb8e779cc7f6ecae2f4772b22b8483 0	.arc-verify/exec-fail-open/implementation/collection.json`. `git show` index bytes matched the worktree, and all 44 selectors were individually resolved with `compactReferences` and `artifact`; decoded bytes, sizes and selected-entry digests matched. The full selector list is below.
- `.arc-verify/exec-fail-open/implementation/canonical-archive.json`: purpose `archive`, SHA-256 `f95972b2b55c2c8149cce244d78382472a2f5fbe85b3bba8406c766660b415cd`. All four archive selections decoded to their encoded bytes and matched their declared digests; `artifact` rejected each as current evidence. Their current canonical paths had different hashes, so this retained archive does **not** independently establish that the archived bytes equal each original prior-canonical file before archival. Attempt 3 separately round-tripped the four current canonical inputs in memory as described above; the fixture round-trip remains unexecuted.
- Inventory at the audit, before writing this report: indexed `143`, filesystem `143`, union `143`. Including the six planned downstream paths yields `149` (<150). A separate post-report observation found indexed `144`, filesystem `144`, union `144`, with five planned additions and the same projected total `149`. No `git add` was run by this reviewer; the report appeared in the index after the operator-described index synchronization. This is a projection, not a completed final inventory.
- The required 142-indexed-plus-one-unindexed transition, 140-file legacy flow, complete combined flow through both terminal validators, and the unfiltered cycle/successor and tamper tests have not been independently executed. The attempt 3 in-memory probes cover only the cases stated above. Reading test bodies is not evidence that those assertions pass.

## Blocker and handoff

**Infrastructure blocker — owner: Chief Director / AV workspace provisioner.** The active process received the mission source checkout as its cwd. `AGENT_VALLEY_MANAGED_RUN=1` was present, but no workspace path was injected. The visible older workspace `a546003e0df37906-555ada841ff8a3cc-10` had fingerprint `fa87a1739dae8de54cd52560c840711519c74d119c1dc0482a040a80d32ce1fe`, and `...-9` had `b25d3187aaef22c08070097c601df84a3f71f1194fcfd3ebd89e5fb4082e4611`, both different from the integrated source fingerprint above. The supplied task-10 and task-11 delivery workspace paths ending `-2` and `-11` were not present. No previous Actor workspace was changed or used for tests.

The Chief should route this task to a fresh AV-provided writable isolated checkout containing the accepted integrated source and staged prior collection. The reviewer must then execute the following commands **unfiltered**, independently and serially, in that checkout, with at least 3,600,000 ms allowed per wrapper and raw logs in that checkout’s `target` directory:

```sh
bun test scripts/harness/exec-fail-open-compact.test.ts
bun test scripts/harness/exec-fail-open-compact-flow.test.ts
```

Repeat the input fingerprint before and after those tests, record the actual host/cwd/timestamps/exits/elapsed times/log digests, and report the 142+1 transition, 140/148-file flow, combined 143/149-file flow, all 44 selections, four prior-canonical round trips, archive/current separation, successor/cycle checks, tamper rejection, and both terminal validator exits. Route any reproduced harness defect to its implementation owner. Task-4 must not treat this preliminary report or its read-only checks as a product pass; fresh gate collection and final independent QA remain downstream.

## Selected collection entries

| Logical path | Bytes | SHA-256 |
| --- | ---: | --- |
| `.arc-verify/exec-fail-open/implementation/commands/clean-checkout.json` | 37091 | `4eb69a664d8f0898e480110d69e2cbab74b01b514615bdce743ce11db45f5cde` |
| `.arc-verify/exec-fail-open/implementation/commands/clippy.json` | 37074 | `ccc7e425bcdeee5a1524b9fd0d07f2480d468e5dc8d4ac730a71f5e8c458f079` |
| `.arc-verify/exec-fail-open/implementation/commands/coverage-cli.json` | 37085 | `783beadb5985d82e75593ad6c9415304ecbca4e297f7c8ab842ffbe380dd090b` |
| `.arc-verify/exec-fail-open/implementation/commands/coverage-harness.json` | 37088 | `8720f75ae9bfe248ee7bfc68e39c6690d1ab4f5ece17750bcbe763318ece9e4b` |
| `.arc-verify/exec-fail-open/implementation/commands/coverage.json` | 37052 | `ddd4529b2cd8d212f0ae9b2895797154f15ad4f3be28e2b917bb541bfeb3cf13` |
| `.arc-verify/exec-fail-open/implementation/commands/docs.json` | 37074 | `18d0ecb4486a098a8a8bd48a14ce4e5e4c07a5d1873aae5c2250dabd9bdf2839` |
| `.arc-verify/exec-fail-open/implementation/commands/fmt.json` | 37033 | `58d1a714517485dd218fc8c6d942f1c8e992b10d11de1154687a2a6e33348bae` |
| `.arc-verify/exec-fail-open/implementation/generated-harness/coverage-verification.json` | 37882 | `50bc8e253e15aadd7ca2e6d9a0ef6b4375f811874def8a4c76f74be9abecd172` |
| `.arc-verify/exec-fail-open/implementation/generated-harness/equivalence.json` | 174684 | `d021123d353bc49423ad12ea38aff3f5cab8cfac746dd762a31c5291bfc42f5a` |
| `.arc-verify/exec-fail-open/implementation/generated-harness/invocations.json` | 15034 | `a977948c0347441d4b403bd9766903827e8eddfc7a8de060dd77377d928f18e7` |
| `.arc-verify/exec-fail-open/implementation/commands/harness.json` | 37231 | `c863d477e4e783fb651c789b369ca47df9f394b447fc75dc1f51cb4cc209f3ff` |
| `.arc-verify/exec-fail-open/implementation/commands/live-coverage.json` | 37083 | `9892bb8e80f5b64e8004d34308d189452b57f671016a019d8f64d85d4b4e46eb` |
| `.arc-verify/exec-fail-open/implementation/task4-attempt8-recomputation.json` | 81117 | `a4635c39f22830d5c8a137720319064dc81616dbeb0321c9e91e604d683df36f` |
| `.arc-verify/exec-fail-open/implementation/task4-attempt8-packaging-diagnostic.json` | 1302 | `51b0a4d3bd462e697e3671c2b4a874f1667f8d2fd45d3de238343366754cdc00` |
| `.arc-verify/exec-fail-open/implementation/task4-attempt8-preflight.json` | 60404 | `8971e48b0016b63e6debc44595721038cbfb41dba822cea60093b1297e1d7e0b` |
| `.arc-verify/exec-fail-open/implementation/commands/preservation.json` | 37239 | `7b801e4ce54c6fd407becab6d35866b1e063b3321e71791dee09c08ed79d0713` |
| `.arc-verify/exec-fail-open/implementation/report-before-attempt8.json` | 31024 | `2bef0ac2f555d8f03e8ddcb6251f3b156e94a8ad1d4e8fa3da6edd4a04d3b85a` |
| `.arc-verify/exec-fail-open/implementation/commands/workspace.json` | 94530 | `8bf5cde649c61f7f04a6ba5e5ab029859b1e859a4ab2f33b9fea1db73f30ddf1` |
| `.arc-verify/exec-fail-open/implementation/native/absent.json` | 1267 | `2e421947cb80a6d2e9c2ea19648105e9b04e836f51412b10b67eb9ac41f3e5c6` |
| `.arc-verify/exec-fail-open/implementation/native/busy-claim.json` | 4177 | `00055fc2d34f099078271bd2aaea4367e9d46cc3412bff607a28486c55564165` |
| `.arc-verify/exec-fail-open/implementation/native/busy-register.json` | 3310 | `717ef0996c6c24fe7df11de2e276d860fb2643ba400880c886f64e24589e5460` |
| `.arc-verify/exec-fail-open/implementation/native/cancellation.json` | 606 | `bac0cb398ee5a33d6428c1572e3c2364e53ee998846ef93da8d75018f09283a4` |
| `.arc-verify/exec-fail-open/implementation/native/committed-claim.json` | 4225 | `5c3815368a7b8ffb443717e9ff445913c4e2c5656cd21bdcba358690bd2d7b71` |
| `.arc-verify/exec-fail-open/implementation/native/committed-register.json` | 3570 | `6fd72faeab3be4a5f0d8ae1ca3c9a848071ef8efcc417da4f215a9e93cc7d5c0` |
| `.arc-verify/exec-fail-open/implementation/native/descriptors.json` | 1026 | `ef7281efee910a39e8301e63c2c089c4268337b6309b50023e15b2583b66fa8d` |
| `.arc-verify/exec-fail-open/implementation/native/disconnect-before-read.json` | 2296 | `d4519678dce559fc158ecd489e19f12287ad05436b961c7d990ef10b40e67897` |
| `.arc-verify/exec-fail-open/implementation/native/disconnect-claim.json` | 4210 | `36221ec899c4b9b28a05f2e00264282333e5ae2134b13dd097691d42c4a08629` |
| `.arc-verify/exec-fail-open/implementation/native/disconnect-register.json` | 3346 | `573098b7006f725c393992f42a03615430f6224730f36743209cfb29ab7c4277` |
| `.arc-verify/exec-fail-open/implementation/native/environment-negative-controls.json` | 768 | `e01054970a9155669c0026d7c84249cb8110a7394f7bebed4745e0d404f000d8` |
| `.arc-verify/exec-fail-open/implementation/native/environment.json` | 357740 | `9e3819bf20b6cd8917bcec7f4e72a11bbc84f195c49cba8787d3fac69f584dc6` |
| `.arc-verify/exec-fail-open/implementation/native/negative-control.json` | 641 | `1df5caefcc90754219749868d47ea83875c1c8e059f6bc37c375c6ed3f455fb5` |
| `.arc-verify/exec-fail-open/implementation/native/nonlaunch.json` | 599 | `a8ef5a7ab053e4e80137549171908d5108ca5a3127629894d979ac7b3dc899f7` |
| `.arc-verify/exec-fail-open/implementation/native/partial-register.json` | 3348 | `542cf6eb5d84117a56cc20079c8c798341e974e535a9dfa706ead59f3d614c75` |
| `.arc-verify/exec-fail-open/implementation/native/pipeline.json` | 610 | `ee96edb75927654e27f3fedb4dd355686b80445206ee70a10e703e8118e54625` |
| `.arc-verify/exec-fail-open/implementation/native/refused.json` | 1269 | `935c406106172edd328d2d3dea7f42c05812a319cb5780006130e66b032d86f9` |
| `.arc-verify/exec-fail-open/implementation/native/shared-budget.json` | 4219 | `077c686649bea2f8128b8dfd050efd2f0a11e009fa9f3634753b85f67925639c` |
| `.arc-verify/exec-fail-open/implementation/native/signals.json` | 637 | `0e64d33534765a338ccf81d444d52aa955ff1f0521475ae27a056bc937b74510` |
| `.arc-verify/exec-fail-open/implementation/native/sigstate.json` | 679 | `b804ec533a2b5eab5dd8083f60152251d4b94918b91f06d8ed6193d0b283d178` |
| `.arc-verify/exec-fail-open/implementation/native/stall-claim.json` | 4203 | `43ec93cb29ab805f2fbb1b3c511d8672286b92803e3252e343046eb40e38f0e8` |
| `.arc-verify/exec-fail-open/implementation/native/stall-register.json` | 3336 | `387652b3f1ba66721a0231dad3d3ebd5bcb55c572ebc884b9a3328bf63ec1a94` |
| `.arc-verify/exec-fail-open/implementation/native/terminal-1.json` | 616 | `b69510c5f9ef8beb7e638fee24de8f0391d452637861303ab943cebdce9b92d4` |
| `.arc-verify/exec-fail-open/implementation/native/terminal-2.json` | 608 | `734fa34a415f92d58f78b7d51b704746cc48db508d036a9b4d19a8360b53dfc7` |
| `.arc-verify/exec-fail-open/implementation/native/terminal-3.json` | 606 | `891a3b6803a0b88bfd3af604f08407ba6b965a71e98bc0bc9f1306fb5fc67330` |
| `.arc-verify/exec-fail-open/implementation/native/terminal-4.json` | 613 | `859d3afc10640e1792c2ef84a8b1a710c8abdeb259b9516569cebbba9db55901` |

## Archive selections

| Logical path | Bytes | Archived SHA-256 | Current file SHA-256 | Rejected as current |
| --- | ---: | --- | --- | --- |
| `.arc-verify/exec-fail-open/implementation/gates.json` | 43341 | `f1b4cc5ee96503c33036dce0e3c9ebfcaeea650918309b97af0e5037d50afd18` | `f63b72e5c35d74beac12c50093cea6bde3f9e9a35ccc0e7eb8e65297cf405dff` | true |
| `.arc-verify/exec-fail-open/implementation/timing.json` | 84772 | `702e77025746b5e8b0270930220bee17f9dfd2b8faff22e691ecae50153f6b22` | `8fc38848c9e09d1623b56eca89068bc0eb6cb7cd9ec185f9b39b86bfa71f01f6` | true |
| `.arc-verify/exec-fail-open/implementation/historical.json` | 73215 | `357f46bd33822efff2ce5f9c6aeadfa1518530060e21d75b9db5143f05a32539` | `d4dfb56b6d6315631e51fc00a9d2db10a7c97ab01077eeeea85c31b6d41f71c3` | true |
| `.arc-verify/exec-fail-open/implementation/preservation.json` | 35060 | `722f4c38ef3d2a9938e5209155c50aa534411843f98f5024ab3abb7e91a3bf9c` | `05c0165a784ae857de617f405b807cefea801b376b593e7e9df9da1fe259d986` | true |

## Contract input file hashes

Each mode is octal permission bits, matching the `currentInputs` implementation. SHA-256 values are of worktree file bytes at the read-only audit; the fingerprint is SHA-256 of the implementation’s ordered JSON map of these entries.

| Path | Mode | SHA-256 |
| --- | --- | --- |
| `.arc-verify/exec-fail-open/claims-review.md` | `644` | `1e5acbb46ab202e963c0560e54c9ce7b265d7c85755b653d651c48d9f066d8b3` |
| `.arc-verify/exec-fail-open/contract.json` | `644` | `474fbf2004483dfd56d3858c689dd77674c3dab008d34f8fa28b121dee524ff3` |
| `.arc-verify/exec-fail-open/contract.md` | `644` | `bb25c8603508ef555e786a4c0acc72b43f28e260d24f5b7c0400309d5375d6ed` |
| `.arc-verify/rust-coverage/contract/accounting.json` | `644` | `850f59740a644f3807b104bd6471a028bc625b10ca322cc2379f4c4eed0d14bb` |
| `.gitignore` | `644` | `c38ed4c5d6f44c8efb96edfbcd33ec8464423c56598d148c4bec90366e78aa88` |
| `Cargo.lock` | `644` | `cf2c459f1af04632d571d21c16961c328c43424939b7b3c5176695de6881fa7f` |
| `Cargo.toml` | `644` | `36231fcca8a23d18329a44d3440a30f23bf550ee31f7f540eb02f8a88a6614cd` |
| `README.md` | `644` | `20daf6024c582bcbeb2d64e4596a3124f44125a3cdecc641d3da4556f6afe5f6` |
| `crates/arc-cli/Cargo.toml` | `644` | `6f195995dbbdc42e85628ee09763cdb268d14e3d98d15579b68bb558b581cb33` |
| `crates/arc-cli/src/classify.rs` | `644` | `5721f556725dca554d0e7de130c84f6831a6989eef2d39e137a65d58fcff0028` |
| `crates/arc-cli/src/doctor.rs` | `644` | `0bf990c8dd1a694ee42307e9a8cb5c471994746916c5609d82a610b0e0765221` |
| `crates/arc-cli/src/exec.rs` | `644` | `355e1b6c4fbd823a72d63cd1029da47343c372415dccac1132c1e7468be0f955` |
| `crates/arc-cli/src/exec/descriptors.rs` | `644` | `059437b7c7fa2cbe19eeb2d70d286043e59e2d5aaca6c1b481eb7b06e1d1d668` |
| `crates/arc-cli/src/exec/preparation.rs` | `644` | `757dcc559ecddb58cf311750033e49c36d5268f3efc24ff26df8e01e9acf0aa1` |
| `crates/arc-cli/src/exec/signals.rs` | `644` | `9be11a96aa76a80ae364f3352d84e9b3ac9b3ae0ec5cab75efa075ff0620ff1e` |
| `crates/arc-cli/src/exec/signals/handoff.rs` | `644` | `a48d58a0c973accc674a4cd4a6c941cdb3b94268f3330e54c30c45bb1acb1176` |
| `crates/arc-cli/src/exec/signals/tests.rs` | `644` | `d7744025d5dc7759db58ae41f3ac862331c347b606aeda280dbba7734268e23d` |
| `crates/arc-cli/src/exec/terminal.rs` | `644` | `c3a5c74713d36c1fde3a332563d5cdb4260aa341a86e50573587f9eb95a882df` |
| `crates/arc-cli/src/exec/transport.rs` | `644` | `fe0bb94493b0813000d9bbe795fa656d6ba11ebf4c9017de576aea0803d1cfdd` |
| `crates/arc-cli/src/exec/transport/access_tests.rs` | `644` | `7f22a37a060787d899aadeaebd6c926204b3dba78bab79c18a9bda13186fbdbf` |
| `crates/arc-cli/src/exec/transport/tests.rs` | `644` | `83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3` |
| `crates/arc-cli/src/job.rs` | `644` | `d96ef24bf66f511ef5b1030a5a08d3197f96e9c083fbec6366671160cdcf689b` |
| `crates/arc-cli/src/main.rs` | `644` | `9f2a9a2e635aaab068659a4319886862ca7af09c5f48fe05cdc71bb7b0ce8c03` |
| `crates/arc-cli/tests/binary.rs` | `644` | `fcdf17704312c735e33af4b8cc9947abbecc06ef1205f3a81c551fe0c9fe8da3` |
| `crates/arc-cli/tests/classification.rs` | `644` | `0ba8a5ffcb0cdd54d5299941d61926e8bf4f5b358764219a25c4a205efbd6515` |
| `crates/arc-cli/tests/fixtures/child.rs` | `644` | `85121952a46c37c0577cfaf96561fe17a9b25e4c4b06f62396fdfda5961ee53a` |
| `crates/arc-cli/tests/fixtures/pipeline.rs` | `644` | `42074f27e690c995aa27dd7629f3a32499a9bf8453e0b7e18b749771e4dbea86` |
| `crates/arc-cli/tests/schema.rs` | `644` | `d63b9f3ec80835ede61caf740c5a632a54df91d600a35b019fcff7270eb29a5a` |
| `crates/arc-core/Cargo.toml` | `644` | `2f28861717fe5aefb8f1124157aa9d8b1ba1cdbabc9593bde7749a03cb3822fd` |
| `crates/arc-core/src/lib.rs` | `644` | `9a1d25ac424d2a9d19edf296f7eb0b276de9550392b2eaa107c89f09cb29f95c` |
| `crates/arc-core/src/process_identity.rs` | `644` | `1c7c7a5e9dce3941541ca0646a22aaab1713f3047334eebc7e533d0110ad8f56` |
| `crates/arc-core/tests/coverage.rs` | `644` | `04a5e8477d33a1a224a77316d843f291890f1ee8176a524fa4714d8051795e18` |
| `crates/arc-core/tests/identity.rs` | `644` | `449db78dbdcd3dcda98ea8af3fe2ea7b54d40a571496b5c832e2707093a18f07` |
| `crates/arc-core/tests/runtime.rs` | `644` | `adcc3e99f8eba6996a4c9069a99ed85715a5656a2cf2ae53679ffa02df63fab8` |
| `crates/arc-daemon/Cargo.toml` | `644` | `13c2ae685dcc69ffa1465f42095b64457a21ee3ce4418506eb259ddf41cac29e` |
| `crates/arc-daemon/src/accept_recovery.rs` | `644` | `5de663067e87e7317e763e036c2ff450b6e3b5121027a69546c8165c1cac9676` |
| `crates/arc-daemon/src/accept_recovery_tests.rs` | `644` | `b31bc45618e9a05c005e2d1614737179e5a62939237d3c90507989498f72edbb` |
| `crates/arc-daemon/src/admission.rs` | `644` | `cc80fca15e83d1b73dc20f99d619223e46425016588f9a1e649c81d0bc27b883` |
| `crates/arc-daemon/src/connection.rs` | `644` | `0a3cedf6b9b1103e2aa2d79d02302a1c2eee754c8b44c37169431ce82d330289` |
| `crates/arc-daemon/src/database_failure_tests.rs` | `644` | `37e68da1595953b59a7eed778bbfb0930a5299eef6edd89747299c572d7ac0cf` |
| `crates/arc-daemon/src/db_worker.rs` | `644` | `9956e9b3b28f8ea1d58f9a08b42c2df0ada9b7dfc8d61710b06cf0cb7aaa2b69` |
| `crates/arc-daemon/src/db_worker/execution.rs` | `644` | `aa2d4c01fbb7b2b1a5ca9220eca4315fefab5a1ea0817a6af6cb2fd79500861b` |
| `crates/arc-daemon/src/db_worker/jobs.rs` | `644` | `37423dee5cd9827f79e69d505e4f7f50745c39d571f6421fa8051f702e36cc6d` |
| `crates/arc-daemon/src/db_worker/jobs_tests.rs` | `644` | `023ca7117c76443d568d324acc7c2a86e57099620f5e87373a638de5feb1cfcd` |
| `crates/arc-daemon/src/db_worker/schema_tests.rs` | `644` | `8a57ab2b413dca797b4c63066329f01df783a816b5d49c86eb44c47ceee4f0ce` |
| `crates/arc-daemon/src/db_worker/tests.rs` | `644` | `b7971fc13ca4aeb8793bbd48a811b1630be5c508dbb105b2396e39f4d164b203` |
| `crates/arc-daemon/src/instance.rs` | `644` | `355b112ac27ec5adec3e74d95b422d357671858b90bf4017b1c01489bea077af` |
| `crates/arc-daemon/src/lifecycle.rs` | `644` | `70692d247d8245e075e961dd20fe72485eed4eda1b0884f088a3ad688c2c4bb2` |
| `crates/arc-daemon/src/lifecycle_tests.rs` | `644` | `b17b666695426d85c21d28f71b45d12e6d256454724809f3a53d939591a82b0e` |
| `crates/arc-daemon/src/limits.rs` | `644` | `cd9a9afd80274b2e61f16880395382f73e2dda2a17540557b8188d7be8b47410` |
| `crates/arc-daemon/src/main.rs` | `644` | `5d709825a6d5ca2cd003864460e1cc1f5fc185e83e9211ccdb2a35f0d5a7cd4b` |
| `crates/arc-daemon/src/platform.rs` | `644` | `20f383e53f94e4180e0e3165c57edf1f4c0d9d5c9f1d4343053efb2cb1b5b7e5` |
| `crates/arc-daemon/src/report_lifecycle_tests.rs` | `644` | `49b306ba578c19b0fb1bdd22af7d48797de1b5a7a3d55d6b126088275cacc823` |
| `crates/arc-daemon/src/runtime_metrics.rs` | `644` | `23ac33070705808edd12d0d32d854940faf6184336955746ec53cacf93f28c1d` |
| `crates/arc-daemon/src/runtime_report.rs` | `644` | `57520df893abd2176116dc22bbada4941e521010536c25229070bc676f182427` |
| `crates/arc-daemon/src/runtime_report_tests.rs` | `644` | `daecafe96ef067cb973b5b48c8e0b6f1050f7bb49a90044dc5971adba55af3d0` |
| `crates/arc-daemon/src/server.rs` | `644` | `769a8182a539869eabe4bc62b76f0ab25c33f6823a47afb9f982122a965d4bcd` |
| `crates/arc-daemon/src/server_tests.rs` | `644` | `8b80eea5299aa13380524705726c3ffc249bc1878ed89b82fd7fdad4ada9ccf3` |
| `crates/arc-daemon/src/transport.rs` | `644` | `37fd61770d74b57e9b296a697bfe379ee6d1e6be45006b0dadaa7cb3223f7de0` |
| `crates/arc-daemon/src/transport_test_support.rs` | `644` | `00de7f22e28ea681647edeebb1bbd7c09e85695591231778c86097d1bc56207f` |
| `crates/arc-daemon/src/transport_tests.rs` | `644` | `d75ef33f0d329b982acf2bdb1ff7b89d8dfae4f9ecbb2b353b6ccdb4c8e5307b` |
| `crates/arc-daemon/tests/bounded.rs` | `644` | `208a6bf1518117d688f1e86ac9e7a6d35afa2861d4c64642ad421f7c71f51406` |
| `crates/arc-daemon/tests/classification.rs` | `644` | `0cdd538606c78891a8715d3ee248b1e3e08ab8dce0c6314b454dfd56cbfe0d78` |
| `crates/arc-daemon/tests/classification/boundaries.rs` | `644` | `ae3e8994c846711e84f8beef4b34ecb393a832d0d6b236e18323a64bdb02f6db` |
| `crates/arc-daemon/tests/classification/privacy.rs` | `644` | `3c7fd3ba43eded12ddaa071cbce6553c05370fe3898f4034912a6653620eadee` |
| `crates/arc-daemon/tests/classification/saturation.rs` | `644` | `5df00b4a5d5b2c70e2036487ec130309d71d470bd6869ef7b18fc539b89c64d0` |
| `crates/arc-daemon/tests/db_worker_unit.rs` | `644` | `32b93430b4ee67a46b9765159ffc2d5537f59e1926255c673d4c4b5506c90be9` |
| `crates/arc-daemon/tests/managed_exec.rs` | `644` | `56410a2ba90e75f8b1f421bd3e73d40aeb926c1296653f0a7fea26120ff20ad2` |
| `crates/arc-daemon/tests/managed_exec/cli.rs` | `644` | `c03114f3c3a8e5e853d953f5080fa0e4d5cec87ce01ae23cdb575e04d4df3c71` |
| `crates/arc-daemon/tests/managed_exec/connect_access.rs` | `644` | `1730bc259555071b578be0e35db88bcf856ce04f0315b07fa6bd0a6adb50cedf` |
| `crates/arc-daemon/tests/managed_exec/fail_open.rs` | `644` | `5ddc6c705aa6f68f60f126473990073d66e1cc64ec78bc636a798db5605385b4` |
| `crates/arc-daemon/tests/managed_exec/fallback_descriptors.rs` | `644` | `b6ae1eb6bc486a5e2838efb9c24618a50108ddbce6f123c4cdbf1741107167a6` |
| `crates/arc-daemon/tests/managed_exec/fallback_environment.rs` | `644` | `f778416771edac2fcc692f570b92480cb5ea908e25af88b69a21e749e131fd72` |
| `crates/arc-daemon/tests/managed_exec/fallback_pipeline.rs` | `644` | `788038622ea39f8ffb6bce41f618aa942cb3d5dc3ff630c9f99f48e42b834dde` |
| `crates/arc-daemon/tests/managed_exec/fallback_sigstate.rs` | `644` | `1e90f5157b6d252b73cf0f4bc5d32d33895b95cafb56fb8ffdfc684b672bb7b7` |
| `crates/arc-daemon/tests/managed_exec/fallback_terminal.rs` | `644` | `73856787381954a0c6de1df38726c17de5f450dca2cda01006e5415f4c492e31` |
| `crates/arc-daemon/tests/managed_exec/fault.rs` | `644` | `ff241df411924913490d9348d90d0c3a7d991127812332c9175289fbca756356` |
| `crates/arc-daemon/tests/managed_exec/historical.rs` | `644` | `3ad844276c8ed77abc7e688f717bb0050be53a26e2bf09e56cdc0ef7b8fbca19` |
| `crates/arc-daemon/tests/managed_exec/invalid_socket.rs` | `644` | `eaceecac271277e76095a6ad96e0329e8dbbfee9338e1c0711e8ad5650421585` |
| `crates/arc-daemon/tests/managed_exec/pipeline.py` | `644` | `9df7d5ed99f58994d2306de910aa5526032d8a5e2a9340be5fbdb5c25398869a` |
| `crates/arc-daemon/tests/managed_exec/pipeline.rs` | `644` | `323b7242497904e88ba7112c9c3e5b15039bdc361982e29410d67f1d513ce0c2` |
| `crates/arc-daemon/tests/managed_exec/protocol.rs` | `644` | `957693d38491c0842f6a6ab6731ef15d089ad6a998d94e9cbbcb3488289064d7` |
| `crates/arc-daemon/tests/managed_exec/retries.rs` | `644` | `88ae6d517a6f3fde9049cf0b7ce98fce370980f973025a32d55a335786c16f9e` |
| `crates/arc-daemon/tests/managed_exec/runtime_environment_probe.rs` | `644` | `519e6ee322c32a57feb329b99a039df1e34a8e8c6f8b31b53c04e7f3611f7336` |
| `crates/arc-daemon/tests/managed_exec/scenarios.rs` | `644` | `53e410aa82c29cbddcf42d21fa03c89d968b3f29552246a9974ee72871d715eb` |
| `crates/arc-daemon/tests/managed_exec/terminal.rs` | `644` | `7be8b6c5c1eb5570abb0e2d788851ff8dffb980c5b04dd359c77c838ccf8ab6f` |
| `crates/arc-daemon/tests/resident.rs` | `644` | `dc33dda0bed1ed89d7ad427727f316cb606c1f16f8c1a75f9aa4a54520857486` |
| `crates/arc-daemon/tests/resident/coordination.rs` | `644` | `8f5a912b512b2abc368739e1daecbb77ef0e3e322126c91cb5b9413501f1a13c` |
| `crates/arc-daemon/tests/resident/coverage.rs` | `644` | `27502ce76b66ede028c87af92c6f398a8e359478da0eacce03e841526e97ea07` |
| `crates/arc-daemon/tests/resident/identity.rs` | `644` | `47368da2e02a107dae24f4b4fed049add2b9d0f3044a427cfda072a152f9c2e3` |
| `crates/arc-daemon/tests/resident/lifecycle.rs` | `644` | `117b2f20381392c8235f91d4a8a802ca0782ec589c75d8727482b440758efb0a` |
| `crates/arc-daemon/tests/resident/provisioning.rs` | `644` | `328ee3deb4d18d145f5eafef42cd3903b8db6d85641e89caed8c3731b3aa43af` |
| `crates/arc-daemon/tests/resident/schema.rs` | `644` | `d26766b77f79df97987f3fa91ed10128eccbfc936ece9492574f71b3c8d35522` |
| `crates/arc-daemon/tests/startup.rs` | `644` | `e1f88c56c83c7fa5371eccfb7d50accd6a6126d6336a0a8b05b6be6d06171792` |
| `crates/arc-daemon/tests/support/admission_load.rs` | `644` | `db347f8a26d9a6083a684a8343c577a5a65c929ef1bdd9c1b9778165fa89925d` |
| `crates/arc-daemon/tests/support/deadline_recovery.rs` | `644` | `5604e9561642870fe2244e6e205d2c5e9d5fd19e0d14059f6d747f771cf07c93` |
| `crates/arc-daemon/tests/support/measurements.rs` | `644` | `85f606dd8b98fc7ba3dc08178e6174a6aec983e3b3ac0c487e12b044f72e7db3` |
| `crates/arc-daemon/tests/support/mod.rs` | `644` | `7aa5ab2c6a1b01100ba14fba5a2d9fe61fff6b72175387bc33e18dbf6361a38b` |
| `crates/arc-daemon/tests/support/native.rs` | `644` | `60538921edadf7b4b1d5bbf33bc4c0f2293c142e5f86bb50de27113eadbbbcc8` |
| `crates/arc-daemon/tests/support/runtime.rs` | `644` | `3e8dfc30ba02774c8c35ed448fb9c5b9756466924fa60da82076edaad02f8ba8` |
| `crates/arc-daemon/tests/transport_unit.rs` | `644` | `43dd4f3d92807d03a97a0f1fa1d1a569e064cf373d1eca4d3d3fd8fb0952fd4b` |
| `crates/arc-daemon/tests/worker_runtime.rs` | `644` | `8e9dcd0f00ffa0ffce506f12f5effbaa346571647a03022b5d54a423360a27b8` |
| `crates/arc-detector/Cargo.toml` | `644` | `c7537449f0009b1785112aea82ce15f20aae96a9ddaaf317a8dd514c890c36ab` |
| `crates/arc-detector/src/argv.rs` | `644` | `202d315b3e649883c68dbcba609e5e46568fa87368a074922107e486030cbe0e` |
| `crates/arc-detector/src/budget.rs` | `644` | `c07b71d3963e1706465f4903b1500cf34646319dfbdf27d9327773d91f348883` |
| `crates/arc-detector/src/coverage_tests.rs` | `644` | `7c8ea0919bffaabe168a2547293c648de4dc2407902dc53b3e7f3750270d5e04` |
| `crates/arc-detector/src/lexer.rs` | `644` | `902e5d3dd133e28ef9459d1a0db82627080584061819ccdc736570afd94eab59` |
| `crates/arc-detector/src/lib.rs` | `644` | `09b1158588d228d699517374002d66b1bfeda82b94b90ad8b17212b6b8a3d6c9` |
| `crates/arc-detector/src/parser.rs` | `644` | `177908cae1059036f11e0f9ff6ea6cfe38b407e32742a4613a113c5273a79cb3` |
| `crates/arc-detector/src/rules.rs` | `644` | `d6bc178db0e330b4f0fd72f4df6fc612ff0258c737a2ee4dccca475ba892f253` |
| `crates/arc-detector/src/tests.rs` | `644` | `21c5eb2fd0bda0ec994389cabcbe5664752618b17f6c6ea5fc176029884674ae` |
| `crates/arc-detector/src/types.rs` | `644` | `9a882ebafc075ed3057387fc8856715acf6ca18d4ccda2698f0cafdee64264bd` |
| `crates/arc-detector/tests/argv.rs` | `644` | `84b2856f8fa4e381637799b7a93887cd1256b43ebe773b96d0b60251039fc741` |
| `crates/arc-detector/tests/support/contract.rs` | `644` | `84e840a218b8d1955de19815901f86987067acbe451e8dc4a310aa2f016ae83d` |
| `crates/arc-detector/tests/support/ssh.rs` | `644` | `f4062e0b099e8aced7d91759a0a398147e30ceba5a8100e9a44bbb3083e55ff7` |
| `crates/arc-ipc/Cargo.toml` | `644` | `46b6a44cdec04ac4de97acfd9b856091d125c0a37e7bc1145cc3d27fb6c08816` |
| `crates/arc-ipc/src/jobs.rs` | `644` | `c75464c6cd8239c862ba100a501975b2189ae37aa6202a0931f7875432619a3e` |
| `crates/arc-ipc/src/jobs/types.rs` | `644` | `9459b9a22f0e3cd2ca2c921e5039117007adc928354587612759ad840380ff7b` |
| `crates/arc-ipc/src/jobs/validation.rs` | `644` | `ac9eda72ed28c82d28f646e00b0b66327f516e39769160b297b787eab4ebc11a` |
| `crates/arc-ipc/src/lib.rs` | `644` | `5b667c7e97df90007ed8f273358e09b8541e5fdebc9da6e020f87ba0144e1c43` |
| `crates/arc-ipc/src/schema.rs` | `644` | `98b8b5ccbbd63e6110dd5b086b6ab717928611332cb92f0f04e1a66d218e633f` |
| `crates/arc-ipc/src/tests/dispatch.rs` | `644` | `4da6870989d8e361e04e7e8de7e8fa33575b90261fc8fe52865e2648dcee1e1d` |
| `crates/arc-ipc/src/tests/framing.rs` | `644` | `34b2651d525318707e4e0f3d065ad27580350427e96fddf7a26c4dc268b42960` |
| `crates/arc-ipc/src/tests/mod.rs` | `644` | `02864c3325949cfc2eb133bb88808ed12e80d3b6fee913caf1af73d0ff7e56f6` |
| `crates/arc-ipc/src/tests/schema.rs` | `644` | `be43357ea5f84142cb2cfe26e5ab9fc82d57c13ce46279527f879a67c37b81fd` |
| `crates/arc-ipc/tests/classification.rs` | `644` | `c917ec23c45144109afe95aaa8057dd19b18ee0ee03440a240954e6d07f0211d` |
| `crates/arc-ipc/tests/coverage.rs` | `644` | `9438b91fe738e971a64e61704c783180697de24c9c8f1a233761da617b1a6341` |
| `crates/arc-ipc/tests/jobs.rs` | `644` | `f2e3334034e79234afbdf6f0a7b6566609ab6b52c3ca4a3ddfd582a00debc754` |
| `crates/arc-storage/Cargo.toml` | `644` | `a39d67610202aa6798357bb640cfb97847bcf6e7ceb78b9cf3abe17f02845747` |
| `crates/arc-storage/migrations/0001_phase0.sql` | `644` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |
| `crates/arc-storage/migrations/0002_managed_exec.sql` | `644` | `464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814` |
| `crates/arc-storage/src/history.rs` | `644` | `6b9739f7b354724a1793c3ba58581a90ebe1de9ef294ee50e5cd1e0ccb73bb23` |
| `crates/arc-storage/src/jobs.rs` | `644` | `198dbe9897b9ab270f46930467ef3ed48600307a57174a1dd62536b8496e4368` |
| `crates/arc-storage/src/jobs/codec.rs` | `644` | `cab00f289829e63019d1d8c86e2cdd986060317cc3b1050b08c6a22955c2796f` |
| `crates/arc-storage/src/jobs/reconcile.rs` | `644` | `2232fc199f0e1dac573017d4e92ef7fcde02689268111bfcd218a47a38186d00` |
| `crates/arc-storage/src/jobs/tests.rs` | `644` | `b5711683f20897812b86a0d0b37f1198ecb8f3379fa59119b56ebc3e83d83098` |
| `crates/arc-storage/src/jobs/tests/migration.rs` | `644` | `7a7222f07c8d19e0003764056cb75ad1e88a5d4f6b65c292720b165d34b4624b` |
| `crates/arc-storage/src/jobs/tests/recovery.rs` | `644` | `3d6dd5dd75ed704df242ecc232a94b38d65ced376233306e75b965a67f582e47` |
| `crates/arc-storage/src/jobs/tests/regression.rs` | `644` | `7ea3f59e27a5f08da6a34bc01af1345a1475420f555303e39b7fa138713939c7` |
| `crates/arc-storage/src/jobs/tests/transitions.rs` | `644` | `525be0baaab7ec0d0a9499676d26a24a91af31d2fa2129fe35917deabb98e3bd` |
| `crates/arc-storage/src/jobs/transitions.rs` | `644` | `e746d99e944903392a688aedf7ab9efd6621d879f169c1fa8065abb7477e8193` |
| `crates/arc-storage/src/lib.rs` | `644` | `5796cabfc0608d5a4fc8d54d608a024aabda8f053576763469f5c72010a369bd` |
| `crates/arc-storage/src/migration.rs` | `644` | `e33a0ec9b3e1c9afab08e1bfa69b1988588729a4b3fe9942477a9cf34bb6551b` |
| `crates/arc-storage/src/migration/tests.rs` | `644` | `a11ad158e489019ec8be1f76f19813b79ada8e5c61d743ce0c0270c9bc959e34` |
| `crates/arc-storage/src/migration/tests/acceptance.rs` | `644` | `e763fd6cbf586e1eda83d8207d20765d986304d432cdb4d3a8ab4104b4e25404` |
| `crates/arc-storage/src/migration/tests/concurrency.rs` | `644` | `459c6efe04c9f6aa2995db4c7184fb26b052318fffddebf5c5c23aad3182bfa3` |
| `crates/arc-storage/src/migration/tests/rejection.rs` | `644` | `a820e85023c80bb69083b434c7d699e538bdab33cd582b2c8420b88f837edf59` |
| `crates/arc-storage/tests/coverage.rs` | `644` | `7c863e42b3cff8c7a8cdb1c347dfc51bf65810732d9b6388308f3b1baa9c48de` |
| `crates/arc-storage/tests/fixtures/0001_fixture.sql` | `644` | `dde14974272eb5d70354fbce668ca8e9f2f88ff3d94b4138245e7c3d706dc639` |
| `crates/arc-storage/tests/fixtures/0002_additive.sql` | `644` | `bd3825ecd763c108950dc6b5c78e7bc9a8d30e0e15a9a312ae00fa3b71933127` |
| `crates/arc-storage/tests/managed_exec_migration.rs` | `644` | `ef48c032591ad3bceadf546ae75d3f2954a552a1e628da3052a2f246ccfa8b71` |
| `crates/arc-storage/tests/nfr010.rs` | `644` | `b7db33dab1d8d0d1a503f2cb40f92f345840d0a454d0f1dc29a02d6c7356cf56` |
| `docs/managed-exec.md` | `644` | `604d1a670d022d087dbe1bdb2128562eff084080b7d7052c25e136a69f682257` |
| `docs/rust-coverage.md` | `644` | `55d2cabc9f3431330e6f98e48ef912b4f8da89b4c80fdd016a4305efdd531486` |
| `mise.toml` | `644` | `5af71f4d3bb6e8da776de80f1252a0601ecde35114b18a0cc87946c74c1e7600` |
| `package.json` | `644` | `d9f600d7586838dc24ca522ecb30a93f3da6b430712cc911a1de4145706eb9c0` |
| `rust-toolchain.toml` | `644` | `f6d2a8e736e4a30b034416cc5c00fcac466d708e385e61b07285949f7074d823` |
| `scripts/coverage/acceptance.ts` | `644` | `64d745b2a59e5dfcebab1fb2dc5adc3d777a7fd53a0f6c185f099790b1bd8d68` |
| `scripts/coverage/accounting.test.ts` | `644` | `c9ca3390bb612943a7ba29f4917314a5f5fa5f26ed4a0fbfcb8356ca277c8882` |
| `scripts/coverage/accounting.ts` | `644` | `80fee69473ed65d9a16f11c614743223feeaa23ef0ac31570d8acd740c97b6e1` |
| `scripts/coverage/clean-checkout.test.ts` | `644` | `0a991995293be5f2ff4dac9c653ecace05cc2f962c7ec108e3e1eaa4ad45f579` |
| `scripts/coverage/commands.ts` | `644` | `c37ef4a710a8b8280c6a67b90deba9f81447234b77164c2efb823d2e9481f40a` |
| `scripts/coverage/config.ts` | `644` | `3510d64cae2a45ed4a40cbe073a7c4ad6cf12424c8ce15368b92c2eedc96b52a` |
| `scripts/coverage/coverage-cli.test.ts` | `644` | `4ee19897c0ee5882a88d0a47752e3013e04085ccdbae1e268a5dac3c15eee463` |
| `scripts/coverage/current-preservation.ts` | `644` | `2ec0a7cf1b48bb565492286c7856c728e3e3f3fc24f1fbd8c2e08d4f3bb47f72` |
| `scripts/coverage/evidence.test.ts` | `644` | `211c300429c1ddc217bece5bc4742e05074e1a6ce9636ae4391e5e4b31e8f3d8` |
| `scripts/coverage/fixture-runner.ts` | `644` | `20b34c70b6b0179061488a5834fb6b53893a1e3d57f4df08ac8ea3143e57e85d` |
| `scripts/coverage/fixtures.ts` | `644` | `8b8c6cbf0581e98ce9bc05fb1ec05ccd5a73884a828d33b8e7437c40e9fd466d` |
| `scripts/coverage/gate.test.ts` | `644` | `1d6f838ea56afa72638ee6522330457e5a850cca19a7e907642c7d5436580dda` |
| `scripts/coverage/gate.ts` | `644` | `72a2c41de3c469827c2c3d1d86028fa902b9d0fdb4c57501a147405148678e29` |
| `scripts/coverage/harness.test.ts` | `644` | `13d763c0cf4820043e07e4360ae813d3666c099801958bd473d53726b73f5c7f` |
| `scripts/coverage/historical.ts` | `644` | `2625971b6fc5e0dc3f8892e485c61180a15ac16b8e43f71a1a69940d3075cedf` |
| `scripts/coverage/hygiene-preservation.test.ts` | `644` | `0bef64f36a7198ebd402cf9d4e86290a0a1e23120cffa7b92ec19232f17a1f53` |
| `scripts/coverage/hygiene.test.ts` | `644` | `f1be90fc767c35d51758b68672afb34f224bd2d37ee67864a388ac12bbf5692c` |
| `scripts/coverage/hygiene.ts` | `644` | `19a40f1a88691ad425531378acb609885552b22e6add55474a189c536aa03ceb` |
| `scripts/coverage/preservation.test.ts` | `644` | `9a353997b7734fe2f06861b29fc8c8e3bd83a37fa4a26140ba5f4b3b283f5df3` |
| `scripts/coverage/preservation.ts` | `644` | `68a9bb1856541af2e2043047e5cc528671961fbb70110b93a68706ea5726c2bb` |
| `scripts/coverage/provision.sh` | `644` | `36d76c4ef21556e6d39686ccc9b3e18a7a22cd12e7191b987580116f576d477d` |
| `scripts/coverage/run.ts` | `644` | `d32ec1940cafd539d10f259cc984ab019b90a54d17cbd49ce0613fc3815203da` |
| `scripts/coverage/schema-adaptations.ts` | `644` | `50d0907d63848f4fa05ff0f0fb8bc6548829c2edd17ee048929eb5577e1c4b9a` |
| `scripts/coverage/source.ts` | `644` | `b09fadb42ab753c3dc78dd8db6d661b68033b4de8e70ab8382e3f111b769b062` |
| `scripts/harness/benchmark.ts` | `644` | `51518f03d04be5fe48e903a55e8c7a1c59015881c479bb1212b1955dd6c9da6e` |
| `scripts/harness/coverage-verification.test.ts` | `644` | `f2925f1d7002b8d597a44f96ea3f4fa564ca4035bc83a01d74ba02e8f0bdabcb` |
| `scripts/harness/equivalence.test.ts` | `644` | `19c6a2c7181e4be992e3e75feb29eb8fa61dcaa7b3de94a430cf2b8216b05a25` |
| `scripts/harness/evidence-budget.test.ts` | `644` | `132f56350764dbf9c0de247b03c3fa7f6c313e742c0a8ffbf637a955e659aac4` |
| `scripts/harness/exec-fail-open-assemble.ts` | `644` | `1c943e6a2c2fa8896ed6b62e60d926c3144203035850c9611bc0340c88db968d` |
| `scripts/harness/exec-fail-open-collection.ts` | `644` | `92c5d4599e8545a2e9a36af6c4f97bc322fbd0f39b4a883682f0b73dacca9a00` |
| `scripts/harness/exec-fail-open-compact-fixture.ts` | `644` | `cc15be68cbdf0fc8564d0793e97c9f1da9fce9af4aab5a341aec5e611ad09565` |
| `scripts/harness/exec-fail-open-compact-flow.test.ts` | `644` | `0afa04289ff8bdd3d4f87e78a9b477488fc9add6d2c0161975188c0844f5a43b` |
| `scripts/harness/exec-fail-open-compact.test.ts` | `644` | `e12cef3647dcc3d357781f119a8842fdd3a180409f69130f631d489341bb38e8` |
| `scripts/harness/exec-fail-open-compact.ts` | `644` | `b4e433ffb9e95cb0494d9b4136847e886decace3a4d6592651ec1bef49491f3e` |
| `scripts/harness/exec-fail-open-container.ts` | `644` | `f3f4d6798c72fc84545a8067ebf717af2167916696c1b829d6cf8a18b53121ec` |
| `scripts/harness/exec-fail-open-coverage.ts` | `644` | `9c106b7f31ab77d903187d000086bd3fe7d469b2ac46fcf029ff560aba44e6dc` |
| `scripts/harness/exec-fail-open-cross-workspace.test.ts` | `644` | `08960e1251a08ca38b7c6f8902aaa3ff907228144cc2bcbb24fa570b056d8c73` |
| `scripts/harness/exec-fail-open-docs.test.ts` | `644` | `73fd468d211d366b77cacbc00a93f1d76a3684dd7b41b7e514d12a717810f518` |
| `scripts/harness/exec-fail-open-docs.ts` | `644` | `be2d0464399b064473b9e436694694cb485aaf1a5d7d8be98ae3ef6f1545ec8e` |
| `scripts/harness/exec-fail-open-evidence.test.ts` | `644` | `e80d7fb4cfc6fc02799e4a7a2abfc65d5215909165aeeba1e0b3225fa5f9b8de` |
| `scripts/harness/exec-fail-open-evidence.ts` | `644` | `6d9e530bc1fc0b73ce6e9a70734d1b40042f73405db040b2d186ae2c521a5d88` |
| `scripts/harness/exec-fail-open-fixture-recipe.ts` | `644` | `ef74d0cb89f644bc6d6ae8cf29ae7d0fcd35515547e8b858a3b93b9b9403ef47` |
| `scripts/harness/exec-fail-open-fixture.ts` | `644` | `400e5c07dbc6b1b6af28d42fa62ff2bb5dd3510a5aba454a52af3dc8cffe3766` |
| `scripts/harness/exec-fail-open-gates.test.ts` | `644` | `fb82b03441f9240f09cc934db389fa63d186914a286ac223f8a72adc5d70de46` |
| `scripts/harness/exec-fail-open-gates.ts` | `644` | `194c6d062fa432607673ebf4571019b21579d7aead41ed62d713274bcdebc8a5` |
| `scripts/harness/exec-fail-open-graph.ts` | `644` | `686646f98da85c6f6ab849948928f7022b9f654772db8413073fa4f1384c4703` |
| `scripts/harness/exec-fail-open-history.ts` | `644` | `7b0c1cf9cdbe5859fd1835715a84f2d479819f7dc57859e29a8a2eebcca0f868` |
| `scripts/harness/exec-fail-open-inputs.ts` | `644` | `39b11148697fad3f74ca0283cec32cff0a082f2e7a800a1e48f906c727e431be` |
| `scripts/harness/exec-fail-open-inventory.ts` | `644` | `850b09a8344eddb1ae88cdc631901169531679eec78c3baa06e233afeae508ea` |
| `scripts/harness/exec-fail-open-native.ts` | `644` | `3185ab78bb09e261301b78f1e1a4a54eda53f6d55bcc0a93cd5145131caf4075` |
| `scripts/harness/exec-fail-open-preservation.test.ts` | `644` | `880c6e1cfb134d928bba8613086d0927cc2615acd248f4bebc7eb3ab939b0be5` |
| `scripts/harness/exec-fail-open-preservation.ts` | `644` | `09a34897c4a6ac0055e5a0de895d6325ff2e2bf87fd49aabae2dee1ada9c3afa` |
| `scripts/harness/exec-fail-open-reads.ts` | `644` | `eff4cc4af024b2f2321027e9565fa256e355b171a466b080f0a3ab00722302c8` |
| `scripts/harness/exec-fail-open-receipts.ts` | `644` | `c8e32a0faa4676c9aa9e9b20b9677317cd58fdcd11aa0d9b3a1dac6b2fbb142d` |
| `scripts/harness/exec-fail-open-recipe.ts` | `644` | `86fffc6f413790260852c85bcabc6a8d92287bf24f95db02d32e60335e2584fa` |
| `scripts/harness/exec-fail-open-traversal.ts` | `644` | `42ccba51be85fdc21a3fc1b50ab0aabd661692ed6507286a18ca2705fdaa1a28` |
| `scripts/harness/fixtures.ts` | `644` | `7659bf31f6a1510f35a4af67985645cfc444211972b0c6d94fe27aa7195df31a` |
| `scripts/harness/gc.sh` | `755` | `bce2a0608605b311c7df624cc12aa0fadf14603b81112e3068dc188fc9963f73` |
| `scripts/harness/managed-exec-coverage-fixture.ts` | `644` | `ae98744754f2783b92360828121ebe6fd5176a29868f0cc1df8d879654e8523a` |
| `scripts/harness/managed-exec-coverage-identity.test.ts` | `644` | `cb039d12dd70e2e38da3df5c9f8da8371c1951d32b4e14011745ee57d48ad986` |
| `scripts/harness/managed-exec-evidence.test.ts` | `644` | `eec3076e8bd7bd27e7c7e79780b45f367adb4e09363efb76100affd9fbd9faec` |
| `scripts/harness/managed-exec-evidence.ts` | `644` | `26fc4bc8ac8010036cdc781bf52b3d1c3cd37f3281f9be6faaf4b49e15e152c8` |
| `scripts/harness/managed-exec-immutability.test.ts` | `644` | `0065d0bdcbc96900c42168b14a0ae0e9b8802bd4944dfe2b1b424d14a8b94aca` |
| `scripts/harness/managed-exec-test-output.ts` | `644` | `1c67e7c1f094b40c58bd2f93c0d48d18e1050075457a5562ebcc077f8f54e9c3` |
| `scripts/harness/managed-exec.test.ts` | `644` | `1119fbf513db7076f17a32ac6d83664c0b51c7833a626602a225fe09659711a1` |
| `scripts/harness/performance.test.ts` | `644` | `f86aff9d4083375d7ed1ac5558ad5cda291a7227fc5c4213271fc02408ffc242` |
| `scripts/harness/test-support.ts` | `644` | `79ea2475d8cfca56619cfa1d450f81a995f44405bcf27a553bd5e0108a2d3b3a` |
| `scripts/harness/validate.sh` | `755` | `6f3eacf963b2c9d7f026f2847f2d37baa861dd4bbf70a5bd33803bbf471d97b1` |

## Reproduction program for the executed read-only receipt

The command above ran this exact program from `target/exec-fail-open/task-12-review/read-only-audit.ts`, with the imports resolving to the inspected checkout:

```ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { currentInputs, directory, root, reference, artifact, hash, canonicalBytes } from "../../../scripts/harness/exec-fail-open-inputs";
import { compactReferences, canonicalArchive, archiveBytes } from "../../../scripts/harness/exec-fail-open-compact";
import { decodeContainer } from "../../../scripts/harness/exec-fail-open-container";
import { inventory } from "../../../scripts/harness/exec-fail-open-inventory";
function must(v: unknown, message: string): asserts v { if (!v) throw Error(message); }
const before = currentInputs(root), p = `${directory}/implementation/collection.json`;
const ref = reference(root,p), raw = canonicalBytes(root,p), decoded = decodeContainer(raw);
const refs = compactReferences(root,ref);
must(ref.sha256 === "7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529", "unexpected collection digest");
must(refs.length === 44 && decoded.entries.length === 44, "unexpected entry count");
const rows = refs.map(r => {
  const e = decoded.entries.find(x => x.path === r.selector!.path)!;
  const bytes = artifact(root,r);
  must(bytes.equals(Buffer.from(e.bytes,"base64")) && hash(bytes) === r.selector!.sha256, `selected mismatch: ${e.path}`);
  return {path:e.path, size:bytes.length, sha256:r.selector!.sha256};
});
const idx = spawnSync("git", ["show", `:${p}`], {cwd:root, maxBuffer:8*1024*1024});
const mode = spawnSync("git", ["ls-files", "--stage", "--", p], {cwd:root, encoding:"utf8"});
must(idx.status === 0 && idx.stdout.equals(raw) && mode.status === 0 && mode.stdout.startsWith("100644 "), "collection index mismatch");
const archivePath = `${directory}/implementation/canonical-archive.json`, ar = reference(root,archivePath);
const archive = decodeContainer(canonicalBytes(root,archivePath));
const arefs = canonicalArchive(root,ar);
const archives = Object.entries(arefs).map(([path,r]) => {
  const e = archive.entries.find(x=>x.path === path)!;
  const bytes = archiveBytes(root,r);
  must(bytes.equals(Buffer.from(e.bytes,"base64")) && hash(bytes) === r.selector!.sha256, `archive mismatch: ${path}`);
  let currentSha256: string | null = null;
  try { currentSha256=hash(canonicalBytes(root,path)); } catch {}
  let currentRejected = false; try { artifact(root,r); } catch (err) { currentRejected = String(err).includes("archived bytes"); }
  must(currentRejected,"archive selection accepted as current");
  return {path,size:bytes.length,sha256:hash(bytes),currentSha256,currentEqualsArchived:currentSha256===hash(bytes),rejectedAsCurrent:currentRejected};
});
must(archives.length === 4, "unexpected archive count");
const planned=["implementation/collection-next.json","qa/review.md","qa/review.json","final.md","final.json","terminal.json"].map(x=>`${directory}/${x}`);
const counts=inventory(root,planned), after=currentInputs(root);
must(before.sha256===after.sha256 && JSON.stringify(before.files)===JSON.stringify(after.files), "source fingerprint changed");
console.log(JSON.stringify({baselineHead:"00773c69a8e5e7fd417fd5cc78d03e222364f609",sourceHashBefore:before.sha256,sourceHashAfter:after.sha256,currentInputs:before.files,collection:{path:p,size:raw.length,sha256:ref.sha256,index:mode.stdout.trim(),stagedBytesEqual:true,selected:rows},archive:{path:archivePath,sha256:ar.sha256,purpose:archive.purpose,entries:archives},inventory:{filesystem:counts.filesystem,indexed:counts.indexed,retained:counts.retained,added:counts.added,projected:counts.projected}}));
```
