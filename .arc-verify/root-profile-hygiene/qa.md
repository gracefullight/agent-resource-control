# Independent QA assessment — 2026-11-10 04:32 UTC

Review result: **PASS for the mission-worktree hygiene and verification contract**. Completeness: **completed for that scope**. The global worktree status is not established because access to `av.yaml` is denied. The previous report is retained verbatim below as historical evidence; this assessment supersedes its earlier gate and budget verdicts. Skill used: `oma-qa`.

## Findings

No reproducible CRITICAL, HIGH, MEDIUM or LOW defect was found within the assigned hygiene and contract scope. The two HIGH findings in the historical assessment are resolved: the corrected budget test and the gate-proof test both passed in the exact contract commands below. No product or test file was edited by this reviewer.

## Acceptance and independent observations

- The mission worktree HEAD is `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`. Its index stages the four requested profile deletions and contains **zero** tracked paths ending in `.profraw`, `.profdata` or `.lcov` (case-insensitive scan of all 3,585 index paths). The four local files are still 383,272 bytes each, mode 0644. Each is byte-identical to its task-1 backup, its accepted-HEAD blob and its saved baseline SHA-256. Direct comparisons also found the same four byte-identical files in the outer checkout `/Users/gracefullight/workspace/arc`.
- The four SHA-256 values, in suffix order `29001`, `29002`, `29029`, `29031`, are respectively `200253464f2de199a3319c5bca9e777a92ec857c3f2e5c318f9f268bd0d09ee6`, `c73320b5c5ae0043057c8a3a480611885af6a8ad10699689f177dd47bccdd31b`, `beb51c46d45de1b2c66c6b9bb2eb7f578d88eef29791ced1b39f7445585c1d42` and `c5893510c8257c602de32f2f5b0a6b325c5cfa3444e7f5c8312bcfe9987fa568`.
- The only `.gitignore` differences from HEAD are the three root-anchored patterns `/*.profraw`, `/*.profdata` and `/*.lcov`. In independent temporary Git repositories with system and global excludes disabled, **3 root probes** changed from visible to ignored, while **13 existing .arc-verify ignored probes** and **9 unignored probes** kept their prior behavior. The latter include `docs/example.profraw`, `fixtures/example.profdata` and `fixtures/example.lcov`. The existing hygiene test also ran its positive and negative ignore probes.
- Scoped `git diff --name-status HEAD` and untracked-file checks over crates, the harness, coverage scripts, manifests, lockfiles and docs found only the intended `scripts/coverage/hygiene.test.ts` change. The mission preservation test separately checked its listed configuration and coverage protection paths. The pre-existing `.arc-verify` ignore rules remain unchanged in the diff. The scoped `git diff --check` and staged `git diff --cached --check` both exited 0.
- After this assessment was written, the mission evidence directory contained **10 regular files, 116703 bytes**, below 30 files and 5,000,000 bytes. The latest operator note scopes this budget to `.arc-verify/root-profile-hygiene/`: the separately retained profile backups, `.arc-verify/rust-coverage/` output and `target/` build artifacts are outside it. The budget test rejects symlinks and non-regular entries and passed with the corrected scope.
- The outer checkout is at HEAD `41e59f2b16f727e1fcaef1acf9c9073b9109a49f`. Its four profiles are untracked and present with the baseline bytes, but its root probes for these three extensions are not ignored. This is a separate checkout; the observations above do not claim that mission-worktree changes have been integrated there. Integration and any destination follow-up belong to the Chief.

## Harness and coverage evidence

The saved `./scripts/harness/validate.sh` run began at 04:24:41.596119 UTC and ended at 04:27:29.065254 UTC on 2026-11-10, with recorded exit **0** and `skipCoverage: false`. Its captured output, `.arc-verify/root-profile-hygiene/harness-output.txt`, hashes to `da5f1f6f1d7256fa52f22f28fe0b8cdd5e0d90c5b591acae77f3d0549cf612eb`, matching `gate-results.json`. It contains all five harness checks, the coverage step, `Coverage thresholds met` and `All checks passed (0 violations).` The unchanged harness invokes `bun run test:coverage`; this was the coverage child of the harness, not a separate standalone invocation.

The live coverage run ID is `2026-11-10T04-24-48-674Z-c4014030-b332-4718-92ba-1f71581058d7`. Its `latest.json` and `receipt.json` agree on that run ID, `pass: true` and source identity `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f`; the receipt records exit 0. The actual receipt and latest-file SHA-256 values match their recorded values: `1b2d0b91a82c08883fe16d9e747dea4f3b41ac08cf341ce5e3dfc8eb89fce9f6` and `6cdc6e156c5154e1aee0eae1c03412777ec28bef88a4a956d375186ace0d1a6b`. The cargo-llvm-cov workspace collection inside that receipt ran 04:24:49.296–04:27:14.947 UTC and exited 0.

The current coverage input identity is the same hash across 185 files. The before/after identities in the gate record match it. The harness SHA-256 is `6f3eacf963b2c9d7f026f2847f2d37baa861dd4bbf70a5bd33803bbf471d97b1`; the coverage runner SHA-256 is `d32ec1940cafd539d10f259cc984ab019b90a54d17cbd49ce0613fc3815203da`. Current files match both recorded script hashes. The output reports arc-core **96.48%**, arc-detector **99.84%**, arc-storage **96.88%**, arc-ipc **98.47%**, arc-daemon **93.30%**, and arc-cli **88.00%**; each exceeds the existing 80% floor. The gate-proof and existing coverage-evidence tests below independently validated the live receipt, reports, raw inputs and child profiles.

An earlier authorized collection at 04:15 UTC failed: its receipt records exit 1 and the saturation test returned OS error 22 `Invalid argument` while setting a socket timeout. Subsequent unchanged collections passed, including the current recorded run. The cause of that earlier failure remains unknown; it is not attributed to this hygiene change. This reviewer did not rerun the expensive harness solely to duplicate the existing source-bound receipt.

## Reproduced commands

Each fixed Bun command below was executed from the mission worktree by a Python `subprocess.run` wrapper allowing **7,200 seconds** per command. All exited 0.

| Command | Result |
| --- | --- |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | 2 passed, 74 assertions; both named preservation tests ran. |
| `bun test scripts/coverage/hygiene.test.ts` | 4 passed, 224 assertions. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | 9 passed, 318 assertions across all 3 intended files. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | 3 passed, 4,550 assertions across both intended files. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | 1 passed, 2 assertions; budget test ran. |

Bun reports the three hidden mission tests through `mission-test-discovery.arc-verify/root-profile-hygiene/` shims. Each shim conditionally imports the corresponding hidden test when the baseline's workspace equals the current directory. The output showed the expected underlying test names, so a zero-test discovery result was not mistaken for a pass.

The following checks reproduce the profile and full-index comparison without changing the index:

```sh
python3 - <<'PY'
from pathlib import Path
import hashlib, json, stat, subprocess
root = Path.cwd()
outer = Path('/Users/gracefullight/workspace/arc')
baseline = json.loads((root / '.arc-verify/root-profile-hygiene/baseline.json').read_text())
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == baseline['acceptedHead']
digest = lambda b: hashlib.sha256(b).hexdigest()
for p in baseline['profiles']:
    blob = subprocess.check_output(['git', 'cat-file', 'blob', baseline['acceptedHead'] + ':' + p['path']])
    assert digest(blob) == p['sha256'] and len(blob) == p['sizeBytes']
    for f in (root / p['path'], outer / p['path'], root / p['backup']['path']):
        assert f.is_file() and stat.S_IMODE(f.stat().st_mode) == 0o644
        assert f.read_bytes() == blob, f
    print(p['path'], len(blob), digest(blob), 'mission/outer/backup/HEAD match')
paths = subprocess.check_output(['git', 'ls-files', '-z']).split(b'\0')
bad = [p for p in paths if p.lower().endswith((b'.profraw', b'.profdata', b'.lcov'))]
assert not bad, bad
print('tracked coverage-extension paths:', len(bad))
PY
```

