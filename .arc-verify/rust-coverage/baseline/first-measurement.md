# First valid production-source measurement

Run: `2026-11-07T23-40-54-943Z-14242ab5-17e2-4b4d-a853-e0ce9047404d`. Host: `aarch64-apple-darwin`.
Source/configuration SHA-256: `23db85747b5e61c3637d93684e84aba95fd6c8bd5d924135541bc3c875859399`. Accepted HEAD: `fbe76a5f15101ad415f355ef06f1deeb1f97d568`.

The direct `bun run test:coverage` exited 0. Each crate passed the new 80% engineering target; this is the first measured result, not a previously established baseline or SRS requirement.

| Crate | Covered/total lines | Coverage |
| --- | ---: | ---: |
| arc-core | 64/64 | 100.00% |
| arc-detector | 572/578 | 98.96% |
| arc-storage | 214/227 | 94.27% |
| arc-ipc | 178/215 | 82.79% |
| arc-daemon | 1502/1613 | 93.12% |
| arc-cli | 200/213 | 93.90% |

Native exports, per-file line maps and uncovered lines: `.arc-verify/rust-coverage/baseline/runs/2026-11-07T23-40-54-943Z-14242ab5-17e2-4b4d-a853-e0ce9047404d/`. Command receipts cover the real instrumented workspace all-targets tests and explicit CLI/daemon probes. All 832 profiles, including the two recorded child PIDs, entered the native merge input list. The daemon probe deliberately exits 1 at invalid socket-path startup; that expected rejection is separately recorded, not a successful resident startup claim. Existing integration tests exercise resident daemon and real CLI paths.

Standalone malformed-LCOV validation remains strict. Real reports additionally verify every DA line/count against fresh native LLVM JSON segments and each LF/LH against its native function-group summary. Both raw files remain unchanged.

Only active macOS compiler cfg is measured. Linux peer credential code and the non-macOS/Windows runtime branches remain unmeasured. Stable Rust doctest coverage is outside the collected unit/integration/executable run.

- desktop: incomplete
- native Windows: incomplete
- remaining Phase 0: incomplete

This measurement does not establish completed harness acceptance or independent QA. Those obligations retain their own receipts and review.
