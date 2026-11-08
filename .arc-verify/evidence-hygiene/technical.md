# Technical handoff: evidence hygiene

Status: completed for task-1 baseline and retention analysis. Mission implementation and acceptance remain pending.

## Problem and recommendation

Accepted HEAD is `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`. The evidence tree mixes durable accounting and verdicts with raw, checkout-specific measurement inputs. Removing those inputs from tracking must preserve their local bytes and the live native-report checks. A historical pass cannot satisfy a fresh checkout.

Use the existing Bun scripts, pinned Rust/Cargo/LLVM tools and Git index. No dependency, service, archive format or product change is needed. `retention.json` selects **24 rust-coverage destinations**: 19 existing files kept byte-for-byte, three exact historical copies outside `runs/`, and two compact derived command receipts. This leaves 125 slots below the maximum permitted count of 149. Untrack the other 908 accepted-tree rust-coverage paths with `git rm --cached`; preserve their local bytes and history. These are proposed counts after implementation, not a completed cleanup.

| Option | Implementation and future change cost | Operational cost and team complexity | Decision |
| --- | --- | --- | --- |
| Explicit retained files plus separate ignored raw runtime evidence | Small: preserve accounting format and verification interface; update paths, tests, documentation and index policy | Native verification still needs local exports/profiles; one engineer controls index/runtime changes | Recommend; keeps original verdict and summary bytes inspectable |
| One newly derived report containing all durable evidence | More transformation and validation; maintain a new aggregate schema and conversion logic | Fewer navigation files, but reviewers must audit extraction and future format changes | Unnecessary for a limit of 149 files |

The architecture method is a bounded recommendation with two options. No runtime/product boundary changes or new ADR are proposed. The task owns only baseline.json, retention.json and this report; this explicit ownership takes precedence over generic skill output locations. No workflow, mission-state or agent dispatch operation was invoked.

## Fresh measurements

Measured directly in this attempt using accepted HEAD blobs, not copied from supplied Director advice:

| Scope | Files | Logical Git blob bytes |
| --- | ---: | ---: |
| `.arc-verify/rust-coverage/` | 927 | 260,548,113 |
| All `.arc-verify/` | 2,628 | 278,635,154 |
| Union of prohibited categories | 456 | 252,272,711 |

The index's path/OID/stage tuples exactly matched accepted HEAD's evidence blobs at measurement time. Each blob path counts once; identical content at two paths counts twice. Bytes are uncompressed blob sizes, not pack size, disk use, clone transfer or monetary savings. History retention means this work does not establish smaller historical clones.

| Overlapping category | Files | Bytes |
| --- | ---: | ---: |
| `runs/` directory component | 449 | 218,352,659 |
| `profiles/` directory component | 0 | 0 |
| `.profraw`, `.profdata`, `.lcov` extension | 16 | 13,114,041 |
| `raw.json` or `llvm-export.json` filename | 14 | 210,777,833 |
| Native LLVM coverage JSON content | 14 | 210,777,833 |

Do not add these rows. The disjoint union is 426 runs-only paths (28,380,837 bytes), 12 runs-plus-extension paths (7,159,655 bytes), 11 runs-plus-native-JSON paths (182,812,167 bytes), four extension-only paths (5,954,386 bytes), and three native-JSON paths outside runs (27,965,666 bytes). Known-name and content classifications coincide here; the content-only count is zero. The three exports outside runs are the two `library/libraries-2026-11-07T23-{57-50-650Z,59-54-881Z}/raw.json` exports and `tooling/attempt-5/llvm-export.json`. All 456 prohibited paths are inside rust-coverage at this baseline; final scanning must still cover all `.arc-verify/`.

Classification read every tracked evidence blob, regardless of extension. It parsed JSON objects and recognized `type: llvm.coverage.json.export` or native `data[].files[]` entries containing `filename` with `segments` or `branches`. Required summary/accounting JSON stays trackable. Synthetic exports placed under tracked evidence would also be prohibited by this rule; tests should generate fixtures under ignored temporary/run locations.

## Reproduce accounting

The actual measurement invoked `git ls-tree -r -l -z HEAD -- .arc-verify`, `git cat-file --batch` for each distinct blob, and `git ls-files --stage -z -- .arc-verify` to compare the index. All succeeded. The full classified inventory is local in the ignored manifest, not another tracked generated inventory.

This compact recipe reproduces counts and byte totals, using the same aggregation for accepted-tree and final-index accounting:

