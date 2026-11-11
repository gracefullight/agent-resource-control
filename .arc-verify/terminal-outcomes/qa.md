# Independent task-5 repair review

Outcome: PASS

## Review Result: PASS

Independent reviewer: `independent_qa`, task-5 attempt-9, using `oma-qa`. The bounded review is complete. The six test-only repairs preserve all original assertions, current required local gates pass, and this reviewer independently executed the four mission evidence checks. Final mission acceptance remains false while the external framework workspace rerun is pending.

## Repair and findings

The two named descriptor failures were reproduced with closed fd0 in the direct baseline. The PATH `python3` mise launcher reopened fd0 before Python. Resolving `os.fsencode(sys.executable)` while standard streams are open makes the direct and wrapped commands use the same native interpreter. Descriptor masks, inherited fd9 bytes, child exit 17, stdout/stderr checks, one-launch markers, fault-injection checks and direct/wrapped equality remain intact.

The subsequent local workspace failure was also reproduced: the Bun launcher added its PID to `__MISE_SHIM_FALLBACK_CHAIN`, so otherwise equal direct/wrapped environments differed. The fixture now reuses canonical `process.execPath` and prepends its native directory to the shared caller PATH. Bare `bun` argv, full environment equality, lineage cases and permit/state assertions remain intact.

No current CRITICAL, HIGH, MEDIUM or LOW defect was verified. No remediation code is required for this review. Both resolved fixture failures and their RED/probe/GREEN receipts are preserved under `target/terminal-outcomes/task-5-attempt-9/`.

## Verified evidence

| Check | Execution and result |
|---|---|
| Native compilation, formatting, warning-denying Clippy | Root repair owner; all exit 0 |
| Full workspace | 591 passed; zero failed, ignored or filtered; all 59 managed-exec tests pass |
| Terminal outcomes / status compatibility | 10 / 7 passed; zero failures or ignored tests |
| Full harness | Five checks, zero violations, coverage executed |
| Active Bun suite | 43 passed, zero failures/skips across seven files; one full-suite invocation |
| Independent mission evidence checks | This reviewer executed four tests; four passed, zero failed, 439 expectations |

The independent command was `bun test scripts/coverage/terminal-outcomes.test.ts`, with DEVNULL stdin, no TTY, a new session and a 7,200,000-ms allowance. Its process group was absent after completion. Native compilation and suite execution above retain the root repair owner's attribution.

All 249 collector inputs and all 18 accepted implementation hashes match. Current source identity is `17fe50cbd7d5f9d332e16616723cc68037b5c111f772a73a0c4abed57afa01a4`. All 117 phase-one and all 120 final direct gate references match their hashes/sizes. The required-software evidence projection is unchanged after the independent verdict.

The independent coverage check recomputed all seven exact 80-percent production-line floors: 247/256, 706/711, 1811/1892, 569/576, 2244/2457, 1895/2255 and 111/111. The lowest result is arc-cli at 84.04%. Twenty accepted protocol/migration/manifest bindings remain byte-identical. The archived 69 files, including 28 historical test files, retain current/original hashes and stay outside active Bun discovery.

The binding operator inventory resolves container checks: configurationVerified=true, allTargetsHealthy=true, targetCount=0, targets=[].

## Remaining verification and historical provenance

The external framework `cargo test --workspace` rerun remains required for final mission acceptance. The fresh local PASS is recorded separately. Same-database old-daemon downgrade and external exhaustive-match compatibility remain unverified. Earlier accepted dependency provenance, retired V1/historical validators, missing predecessor logs, timing overrun and resource-introspection limits remain in the immutable prior audit with original attribution.

Attempt-9 sources, native results and fresh coverage supersede attempt-8 unchanged-source bindings. Previous QA/audit are preserved byte-for-byte as `target/terminal-outcomes/task-5-attempt-9/before-qa.md` and `before-audit.json`.

## Frozen bindings and final accounting

`gates.json`: SHA-256 `3a9788af04441d1aafa1ae6f9cebbd29d7baa1cef072893b032d843047890509`.

`gates.md`: SHA-256 `eccb56484a1987fedfb6fe6c75141ea51eb6e598eb5b008492b9652c102acc5c`.

The independent verdict is `target/terminal-outcomes/task-5-attempt-9-independent-qa/independent-verdict.json`. `phase2-authentication.json` in the same directory records the final binding audit.

Final evidence union: **7 regular files totaling 288934 bytes**, strictly below 30 files and 5,000,000 bytes. Audit self-size is included. Final report hashes and the exact directory inventory are retained separately as `final-evidence.json` under the review target directory.

Only reviewer-owned `qa.md`, `audit.json` and new immutable raw review artifacts were written. Product/tests, gate reports, manifests/lockfiles, archives, index, instructions and mission state were preserved.
