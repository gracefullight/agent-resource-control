# Test-module preservation

Task: task-2. Skill: oma-refactor; installed refactor-engineer definition.
Accepted revision: `a905653d557f4c5246a1f656042cb8d29e46c052`.
Baseline inputs are unchanged under `../baseline/`. `mapping.json` follows its
mapping-contract.json: 24 tests, 27 helpers, 137 assertion macro sites, all 28
original attributes and all 22 nonfunction declaration records are mapped.
`map_preservation.py` regenerates comparisons from accepted Git and current bytes.
The collector is a scoped lexer, not a complete Rust parser; independent QA must
read actual source and verify the mapping rather than trust stored flags.

| Responsibility | Destination | Tests |
| --- | --- | ---: |
| Resident lifecycle and CLI contracts | tests/resident/lifecycle.rs | 2 |
| Socket directory provisioning and rejection | tests/resident/provisioning.rs | 5 |
| Native identity, admission and recovery | tests/resident/identity.rs | 2 |
| Cross-process fixture coordination | tests/resident/coordination.rs | 1 |
| Exact subprocess probe | tests/resident.rs | 1 |
| IPC framing and decoding | src/tests/framing.rs | 9 |
| IPC dispatch | src/tests/dispatch.rs | 4 |

Resident paths are relative to crates/arc-daemon; IPC paths to crates/arc-ipc.
`resident.rs` remains one integration crate. Explicit path attributes point at
ordinary scenario modules under resident/. Each imports its parent helpers.
Root helper implementations remain single and private; no helper visibility
changes were required. IPC uses `#[cfg(test)] mod tests;`, tests/mod.rs and two
ordinary child module declarations. The shared hello helper stays in tests;
Cursor is imported by framing. IPC production code and visibility are unchanged.

Every original function body, literal, loop, call order, assertion and comment
matches accepted bytes after only removing the IPC enclosing module indentation.
Resident bodies are byte-identical. Function attributes are paired by text;
the macOS cfg stays on its original test. The IPC cfg(test) stays at lib.rs:355.
Nonfunction helper declarations are unchanged; IPC inline module wiring is mapped
to the external declaration and its children. New path attributes and module
imports are wiring only. Rustfmt sorted the root module declarations and removed
extra boundary blank lines. No test case, expected output or assertion changed.

The root `fixture_serialization_probe` and subprocess `--exact` selector both
remain exactly `fixture_serialization_probe`. Coordination still writes ready
before attempting support::serial(), asserts acquired absent under the parent's
process lock, releases only that file lock, waits for successful child exit and
asserts acquired present. The parent thread guard remains held. This unchanged
handoff cannot pass on a zero-test child exit because ready would never appear.
The focused resident check passed this scenario and all ten other resident tests.

`tests/support/mod.rs` remains byte-identical: one per-thread mutex and one
UID-specific process file lock, release_process_lock retaining the thread guard.
Scenario local declarations and reverse drop order remain unchanged. Fixture
construction follows the serialization guard; owned Daemon children drop before
fixture directories and locks. Gated startup still uses shell exec to preserve
owned child identity. Daemon Drop still checks, kills its own live child and
waits; no broad process cleanup or replacement fixture lock was introduced.

`protected-paths.json` verifies all 58 baseline frozen files byte-for-byte,
including daemon production, migration, manifests, lockfile, shared support and
harness scripts/rules. `preservation-result.json` verifies the first 354 IPC lines
against their baseline byte hash. Public framing and dispatch behavior remain
covered by the unchanged unit tests and existing integration/CLI assertions.

Readability verdict: named scenario modules let maintainers inspect one contract
without scanning unrelated scenarios. Keeping common fixtures at root avoids
visibility churn and duplicate cleanup/serialization implementations. Resident
root shrank from 901 to 421 lines; IPC root from 551 to 356. New source modules
range from 8 to 166 lines. No complexity improvement beyond file organization is
claimed. Coverage/mutation scores and flakiness rates remain unmeasured.

Deferred: unrelated Bun coverage configuration, commercial recommendations and
production changes. Independent QA and final mission evidence are owned by later
Actors. No baseline, mission assignment, harness rule or dependency was edited.
