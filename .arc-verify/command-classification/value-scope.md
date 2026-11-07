# ARC command classification: value and evidence scope

Status: completed documentation assignment, task-3, attempt 1, 2026-11-08.
Owner: Marketing Director. This task delivers only this report. It does not
certify the implementation or the full mission.

## Product and intended benefit

ARC means **Agent Resource Control**. The inherited Agent Valley orchestration
product is separate; its feature lists and business targets do not establish
ARC capabilities, customers or revenue.

This slice is a bounded, non-executing command-classification foundation for
agent-tool and developer-platform maintainers. Its intended interface is an
`arc-detector` library, protocol-1 `job.classify` operation and `arcctl`
classification command returning typed job class, locality, reason and
uncertainty. It analyzes supplied command text without running that command.
These are mission requirements, not delivered capabilities verified by this
documentation task.

The intended user benefit is a consistent way to distinguish recognized local
work, remote CI, remote execution and resolved mixtures, while retaining
UNKNOWN/OBSERVE for unresolved input. A maintainer could use that result when
reviewing command handling or developing a later integration. Explicit reasons
and uncertainty may reduce unsupported assumptions about execution location.
That is a product judgment; comprehension, review time, error reduction and
willingness to pay have not been measured. An uncertain result is not an
execution decision, permission, denial, scheduled job or completed action.

The commercial priority is to establish this behavior before making an offer
that depends on it. Technical completion can make a demonstration credible;
it cannot establish demand or profit. Commercial unknowns add no gate to the
authorized implementation.

## What the inspected evidence establishes

The accepted committed source baseline is
`05f0812d0841485bbe07b3f1f8f6fde3861a1d55`, with product tree
`ab739c266e893550181917a906051ec34a441b51`. The Actor's inspected local HEAD and
supplied snapshot commit are `fdad98c26c405a2dad9bde9aa7858d747355fbd2`; their
tree is the same baseline tree. These are distinct commits, not interchangeable
revision identifiers. Direct byte comparisons confirmed that the eight inputs
listed below match their exact accepted-baseline Git blobs.

| Evidence category | Observation and limit |
| --- | --- |
| Current source inspection | The [workspace manifest](../../Cargo.toml) lists five crates without `arc-detector`; the [CLI](../../crates/arc-cli/src/main.rs) exposes status/doctor; the [IPC dispatcher](../../crates/arc-ipc/src/lib.rs) supports hello/health/doctor and reports classification `not_checked`. This describes the inspected input, not the implementation Actor's later work. |
| Recorded organization evidence | Earlier [bounded-IPC](../../docs/bounded-ipc-value-scope.md), [runtime-identity](../../docs/runtime-identity-value-scope.md) and [module-conformance](../module-conformance/evidence-scope.md) reports describe technical work and unknown commercial outcomes. Their assertions retain their original scope; they are not new classification results. |
| Historical execution receipt | The earlier [harness receipt](../module-conformance/implementation/harness.json) records exit 1. Its [retained output](../module-conformance/implementation/harness.part-01.txt) reports missing `test:coverage` and no oversized files. This Actor read the receipt and output; it did not rerun the harness or certify current classification sources. |
| Current task result | No classification evidence directory or owned report existed on entry. This report is a new, uncommitted Actor-local deliverable, absent from the inspected baseline and snapshot. No customer, runtime, campaign or financial experiment was performed. |
| Judgment | Stable typed results and conservative uncertainty are a useful foundation for maintainers with command-locality decisions. Customer importance and economic value remain untested. |

The supplied snapshot and worker refs under
`refs/agent-valley/task-evidence/4f3f9fc6246046a5-1724980aa084bbaa-1/` did not
resolve locally (`git rev-parse --verify`, exit 128). The snapshot commit itself
resolved and was inspected. No missing worker revision was invented or replaced
with another checkout's HEAD. The Chief must verify integrated source and
delivery evidence before making implementation claims.

Input SHA-256 values, checked against the accepted baseline:

| Repository path | SHA-256 |
| --- | --- |
| `Cargo.toml` | `8e8383e8a2356b9eb6135c64554b8bfd4d8ddfb421c4e850f20b54a1fe5f5dad` |
| `crates/arc-cli/src/main.rs` | `f016929b258c35cf357cfcd1cf23b12fb541c5824f7876837385d7af5a7786ed` |
| `crates/arc-ipc/src/lib.rs` | `08e2a76995e10ef45c6229ceda943817107798acc425ff35ff46840c096d40b1` |
| `docs/bounded-ipc-value-scope.md` | `f0f6f0f932eeab477d9ba99339b302b4c57853f283d880a6eb7c490647fc0026` |
| `docs/runtime-identity-value-scope.md` | `85ef668df693c92241f420bdc05a53440dfe2231d9a9bdae59db46a464c9f311` |
| `.arc-verify/module-conformance/evidence-scope.md` | `0e2ce18fb87ac2193e7419c1fc6d7ecce9fb2be8a8ef6fe6459b8b3bfcaf1122` |
| `.arc-verify/module-conformance/implementation/harness.json` | `fb8be3f7e21264a24d78853522ed44d84506f450011b92361167eee08ea6f452` |
| `.arc-verify/module-conformance/implementation/harness.part-01.txt` | `8049b82826700b637f5816ba661d684a54ce3357d9f086d9dce23df73de44c0a` |

## Commercial hypotheses remain outside delivery

Buyer demand, budget ownership, pricing, delivery costs, support costs, CAC,
LTV, payback and ROI are **unknown**. Revenue, margin, acquisition conversion,
retention, churn and resource savings are also unknown. Unknown costs are not
zero; a passing test is not a sale, and a request count is not a conversion.
No market price, cost saving or revenue forecast is asserted.

