# Task-6 timeout repair preservation audit

Accepted baseline: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`.

The operator note recorded at 2026-11-08T00:42:45.775Z expressly assigns these mission-owned constants to task-6/task-1 and requires at least 900000 ms. This attempt changes only `scripts/coverage/acceptance.ts` (115000 to 900000 ms) and `scripts/coverage/harness.test.ts` (120000 to 960000 ms). The extra test allowance lets the child return its receipt before the outer test deadline. The separate integration receipt runner allows 1020000 ms. No assertion, command, threshold, report requirement or protected file changed. No saved task, mission state, verification contract or Actor assignment was edited.

`source-change.json` compares all 114 current source/configuration inputs with the preceding integrated snapshot: precisely these two paths differ. `timeout.patch` records their complete unstaged diff at the start of this attempt. `preservation.json` recomputes preservation against the accepted baseline: all 77 existing Rust files retain their original ordered tokens, production tokens match, and both protected files match byte-for-byte. The original dependency checks and ownership boundaries remain in `../inputs-reconciliation.json`, `../dependency-artifact-check.json` and `../preservation-audit.md`; their historical execution dates are not presented as fresh checks.

The preserved harness SHA-256 is `93140173df8b8bd28b067aff68301134b0398b643240b0b1e6a13da4487ebe33`. The preserved migration SHA-256 is `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384`.

The helper still invokes the real command and checks stable source before/after, records nonzero exits and fails on them. The harness test still requires all six PASS verdicts, `Coverage thresholds met`, `All checks passed (0 violations)`, no skipping diagnostic and no violation diagnostic. It does not use fixtures, recursion, command replacements or an alternate Git scope. Both standalone and nested coverage invocations remain required. Earlier timeout failures and command receipts are retained.

Production behavior, Cargo dependencies, runtime interfaces, migration, documentation and other Actors' source tests remain unchanged. Independent oma-qa remains task-7; this is the implementer's audit.