```sh
python3 - <<'PYCODE'
import subprocess

def git(*args):
    return subprocess.check_output(['git', *args])

def totals(rows):
    for scope in ['.arc-verify/rust-coverage/', '.arc-verify/']:
        selected = [(p, size) for p, size in rows if p.startswith(scope)]
        print(scope, {'files': len(selected), 'bytes': sum(n for _, n in selected)})

before = []
for row in git('ls-tree', '-r', '-l', '-z',
               'aacb3b4359f13c31c0705c0cffc5bf1ac00f387e', '--', '.arc-verify').split(b'\0'):
    if not row:
        continue
    metadata, path = row.split(b'\t', 1)
    mode, kind, oid, size = metadata.split()
    assert kind == b'blob'
    before.append((path.decode(), int(size)))
print('before'); totals(before)
index = []
for row in git('ls-files', '--stage', '-z', '--', '.arc-verify').split(b'\0'):
    if not row:
        continue
    metadata, path = row.split(b'\t', 1)
    mode, oid, stage = metadata.split()
    assert stage == b'0'
    index.append((path.decode(), oid))
query = subprocess.run(['git', 'cat-file', '--batch-check'],
                       input=b''.join(oid + b'\n' for _, oid in index),
                       stdout=subprocess.PIPE, check=True).stdout.splitlines()
after = []
for (path, oid), row in zip(index, query, strict=True):
    got, kind, size = row.split()
    assert got == oid and kind == b'blob'
    after.append((path, int(size)))
print('index at invocation'); totals(after)
PYCODE
```

Stage all intended evidence changes before final index accounting. Repeat `ls-tree` on the final committed tree when available. A still-tracked worktree file can have modified bytes that differ from its index blob; final delivery must bind and reconcile the final index/tree, not substitute working-file sizes. Never count ignored raw artifacts as tracked savings twice.

## Coverage interface and provenance

Known-path reads establish the existing interface:

- `config.ts` retains historical accounting baseline `fbe76a5f15101ad415f355ef06f1deeb1f97d568`, six crates and the 80% threshold. Do not replace this baseline with the new amendment HEAD.
- `run.ts` writes UUID runs under `baseline/runs/`, fresh owned targets under `target/arc-coverage/`, raw LCOV and native JSON, raw-profile hashes, explicit child PIDs and the exact merge list. It updates `baseline/latest.json` on success and failure.
- `evidence.test.ts` requires that local latest pointer, matching current input fingerprint, both native report hashes, native recomputation, profile bytes/hashes and child merge membership. Removing tracking must not replace any of these checks with a retained summary.
- `acceptance.ts` currently writes transient wrapper receipts under `tooling/acceptance/`. The engineer must move future receipts below an ignored `runs/` directory without changing `realAcceptance(name, argv)` or weakening wrapper assertions.
- `source.ts` fingerprints crate files, coverage scripts/tests, manifests/configuration and accounting.json. Finish implementation before collection-dependent verification; changed coverage tests invalidate the prior fingerprint. Serialize collection-dependent checks so latest.json has one writer.

The selected historical run is `2026-11-08T01-44-40-398Z-a4294aac-18e6-499d-b452-44ffb3b612ab`. Its source hash is `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8`; host is `aarch64-apple-darwin`; recorded commands span `2026-11-08T01:44:40.486Z`–`2026-11-08T01:46:03.208Z`. It records pass=true/exit=0. These are inspected **Historical** receipts, not executions in this task. Accepted storage commit aacb3b4 is not asserted as that run's executed HEAD. The run records an older accounting baseline and exact input hashes; do not invent an unrecorded execution commit.

`retention.json` records every selected source path, blob OID, SHA-256, bytes, destination and action. Exact copied summaries retain their original input map, run ID, baseline, host and native report hashes. Compact receipts retain original argv, cwd, PID, started/ended timestamps, actual exit and exact output hashes, plus original receipt path/hash/storage commit. Missing original metadata stays unavailable. Keep the byte-bound historical reports unchanged and label them through the maintained index. References to removed evidence become explicit archived Git references (`git show <storageCommit>:<originalPath>`), not live receiving-checkout links.

## Preservation and integration

At task entry all **82 existing crate Rust files**, including test files, plus `crates/arc-storage/migrations/0001_phase0.sql` and `scripts/harness/validate.sh` matched accepted HEAD bytes. `baseline.json` stores each path, Git mode/blob, byte count and SHA-256. This stronger scope avoids classifier ambiguity. It does not authorize edits to tests or any protected product path.

The engineer's mission-specific check must compare the complete accepted/current protected path sets, raw bytes, regular-file/symlink state and modes. Scan local crate directories for added `.rs` files, including ignored/untracked additions; compare final index/tree paths too. Fail additions, deletions, renames or any byte change. Require negative tests for these failures. Keep the older token-subsequence preservation checks separately; token equality cannot prove this mission's byte obligation. The harness command and bytes are immutable.

