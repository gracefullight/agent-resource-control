# Measured operational payoff

Status: completed for task-5. Owner: Marketing Director. Only this report is owned by this task; implementation, independent QA and Chief acceptance remain with their assigned Actors.

The independent benchmark shows a 91.04% reduction in Checks 1–4 elapsed time and a 60.82% reduction in full-harness elapsed time, comparing one original run with the mean of three candidate runs. This establishes a local engineering improvement. Financial value and customer effects remain unmeasured.

## Timing evidence

Source: [independent timing receipt](timings.json), SHA-256 `207a33dd76590dd610fb23e8a501b57007b2a384aeb729fda0b4915315b849c3`. The following values come from `/runs/0` through `/runs/3`, identified by each run's `label`, and their `durationsSeconds.checks1to4`, `processStartToCheck5`, `check5` and `total` fields. Calculations use the unrounded receipt values; displayed seconds are rounded to three decimals.

| Run | Checks 1–4 s | Start to Check 5 s | Check 5 s | Total s | Check 5 / total |
|---|---:|---:|---:|---:|---:|
| original | 163.421 | 164.162 | 83.569 | 247.733 | 33.73% |
| candidate-1 | 14.617 | 15.350 | 82.111 | 97.467 | 84.24% |
| candidate-2 | 14.657 | 15.173 | 81.838 | 97.017 | 84.35% |
| candidate-3 | 14.675 | 15.185 | 81.483 | 96.675 | 84.29% |
| candidate mean | 14.650 | 15.236 | 81.811 | 97.053 | 84.30% |

The mean-row share is mean Check 5 time divided by mean total time. Per-run shares divide that run's `check5` by `total`.

| Interval | Original minus candidate mean | Elapsed-time reduction | Original / candidate mean |
|---|---:|---:|---:|
| Checks 1–4 | 148.771 s | 91.04% | 11.16× |
| Full harness | 150.680 s | 60.82% | 2.55× |
| Check 5 | 1.758 s | 2.10% | 1.02× |

All three scan intervals are strictly below 20 seconds. Even the conservative process-start-to-Check-5 intervals, which include startup, are below 20 seconds. The scan speedup must not be presented as an 11.16× full-harness speedup. The small observed Check 5 difference is not evidence of a coverage optimization; this experiment does not isolate its cause. Check 5 now accounts for about 84.30% of total time and is the largest remaining elapsed-time component.

## Scope and measurement limits

The receipt binds `/acceptedHead` to `f85ae0430038cc3d7a6125f650872dbfce6b5529` and `/benchmarkCorpus` to 4,044 entries and 285,669,775 blob bytes. Its path/mode/content manifest hash is `5a4cc80b455f45901c8c70825972150937f2d6c22402f706562247a1b72733b3`. The candidate runs outside that accepted corpus. Each run records `scanInputsPreserved: true`, `corpusPreserved: true`, `gatePassed: true` and `exit: 0`. Real coverage ran without skip flags. The explained post-scan latest-pointer update is recorded separately in each run's `runtimePointer`.

`/clock`, `/observer` and `/boundaryNotes` describe an external monotonic observer: Checks 1–4 runs from the Check 1 label to the Check 5 label; Check 5 runs from its label to `Coverage thresholds met`; total includes startup, finalization and stream draining. These are observed phase intervals, including marker delivery and the boundary audit, rather than isolated inner-function timings.

Measurements were collected on one Darwin arm64 host with Bash 3.2, BSD grep and Bun 1.4.4, during the receipt's 2026-11-08 08:05–08:14 UTC window. `/schedule` records one original followed by three candidates, serially with real coverage. Fixed order, cache/toolchain warmup and host load may affect the comparison. The reviewer started no competing suites, but host-wide process inspection was denied; host idleness is unproven. One baseline and three candidates establish these observations, not a population estimate or a guarantee for other machines, corpora, staged runs or violation-recovery tasks.

[Independent QA](qa.md) reports 79 differential fixtures, four supplemental cases and all eleven required command gates passing, with preservation and six independent 80% coverage floors retained. Those checks support treating the timing improvement as useful preserved-behavior work. They do not measure contributor reactions. The inherited coverage-artifact hygiene failures were resolved through authorized untracking and passed fresh checks; their history is documented in QA's “Pre-existing hygiene failures” section. This task did not rerun the implementation suites or benchmark.

## Contributor and maintainer benefit

Judgment: a contributor waiting for a complete successful validation should experience a shorter interruption on a comparable prepared host: roughly 1 minute 37 seconds instead of 4 minutes 8 seconds in this benchmark. Maintainers should also spend less elapsed time waiting for the scan phase. Preserved output and reporting order should let them use the existing repair workflow without learning new commands. Actual repair speed, adoption, repeat use, abandonment, satisfaction and throughput were not measured.

Check 5's larger share directs attention to where future measurement could be useful; it does not justify changing coverage requirements or starting additional optimization in this task. The current mission's value is the verified reduction in scan latency with acceptance protections intact.

## Financial and commercial limits

Actual run frequency, the share of runs matching this benchmark, recoverable developer waiting time, labor rates, compute billing rates, implementation/QA/maintenance costs and parallel work during validation are unknown. Revenue, price, acquisition costs, conversion, retention and attributable customer outcomes are also unknown. No dollar savings, ROI, payback, profit or user reaction can be calculated from these timings alone. Unattended elapsed time must not be counted as salary savings.

The supplied Marketing Director advice identifies a later technical case study and a bounded validation-performance assessment service as hypotheses. Accepted timing and equivalence evidence could support such work if separately assigned. Buyer demand, willingness to pay and channel performance remain untested. No prospecting, outreach, publication, spending, pricing experiment or external market-engine action was performed.

## Task checks and handoff

Inspected existing staged changes and all 36 existing evidence files by directory inventory before writing; `value.md` did not exist. Known-path reads consumed the timing receipt and QA report. A Python read-only check verified the receipt's `/sourceReceipt` hash against `timing.json`, all 30 `/integratedSourceIdentity/entries` content hashes and modes against current files, and the current harness hash against `/candidateScriptSha256`. Arithmetic was recomputed from the receipt fields. These checks exited 0. Scoped `git status --short` and `git diff --stat` also exited 0; existing staged work was preserved.

Skill applicability: read the configured `oma-market` instructions. Its community-research workflow is not applicable to this local receipt assessment and external market-engine action is excluded by the assignment; no skill engine or external research was run. Shared execution and language policies were read. No subagents, builds, browser checks or mission-state operations were used.

Delivery adds only `.arc-verify/harness-speed/value.md`, bringing the evidence inventory to 37 files, below 150. No source, test, manifest, lockfile, index or dependency interface was changed. Receipt links and field names remain compatible with task-4's outputs. Remaining limits are the benchmark conditions and unknown economic/customer inputs above; final mission acceptance belongs to the Chief.
