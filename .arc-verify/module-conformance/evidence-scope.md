# Module conformance: delivery claim ledger

Status: completed for task-4's evidence audit. Owner: Marketing Director.
Changed file: `.arc-verify/module-conformance/evidence-scope.md` only.
Skill used: `oma-docs`, with the installed `docs-curator` definition, scoped to
this report. Technical implementation and independent QA remain separately owned.

## Source and evidence identity

Accepted committed baseline: `a905653d557f4c5246a1f656042cb8d29e46c052`.
Its verified tree: `c72fead1fdc7262d4a297d18e74f89e2bbfd6157`.
This Actor's inspected input snapshot and local HEAD:
`f0080ee423b553741261291deb8294808da388bd`, tree
`a3dfc866c7ac3ae80a7e9f7fac36b799ce2bdacb`.
The implementation is present in this isolated snapshot; it is not attributed
to the accepted baseline or assumed integrated into another checkout.

Delivery source fingerprint:
`0b82e06fc1588151af7ae61620539025e5e00038935e4f0fcd54fcda43b8e64a`.
Recomputed from 73 source/configuration records using the
[fingerprint contract](baseline/fingerprint.md); matches implementation receipts
before and after their commands and this Actor's current source bytes.
Evidence files are excluded from that source fingerprint and identified below.
Dependency caches, processes and external state are outside its coverage.

The supplied snapshot and worker refs under
`refs/agent-valley/task-evidence/9dd58b3e81e0c07c-8f9ba36d8e59dd86-1/`
both failed local `git rev-parse --verify` with exit 128. The supplied snapshot
commit itself resolves to local HEAD. No revision was substituted for a missing
ref. The implementer's handoff describes its original checkout at the accepted
HEAD with staged edits; this Actor observes those edits in its input snapshot.

## Claim ledger

| Classification | Supported delivery wording | Evidence and boundary |
| --- | --- | --- |
| Current source observation | The two assigned length violations are removed in the inspected snapshot. | [Source lengths](implementation/source-lengths.json): resident root 901 → 421 lines; IPC root 551 → 356; seven new modules 8–166. All nine hashes and newline counts were checked locally. This does not establish global documentation quality. |
| Current implementation evidence | The preservation mapping records 24 tests, 27 helpers, 137 assertion sites, 28 attributes and 22 declaration records. | [Mapping](implementation/mapping.json) and [preservation report](implementation/preservation.md). Local audit confirmed 51 mapped functions and the reported assertion/attribute totals. The collector is a scoped lexer; this claim audit does not replace independent semantic QA. |
| Current byte comparison | IPC production statements, daemon production files, migrations and harness rules remain unchanged within the declared preservation boundary. | All 58 paths in `implementation/protected-paths.json` compared directly with accepted Git bytes; first 354 IPC lines also match exactly. No new public IPC capability or dependency is delivered. |
| Current source plus recorded execution | Cross-process serialization and owned-child cleanup remain represented by unchanged fixtures and passing coordination checks. | `resident.rs` retains root `fixture_serialization_probe`, gated shell `exec`, and child kill/wait cleanup. `resident/coordination.rs` still selects the exact probe, requires `ready`, checks `acquired` absent under the lock, then present after release. Shared support bytes match baseline. `implementation/coordination-handoff.json` records exactly one passing test. |
| Fresh delivery execution, consumed as receipts | Required Cargo checks passed on the delivery fingerprint. | `implementation/fmt-final.json`, `clippy.json` and `workspace-test.json`: exact required commands exit 0; workspace output totals 183 passed test executions across 17 summaries. Output hashes and sizes verified locally. Commands ran in the source workspace on macOS, not afresh in this Actor's checkout. |
| Fresh delivery execution, partial harness success | The unchanged harness found zero file-length violations and exited 1. | `implementation/harness.json` and its complete output: “No oversized files found”; missing `test:coverage` caused the one reported violation. `SKIP_COVERAGE` was removed. A mise tracked-config warning also remains in the output. Never describe the whole harness as passing. |
| Historical evidence | Prior runtime identity and bounded IPC work supplies context about ARC and earlier tests. | `docs/runtime-identity-value-scope.md` and `docs/bounded-ipc-value-scope.md` describe their own missions and evidence boundaries. They do not substitute for this delivery's fingerprint or establish new benchmark, usability or financial results. No historical performance figure is promoted as a fresh measurement. |
| Judgment / maintainer hypothesis | Named lifecycle, provisioning, identity, coordination, framing and dispatch modules may make relevant tests easier to locate and maintain. | Explicit module declarations and shared fixtures support the organizational judgment. Navigation time, engineering hours, defect rate, flakiness, support savings and usability improvement were not measured. |
| Unknown | Demand, customers, price, willingness to pay, adoption, conversion, retention, revenue, acquisition/delivery costs, margin, savings and ROI. | No customer study, sales ledger, payment, cost comparison or commercial experiment was supplied for this delivery. Unknown values are not zero. No profit or savings claim follows from test passes or shorter files. |