The local manifest is `target/evidence-hygiene/baseline-local-manifest.json`, ignored by existing `.gitignore:11:/target/`. Its SHA-256 is `c7aa81c97561770a8317efaa23ea2f89c4143660f4f558bd0012a899916c41c0`, 773,326 bytes. It records all **456 existing local prohibited files**, 252,272,711 bytes, with no read errors; a canonical raw-record digest and receipt are in baseline.json. It also contains tracked classification and protected-file inventories. Its inventory domain was `.arc-verify/` plus any existing `target/arc-coverage/` profiles; no inherited coverage target existed in the latter at measurement. Preserve the manifest locally and never stage it.

For integration: copy selected historical outputs first, then remove index entries with `--cached` only. Do not reset/checkout the staged-deleted raw paths or remove the original worktree. After integration read every manifest path, compare its size and SHA-256, and verify accepted and historical baseline commits/tree objects still exist. Only then write rawFilesPreserved/historyPreserved results; a successful merge alone proves neither. If later verification uses a new worktree, keep this original raw-bearing worktree and manifest intact. Record any actual failure and route it to the Chief.

## Engineer-to-documentation handoff

Use the existing summary, uncovered report and command receipt formats. The engineer supplies selected historical copies/compact receipts, actual fresh command receipts, final source binding, comparable before/after index counts/bytes and local-preservation results. Documentation then supplies `index.md` with separate **Historical** and **Fresh** sections and updates `docs/rust-coverage.md`. Preserve the contributor journey: root coverage command → six independent pass/fail verdicts → local summary/uncovered report. Link historical evidence directly without implying current native data is distributed.

A clean checkout needs both required Git baselines/history, final implementation/tests/contracts and the pinned installed Bun 1.4.4, Rust 1.95.0, cargo-llvm-cov 0.6.21 and matching LLVM tools. Locked dependency/tool caches may be reused. The checkout must have no inherited target directories, coverage runs, profiles, raw exports or runtime latest pointer before collection. Record those absence checks, collect coverage there and run evidence verification there after the last collection. Never copy historical exports containing absolute checkout paths. Verify prerequisites once before expensive runs; task-1 did not establish their current operational availability.

Required later commands remain those in the saved contract: hygiene and clean-checkout tests; gate/accounting/preservation tests; coverage CLI, harness and evidence tests; native fmt, Clippy and workspace tests; exact-byte hygiene-preservation tests; and the root coverage/harness invocations recorded by their required wrappers. Retain any separately required direct invocation. Do not change commands, add skip flags or infer a pass from a timeout. Independent oma-qa must inspect final implementation and evidence. Task-1 does not provide that review or any fresh product-test pass.

## Check evidence and limits

- Skill read and used: `oma-architecture`; installed architecture-reviewer definition followed. Method: source-grounded recommendation and retention boundary.
- `git rev-parse HEAD`: exit 0, accepted HEAD matches. Scoped staged/unstaged diffs: empty at entry. Initial unrestricted diff printed `av.yaml: Operation not permitted`; global cleanliness is not asserted and no bypass was attempted.
- Git tree/index/blob accounting, native JSON classification and SHA-256 inventory script: exit 0; index matched accepted evidence tree; no local inventory read errors.
- `git check-ignore --no-index --verbose target/evidence-hygiene/baseline-local-manifest.json`: exit 0, existing `/target/` rule matched.
- Known-path reads covered run/evidence/config/source/preservation/acceptance and the coverage CLI/harness/accounting tests, contracts, guide, historical summaries and QA verdict. Prior decisions index and the relevant Chief ownership decision were read; no ADR is changed.
- Serena was absent from callable tool metadata. No native repository code search was used. Git/evidence metadata and supported known-path reads were sufficient; the unavailable tool is a recorded limitation.
- No build, coverage collection, suite, provisioning, external delivery, screen check or mission-state mutation ran in this task. Implementation and fresh mission acceptance belong to downstream assigned owners.

Remaining risks: future generated raw names need matching ignore probes and content auditing; absolute-path historical exports cannot validate a clean checkout; concurrent collection invalidates latest-pointer evidence; staged deletions must not delete local raw bytes during integration. Cost and runtime savings are unmeasured. The measured byte reduction will describe the current tree only.

Final task-specific verification: exit 0. Both JSON reports parsed and their references/categories validated; all 24 selected source hashes matched accepted Git blobs. The documented accounting recipe ran successfully and reproduced 927/260,548,113 and 2,628/278,635,154 for both accepted tree and unchanged index. The ignored manifest hash still matched, and all 456 recorded local files remained present with identical sizes/hashes. The complete current Rust path set matched all 82 accepted paths, and all 84 protected files matched bytes/modes. Both baseline commit objects remained available. Scoped implementation/product/OMA diffs were empty; exactly the three owned reports were nonignored untracked additions. These checks do not claim fresh coverage or mission acceptance.
