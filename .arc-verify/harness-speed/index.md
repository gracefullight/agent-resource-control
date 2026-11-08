# Harness speed evidence

Closeout: passed. Accepted HEAD: `f85ae0430038cc3d7a6125f650872dbfce6b5529`.

[Final acceptance](final.md) maps every unchanged success criterion to executable evidence. [Machine-readable acceptance](final.json) binds the final sources, commands and receipts. No product or mission-state changes were made during closeout.

| Requirement | Evidence |
|---|---|
| Equivalence | [79-case matrix](equivalence.json), [native invocation counts](invocations.json), [independent supplemental cases](qa-edgecases.json) |
| Checks 1–4 under 20 seconds | [Independent timing table](timings.json), [complete timing receipt](timing.json) |
| Check 5 and total recorded separately | Four real unskipped full runs below; host, tools, environment, corpus audits and streams are bound in the timing receipts |
| Coverage verification and preservation | [Complete suite inventory and five serial child suites](coverage-verification.json), [independent audit](qa-audit.json); four collecting/live suites are in command receipts |
| Cargo formatting, Clippy and workspace tests | [Exact command outcomes and retained-log hashes](qa-commands.json) |
| Full unskipped harness and current native evidence | [Independent source-bound full-harness and native receipt references](qa.json); Chief reran native-evidence verification successfully |
| Independent review | [oma-qa review](qa.md), [structured findings](qa.json): PASS, no unresolved findings |
| Final evidence budget | [Recursive budget receipt](budget.json): 40 files, below 150; command exit 0; 141 assertions |

| Run | Checks 1–4 s | Start to Check 5 s | Check 5 s | Total s | Exit |
|---|---:|---:|---:|---:|---:|
| original | 163.421057 | 164.162468 | 83.568649 | 247.733245 | 0 |
| candidate-1 | 14.616738 | 15.349835 | 82.110619 | 97.467000 | 0 |
| candidate-2 | 14.657121 | 15.172532 | 81.838186 | 97.016641 | 0 |
| candidate-3 | 14.675457 | 15.185321 | 81.483307 | 96.674869 | 0 |

The unchanged accepted corpus contains 4,044 entries and 285,669,775 blob bytes. Its path/mode/content manifest is `5a4cc80b455f45901c8c70825972150937f2d6c22402f706562247a1b72733b3`. Before-scan and Check-5-boundary audits match; the accepted collector's later latest-pointer write is retained and bound separately. Timings use external monotonic marker observation, including marker delivery and boundary-audit overhead. Scratch streams and native exports remain at receipt-linked runtime locations.

[Technical contract](technical.md), [machine contract](contract.json) and [CLI checklist](interaction.md) preserve the original scope. [Implementation history](implementation.md) retains earlier partial/failed results; its later [authorized recovery](attempt-6-handoff.md) and [untracking proof](attempt-6-recovery.json) resolve the inherited hygiene blocker. [Pre-existing hygiene failures](pre-existing.json): the accepted snapshot had 932 indexed coverage files, including 457 prohibited artifacts. Operator-authorized untracking removed 908 index entries, retained files on disk and left 24 whitelisted durable files. Fresh hygiene and clean-checkout checks passed. Earlier partial reports are historical evidence, not current acceptance.

[Measured payoff](value.md): mean scan time 163.421 → 14.650 s (91.04% lower); total 247.733 → 97.053 s (60.82% lower). Check 5 is now 84.30% of mean total. Financial and customer outcomes are unmeasured. Measurements cover one host, one original run and three serial candidate runs; host-wide process absence was not established. No remaining required implementation work was identified. Framework final verification remains its own step.

The inherited coverage-failure wording is unchanged; use the [Rust coverage guide](../../docs/rust-coverage.md) for that diagnostic. Skills: implementation used oma-dev-workflow/oma-refactor; independent review used oma-qa; Chief acceptance used oma-pm. No browser or visual check applies.