The deterministic ignore comparison can be reproduced using accepted-HEAD and current `.gitignore` bytes in a temporary repository. The existing `scripts/coverage/hygiene.test.ts` performs the corresponding exact-pattern and negative checks; its four tests passed above. For source identity and output integrity, use:

```sh
bun -e 'import { inputIdentity } from "./scripts/coverage/source"; import { root } from "./scripts/coverage/acceptance"; let x=inputIdentity(root); console.log(x.sha256, Object.keys(x.files).length)'
python3 - <<'PY'
from pathlib import Path
import hashlib, json
r = json.loads(Path('.arc-verify/root-profile-hygiene/gate-results.json').read_text())
for kind in ('harness', 'coverage'):
    x = r[kind]
    print(kind, x['status'], x['exitCode'], 'skipCoverage:', x['skipCoverage'], x['argv'])
    for name in ('output', 'receipt', 'latest'):
        if name in x:
            f = x[name]
            actual = hashlib.sha256(Path(f['path']).read_bytes()).hexdigest()
            assert actual == f['sha256'], f['path']
            print(name, f['path'], actual)
print(Path(r['harness']['output']['path']).read_text())
PY
```

## Verification limits and handoff

`git status --short --untracked-files=no` returned exit 0 but printed `av.yaml: Operation not permitted`; `stat av.yaml` independently exited 1 with the same permission error. Scoped comparisons and tests passed, but these results do not establish global worktree cleanliness. Serena is configured but was not exposed in the available tool inventory, so this review used known-path reads and Git metadata rather than project-wide source search. No browser, GUI or external action was used.

---

## Historical assessment retained verbatim

# Independent QA refresh — 2026-11-10, after authorized gate runs

Review result: **FAIL**. Completeness: **partial**. The requested hygiene behavior, preservation checks, and the unchanged harness and nested six-crate coverage gate passed on retry. Two fixed contract checks still fail: the budget test counts excluded gate outputs, and the gate record has not yet been updated from the actual run. This review used `oma-qa`. Its only edited deliverable is this report; the existing harness and coverage runner created their normal ignored run and build artifacts.

## Findings

### HIGH — Budget contract counts excluded gate artifacts

`.arc-verify/root-profile-hygiene/budget.mission.test.ts:25-50` inventories `target/` and `.arc-verify/rust-coverage/` in addition to the mission directory. The 04:03:56 UTC operator note restricts the budget to `.arc-verify/root-profile-hygiene/`. After the successful gate, the exact budget command failed while reporting **22,562 files and 3,218,984,365 bytes** and a Bun test timeout of 5,000 ms. Those figures include excluded build and coverage outputs; they do not describe the mission directory. Before the gate, two isolated simulations using the actual test file each added 31 one-byte files to an excluded location and reproduced an exit-1 budget failure. The reproduction is retained below under `Budget defect reproduction`.

**Remediation for the Technical Director or assigned test owner:** replace lines 25-50, including the old log and assertions that reference `changed`, with the following, retaining the existing `walk` and `current` definitions. This scopes the existing contract to the operator's budget and does not leave an undefined variable:

```ts
walk(resolve(root, ".arc-verify/root-profile-hygiene"));
const evidence = [...current.values()];
const bytes = evidence.reduce((total, file) => total + file.size, 0);
console.log(`Mission evidence: ${evidence.length} files, ${bytes} bytes`);
expect(evidence.length).toBeLessThan(30);
expect(bytes).toBeLessThan(5_000_000);
```

Then rerun the unchanged budget command. This reviewer did not change the test.

### HIGH — Fixed gate proof still reads a stale gate record

`.arc-verify/root-profile-hygiene/gate-results.json:11-27` still records both gates as unavailable and not run because it predates the operator's build authorization. The exact fixed command `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` exited 1 after the successful retry: both mission proof tests failed on that stale record, while the existing live coverage-evidence test passed with 4,509 assertions. The JSON-pointer checks for `passed` therefore also remain unsatisfied.

**Remediation for the gate-record owner:** preserve the successful output log in the mission evidence directory, update both records using the actual run evidence and the fields required by `gate-proof.mission.test.ts:12-61`, then run the exact fixed proof command again. The harness invoked `bun run test:coverage` without skipping it; the same captured harness output contains the six coverage results and can support both records. Use the observed log, run ID, source and script hashes below; do not substitute the earlier unavailable record. The task assigns this reviewer only `qa.md`, so the JSON and output-log handoff require the owner.

No other reproducible CRITICAL, HIGH, MEDIUM, or LOW defect was identified in the assigned hygiene and contract scope. The first gate failure below is an observed test failure, not an established cause or a defect attributed to this hygiene change.

## Actual gate runs

Both invocations executed `./scripts/harness/validate.sh` unchanged with `SKIP_COVERAGE` unset. The harness itself invokes `bun run test:coverage`; no separate coverage invocation was used. The process wrapper allowed 7,200,000 ms. The current source identity was `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f` across 185 files both before and after each run. The harness SHA-256 was `6f3eacf963b2c9d7f026f2847f2d37baa861dd4bbf70a5bd33803bbf471d97b1`; the coverage runner SHA-256 was `d32ec1940cafd539d10f259cc984ab019b90a54d17cbd49ce0613fc3815203da`.

- **First run:** 04:15:19.823321–04:16:25.465835 UTC, exit 1. Coverage collection exited 1 because `saturation::maximum_and_exhausting_classification_load_preserves_health_and_shutdown` panicked at `crates/arc-daemon/tests/classification/saturation.rs:41`, where `set_read_timeout` returned OS error 22, `Invalid argument`. The runner reported all six measurements unavailable. The receipt is `.arc-verify/rust-coverage/baseline/runs/2026-11-10T04-15-27-677Z-08765d2a-5e1f-4a53-94e0-74acb48fc9fc/receipt.json`. The output is `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-task6-harness-mi353t6f.log`, SHA-256 `9ca6e9b7ed73bb8ae1e4e8fef3675c203f10781132cf1debd28944064fec6b36`.
- **Unchanged retry:** 04:16:50.486242–04:19:49.150797 UTC, exit 0, with `Coverage thresholds met` and `All checks passed (0 violations).` The nested coverage run ID is `2026-11-10T04-16-57-776Z-99194bee-a8df-4300-82c9-826570d56828`; its collection command exited 0 at 04:19:37.480 UTC. `.arc-verify/rust-coverage/baseline/latest.json` has `pass: true` and the source identity above. Its live `receipt.json` has `pass: true`, `exit: 0`, the same run ID and source hash; its SHA-256 is `e50ed19d94a89188050a42ff7952d05b4612b7267657fdc8703570984d66c015`. The captured combined output is `/var/folders/r9/0qvpcx794n385zwgzwfvt55h0000gn/T/arc-task6-harness-retry-q6224ymg.log`, 1,487 bytes, SHA-256 `78c981d3b3605b369d20aa402983bddc44fe8b7a902e2dc9046a953a170ce00e`. This temporary path must be retained or copied by the gate-record owner before it is used as lasting proof.

The retry measured production-line coverage of arc-core **96.48%**, arc-detector **99.84%**, arc-storage **96.88%**, arc-ipc **98.47%**, arc-daemon **93.24%**, and arc-cli **88.00%**, all above the existing 80% floor. The successful retry does not explain why the first test failed.

## Acceptance evidence

