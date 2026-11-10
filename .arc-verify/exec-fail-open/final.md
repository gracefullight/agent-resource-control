# Exec fail-open: corrected-source final assessment

**Decision: pass for the corrected implementation and its recorded prerequisite gates; no product blockers remain.** This decision is sealed before the two ordered terminal validators. Their actual results belong to [terminal.json](terminal.json). Framework verification of the unchanged contract and an independent final review remain required; this decision does not claim either has passed.

The accepted HEAD is `00773c69a8e5e7fd417fd5cc78d03e222364f609`. The [backend gate](implementation/gates.json), [independent QA](qa/review.md), and this decision bind the corrected 242-input SHA-256 fingerprint `bd81c863144d1dd2b149122fa47299c60f503064eda68262fa1d464c08212fdb`. The Chief recomputed the current input hashes and modes, matched all 242 to the index, validated the 11 selected gate receipts and their outputs, and checked the QA bindings. This assessment uses the post-repair gate and task-24 review; the earlier blocked decision and its pre-repair observations remain historical.

## Resolved blockers

- **Peer authentication.** The CLI checks the connected peer's OS user credentials before sending IPC; an untrusted peer cannot supply an authenticated rejection. Task-24 reproduced a same-user rejection with zero launches. It also fault-injected a credential-query failure in a copied macOS binary: three connections received zero request bytes, the child ran once and exited 17, and the count remained one after daemon recovery. This was a simulated credential failure against a same-user peer, not a real foreign-user socket test. See [QA](qa/review.md) and `crates/arc-cli/src/exec/transport/peer.rs`.
- **Lost acknowledgement followed by endpoint replacement.** The current transport retains uncertainty after a sent request; preparation also retains it after confirmed REGISTER if CLAIM encounters an invalid replacement endpoint. Independent real-daemon probes withheld committed REGISTER or CLAIM responses after replacing the socket. Each invoked the child once, exited 17 and remained at one invocation after recovery; a third probe covered confirmed REGISTER before CLAIM. Initial `ENOTSOCK` and `ENOTDIR` configurations remain non-launching invalid endpoints as directed by the operator. See [QA](qa/review.md) and [historical routes](implementation/historical.json).

## Execution and native compatibility

One 1,800 ms absolute metadata deadline is shared by REGISTER, CLAIM and retries. The existing CLI-owned child has one spawn path. Outages and lost acknowledgements reach it once with unavailable or unconfirmed tracking diagnostics; cancellation, invalid arguments, authenticated rejection, incompatible protocol, complete invalid replies and initial invalid endpoint configuration remain non-launching. Fallback does not assert lifecycle persistence or expose command secrets, and the reviewed recovery probes found no reservations or ARC cleanup.

The independent unfiltered `cargo test --workspace` run recorded **412 passed, 0 failed, 0 ignored and 0 filtered**, covering native argv, cwd, environment, inherited descriptors and streams, exit 17, directed and terminal signals, foreground pipelines, managed tracking, idempotency, protocol-1 diagnostics and daemon-loss survival. The independent macOS arm64 outage matrix recorded **23 cases**, each with one invocation, exit 17 and no replay after recovery; its largest parent-before-wrapper-spawn to child handshake was **1,817.616 ms**. The backend gate's corresponding maximum was **1,818.602 ms**. Both are unadjusted observations under the 2,000 ms target, not hard real-time guarantees. Startup, metadata and handoff subdivisions were unmeasured; Phase 0 has no separate scheduler stage. See [timing](implementation/timing.json) and [QA](qa/review.md).

## Gates and preservation

The current-source [gate](implementation/gates.json) binds 11 passing prerequisite commands: formatting, warning-denying workspace Clippy, workspace tests, coverage collection, the full no-skip harness, preservation/equivalence checks, clean-checkout and coverage CLI/harness checks, live coverage verification after the final collector, and documentation tests. QA reran the workspace, live-coverage and documentation checks and validated the retained backend gate outputs; it did not recollect those backend runs.

Production-line coverage, independently recomputed from the selected coverage collection, exceeds the 80% floor in every crate:

| Crate | Covered / total | Coverage |
| --- | ---: | ---: |
| arc-core | 247 / 256 | 96.48% |
| arc-detector | 609 / 610 | 99.84% |
| arc-storage | 653 / 674 | 96.88% |
| arc-ipc | 387 / 393 | 98.47% |
| arc-daemon | 1628 / 1746 | 93.24% |
| arc-cli | 968 / 1100 | 88.00% |

Migration `0001_phase0.sql` has SHA-256 `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` in HEAD, index and worktree. The [preservation receipt](implementation/preservation.json) retains the coverage-accounting guards and 11 positive tamper rejections. The current source audit checked 222 scoped files; the maximum was 483 lines.

The old before-register disconnection and post-shutdown assertions require fail-closed behavior and conflict with the new live contract. Their original assertion bodies execute against the fixed accepted-baseline CLI; separate live tests use the corrected CLI, and an expected negative control confirms the route distinction. The [historical receipt](implementation/historical.json) binds both routes and their execution.

The CLI [help](../../crates/arc-cli/src/main.rs) and [documentation](../../docs/managed-exec.md) describe fallback, uncertain tracking and recovery without replay. The [claims review](claims-review.md) identifies the limits of those claims. Desktop integration, native Windows acceptance, vendor hooks and scheduling remain outstanding. User comprehension, adoption, support savings, retention, revenue and ROI remain unmeasured.

## Evidence handoff

The contract retains 13 distinct commands: 11 prerequisites and two ordered terminal validators. Its nine-node receipt graph is acyclic and terminal is a sink. The indexed evidence inventory has 149 files including the terminal slot; the compact collection is 1,273,952 bytes. No predecessor depends on terminal results.

Before replacement, the blocked final Markdown, final JSON and terminal sink were copied unchanged to ignored `target/exec-fail-open/task25-attempt1/historical-*`; their respective SHA-256 values are `2e94d13c3bb56ea8dadccab7a01c4a05d3fdc0fcdc3c3e06ca14ef40222887b4`, `69fe31d1c267fb1f091cb121dda8d1338695ffe921d283d8cba176fdaabc4d7f` and `96264e89fe89ae0c9ade101a985d1e1ae0050f6106a2d3552acfa2eaaabc3e75`. Their former blocker findings and failed terminal evidence check describe the earlier source only.

The Chief's first local audit exited 1 because its temporary script treated the contract's argv arrays as objects; the corrected audit exited 0 and its logs are retained under `target/exec-fail-open/task25-attempt1/`. QA's debugger attachment was denied by macOS; its separately documented fault-injected binary supplied the simulated credential observation. No browser or GUI was used.
