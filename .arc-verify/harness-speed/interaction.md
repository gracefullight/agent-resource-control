# Harness CLI compatibility: task-2

Status: completed. Owner: Design Director. Handoff: Chief Director and task-3 Harness Engineer. Only this report is owned and changed by task-2.

## Source identity and evidence scope

- Accepted source: commit `f85ae0430038cc3d7a6125f650872dbfce6b5529`, tree `b7e6c535d7e0b963e6af9bc15d7645ad0f7cded2`.
- Actor HEAD inspected: snapshot commit `c604fc32a247244d4291eaf45d03a8dfc5da342e`, with the same tree. Snapshot ref: `refs/agent-valley/task-evidence/c79eadbc796a4c83-78686d2704d4d690-1/snapshot`. Worker ref supplied: `refs/agent-valley/task-evidence/c79eadbc796a4c83-78686d2704d4d690-1/worker`; its later value is not asserted here.
- Accepted `scripts/harness/validate.sh`: Git blob `d90f4626868bd1c6ccaed84200a8f23ac83eef0e`; SHA-256 `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. The Actor-local file matched this blob at inspection. All source line numbers below refer to the accepted commit, not a future optimized file.
- Initial scoped status/diff for this directory and the harness were empty; `interaction.md` did not exist. This establishes neither global cleanliness nor integration into the Chief's checkout. This report is a new local deliverable, absent from the accepted source tree.
- Source inspection used `git show <accepted>:<known-path> | nl -ba` for the harness and `docs/rust-coverage.md`. Tool discovery returned no Serena tools; no native repository code search was performed.

Route: `oma-design`, local documentation contract through `resources/external-design-skills.md`. Used the execution protocol and applicable scope/provenance/handoff checklist. No external design supplier or visual judgment was needed. No product, documentation, prompt, default, retention or commercial change is proposed by this handoff.

## Contributor journeys: hypotheses

These personas and comprehension needs are inferred from the CLI and coverage guide. They are not measured user observations, simulations of people, field tests or retention results.

| Inferred persona | Shortest existing journey to preserve | Hypothesized need |
| --- | --- | --- |
| First-time contributor | Prepare documented prerequisites when necessary → run `./scripts/harness/validate.sh` → read result. | Recognize current check and distinguish setup failure from a code violation. |
| Repeat contributor | Run validator → follow named location and advice → repair → rerun. `--staged` remains the existing explicit selection. | Fast feedback without changed messages, extra prompts or hidden scope changes. |
| Maintainer | Run complete validator → inspect total and coverage evidence → make acceptance decision. | Distinguish empty input, a skipped gate, actual coverage success and overall success. |

Prepared users keep one validation command. The separate tests and timing receipts required for mission acceptance do not become extra everyday CLI steps.

## Observable state and stream checklist

All rows are source observations unless marked as fixture evidence. Preserve exact strings, indentation, punctuation, line endings, ANSI sequences and exit status; the descriptions below do not replace byte comparison against the accepted script.

| State / accepted lines | Required behavior |
| --- | --- |
| File selection, 47–55 | Only first argument equal to `--staged` selects `git diff --cached --name-only --diff-filter=ACM`; otherwise use `git ls-files`. Git stderr is suppressed and failures are swallowed. No new argument rejection: an unknown nonempty first argument still scans all tracked files and appears as the mode label. |
| Empty selection, 57–60 | stdout: green `[ok]` with `No files to validate.`; exit 0 immediately. No mode, count, check labels, coverage invocation, summary or extra final blank line. An empty staged selection has this same early state. It does not prove coverage ran. |
| Start, 62–65 | stdout: blue `[validate] Mode: all-tracked` by default, or the first argument verbatim; then `[validate] Files to check: N`. Count entries in the selected list before existence checks/exclusions; a missing tracked file still contributes to N. |
| Check 1, 111–163 | stdout label `Check 1/5: Hardcoded secrets`; eligible files in selected-list order, then nine secret expressions in their declared order, then matching lines. For each matched file/expression: stderr header containing the unchanged pattern, Fix, See, then indented `file:line:text` entries. Increment once per matched file/expression. stdout success: `No hardcoded secrets found` only when cumulative count is zero. |
| Check 2, 169–230 | stdout label `Check 2/5: Dangerous shell commands`; per file: absolute-root removal, main/master force push, bare force push. stderr contains each rule's unchanged header, advice and matching lines. Increment once per matched file/rule, not per occurrence or line. stdout success: `No dangerous shell commands found` if this check added no violations, even after Check 1 failed. |
| Check 3, 237–332 | stdout label `Check 3/5: Architecture layer violations (domain must be pure)`; per domain file: non-domain layer, legacy infrastructure, Python infrastructure, Go infrastructure, Python ORM/SDK. Each matched rule adds one and emits its own unchanged advice and line entries. stdout success: `No architecture layer violations found` if this check added none. |
| Check 4, 337–389 | stdout label `Check 4/5: File length cap (CONSTRAINTS.md §5: max 500 lines per file)`; eligible files in selected-list order. Use `wc -l` newline count, strict `> ceiling`; accepted grandfather list is empty. Each overlong file adds one and emits header plus Fix on stderr. stdout success: `No oversized files found` if this check added none. Keep the dormant grandfather branch and wording. |
| Check 5 entry, 398–411 | stdout label `Check 5/5: Test coverage thresholds`, even after earlier violations. Exact `SKIP_COVERAGE=1` produces the existing yellow warning on stderr, invokes no coverage and prints no `Coverage thresholds met`. Otherwise retain caller WORKSPACE_ROOT or export the scratch directory created by mktemp. |
| Missing Bun, 413–416 | stderr: `bun not found on PATH — cannot run 'bun run test:coverage'.` and unchanged installation/local-skip Fix. Add one violation; proceed to final result. No success acknowledgment for this check. |
| Coverage invocation, 418–430 | `bun run test:coverage` inherits stdout/stderr separately. Preserve each child stream on that same stream. A nonzero child exit adds one violation, regardless of its exit number, with all five inherited failure/advice lines on stderr. A successful child prints stdout `Coverage thresholds met`, even when earlier checks failed. |
| Final result, 436–443 | Always append a blank line to stdout after nonempty selection. Zero cumulative violations: stdout green `[ok] All checks passed (0 violations).`, exit 0. Otherwise stderr red `[VIOLATION] Total violations: N` then `Fix all violations above and re-run: ./scripts/harness/validate.sh`, exit 1. No early stop for detected violations. |

Stream details (40–45): `info` and `ok` write stdout; `warn` and `violation` write stderr. ANSI color is unconditional, including redirected/non-TTY output: prefix escape bytes are `ESC[0;34m`, `ESC[0;32m`, `ESC[0;33m`, `ESC[0;31m` respectively, each reset by `ESC[0m`. After reset, spacing is one space for `[validate]`/`[VIOLATION]`, seven for `[ok]`, five for `[warn]`; advice and match strings carry their own leading two spaces. Color-independent label text remains present.

Order means check order → selected-file order → rule declaration order → grep line order. A combined pattern scan must not deduplicate a line matching multiple rules or sort results globally by line number. Keep stdout and stderr as separate byte comparisons: a merged terminal rendering does not establish a deterministic relative display order across buffered streams. Child stdout/stderr ordering is assessed within each stream too.

## Eligibility and inherited wording

Patterns remain those in accepted lines 129–144, 188–218 and 259–318; do not translate them into a different regex engine. The unused `check_pattern` helper (67–104) is not the active scan path; its PCRE fallback is not the semantics of Checks 1–3, which use grep ERE. Preserve separate force-push trigger/report expressions (202–204).

- All active loops skip non-files using `-f`; staging selects names but scans current working files, not staged blob contents.
- Check 1 excludes exact root paths `.env.example`, the harness, `package-lock.json`, `yarn.lock`, `go.sum`, `poetry.lock`, plus suffixes png/jpg/jpeg/gif/ico/woff/woff2/ttf/eot. Do not generalize this to all lockfiles or all binary types. Test-file exclusions from Check 4 do not exempt secret detection.
- Check 2 scans sh/bash/zsh and `.github/workflows/*.yml`/`*.yaml`, excluding the harness. Do not generalize to all YAML.
- Check 3 scans `*/domain/*` and `domain/*` with no extension filter. Only its first rule drops lines matching a `from './` or `from "./` prefix; legacy and language rules remain independently active.
- Check 4 uses only ts/tsx/js/jsx/mjs/cjs/py/go/rs. Preserve exact shell globs at 369–372 for dependency/build/test/declaration files, managed agent directories and canvas paths; their anchoring is part of compatibility. Do not add Markdown, shell or other extensions.

The Vitest references in the Check 5 comment and failure advice (`see vitest output above`, `vitest.config.ts`) are frozen wording, not evidence that this project currently uses Vitest coverage. Evidence readers should use [Rust coverage](../../docs/rust-coverage.md), especially its Repeat use, Diagnose a failed run and inherited-guidance note (accepted lines 43–72). That guide identifies the actual Rust diagnostics and six independent exact 80% crate floors. Leave CLI wording unchanged; a fixture child cannot verify those real floors.

## Differential cases for task-3

Use deterministic isolated Git roots and the accepted script as oracle. Generate violating strings inside temporary fixtures, not retained tracked payloads. Hold host tools, locale, Git settings, child outputs and environment equal for each original/optimized pair. Compare complete stdout bytes, complete stderr bytes and exit; do not strip ANSI, trim whitespace, normalize paths in output or replace equality with substring checks. Pin WORKSPACE_ROOT to avoid incidental scratch names. Controlled coverage children are simulations of child success/failure, not real Rust coverage acceptance.

| Case | Concrete setup and assertion |
| --- | --- |
| Clean/default/staged/unknown mode | Clean tracked file with child success; exact five labels, mode/count, acknowledgments, final blank and exit 0. Separately run `--staged`, no staged changes, and unknown first argument; preserve the actual accepted handling. |
| Nine secret expressions | Generate a qualifying value for each accepted expression, then subthreshold/placeholder controls. Treat the accepted host grep behavior as oracle for escape quirks. Add two occurrences on one line and a match on a second line: output two numbered lines, one count for that file/expression. |
| File-first vs pattern-first | Two eligible files `a.txt`, `z.txt`, each containing matches for two different secret expressions; place the later expression on an earlier line. Expected blocks are a/first-rule, a/second-rule, z/first-rule, z/second-rule, with line order inside each block. |
| Multiple checks and cumulative count | `domain/a.sh` with a generated secret, absolute-root removal, main force push and multiple matching architecture statements. It participates in Checks 1–3 but not the code-length extension scope. Sum file/rule increments; later clean checks still acknowledge success; Check 5 still executes before overall exit 1. |
| All shell rules | Include absolute-root removal, main/master long and short force forms, and bare force form; repeat lines and place rules out of line order. A second eligible file proves file/rule/line replay. Put equivalent text in ordinary YAML as an exclusion control. |
| Force-push trigger/report split | File containing only `git push --force origin/main` does not trigger main/master detection. Add `git push --force origin main`: the single rule now reports both lines and increments once. Include `git push -f origin master` to exercise the second trigger. Do not replace trigger tests with the broader reporting expression. |
| Architecture overlap | In `domain/a.ts`, `import x from '../infrastructure/x'` matches non-domain, legacy and Python infrastructure rules: total 3. The double-quoted equivalent also matches the Go-shaped expression: total 4, despite the ts extension. These totals were checked against the accepted oracle. Expect repeated location lines in separate rule blocks and count each matched rule. |
| Within-domain filtering | `import x from './ports/tracker'` alone yields no first-rule report. `import x from './infrastructure/x'` remains excluded from the first rule but matches legacy and Python infrastructure; a double-quoted form also matches Go-shaped syntax. Add an escaping `../tracker/x` import and verify only eligible first-rule lines survive its filter. |
| Remaining architecture families | Dedicated domain fixtures for Python infrastructure, quoted infrastructure import, Python ORM and SDK imports; repeat lines, include a non-domain path control, and assert all overlaps from the oracle. Each expression runs by domain path, not language extension. |
| Length boundary | 500 newline characters pass; 501 fail once with exact count/advice. Add an unterminated final line: retain wc newline semantics. Test two eligible files and every exclusion group; no new grandfather entries. |
| Exclusions are per check | Generated secret in `.env.example` is excluded; same content in a nested `.env.example` is not automatically excluded. Secret in a `*.test.ts` remains detectable even when its length is excluded. Exercise all accepted suffix/path exclusion groups independently. |
| Missing and staged files | Add a tracked name then remove its working file: count still includes it, scanning skips it. Staged deleted names are excluded by ACM. Stage clean content then change only working content to violate; reverse the setup too. Verify staged mode reads current working content. |
| Empty input | Empty Git root and nonempty repo with empty staged selection each emit exactly the one green no-files line, empty stderr, exit 0, no child invocation. Distinguish selected-but-all-missing files, which still run checks and coverage. |
| Coverage streams and exits | Controlled child writes distinct stdout/stderr markers and exits 0, then a nonzero value such as 7. Success retains child streams and coverage acknowledgment; failure retains child streams, adds exactly one, preserves frozen Vitest advice and returns harness exit 1. Separately simulate unavailable Bun with a controlled PATH. |
| Existing skip behavior | Explicit skip produces exact warning, no child invocation and no coverage acknowledgment; earlier violations still cause exit 1. This equivalence fixture is not full mission acceptance without skip flags. |
| Repair and rerun | Fail on two rules, repair one, rerun: total decreases by one and remaining advice stays exact. Repair both: all checks pass with child success. Verify counts reset each process. Test staged-name selection plus working-file repair without restaging. |
| Batch edge handling | Multiple filenames, a space/colon in a name, matching text with colons/backslashes, no-match batches and empty eligible lists must preserve baseline formatting and silence. Test a binary-content file outside listed suffix exclusions against the host oracle; do not invent text-only behavior. |

## Executed checks and limits

Commands executed in this Actor checkout:

1. `git rev-parse HEAD 'f85ae0430038cc3d7a6125f650872dbfce6b5529^{tree}'`, then `git rev-parse 'HEAD^{tree}'`: exit 0; identities recorded above.
2. Scoped `git status --short --untracked-files=normal -- .arc-verify/harness-speed scripts/harness/validate.sh` and `git diff -- .arc-verify/harness-speed/interaction.md scripts/harness/validate.sh`: exit 0; initially empty. `git hash-object scripts/harness/validate.sh` and accepted `git rev-parse <commit>:scripts/harness/validate.sh`: exit 0, equal blob IDs.
3. Known-path `git show` reads with numbered lines: exit 0. Local oma-design route, execution policy, result contract, language rule and applicable checklist reads: exit 0.
4. `python3 - <<'PY'` isolated CLI probe: exit 0, six accepted-oracle executions in a temporary Git root. Extracted original bytes with `git show`; an untracked deterministic shell stub named bun emitted distinct child stdout/stderr markers and obeyed `FIXTURE_COVERAGE_EXIT`. Removed inherited SKIP_COVERAGE and pinned WORKSPACE_ROOT. Assertions passed for exact empty output; clean staged mode/count/ANSI/child streams; coverage failure child exit 7 mapping to total 1 and harness exit 1; report-only force-push exit 0; adding a trigger reporting both lines with total 1 and continued coverage; working-file repair returning exit 0. The last repair invocation used default mode; staged repair is separately checked below.
5. A second `python3 - <<'PY'` isolated probe: exit 0, four accepted-oracle executions with a success-only stub child. `--staged` failed with total 1 for a staged force-push file, then passed after only the working file was repaired, without restaging. A staged `domain/a.ts` single-quoted infrastructure import failed with total 3; changing its working content to double quotes failed with total 4. This verifies overlapping rules, not a language-aware parser.
6. Final report checks: `python3` checked the local Rust guide link, required report sections/source identities and trailing whitespace; exit 0. `git diff --check -- .arc-verify/harness-speed/interaction.md scripts/harness/validate.sh` exited 0. Final scoped status showed the new evidence directory; a direct file inventory confirmed its only file was this report (1 file, below 150). The local harness blob still matched the accepted blob.
7. Access limitation: the earlier unscoped `git diff --check` / final-status command block emitted `av.yaml: Operation not permitted`. The block's final exit was 0, so that does not establish the individual unscoped check's success or global cleanliness. No access workaround was attempted. The subsequent scoped diff check and direct report whitespace check above passed independently.

Measured observations here are source identities and these isolated CLI results. No original-versus-optimized equivalence suite, timing benchmark, real coverage collection, Cargo checks or full product harness acceptance was run by task-2. Those belong to the implementation/reviewer tasks. No browser, screen check, installation, build or user study was run. No speed, comprehension, retention or profit outcome is established.

Acceptance: source state/stream/order mapping supplied; edge-case handoff supplied; inferred user needs labeled; interface and product files unchanged. Remaining risk: task-3 must prove full byte equivalence on the optimized implementation and independent QA must verify mission-wide requirements in the integrated worktree.