- **Destination and preservation:** `baseline.json` and `integration.md` identify this mission workspace as the destination. Its HEAD is `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`. The four staged deletions leave the four local 383,272-byte, mode-0644 profiles in place; their SHA-256 values match their baselines, separate backups and accessible accepted-HEAD Git blobs. The historical `Profile, history and index` reproduction block was independently rerun and printed all four matching hashes. The separate `/Users/gracefullight/workspace/arc` checkout was checked only as context; no outer-checkout integration handoff is required.
- **Index and ignores:** a case-insensitive NUL-delimited whole-index scan found zero tracked `.profraw`, `.profdata`, or `.lcov` paths. `.gitignore:12-14` contains the three root-anchored rules. An independent isolated Git comparison passed 28 probes against each of accepted HEAD and the current rules: three root probes changed from visible to ignored, while 16 selected `.arc-verify` and nine visible paths, including the docs and fixtures examples, retained their behavior. The exact reproduction is under `Deterministic ignore comparison` below.
- **Protected scope:** after the gate, the combined hygiene and preservation command passed nine tests with 318 assertions. Scoped `git diff --name-status HEAD` and untracked checks found only the intended `scripts/coverage/hygiene.test.ts` change in the inspected crates, harness, coverage scripts, manifests and documentation. The `.gitignore` addition and four staged profile deletions are the other requested changes. Both scoped `git diff --check` and `git diff --cached --check` exited 0.
- **Discovery and checkout simulation:** the exact unprefixed contract commands reported the intended mission tests through their discovery shims. In a disposable different-directory simulation, the three unprefixed commands each exited 0 with zero tests, so test counts must be checked outside the mission workspace. A separate simulated candidate checkout without any of the four profiles or backups passed the four hygiene tests and a sentinel: five tests, 225 assertions. This is a scoped simulation, not a complete fresh-checkout test suite.
- **Evidence budget:** the current `.arc-verify/root-profile-hygiene/` directory contains **8 files and 99704 bytes**, below 30 files and 5,000,000 bytes. This census includes this report and all files in that directory. The 1,533,088-byte separate profile backups, existing gate outputs under `.arc-verify/rust-coverage/`, and build outputs under `target/` are outside the budget scope in the latest operator note. They remain in place.

## Commands and outcomes

Fixed commands were executed exactly as written from the mission workspace, with no wrapper timeout shorter than 7,200,000 ms. Bun's own 5,000 ms test timeout was separately reported by the budget test after the gate.

| Command or independent check | Result |
| --- | --- |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; 2 tests, 74 assertions. |
| `bun test scripts/coverage/hygiene.test.ts` | Exit 0; 4 tests, 224 assertions. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0 after the gate; 9 tests, 318 assertions, across 3 files. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | Exit 1 after the successful retry; 2 stale-record proof failures and 1 live evidence pass, 4,509 assertions. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | Exit 1 after the gate; counted 22,562 files and 3,218,984,365 bytes from the wrong scopes, and reported a 5,000 ms Bun test timeout. |
| `./scripts/harness/validate.sh` | First execution exit 1 at the saturation test; unchanged retry exit 0 with six coverage passes. The actual logs and receipts are identified above. |
| Historical `Profile, history and index` and `Deterministic ignore comparison` reproduction blocks; case-insensitive whole-index scan | Exit 0; four profile/backup/blob matches, 56 expected ignore results, zero tracked coverage-extension paths. |
| Historical `Fresh-checkout simulation`, `Standalone discovery simulation`, and `Budget defect reproduction` blocks | Exit 0 for each wrapper; candidate checkout 5 tests passed without local profiles, each separate-directory mission command ran zero tests, and both excluded-output budget simulations reproduced failure. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts`; `git diff --cached --check` | Exit 0 each. |
| `bun --version`; `cargo --version`; `cargo llvm-cov --version`; `rustup toolchain list` | Bun 1.4.4, Cargo/Rust 1.95.0, cargo-llvm-cov 0.6.21. |
| `git status --short --untracked-files=no`; `ls -ldO@ av.yaml` | Git returned 0 but printed `av.yaml: Operation not permitted`; `ls` returned 1 with the same error. Global worktree cleanliness remains unverified. |

The first simulation wrapper failed to match the fresh-checkout heading; the corrected wrapper ran both simulations successfully as recorded above.

Serena was not exposed in the available tool inventory; this review used known-path reads, Git metadata and the existing validators. No browser, GUI, Playwright or external delivery was used. The fixed proof and budget failures above prevent mission acceptance until their respective owners complete the handoffs and the commands pass.

---

## Earlier independent QA — 2026-11-10 before these gate runs

The earlier report below is retained as a dated historical record. Its statements that the gates were not run and that coverage evidence was absent describe the earlier point in time and are superseded by the current results above. Its destination and budget-remediation text has been corrected to remove the prior handoff and undefined-variable errors.

# Independent QA refresh — 2026-11-10 after the 04:03:56 UTC operator note

Review result: **FAIL**. Completeness: **partial**. The scoped hygiene and preservation checks pass. One reproducible defect in the required budget contract can make authorized gate output fail that contract. The required harness and six-crate coverage results are **unverified**, not passed. This review used `oma-qa` and changes only this report.

## Findings

### HIGH — The budget contract counts outputs the operator excluded

`.arc-verify/root-profile-hygiene/budget.mission.test.ts:25-38` walks the baseline's `target/` and `.arc-verify/rust-coverage/` scopes and scans native coverage paths. The 04:03:56 UTC operator note limits this mission's budget to `.arc-verify/root-profile-hygiene/`, excluding those existing gate and build outputs. The current check happens to pass before a coverage run; it can falsely fail afterward. In two disposable simulations using the actual test bytes, 31 one-byte files under either excluded `target/arc-coverage/` or excluded `.arc-verify/rust-coverage/baseline/runs/` produced `Mission evidence: 33 files` and exit 1, even though the mission directory held only two files. The reproduction is below. This is a contract-check defect, not a defect in the hygiene rules or an observed failure of the coverage gate.

**Owner action (corrected):** the Technical Director or assigned test owner should replace lines 25-50 with a census restricted to the mission evidence directory. Retaining the existing `walk` and `current` definitions, the replacement also updates the log and both assertions:

```ts
walk(resolve(root, ".arc-verify/root-profile-hygiene"));
const evidence = [...current.values()];
const bytes = evidence.reduce((total, file) => total + file.size, 0);
console.log(`Mission evidence: ${evidence.length} files, ${bytes} bytes`);
expect(evidence.length).toBeLessThan(30);
expect(bytes).toBeLessThan(5_000_000);
```

Then run the unchanged budget command after the authorized gate. The reviewer did not modify the test. No other CRITICAL, HIGH, MEDIUM or LOW defect was identified within the assigned hygiene and contract scope.

## Verification gaps and handoff

- **Harness and coverage:** `.arc-verify/root-profile-hygiene/gate-results.json:11-27` was recorded at 03:47:05 UTC and lists both gates as `unavailable`, with null exit codes; its stated lack of build authorization is now stale. The 04:03:56 UTC operator note explicitly authorizes compilation and requires the unchanged gates. The current evidence does not establish that either gate ran: `.arc-verify/rust-coverage/baseline/latest.json` is absent, and the fixed proof command failed three tests. This reviewer did not run the gates or write the other Actor's gate record; the task assigns this reviewer only `qa.md` and specifically routes gate execution and recording to the owner. The Chief should coordinate the authorized unchanged harness with coverage enabled, record actual command outputs, exit codes, timestamps, source hashes, script hashes and coverage run ID in `gate-results.json`, and rerun the exact proof and evidence command. `scripts/harness/validate.sh:434` invokes `bun run test:coverage`, so its actual nested coverage run can supply the coverage record when all required source-bound fields and output are captured. Neither a source hash nor an older receipt is a fresh pass.
- **Destination correction:** the mission workspace named by `baseline.json` and `integration.md` is the delivered destination. The separate checkout at `/Users/gracefullight/workspace/arc` has matching profile bytes and three visible root probes; that observation is context, and no outer-checkout integration handoff is required by this task.
- **Test discovery outside this mission workspace:** the exact unprefixed fixed commands did run the intended named tests here. The shims at `mission-test-discovery.arc-verify/root-profile-hygiene/` import them only when the baseline workspace equals the current directory. A disposable different-directory simulation returned exit 0 with **zero tests** for each unprefixed mission command. Do not treat such a result as evidence after integration; verify test counts. A separate candidate-tree simulation, without the ignored profiles or backups, passed the four existing hygiene tests and a sentinel. It establishes that this simulated ordinary checkout did not require those local files; it is not a complete execution of every test in a fresh checkout.

## Acceptance evidence

- HEAD is `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`. Only the four requested profile deletions are staged. Each local mission file, original-checkout file, backup, and accepted-HEAD Git blob is 383,272 bytes, has the baseline SHA-256, and is accessible. The filesystem copies retain mode `0644`. The profile-by-profile hashes and reproducible `Profile, history and index` block are retained in the historical report below.
- A case-insensitive scan of the entire NUL-delimited index found **zero** paths ending in `.profraw`, `.profdata`, or `.lcov`. `.gitignore:12-14` adds the three root-anchored rules. The independent isolated Git comparison ran 28 paths against each ignore file: three root examples changed to ignored; 16 selected `.arc-verify` cases and nine visible cases, including the docs and fixtures examples, retained their behavior. These are deterministic simulated paths, not observations of users.
- `git diff --name-status` against the accepted HEAD found only the intended `.gitignore` and `scripts/coverage/hygiene.test.ts` edits among the scoped product, coverage, harness, manifest and documentation paths; the four profile deletions are the separate staged changes. The preservation tests passed. No unrelated product or coverage-protection edit was found in the checked scope.
- The mission evidence directory contains **8 files and 87812 bytes**, below 30 files and 5,000,000 bytes. This is the operator's current budget scope. The four separately retained backups total 1,533,088 bytes and are outside that directory. The existing budget test presently also counts backups, discovery shims and other scopes; its pass must not be mistaken for proof that its scope is correct after a gate run.
- The current coverage-input identity is `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f` across 185 files; the harness and runner hashes match the informational fields in the old gate record. Historical receipt source hashes `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8` and `23db85747b5e61c3637d93684e84aba95fd6c8bd5d924135541bc3c875859399` differ. None of these hashes is proof of a fresh gate pass.

## Commands executed during this refresh

All fixed commands below were run from the mission workspace exactly as specified. The process runner imposed no timeout shorter than 7,200,000 ms; all finished in seconds. The temporary Git repositories and checkout simulations were isolated and cleaned up by their temporary-directory context managers.

| Command or check | Actual result |
| --- | --- |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; 2 named mission tests, 74 assertions, through the discovery shim. |
| `bun test scripts/coverage/hygiene.test.ts` | Exit 0; 4 tests, 224 assertions. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | Exit 0; 9 tests, 318 assertions, across 3 reported files. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | Exit 1; 0 passed, 3 failed: the two records are unavailable and the live `baseline/latest.json` is absent. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | Exit 0; 1 test, 2 assertions, through the discovery shim. The defect above was separately reproduced. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts`; `git diff --cached --check` | Exit 0 for each. |
| Historical `Profile, history and index`, `Deterministic ignore comparison`, and `Outer checkout` reproduction blocks below | Exit 0 each; four matching profiles, 56 expected ignore results, and outer-checkout root examples visible. An additional case-insensitive whole-index scan also returned zero paths. |
| Historical `Fresh-checkout simulation` and `Standalone discovery simulation` blocks below | Exit 0 for both wrappers. The first ran 5 tests and 225 assertions with the profiles and backups absent; the second reported 0 tests for each of three unprefixed commands in the different directory. |
| Disposable budget reproduction below | Exit 0 for the wrapper; both intentionally exercised budget tests exited 1 with 33 counted files. |
| `bun --version`; `cargo --version`; `cargo llvm-cov --version`; `rustup toolchain list` | Bun 1.4.4, Cargo 1.95.0, cargo-llvm-cov 0.6.21; Rust 1.95.0 available. These version checks do not establish a gate run. |
| `GIT_OPTIONAL_LOCKS=0 git status --short --untracked-files=no`; `ls -ldO@ av.yaml` | Git exited 0 but printed `av.yaml: Operation not permitted`; `ls` exited 1 with the same error. Global worktree cleanliness remains unverified. |
| `ps -Ao pid,ppid,etime,command` | `Operation not permitted`; process inventory was unavailable. |

