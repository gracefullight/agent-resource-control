from pathlib import Path
import json
b=Path('.arc-verify/rust-coverage'); i=b/'integration'; load=lambda p:json.loads(Path(p).read_text())
latest=load(b/'baseline/latest.json'); s=load(Path(latest['directory'])/'summary.json'); p=load(Path(latest['directory'])/'profiles.json'); first='2026-11-08T00-14-55-025Z-6ea2f6cd-40bf-4140-bd3d-c87cebbbd4b9'
inv=load(i/'production-inventory.json'); inv['finalRun']=latest;inv['finalCrates']=s['crates'];inv['bothRunsSameProductionLineSetsAndCrateTotals']=True
old=load(b/'baseline/runs'/first/'summary.json'); assert old['crates']==s['crates']; assert old['files']==s['files']; (i/'production-inventory-final.json').write_text(json.dumps(inv,indent=2)+'\n')
(b/'runs').mkdir(exist_ok=True); runs=[]
for run in [first,latest['runId']]:
 d=b/'baseline/runs'/run; r=load(d/'receipt.json'); summary=load(d/'summary.json'); assert r['pass'] and summary['sourceHash']==s['sourceHash']
 runs.append(dict(runId=run,directory=str(d),sourceHash=s['sourceHash'],receipt=str(d/'receipt.json'),rawLcov=str(d/'raw.lcov'),rawJson=str(d/'raw.json'),perFileUncovered=str(d/'uncovered.md'),profiles=str(d/'profiles.json')))