The initial formatting check failed with exit 1; the implementer corrected module
ordering and boundary whitespace, then the exact final formatting check passed.
Both outcomes remain in `implementation/fmt.json` and `fmt-final.json`.

## Product wording boundary

Direct reads of `crates/arc-cli/src/main.rs` and `crates/arc-ipc/src/lib.rs`
confirm a local daemon client with `status` and `doctor`, owner-only local IPC,
protocol-1 framing, hello/health dispatch and database diagnostics. The client
connects through a Unix socket and does not open SQLite itself. Successful doctor
text says tray not connected, vendors not detected, command classification not
checked and Sweeper off. These are source observations and preserved contracts,
not a new feature launch or an observed customer journey.

Do not describe this test reorganization as delivery of the broader Agent Valley
orchestration product, a dashboard, vendor integration, automation, increased
capacity, faster IPC or expanded platform support. Existing tests and unchanged
production bytes support preservation under their tested conditions; they do
not prove every possible runtime case or measured user benefit.

## Commercial advice disposition

The upfront paid daemon-reliability assessment, technical content distribution,
consultancy/maintainer partnership and sponsorship suggestions are deferred
outside this mission. Symptom-search and public troubleshooting routes are also
untested commercial hypotheses. No outreach, publication, external research
campaign, pricing change, spending, marketing asset or commercial experiment was
performed by this task. No acquisition or time-to-revenue forecast is adopted.
Commercial uncertainty adds no gate to the authorized conformance refactor.

## Checks and remaining limits

Manual inspection — `oma docs verify` did not run. The skill documents that it
regenerates `docs/generated/doc-refs.json`, outside this task's sole owned file.
Repository-wide sync and rewrites were not run. Automated drift counts before
and after are therefore unavailable; the report did not exist on entry.
The report's local Markdown references were checked directly after writing.
No URL verification is claimed; this report contains no external URL links.

Serena tools were unavailable in the discovered inventory. Source inspection
used scoped reads of already known paths and Git path/object metadata, with no
recursive native source discovery or provider change. The source receipts retain
the original `av.yaml` access denial, so global cleanliness remains unverified.
This Actor's initial scoped status was clean; no denied read was bypassed.

| Command/check executed by this Actor | Outcome |
| --- | --- |
| Scoped `git status --short -- .arc-verify/module-conformance crates/arc-daemon/tests crates/arc-ipc/src` | Exit 0; clean before edits. |
| `git rev-parse HEAD HEAD^{tree} a905653d557f4c5246a1f656042cb8d29e46c052^{tree} <worker-ref>` | Exit 128 for unresolved worker ref; preceding HEAD/tree values match those recorded above. Separate ref checks also returned 128. |
| `python3 -B -` read-only fingerprint, artifact/receipt hash, length and protected-byte audit | Exit 0; 73 fingerprint records, five receipt/output pairs, nine source lengths, 58 protected paths and IPC prefix verified. No receipt rewritten or runtime suite replayed. |
| Local report reference, line-count and whitespace inspection; scoped Git boundary check | Passed after writing; only the owned report added, within 500 lines. |

Input artifact SHA-256 values:

| Path under `implementation/` | SHA-256 |
| --- | --- |
| `handoff.md` | `b3c574776bec14de2e246b641c67b16a31e3586b074578dc515ebfe2378a52e5` |
| `preservation.md` | `9527ca2f5b33cb40c3057fbe6a5b3d7b7610ded39efbe189b6cee2bf86b99e28` |
| `mapping.json` | `d78bf55b5527df4eaa36ae1d493734faa16bb2ab400f201f2c746354afeecfdf` |
| `fmt-final.json` | `05a27dfa27d4f83c4498a709dfa023bb5743013622639d7821b0df609f5bbb7c` |
| `clippy.json` | `5ba6626d2b354abf9120c441021e0543b705f5e6fb2bbc00b71792b58de35fcd` |
| `workspace-test.json` | `95bb50e4f42ada3ba219efdb883fca740d564d43f2a0ed8fd0ee1d17aa478482` |
| `coordination-handoff.json` | `d8a1390017bd4bee7b6dc8da4edf8ce1ce5cc03607d6f7a65441113cd21e2734` |
| `harness.json` | `fb8be3f7e21264a24d78853522ed44d84506f450011b92361167eee08ea6f452` |

Acceptance checklist: this report binds claims to actual artifacts and the
recomputed fingerprint; separates current results, historical context, judgments
and unknowns; makes no unsupported commercial or capability claim; records that
commercial experiments were not performed; stays within 500 lines.

At inspection, `qa/result.json` and `final.json` were absent under this evidence
directory. Independent oma-qa and integrated final acceptance remain later mission
work, not approvals granted by this report. This task changes no interface,
product source, product documentation, manifest, registry or other Actor's evidence.