The matching Serena search tools were not exposed in this session. This review used known-path reads, Git metadata and the existing tests. No browser, GUI, external delivery, harness, coverage run, or compilation was used in this review.

## Budget defect reproduction

Run from the mission workspace. This copies the actual budget test and its imports into an isolated temporary directory. It creates only synthetic one-byte files in that temporary directory and removes them on exit.

~~~sh
python3 - <<'PY'
import json, shutil, subprocess, tempfile
from pathlib import Path
source = Path.cwd()
with tempfile.TemporaryDirectory(prefix='arc-budget-repro-') as name:
    root = Path(name)
    for rel in ('.arc-verify/root-profile-hygiene/budget.mission.test.ts',
                'scripts/coverage/source.ts', 'scripts/coverage/config.ts'):
        target = root / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / rel, target)
    baseline = root / '.arc-verify/root-profile-hygiene/baseline.json'
    baseline.write_text(json.dumps({'preMissionInventory': {
        'scopes': [{'path': '.arc-verify/root-profile-hygiene', 'files': []},
                   {'path': '.arc-verify/rust-coverage', 'files': []},
                   {'path': 'target', 'files': []}],
        'rootCoverageExtensionPaths': []}}))
    for label, folder in (
        ('excluded target outputs', root / 'target/arc-coverage/run'),
        ('excluded coverage outputs', root / '.arc-verify/rust-coverage/baseline/runs/run'),
    ):
        folder.mkdir(parents=True, exist_ok=True)
        for i in range(31):
            (folder / f'synthetic-{i}.dat').write_bytes(b'x')
        p = subprocess.run(['bun', 'test', './.arc-verify/root-profile-hygiene/budget.mission.test.ts'],
                           cwd=root, text=True, capture_output=True)
        output = p.stdout + p.stderr
        print(label, 'exit', p.returncode)
        print(next(line for line in output.splitlines() if 'Mission evidence:' in line))
        assert p.returncode == 1 and 'Mission evidence: 33 files' in output
        shutil.rmtree(folder)
PY
~~~

## Evidence census reproduction

~~~sh
python3 - <<'PY'
from pathlib import Path
root = Path('.arc-verify/root-profile-hygiene')
files = [p for p in root.rglob('*') if p.is_file()]
print(len(files), 'files,', sum(p.stat().st_size for p in files), 'bytes')
PY
~~~

---

## Historical refresh — before the 04:03:56 UTC operator note

The earlier refresh and its earlier historical record are preserved below. Their gate-authorization statements and evidence counts describe the time they were written and are superseded by the current review above.

# Independent QA refresh: root coverage profiles

Reviewed 2026-11-10 after task 5 with `oma-qa`. Result: **NOT_ASSESSED**; completeness: **partial**. The independently rerun hygiene and preservation checks pass. The required unchanged harness and six-crate coverage gates were not run and remain unavailable. This report does not accept the mission.

## Findings

No reproducible CRITICAL, HIGH, MEDIUM or LOW implementation defect was found in the reviewed hygiene change. The checks below are scoped to the requested acceptance and contract. The following are verification and interface limits, not claims that the product is defective.

- **Required gates unavailable:** `.arc-verify/root-profile-hygiene/gate-results.json:11-27` records both commands as `unavailable`, with null exit codes. This task prohibits running them without an explicit build request; the repository's `.agents/skills/_shared/core/execution-policy.md:12` also prohibits compilation without one. `scripts/harness/validate.sh:434` invokes coverage, and `scripts/coverage/run.ts:65-69` uses a fresh Cargo target. The Chief must resolve that prerequisite, then arrange the unchanged gate, actual source-bound receipts and fixed proof checks. No gate was run by this review, and no pass is claimed. The exact fixed gate-proof command ran three tests and failed all three: two on unavailable records and one because `.arc-verify/rust-coverage/baseline/latest.json` is absent.
- **Mission-only discovery interface:** `mission-test-discovery.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts:3-5` and the equivalent budget and gate shims load tests only when the working directory equals the absolute `workspace` in `baseline.json`. In this mission workspace the exact unprefixed contract commands did run the intended tests, as the command results below show. In a different disposable directory containing the same shims and baseline, each standalone unprefixed command returned exit 0 with **0 tests**. The Chief must check test counts and independently recheck bytes and ignore behavior after integration in another destination; that zero-test exit must not be used as proof. In the simulated fresh checkout described below, the ordinary hygiene tests and a sentinel passed without profiles or backups. This is a measured scope limit, not a request to harden the tests.

## Current acceptance evidence