(b/'runs/integrated.json').write_text(json.dumps(dict(sourceHash=s['sourceHash'],runs=runs,note='Links standalone and full-harness collections. Native paths/exports remain unchanged; no duplicate exports.'),indent=2)+'\n')
a=load(i/'acceptance.json');table='\n'.join('| `'+ ' '.join(c['argv'])+'` | '+str(c['exit'])+' | '+f"{c['durationMs']/1000:.2f}"+' s |' for c in a['commands'])
crates='\n'.join(f"| {c['crate']} | {c['covered']}/{c['total']} | {c['percent']:.2f}% |" for c in s['crates'])
text=f'''# Task-6 integration handoff

Status: **partial**. All five exact goal commands passed on the combined source. The saved real-command harness verification test failed at its unchanged timeout, so task-6 does not satisfy every acceptance criterion. Independent task-7 oma-qa remains pending.

Accepted HEAD: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`. Combined implementation/configuration SHA-256: `{s['sourceHash']}`. Each receipt includes a full per-file inventory and equal before/after hashes; the HEAD label alone is not the measurement binding. Task instructions, mission state, assignments, verification commands, thresholds and timeouts were not changed.

## Integration and ownership

Task-3, task-4 and task-5 deliveries were already present. `inputs-reconciliation.json` verifies the eleven Rust delivery hashes and records the three documentation hashes, in plan order. Original patches, isolated reports and receipts remain intact and attributed to their owners. Integration staged supplied new tests/documents for the tracked-file harness without changing their bodies or taking ownership.

Task-6 changed `scripts/coverage/coverage-cli.test.ts`, `harness.test.ts` and `evidence.test.ts`: require all six real verdicts, no harness violations and complete source/report/profile/child-merge evidence. Original assertions remain. The other three verification files retain their deterministic requirements. No production implementation, public interface, Cargo dependency or shared manifest changed in this task. Supporting outputs are under `integration/`, `runs/integrated.json`, two append-only `baseline/runs/` directories, the generated `baseline/latest.json` pointer and two new `tooling/acceptance/` directories. Established report paths remain compatible; exports were not rewritten or relocated. `delivery.json` binds the exact delivered artifacts.

## Combined production coverage

| Crate | Covered/production lines | Coverage |
| --- | ---: | ---: |
{crates}

80% per crate is the new engineering target, not a measured pre-task baseline or SRS threshold. Both collections produced identical canonical per-file mapped/uncovered line sets and crate totals. All 29 production files are present, with no missing or zero-denominator production files. `production-inventory-final.json` records file hashes, classification, mapped counts and uncovered lines. Zero-hit production lines remain. Only test-only source is omitted; compiler-inactive platform code is inventoried and unmeasured.

`../runs/integrated.json` links the standalone and full-harness native LCOV/JSON exports, source inventories, versions, per-file uncovered reports and command receipts. Latest run: `{latest['runId']}`. All {len(p['profiles'])} current raw-profile sizes/hashes and the exact native merge list were freshly checked. Distinct reaped arcctl/arc-daemon probe PIDs have matching profiles in that merge. Workspace unit/integration suites also execute the real instrumented binaries. Raw profiles remain in the manifest-recorded `target/arc-coverage/<run-id>` paths. Native exports retain this collecting checkout's absolute paths.

## Command receipts

`commands.json` records exact argv, relevant environment, PID, exit, timestamps, duration, full source binding and tool versions/artifact hashes. It retains the failed harness wrapper and terminated child as one failed attempt. Real nested commands are indexed alongside their invoking tests. Both required coverage invocations ran; no fixture or historical report replaces them.

| Exact required command | Exit | Duration |
| --- | ---: | ---: |
{table}

The full harness reports `All checks passed (0 violations)` and `Coverage thresholds met`. SKIP_COVERAGE is absent; there were no arguments or skip flags. The standalone root command ran through `bun test scripts/coverage/coverage-cli.test.ts`: 1 passed test, 9 assertions. Formatting, Clippy and native workspace tests passed separately. Final gate/accounting/preservation: 27 passed tests, 87 assertions. Final evidence test: 1 passed test, 2,580 assertions, recomputing native accounting and every raw-profile hash. Controlled failures prove nonzero exits for failed tests, missing tools, absent/stale/malformed reports, missing files/crates and a below-threshold crate despite a high workspace average. Exact 80% passes; a below-boundary value displayed as 80.00% fails.

Tools rechecked after the operator's prerequisite change: Bun 1.4.4, Rust/Cargo 1.95.0, cargo-llvm-cov 0.6.21 and matching LLVM 22.1.2. `cargo fetch --locked --offline` exited 0 before native checks. No installation/download was needed. The gate still never installs implicitly. System Bash was 3.2.57. A read-only probe also found Homebrew Bash 5.3.20; it was not substituted into acceptance. Monetary cost is unknown; measured durations are retained without savings claims.

## Acceptance blocker

`bun test scripts/coverage/harness.test.ts` exited **1** after 116.38 seconds. Its unchanged `./scripts/harness/validate.sh` child exited **143** when the existing 115,000 ms realAcceptance timer expired during Check 1, scanning 3,504 tracked files. That timed attempt never reached coverage. The saved outer verification allows 120,000 ms. The later exact full command required **211.95 seconds**, reaching coverage only after approximately **132 seconds** of scans. Its exit 0 does not turn the failed timed test into a pass. `acceptance.json` retains `complete: false`; failure logs and receipts remain in `failed-attempts.json` and the corresponding run directory.

Chief coordination is needed for this measured runtime incompatibility while preserving the saved verification command and protected harness. No cached-success rescue, narrowed file list, timeout increase, test weakening or environment bypass was introduced. Independent review must retain this blocker unless the unchanged real-command test passes in the actual verification environment. Task-6 is not ready to be marked completed.

## Preservation and boundaries

`preservation.json` and `preservation-audit.md` prove byte-identical `scripts/harness/validate.sh` and `crates/arc-storage/migrations/0001_phase0.sql` against accepted HEAD. Cargo.toml/Cargo.lock have no diff. Production-token hashes match, and every existing Rust token/assertion remains in order. The schema blocking-socket fix is the original process Actor's test-fixture edit; no further original-owner correction was made here. Code-only staged whitespace passed. Broad evidence whitespace found a captured blank line at stdout EOF; verbatim logs remain unchanged.

The receipt helper's initial import-path error occurred before any child command and was corrected. A report-generation Python syntax error likewise occurred before writes and was corrected. These are recorded separately from acceptance failures.

Measured host/target: `{s['host']}`. Inactive non-macOS runtime/data/anchor paths in arc-core and Linux/unsupported-platform peer/start-time branches in arc-daemon remain unmeasured. Native Linux needs its own run; native Windows and stable-toolchain doctest coverage are not certified. Every production file is accounted for; comments, declarations and compiler-inactive code are not represented as covered executable lines.

Global git status reported av.yaml access denied; scoped checks succeeded. A process-list probe was denied; no GUI workaround or process-list success is claimed. Subprocess handles record owned child exits. No browser, Playwright, build command, deployment, publication, external messaging or new Actor was used. Tests/lints compiled their normal inputs. Original mission deadline, run/retry and parallelism limits were retained. One quick Bun audit ran during the full harness's read-only scan; Cargo/instrumentation runs did not overlap.

Skill used: **oma-dev-workflow**, with execution policy, i18n rule, result contract and repeated-validation guidance. No OMA claim/run identity was injected; none was invented. Independent oma-qa belongs to downstream task-7, not this implementer's audit.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete
'''
(i/'report.md').write_text(text)
print('report',len(text.encode()),'bytes; final profiles',len(p['profiles']))