| Later hypothesis | Potential value | Why it remains unproven |
| --- | --- | --- |
| Paid corpus assessment | Assess a customer's sanitized command examples, supported classifications, uncertainty and compatibility gaps. | No qualified buyer, permitted corpus, quote, payment or delivery/support estimate is established. Service labor could consume the revenue. No corpus is collected in this mission. |
| Technical content | Demonstrate accepted behavior to maintainers with a matching problem. | Publication, reach, inquiries and paid contribution are unmeasured. Readership alone would not establish return. No article or distribution is delivered here. |
| Referrals | A consultant or maintainer might refer a team with a locality problem. | No partner, referral agreement, qualified lead, fee or conversion exists in the inspected evidence. No contact is made. |
| Sponsorship | Downstream adopters might fund ongoing classifier maintenance. | Adoption, eligibility, sponsor access, commitments and maintenance costs are unverified. Sponsorship is not collected revenue. |

Each experiment needs its own authorized scope for research, contact,
publication, spending, corpus access and measurement as applicable. This mission
does not authorize outreach, publication, paid media, acquisition research,
telemetry expansion or customer-data collection. None was conducted by this
Actor. No commercial infrastructure is added.

A later financial assessment would need attributable payments, refunds, fees,
acquisition expense and labor, and delivery/support expense and labor. CAC needs
paying customers; payback needs cumulative contribution against acquisition
cost; LTV needs repeat-payment and support/churn evidence appropriate to the
offer. A one-off assessment cannot be assigned subscription LTV by assumption.
These measurement requirements describe missing evidence, not work added here.

## Evidence needed for a later demonstration

An accepted demonstration could show the supported fixture results and their
limits once implementation and independent oma-qa provide source-bound evidence
under `.arc-verify/command-classification/`:

- Deterministic detector unit, daemon integration and CLI checks distinguish
  `cargo build`, `cargo test`, `cargo clippy` and `cargo fmt`; classify
  `gh workflow run`, `gh run watch` and `gh run view` as REMOTE_CI; remote SSH
  execution as REMOTE_EXEC; and safely resolved local/remote compounds as MIXED.
- Negative cases prove quoted command-like text creates no false invocations
  and unresolved scripts, expansions and Docker endpoints yield UNKNOWN/OBSERVE.
  Side-effect checks and source inspection establish no analyzed-command
  execution, substitutions, tool starts, endpoint contact or raw-command/secret
  persistence within the tested boundary.
- The contract and deterministic boundaries document 16384 UTF-8 command bytes,
  1024 tokens, 128 invocations, nesting depth 8 and 65536 work steps, with precise
  counting and exhaustion outcomes. These are assigned limits, not performance
  measurements obtained here.
- Compatibility evidence preserves protocol 1, status/doctor, same-user
  authentication, admission bounds, health responsiveness and shutdown.
  Source inventories establish unchanged `0001_phase0.sql` and harness bytes
  and resulting source files within 500 lines.
- Final-source receipts show passing `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`. The unchanged `./scripts/harness/validate.sh` must
  run, retaining its actual outcome and unresolved coverage failure. Historical
  receipts cannot satisfy these new gates.

That evidence would support a bounded statement about tested classification
behavior. It would not establish reduced CPU use, shorter builds, lower bills,
completed vendor integrations, production adoption or full ARC acceptance.

Remaining obligations include the broader **detector catalog and shell
coverage**, **scheduling**, **managed-execution**, **desktop** integration and
**native Windows** implementation and acceptance. Supported fixtures do not
complete all detector or platform coverage. These areas remain separate from
this classification slice even if its required Cargo checks pass. The unresolved
harness coverage failure must remain visible in the final mission report.

## Task checks and handoff

Skill used: `oma-docs`, including its reference-check guidance. Manual inspection
— `oma docs verify` did not run: the documented command regenerates
`docs/generated/doc-refs.json`, outside this task's sole owned file. No sync,
generated index or automated drift count is claimed. Local Markdown targets
were checked directly; this report contains no external URL links.

Serena was unavailable in the tool inventory. Source inspection used direct
reads of known paths and Git object comparisons; content searches were confined
to plain report prose. The global Git status attempt reported
`av.yaml: Operation not permitted`; no denied read was bypassed. Scoped status
covering the evidence directory, manifests, crates, docs and harness was clean
before editing. Python startup also emitted mise tracked-config permission
warnings; the inspection scripts completed successfully.

Commands/checks performed: scoped `git status --short --untracked-files=all`,
`git diff -- .arc-verify/command-classification/value-scope.md`,
`git rev-parse`, direct file reads, prose-only `rg` searches, `command -v oma`
(CLI present), and read-only `python3 -B -` hashing and Git-blob comparisons.
The baseline/snapshot/tree checks and eight byte comparisons passed; the two
named-ref checks failed as recorded above. Final local link, whitespace,
acceptance-content and file-boundary checks passed. No Cargo tests, builds,
runtime checks or harness execution were performed by this documentation Actor;
they remain implementation and independent-review responsibilities.

Acceptance checklist:

- ARC identity and the bounded user benefit are stated.
- Inspected evidence, product judgment and commercial hypotheses are distinct;
  no customer, cost or revenue result is invented.
- Future experiments require separate authorized scope and are outside delivery.
- All named remaining acceptance areas and the coverage gap are retained.

Changed path: `.arc-verify/command-classification/value-scope.md` only. This
report changes no product interface, source, manifest, lockfile, registry,
mission state or other Actor's artifact. Implementation and independent QA are
not blocked by this task; their final acceptance is not granted here.