- The worktree HEAD is `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`. The four deletions alone are staged. All four 383,272-byte, mode-0644 profiles in the mission worktree, the separate outer checkout `/Users/gracefullight/workspace/arc`, and `.agents/results/root-profile-hygiene/profile-backups/` match the retained baseline SHA-256 values and the readable accepted-HEAD blobs. The four hashes and exact reproduction script are retained in the historical record below.
- A fresh NUL-delimited, case-insensitive scan of the entire current index returned **zero** paths ending in `.profraw`, `.profdata` or `.lcov`. `.gitignore:12-14` adds only the three requested root-anchored rules. The independent isolated Git comparison of accepted-HEAD and current ignore files passed 28 probes each: all three root probes changed to ignored; 16 selected `.arc-verify` and nine visible paths, including the docs and fixtures examples, retained their expected behavior.
- The fixed combined hygiene/preservation command executed **nine tests across three files**, including the two restored mission tests, and passed 318 assertions. The scoped tracked/untracked comparisons and the existing preservation tests found no changes in the checked crates, harness, coverage protections, listed manifests or coverage documentation, apart from the intended `scripts/coverage/hygiene.test.ts` edit. `git diff --check` and `git diff --cached --check` passed.
- Current coverage input identity is `61a94138b8f0a5696b123ba654e55241d1e23b2d8eee0172e41558690111543f` across 185 files, matching the informational hash in `gate-results.json`. The harness and runner hashes also match that record. Computing an identity is not a test run. The inspected historical receipts have different source hashes and cannot prove the current gates.
- The mission evidence census, including four backups, the report, required contract files, discovery shims and detected new native coverage paths, is **15 files and 1,609,974 bytes**. This is below 30 files and 5,000,000 bytes. The exact budget contract test was rerun after this report was written.
- The outer checkout has matching profile bytes and no tracked copies, but its three root probes remain visible. It is separate context; the mission workspace is the destination.

## Commands executed

All fixed test commands ran exactly as specified from the mission workspace. The process runner imposed no sub-two-hour timeout on any fixed check; all finished within seconds. Temporary repositories used by the tests and simulations were isolated and automatically cleaned up.

| Command | Exit and observed result |
| --- | --- |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | 0; 2 mission tests, 74 assertions, reported through the discovery shim. |
| `bun test scripts/coverage/hygiene.test.ts` | 0; 4 tests, 224 assertions. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | 0; 9 tests, 318 assertions, across 3 files. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | 1; 0 passed, 3 failed across 2 files. Both gates are unavailable and `baseline/latest.json` is absent. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | 0; 1 test and 2 assertions. Its final census is stated above. |
| `bun test ./.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts ./.arc-verify/root-profile-hygiene/budget.mission.test.ts` | 0; 3 tests and 76 assertions across the two underlying hidden files. This establishes what the mission shims import. |
| Historical reproduction blocks `Profile, history and index`, `Deterministic ignore comparison`, `Historical receipt inspection`, `Scoped tracked and untracked comparisons`, and `Outer checkout` | 0 each. Four worktree/backup/outer/blob hashes matched; 56 ignore results matched; scoped diff listed only the intended hygiene test. |
| Independent case-insensitive `git ls-files -z` scan | 0; zero coverage-extension paths. |
| Fresh-checkout simulation below | 0; 5 tests, 225 assertions: four hygiene tests and one sentinel. The three shims ran without importing mission-only tests; profiles and backups were absent. |
| Standalone discovery simulation below | 0 for its wrapper; each of the 3 unprefixed mission commands independently exited 0 with 0 tests in a different directory. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts`; `git diff --cached --check` | 0 each. |
| `git status --short --untracked-files=normal` | 0, but printed `av.yaml: Operation not permitted`; global cleanliness remains unverified. |
| `ls -ldO@ av.yaml` | 1; `Operation not permitted`. |

The first historical report below predates task 5. Its claim that contract artifacts are absent and its old evidence totals are retained as historical observations; they are superseded by this refresh.

## Reproduction of current independent checks

The prior report's named reproduction blocks remain intact below. This wrapper executes the five used in this refresh:

~~~sh
python3 - <<'PY'
import re, subprocess
from pathlib import Path
text = Path('.arc-verify/root-profile-hygiene/qa.md').read_text()
for heading in ('Profile, history and index', 'Deterministic ignore comparison', 'Historical receipt inspection', 'Scoped tracked and untracked comparisons', 'Outer checkout'):
    match = re.search(r'^### ' + re.escape(heading) + r'\n\n~~~sh\n(.*?)\n~~~', text, re.M | re.S)
    assert match, heading
    result = subprocess.run(['bash'], input=match.group(1), text=True, capture_output=True)
    print('###', heading, 'exit', result.returncode)
    print(result.stdout, end='')
    print(result.stderr, end='')
    if result.returncode:
        raise SystemExit(result.returncode)
PY
~~~

The case-insensitive whole-index scan and the informational source binding were reproduced with:

~~~sh
python3 - <<'PYINDEX'
import subprocess
bad = [p for p in subprocess.check_output(['git', 'ls-files', '-z']).split(b'\0')
       if p and p.lower().endswith((b'.profraw', b'.profdata', b'.lcov'))]
print('Tracked coverage extension paths:', len(bad))
assert not bad
PYINDEX
bun -e 'import {inputIdentity,hash} from "./scripts/coverage/source"; import {readFileSync} from "node:fs"; const x=inputIdentity(process.cwd()); console.log(JSON.stringify({currentSourceHash:x.sha256,fileCount:Object.keys(x.files).length,harness:hash(readFileSync("scripts/harness/validate.sh")),coverageRunner:hash(readFileSync("scripts/coverage/run.ts"))}));'
~~~

The current evidence census can be reproduced using the historical `Evidence inventory` block with `walk(root / "mission-test-discovery.arc-verify")` inserted immediately before `arc = root / ".arc-verify"`. The fixed budget command independently covers these scopes and reports its own result.

Fresh-checkout simulation: clone locally without checkout; apply the four deletions to the temporary index before checkout; overlay the candidate files; then run the non-compiling test paths. This simulates the candidate tree and is not an actual integrated checkout.

~~~sh
python3 - <<'PY'
import json, shutil, subprocess, tempfile
from pathlib import Path

source = Path.cwd()
baseline = json.loads((source / '.arc-verify/root-profile-hygiene/baseline.json').read_text())
with tempfile.TemporaryDirectory(prefix='arc-qa-fresh-') as directory:
    checkout = Path(directory) / 'checkout'
    def git(*args):
        return subprocess.run(['git', *args], cwd=checkout, check=True, capture_output=True, text=True)
    subprocess.run(['git', 'clone', '--quiet', '--shared', '--no-checkout', str(source), str(checkout)], check=True, capture_output=True, text=True)
    git('read-tree', baseline['acceptedHead'])
    git('update-index', '--force-remove', '--', *[p['path'] for p in baseline['profiles']])
    git('checkout-index', '--all')
    for rel in [
        '.gitignore', 'scripts/coverage/hygiene.test.ts',
        '.arc-verify/root-profile-hygiene/baseline.json',
        '.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts',
        '.arc-verify/root-profile-hygiene/gate-proof.mission.test.ts',
        '.arc-verify/root-profile-hygiene/budget.mission.test.ts',
        '.arc-verify/root-profile-hygiene/gate-results.json',
        'mission-test-discovery.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts',
        'mission-test-discovery.arc-verify/root-profile-hygiene/gate-proof.mission.test.ts',
        'mission-test-discovery.arc-verify/root-profile-hygiene/budget.mission.test.ts'
    ]:
        path = checkout / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / rel, path)
    for profile in baseline['profiles']:
        assert not (checkout / profile['path']).exists()
        assert not (checkout / profile['backup']['path']).exists()
    sentinel = checkout / 'fresh-checkout-sentinel.test.ts'
    sentinel.write_text('import { test, expect } from "bun:test";\ntest("fresh-checkout sentinel", () => expect(true).toBe(true));\n')
    files = ['scripts/coverage/hygiene.test.ts',
             'mission-test-discovery.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts',
             'mission-test-discovery.arc-verify/root-profile-hygiene/gate-proof.mission.test.ts',
             'mission-test-discovery.arc-verify/root-profile-hygiene/budget.mission.test.ts',
             sentinel.name]
    result = subprocess.run(['bun', 'test', *files], cwd=checkout, text=True, capture_output=True)
    print('profiles absent: 4; local backups absent: 4')
    print('command: bun test ' + ' '.join(files))
    print(result.stdout, end='')
    print(result.stderr, end='')
    print('exit:', result.returncode)
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert '5 pass' in output and 'fresh-checkout sentinel' in output
    assert 'mission profile preservation covers destination' not in output
PY
~~~

Standalone discovery simulation:

~~~sh
python3 - <<'PY'
import shutil, subprocess, tempfile
from pathlib import Path
source = Path.cwd()
with tempfile.TemporaryDirectory(prefix='arc-qa-discovery-') as directory:
    root = Path(directory)
    names = ('profile-preservation.mission.test.ts', 'gate-proof.mission.test.ts', 'budget.mission.test.ts')
    for name in ('baseline.json', *names, 'gate-results.json'):
        target = root / '.arc-verify/root-profile-hygiene' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / '.arc-verify/root-profile-hygiene' / name, target)
    for name in names:
        target = root / 'mission-test-discovery.arc-verify/root-profile-hygiene' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / 'mission-test-discovery.arc-verify/root-profile-hygiene' / name, target)
    for name in names:
        test = '.arc-verify/root-profile-hygiene/' + name
        result = subprocess.run(['bun', 'test', test], cwd=root, text=True, capture_output=True)
        print(test, 'exit', result.returncode, result.stderr.strip())
        assert result.returncode == 0 and 'Ran 0 tests' in result.stderr
PY
~~~

## Tool and inspection limits

Serena was configured but no matching tools were exposed. This review used known-path reads, Git metadata and the existing tests. The av.yaml access error prevents a claim of global cleanliness. No browser, GUI, Playwright, compilation, harness, coverage gate or external delivery was used.

---

## Historical record — 2026-11-10, before task 5

The following earlier report is preserved as written. Its findings and counts describe that earlier checkout state.

# Independent QA: root coverage profiles

Review result: **NOT_ASSESSED**. Completeness: **partial**. The non-compiling checks passed for the inspected hygiene change, but required contract artifacts and fresh harness and coverage evidence are missing. The mission is not accepted. Reviewed on 2026-11-10 with oma-qa; this Actor changed only .arc-verify/root-profile-hygiene/qa.md.

## Findings

No reproducible implementation defect was found in the inspected hygiene change. The following high-severity **verification gaps** block mission acceptance; they are not claims of a product defect.

### HIGH — Required contract artifacts are absent

`.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts`, `gate-proof.mission.test.ts`, `budget.mission.test.ts`, and `gate-results.json` are missing. The immutable contract requires these paths, and the corresponding Bun invocations cannot establish their checks. The corrected operator note requires their restoration or recreation. The Chief should coordinate that work with the owning task, then rerun the fixed contract commands and verify that Bun actually reports each required test file as executed. This reviewer owns only `qa.md` and did not create those files or a receipt.

Bun 1.4.4's unprefixed hidden path is treated as a filter: the disposable reproduction below returned 1 for the exact unprefixed contract path and 0 for the `./`-prefixed path. The fixed combined hygiene command exited 0 while running only the two visible existing files, not the absent mission test. Restoring files alone does not prove that the unchanged commands discover them.

### HIGH — Harness and coverage evidence is unavailable

`gate-results.json` and `.arc-verify/rust-coverage/baseline/latest.json` are absent. `scripts/coverage/evidence.test.ts:11` failed on the missing `latest.json`, so there is no fresh source-bound coverage result to verify. The harness and six-crate coverage gate were not run in this review: the assigned task prohibits compilation, and `.agents/skills/_shared/core/execution-policy.md:12` requires an explicit build request before compiling. `scripts/harness/validate.sh:434` invokes `bun run test:coverage`; `scripts/coverage/run.ts:65-69` creates a fresh Cargo target and runs instrumented tests. The responsible owner must resolve the build prerequisite, run the unchanged required gate, record the actual results and source binding, and rerun the contract checks. Historical receipts and a computed source hash are not a fresh gate pass. The inspected `historical/accepted-head-commands.json` records source hash `cf31468e23ac45fee3e5f04704e19d4b89f9b56365ef516b9daa9ebc6c9833d8`; `baseline/first-measurement.json` records `23db85747b5e61c3637d93684e84aba95fd6c8bd5d924135541bc3c875859399`. Both differ from this checkout's computed identity `95e4deaa22b8163a2da2eea9f67c01e654f158f3bed1d13d6d85a4377e2a4687`.

## Acceptance evidence

- The mission worktree is at accepted HEAD e0875e7449c4e5d6752dfd1b6508c408ebd42ed3. Four profile deletions are staged. Each worktree file and backup is a regular file with mode 0644 and 383272 bytes, and each SHA-256 matches task-1's baseline and the accepted-HEAD blob. The same checks passed for the four files in the outer checkout. The script below also verifies accepted-HEAD tree modes and object IDs.
- A NUL-delimited scan of the entire index found no .profraw, .profdata, or .lcov paths. The .gitignore diff adds only the three root rules at lines 12-14. scripts/coverage/hygiene.test.ts:47 adds three root probes while retaining prior cases.
- An isolated Git comparison against accepted-HEAD and current .gitignore passed 56 of 56 expected results. For each version it tested three root paths, 16 .arc-verify paths and nine visible paths including docs/example.profraw, fixtures/example.profdata and fixtures/example.lcov. The root paths changed from visible to ignored; the selected .arc-verify and visible examples retained their behavior. These are simulated paths, not a user study.
- Existing non-compiling hygiene and preservation checks passed: seven tests and 240 assertions. A scoped comparison of crates, coverage/harness scripts, selected configuration, manifests and docs found only the intended hygiene test edit. The corresponding nonignored-untracked query returned no paths. A separate diff found no changes in the harness, coverage runner, gate, configuration, source identity, hygiene protections, preservation test, evidence test and package.json.
- The contract files and gate-results.json are missing. The existing source-bound evidence test failed because .arc-verify/rust-coverage/baseline/latest.json is missing. The harness and coverage gate are **unverified**, not passed.
- The inventory below found **8 files and 1578993 bytes** of new or changed retained mission evidence, including the four backups and this report. The note directory contains **4 files and 45905 bytes**. Both totals are below 30 files and 5000000 bytes. The census compares task-1's recorded inventory scopes by path, type, size and SHA-256, deduplicates overlaps, scans .arc-verify runs/profiles and coverage-extension files, and checks root extension additions. No new native coverage output appeared in those scopes. It does not predict a future gate run's size.

All profile names begin with default_16827093085728555556_0_:

| Suffix | Captured and independently confirmed SHA-256 |
| --- | --- |
| 29001.profraw | 200253464f2de199a3319c5bca9e777a92ec857c3f2e5c318f9f268bd0d09ee6 |
| 29002.profraw | c73320b5c5ae0043057c8a3a480611885af6a8ad10699689f177dd47bccdd31b |
| 29029.profraw | beb51c46d45de1b2c66c6b9bb2eb7f578d88eef29791ced1b39f7445585c1d42 |
| 29031.profraw | c5893510c8257c602de32f2f5b0a6b325c5cfa3444e7f5c8312bcfe9987fa568 |

## Commands and outcomes

All commands ran from the mission worktree during this review. The reproduction blocks below were rerun using the extractor shown in the final reproduction block; each named block exited 0. The combined contract invocation did not execute the absent hidden test, despite its exit 0.

| Command | Exit and result |
| --- | --- |
| `git rev-parse HEAD` | 0; accepted HEAD above. |
| `git diff -- .gitignore scripts/coverage/hygiene.test.ts` | 0; only the three root rules and three root test cases. |
| `git diff --cached --name-status` | 0; only the four requested profile deletions. |
| Profile, accepted-HEAD and index script below | 0; worktree, backup, outer and blob comparisons matched for all four; zero tracked extension paths. |
| Ignore comparison script below | 0; 28 accepted-HEAD and 28 current results passed. |
| `bun test scripts/coverage/hygiene.test.ts` | 0; four tests, 220 assertions, one file. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts` | 0; seven tests, 240 assertions, two files. |
| `bun test scripts/coverage/hygiene.test.ts scripts/coverage/hygiene-preservation.test.ts .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | 0; seven tests, 240 assertions across **two** files; the missing mission test did not run. |
| `bun test .arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts` | 1; no matching test file. |
| `bun test .arc-verify/root-profile-hygiene/budget.mission.test.ts` | 1; no matching test file. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts` | 1; no matching test file. |
| `bun test .arc-verify/root-profile-hygiene/gate-proof.mission.test.ts scripts/coverage/evidence.test.ts` | 1; only the existing evidence test ran, failing on missing baseline/latest.json. |
| Bun discovery script below | 0; the unprefixed hidden test invocation exited 1; the ./-prefixed invocation exited 0 and ran one test. |
| `bun test .arc-verify/managed-exec/implementation/canonical-root.test.ts` | 1; existing hidden file was not discovered as a filter. |
| `bun test ./.arc-verify/managed-exec/implementation/canonical-root.test.ts` | 0; one test, three assertions. |
| `git diff --check -- .gitignore scripts/coverage/hygiene.test.ts` | 0; no whitespace errors. |
| `git diff --cached --check` | 0; no whitespace errors. |
| Scoped tracked, untracked and protection comparisons below | 0 for each; the tracked result listed only M scripts/coverage/hygiene.test.ts; the other two returned no paths. |
| `bun -e 'import {inputIdentity} from "./scripts/coverage/source"; const x=inputIdentity(process.cwd()); console.log("sha256="+x.sha256+" files="+Object.keys(x.files).length)'` | 0; input identity 95e4deaa22b8163a2da2eea9f67c01e654f158f3bed1d13d6d85a4377e2a4687 across 185 files. This is not a gate receipt. |
| Contract artifact inventory below | 0; all four contract files and baseline/latest.json missing. |
| Historical receipt inspection below | 0; two historical hashes differ from the current identity. |
| Evidence inventory below | 0; totals above. |
| Report structure check below | 0; required headings and populated totals present. |
| `git status --short --untracked-files=no` | 0 but stderr included av.yaml: Operation not permitted; global cleanliness is unverified. |
| `ls -ldO@ av.yaml` | 1; Operation not permitted. |
| `git -C /Users/gracefullight/workspace/arc rev-parse HEAD` | 0; 41e59f2b16f727e1fcaef1acf9c9073b9109a49f. |
| `git -C /Users/gracefullight/workspace/arc cat-file -t e0875e7449c4e5d6752dfd1b6508c408ebd42ed3` | 128; the accepted object is unavailable in the outer checkout's object database. |
| Outer checkout script below | 0; four profile paths are untracked there and the three root probes are visible. |

## Reproduction scripts

These are the exact scripts used for this review. They create only disposable temporary directories where indicated. The test commands above use existing tests.

### Profile, history and index

~~~sh
python3 - <<'PY'
import hashlib, json, os, stat, subprocess
from pathlib import Path

root = Path.cwd()
outer = Path("/Users/gracefullight/workspace/arc")
baseline = json.loads((root / ".arc-verify/root-profile-hygiene/baseline.json").read_text())
head = baseline["acceptedHead"]
assert subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip() == head
assert Path(baseline["workspace"]) == root
records = baseline["profiles"]
assert len(records) == 4
for record in records:
    path = record["path"]
    for label, file in (("worktree", root / path),
                        ("backup", root / record["backup"]["path"]),
                        ("outer", outer / path)):
        info = file.lstat()
        assert stat.S_ISREG(info.st_mode), (label, path, "not regular")
        assert stat.S_IMODE(info.st_mode) == int(record["mode"], 8), (label, path, "mode")
        assert info.st_size == record["sizeBytes"], (label, path, "size")
        assert hashlib.sha256(file.read_bytes()).hexdigest() == record["sha256"], (label, path, "sha256")
    tree = subprocess.check_output(["git", "ls-tree", head, "--", path], text=True).strip()
    meta, name = tree.split("\t")
    mode, kind, oid = meta.split()
    assert (mode, kind, oid, name) == (record["acceptedHeadBlob"]["mode"], "blob", record["acceptedHeadBlob"]["oid"], path)
    blob = subprocess.check_output(["git", "cat-file", "blob", f"{head}:{path}"])
    assert len(blob) == record["sizeBytes"]
    assert hashlib.sha256(blob).hexdigest() == record["sha256"]
    print(path, record["sha256"], "worktree/backup/outer/blob OK")
tracked = subprocess.check_output(["git", "ls-files", "-z"]).split(b"\0")
bad = [os.fsdecode(p) for p in tracked if p.endswith((b".profraw", b".profdata", b".lcov"))]
assert not bad, bad
print("whole-index tracked coverage extensions:", len(bad))
PY
~~~

### Deterministic ignore comparison

~~~sh
python3 - <<'PY'
import os, subprocess, tempfile
from pathlib import Path

root = Path.cwd()
head = "e0875e7449c4e5d6752dfd1b6508c408ebd42ed3"
base = subprocess.check_output(["git", "show", f"{head}:.gitignore"])
current = (root / ".gitignore").read_bytes()
extensions = ("profraw", "profdata", "lcov")
roots = [f"root-probe.{ext}" for ext in extensions]
arc = [
    ".arc-verify/probe/runs/id/summary.json",
    ".arc-verify/runs/id/receipt.json",
    ".arc-verify/probe/nested/profiles/payload",
    ".arc-verify/profiles/payload",
    *[f".arc-verify/probe/nested/file.{ext}" for ext in extensions],
    *[f".arc-verify/file.{ext}" for ext in extensions],
    ".arc-verify/probe/nested/raw.json",
    ".arc-verify/probe/nested/llvm-export.json",
    ".arc-verify/rust-coverage/baseline/latest.json",
    ".arc-verify/rust-coverage/tooling/acceptance/runs/probe/binding.json",
    ".arc-verify/rust-coverage/library/clippy.stdout.txt",
    ".arc-verify/rust-coverage/integration/repair-4/source-inventory.json",
]
visible = [
    ".arc-verify/rust-coverage/contract/accounting.json",
    ".arc-verify/rust-coverage/contract/source-inventory.json",
    ".arc-verify/rust-coverage/historical/accepted-head-summary.json",
    ".arc-verify/evidence-hygiene/commands.json",
    ".arc-verify/rust-coverage/historical/accepted-head-commands.json",
    "docs/example.profraw", "fixtures/example.profdata",
    "fixtures/example.lcov", "scripts/coverage/hygiene.test.ts",
]
assert (len(roots), len(arc), len(visible)) == (3, 16, 9)
env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
with tempfile.TemporaryDirectory(prefix="arc-qa-ignore-") as directory:
    repo = Path(directory)
    subprocess.run(["git", "init", "--quiet"], cwd=repo, env=env, check=True)
    total = 0
    for label, data, root_ignored in (("accepted HEAD", base, False), ("current", current, True)):
        (repo / ".gitignore").write_bytes(data)
        for path, expected in ([(p, root_ignored) for p in roots] +
                               [(p, True) for p in arc] +
                               [(p, False) for p in visible]):
            result = subprocess.run(
                ["git", "-c", "core.excludesFile=/dev/null", "check-ignore",
                 "--no-index", "--quiet", "--", path],
                cwd=repo, env=env, capture_output=True,
            )
            assert result.returncode in (0, 1), (label, path, result.stderr)
            assert (result.returncode == 0) == expected, (label, path, expected, result.returncode)
            total += 1
        print(label, "28 probes passed")
    print("total:", total, "expected results")
PY
~~~

### Historical receipt inspection

~~~sh
python3 - <<'PYBLOCK'
import json
from pathlib import Path
for name in ('.arc-verify/rust-coverage/baseline/first-measurement.json', '.arc-verify/rust-coverage/historical/accepted-head-commands.json', '.arc-verify/rust-coverage/historical/repair-4-commands.json'):
    data = json.loads(Path(name).read_text())
    print(name, 'classification=', data.get('classification', 'first measurement'), 'runId=', data.get('runId', data.get('provenance', {}).get('runId')), 'sourceHash=', data.get('sourceHash', data.get('provenance', {}).get('sourceHash')), 'pass=', data.get('pass', 'not recorded at top level'))
PYBLOCK
~~~

### Scoped tracked and untracked comparisons

~~~sh
git diff --name-status e0875e7449c4e5d6752dfd1b6508c408ebd42ed3 -- crates scripts/coverage scripts/harness .cargo Cargo.toml Cargo.lock rust-toolchain.toml mise.toml package.json bun.lock bun.lockb docs/rust-coverage.md
git ls-files --others --exclude-standard -- crates scripts/coverage scripts/harness .cargo Cargo.toml Cargo.lock rust-toolchain.toml mise.toml package.json bun.lock bun.lockb docs/rust-coverage.md
git diff --exit-code e0875e7449c4e5d6752dfd1b6508c408ebd42ed3 -- scripts/harness/validate.sh scripts/coverage/run.ts scripts/coverage/gate.ts scripts/coverage/config.ts scripts/coverage/source.ts scripts/coverage/hygiene.ts scripts/coverage/hygiene-preservation.test.ts scripts/coverage/evidence.test.ts package.json
~~~

### Contract artifact inventory

~~~sh
python3 - <<'PY'
from pathlib import Path
for name in ("profile-preservation.mission.test.ts", "gate-proof.mission.test.ts",
             "budget.mission.test.ts", "gate-results.json"):
    p = Path(".arc-verify/root-profile-hygiene") / name
    print(p, "present" if p.is_file() else "MISSING")
p = Path(".arc-verify/rust-coverage/baseline/latest.json")
print(p, "present" if p.is_file() else "MISSING")
PY
~~~

### Hidden-test discovery

~~~sh
python3 - <<'PY'
import subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory(prefix="arc-qa-bun-discovery-") as directory:
    repo = Path(directory)
    hidden = repo / ".arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts"
    hidden.parent.mkdir(parents=True)
    hidden.write_text('import {test, expect} from "bun:test";\ntest("probe", () => expect(true).toBe(true));\n')
    for path, expected in ((".arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts", 1),
                           ("./.arc-verify/root-profile-hygiene/profile-preservation.mission.test.ts", 0)):
        result = subprocess.run(["bun", "test", path], cwd=repo, capture_output=True, text=True)
        print(path, "exit", result.returncode)
        print(result.stderr.strip())
        assert result.returncode == expected
PY
~~~

### Evidence inventory

~~~sh
python3 - <<'PY'
import hashlib, json, os, stat
from pathlib import Path

root = Path.cwd()
baseline = json.loads((root / ".arc-verify/root-profile-hygiene/baseline.json").read_text())
inventory = baseline["preMissionInventory"]
before = {}
for scope in inventory["scopes"]:
    for entry in scope["files"]:
        previous = before.setdefault(entry["path"], entry)
        assert previous == entry, entry["path"]

current = {}
def add(path):
    info = path.lstat()
    rel = path.relative_to(root).as_posix()
    if not stat.S_ISREG(info.st_mode):
        raise RuntimeError(f"non-regular evidence path needs accounting: {rel}")
    current[rel] = (info.st_size, hashlib.sha256(path.read_bytes()).hexdigest())

def walk(path):
    if not path.exists():
        return
    for directory, dirs, files in os.walk(path, followlinks=False, onerror=lambda e: (_ for _ in ()).throw(e)):
        directory = Path(directory)
        for name in dirs:
            child = directory / name
            if child.is_symlink():
                raise RuntimeError(f"symlinked evidence directory needs accounting: {child}")
        for name in files:
            add(directory / name)

for scope in inventory["scopes"]:
    walk(root / scope["path"])
arc = root / ".arc-verify"
for directory, dirs, files in os.walk(arc, followlinks=False, onerror=lambda e: (_ for _ in ()).throw(e)):
    for name in files:
        if name.endswith((".profraw", ".profdata", ".lcov")):
            add(Path(directory) / name)
    for name in dirs:
        child = Path(directory) / name
        if name in ("runs", "profiles"):
            if child.is_symlink():
                raise RuntimeError(f"symlinked coverage directory needs accounting: {child}")
            walk(child)
old_root = set(inventory["rootCoverageExtensionPaths"])
for child in root.iterdir():
    if child.name.endswith((".profraw", ".profdata", ".lcov")) and child.name not in old_root:
        add(child)
changed = []
for path, (size, digest) in sorted(current.items()):
    old = before.get(path)
    if old is None or old.get("kind") != "file" or old.get("sizeBytes") != size or old.get("sha256") != digest:
        changed.append((path, size))
for path, size in changed:
    print(size, path)
total = sum(size for _, size in changed)
note = [(p, size) for p, size in changed if p.startswith(".arc-verify/root-profile-hygiene/")]
print("mission evidence:", len(changed), "files", total, "bytes")
print("note directory:", len(note), "files", sum(size for _, size in note), "bytes")
assert len(changed) < 30 and total < 5_000_000
PY
~~~

### Outer checkout

~~~sh
python3 - <<'PY'
import json, os, subprocess
from pathlib import Path

outer = "/Users/gracefullight/workspace/arc"
baseline = json.loads(Path(".arc-verify/root-profile-hygiene/baseline.json").read_text())
env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
for record in baseline["profiles"]:
    result = subprocess.run(["git", "-C", outer, "ls-files", "--error-unmatch", "--", record["path"]],
                            env=env, capture_output=True)
    assert result.returncode == 1, (record["path"], result.returncode, result.stderr)
    print(record["path"], "untracked in outer index")
for ext in ("profraw", "profdata", "lcov"):
    path = f"root-probe.{ext}"
    result = subprocess.run(["git", "-C", outer, "-c", "core.excludesFile=/dev/null",
                             "check-ignore", "--no-index", "--quiet", "--", path],
                            env=env, capture_output=True)
    assert result.returncode in (0, 1), result.stderr
    print(path, "ignored" if result.returncode == 0 else "visible")
PY
~~~

### Report structure check

~~~sh
python3 - <<'PY'
from pathlib import Path
p = Path(".arc-verify/root-profile-hygiene/qa.md")
s = p.read_text()
assert all(x in s for x in ("## Findings", "harness", "coverage", "NOT_ASSESSED"))
assert ("**8 files and " + "0" * 7 + " bytes**") not in s
assert ("**4 files and " + "0" * 5 + " bytes**") not in s
print("report:", p.stat().st_size, "bytes; required terms and final counts present")
PY
~~~

### Running the reproduction blocks

~~~sh
python3 - <<'PYBLOCK'
import re, subprocess
from pathlib import Path
text = Path('.arc-verify/root-profile-hygiene/qa.md').read_text()
for heading in ('Profile, history and index', 'Deterministic ignore comparison', 'Contract artifact inventory', 'Hidden-test discovery', 'Historical receipt inspection', 'Evidence inventory', 'Outer checkout'):
    match = re.search(r'^### ' + re.escape(heading) + r'\n\n~~~sh\n(.*?)\n~~~', text, re.M | re.S)
    assert match, heading
    result = subprocess.run(['bash'], input=match.group(1), text=True, capture_output=True)
    print('###', heading, 'exit', result.returncode)
    print(result.stdout, end='')
    print(result.stderr, end='')
    if result.returncode: raise SystemExit(result.returncode)
PYBLOCK
~~~

## Destination and limits

The baseline names the mission worktree as its destination. The separate outer checkout at /Users/gracefullight/workspace/arc has matching profile bytes but lacks the root ignore behavior; this is context, not an integration handoff. Earlier implementation and integration notes describe a superseded operator note; the later correction requiring the four contract artifacts applies.

Serena is configured, but no Serena tools were exposed in this session. This review used known-path reads, scoped Git metadata and existing tests; it cannot establish global repository cleanliness. The av.yaml access error remains. No browser, GUI, Playwright, compilation or external delivery was used.

