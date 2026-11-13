# Governor client handoff — task-7

Status: completed for the assigned client implementation and focused checks. **Governor slice only**; task-8 receives implementation and consumer-test ownership for integrated production-daemon acceptance. No full-mission completion is claimed.

## Provenance and boundaries

- Accepted committed baseline: `08e7e526cce7e03ca9264afc926ed2e878dfde6d`, tree `d3cbd610653d0b78adeb2ffc26a9cab1790e877f`.
- Inspected Actor-local snapshot and current HEAD: `d87b8673a4afe0ab9511de90865a803ff7b548f2`, tree `d0dec214f5718fd5235a2de2db17346e46500b9a`. This input includes task-5 contracts absent from the accepted committed baseline. All changes below are staged local task-7 edits over that snapshot, not changes already committed to the accepted baseline.
- Actor branch: `av-task/b29b06c236fcb043-014e4ceaa5a9e511-1`; attempt 1. Source worktree and every command cwd: `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/6dd4dd59-bec1-4e67-a1ba-65f73e0660ce/workspace/.agent-valley/parallel/workspaces/b29b06c236fcb043-014e4ceaa5a9e511-1`.
- Consumed `.arc-verify/governor/contract.md`, `docs/requirements/governor-experience.md`, the Rust Governor DTOs/validators and Rust-derived TS/fixtures. Used the installed backend-engineer profile, **oma-backend** primary, **oma-db** for durable CAS/receipt semantics, and **oma-frontend** for the existing React/Base UI boundary. No subagents or new AV/OMA run were launched; no injected OMA claim identity was available.
- Serena tools are unavailable. Used scoped reads of named/imported files and installed dependency types; no recursive source-search fallback. `av.yaml` remains access-denied and uninspected.

## Delivered behavior and interface compatibility

1. `arcctl daemon pause`, `arcctl daemon resume`, and `arcctl daemon governor --json` use owner-authenticated `arc_ipc::client::Session`. Writes are explicit desired state with the frozen key, CAS base and revision-bound mutation ID. Optional paired `--expected-revision`/`--mutation-id` support stable deterministic retries; readback supports `--mutation-id`. Lost/invalid replies retain and print identity, perform receipt-aware readback once and return failure when the outcome remains unconfirmed. No write is resent or rebased. Existing status/exec semantics remain covered.
2. Supported current clients negotiate `governor.v1`/`governor.execution.v1`. Enqueue/poll use the frozen Traffic wrapper; an unreserved eligibility result still requires a fenced `governor.launch`. Queued and reserved-to-unreserved responses preserve the same job/witness. Resume-before-claim `RETRY` polls admission and uses the returned real generation/context. Unreserved launches export neither an invented generation nor a lineage capability. Attach/complete carry the release ID. The original single `command.spawn()` site, pre-exec socket/deadline fence, cancellation, native signal/exit behavior, witness reconciliation and wait4 Meter reporting remain.
3. The actual production Tauri registry exposes `governor_get_switch({query})` and `governor_set_switch({request})`, returning `Received` of the same Rust DTOs. Both share the existing bridge single-flight permit. Sent writes use a non-replaying Session; DB_BUSY, dropped replies, malformed post-dispatch results and explicit uncertainty are Uncertain. Only proven pre-dispatch failures become Rejected. A dropped async waiter does not drop the blocking task permit; clock/shutdown failure after a possible commit remains uncertain.
4. One Base UI dashboard master switch is named **Agent Resource Control**. It renders a loading/unavailable placeholder before any boolean is known. Checked follows confirmed evidence; applying text shows intent. Pending/read-only state preserves focus, associates help and safe errors, and coalesces activation. The mutation owner survives About/hide/reopen and polling cancellation. Identity-aware reads reject invalid/equal-conflicting evidence and ignore older revisions. A missing receipt at the CAS base cannot settle a delayed write. A later revision can supersede an evicted receipt while explicitly retaining an unknown historical outcome. Replayed receipts never masquerade as current state before readback.
5. Paused copy explains that supported waiting jobs can proceed without resubmission while running work and measurement continue. Critical Meter readings remain visible. Memory-policy panels qualify historical holds and do not present them as active holds while paused or Governor state is unavailable. Connection/refresh state never establishes Governor state.

## Acceptance evidence

All eight final focused commands exited 0 on the source input set below. The wrapper allowed **7200 seconds per command**, ran sequentially, and did not change any operator verification command. Raw logs remain local under `target/governor/task7/final/`; the counts, cases, identities and hashes needed for the handoff are retained here.

| Check | Exact command | Outcome | Raw log SHA-256 |
| --- | --- | --- | --- |
| cli | `cargo test -p arc-cli` | exit 0; 101 passed; 15.505 s | `49eec0fc21dcaaea51e8d6997a7e62c80e7dc6fecd1c0198d943fbbf449c344b` |
| bridge | `cargo test -p arc-desktop --lib --test registered_commands` | exit 0; 26 passed; 8.403 s | `84fb0742d23459ac67bf57b93dd6cac58d6ae936e96fbf19e6e59c40c2068ab2` |
| rust-contract | `cargo test -p arc-ipc --lib governor::typescript::generated_types_are_current_without_writing -- --exact` | exit 0; 1 passed; 6.024 s | `950e48b1f1c5a8b6cfbb8508c7b566b43d0d67708fa17c02e1677de648bbbbac` |
| clippy | `cargo clippy -p arc-cli -p arc-desktop --all-targets -- -D warnings` | exit 0; passed; 2.749 s | `0027d19432ce2bed52273f0f271123dbd1ac9701bcd6de484b6d80a7c26d12e6` |
| fmt | `cargo fmt -p arc-cli -p arc-desktop -- --check` | exit 0; passed; 0.31 s | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| bun | `bun test apps/desktop/tests/governor.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts` | exit 0; 133 passed; 15.631 s | `eefce4bd6ec72e734fa32feca579b8e54e0d205709022412f48a34272cfc1056` |
| typecheck | `bun run desktop:typecheck` | exit 0; passed; 1.951 s | `7c3cb6045e4508ad4ca264a0bdbb3207275c78410b843909c0ea071a97268c0f` |
| desktop-check | `bun run desktop:check` | exit 0; passed; 0.203 s | `0cf747425e368e90b642d8f90235b187e4ca771c2d9e7645a59f7661924abd22` |

- CLI: 101 tests total. The six Governor test functions include nine process scenarios: new, queued, reserved conversion, resumed reserved admission, lost ACK, lost release, cancellation, lineage failure and a pressure hold converted after one reserved checkpoint. Each of six successful scenarios asserts exactly one real child marker and native exit 17; the three nonlaunching scenarios assert zero target markers. ACKs are validated against the actual request/release. Native CPU reporting is observed; the socket peer deliberately rejects persistence, so this is not a persisted-Meter or permit-accounting claim.
- Bridge: 26 tests (5 library, 21 registered command tests). New cases exercise actual Tauri registration, DTO validation before socket I/O, typed conflicts, lost/DB_BUSY/malformed/explicit-uncertain acknowledgments, dropped-waiter single-flight ownership and post-commit clock failure. Same-user socket fixtures are transport evidence, not production-daemon SQLite durability proof.
- Bun: 133 tests across the eight explicit files, including 18 Governor/contract cases. The latter cover G01–G13, Space/Enter, focus/ARIA help/error behavior, applying, confirmed transport/typed rejection, cross-client conflicts, missing receipts, reconnect, revision ordering/equal conflicts, replayed receipts, navigation/hide and unavailable state independent of healthy daemon status. A queued-memory-hold fixture verifies Paused suppresses active-hold/recovery claims while Critical stays visible.
- Rust exporter: the actual nonmutating ts-rs generation agreement test passed. Bun also compiles Rust-produced literals against the generated declarations with exact optional-property types and negative structural expectations, then tests runtime tags, identity correlation, object fields, omitted/null optionals, integer bounds and receipt ordering. Generated bytes were not rewritten.

### Regression repair evidence

`TDD_EVIDENCE`: `cargo test -p arc-cli --test governor lineage_failure_retracts_release_before_cancellation_needs_the_writer -- --exact` failed before the cleanup repair (exit 101, missing same-connection UNSPAWNED ACK, peer `Frame(Closed)`), then passed after the repair (exit 0; one passed). The test now lives at `execution::lineage_failure_retracts_release_before_cancellation_needs_the_writer` after splitting the test file. Cleanup now acknowledges/drops the known-unspawned launch fence before queueing cancellation to the writer.

Other repaired attempts: initial `desktop:typecheck` exited 127 because this isolated worktree had no installed dependencies; `bun install --frozen-lockfile` restored 85 packages without manifest/lock changes. A test-only ES2023 `findLast` exceeded the configured target and was replaced by ES2022-compatible code. Existing read-only lifecycle allowlists gained the additive Governor read, and the reopen race now waits until its deliberately pending health call actually starts. The new CLI fixture initially used an unsupported absolute executable spelling; it now resolves literal `cargo` through its isolated PATH. Ambiguous test imports and a moved fixture socket were fixed. Formatting and a Clippy argument-count finding were repaired without suppressions. A Meter assertion was scoped to the actual combined status text. None of these failures are reported as passing evidence.

## Preserved inputs and handoff limits

Byte comparison confirms all eight protected shared inputs match the Actor snapshot: root `Cargo.toml`, `Cargo.lock`, `package.json`, `bun.lock`; desktop `package.json` and `src-tauri/Cargo.toml`; `governor.generated.ts` and `governor-wire.json`. Original migration files 0001–0005 match accepted HEAD byte-for-byte. No daemon/storage implementation, saved task, acceptance criterion, mission graph, manifest, generated contract or migration was edited. No ownership/interface conflict was found; runtime and integration ownership remains with tasks 6/8.
Final mission gates, production-daemon A/B sequencing, durable OFF restart, real SQLite/CLI/registered-bridge reconciliation, reservation leaks/races and final coverage remain task-8/final-gate obligations. **Zero coverage collections** were run here. This Actor does not claim the six mission gates, 80% per-crate coverage, daemon durability, native Critical OS events, screen/VoiceOver validation, traceability completion or independent QA. No Playwright, GUI, browser download, installed-daemon restart, build distribution, commit or push occurred. Focused checks are not final-input mission receipts.

## Changed paths and reproducible source identity

All 34 source/test paths below are nonignored and staged. For this Actor-local source set, sort rows by path, construct `{path,sha256}` records, serialize the array as compact JSON with sorted object keys and hash UTF-8 bytes with SHA-256. Owned input digest: `b0854e6770254c9cfdf5f67aca1f63f4914b34bd80c664b43e06e101e27ed047`. This excludes this report to avoid a circular identity and is distinct from the later integrated coverage identity. The staged binary Git delta over snapshot HEAD for exactly these paths hashes to `52e80cce93701afb84537c99a5cb60f2adc403c00be971fbe6e7e89d810592f3`.

| Path | SHA-256 of delivered bytes |
| --- | --- |
| `apps/desktop/src-tauri/src/bridge.rs` | `6f645cced5e2657070e80000cb50d1b90d8c4227a8a1c531cce7daf5401e2b9f` |
| `apps/desktop/src-tauri/src/bridge/governor.rs` | `5095293d958dacefce95387ad879b34433703f0b3c03ffaf6a300c9ed484cb05` |
| `apps/desktop/src-tauri/src/commands.rs` | `548a76ccd679a094a92b6c36678465026966101b6e24d39a3b970baf6d40ed6b` |
| `apps/desktop/src-tauri/src/lib.rs` | `d2d350f75c912b4285cd8efa38723d58481eb44e4b3b9eaec003e98b421cc807` |
| `apps/desktop/src-tauri/tests/registered_commands.rs` | `983651c0ef108652eeef200aaf601197d314efeda18938931f6fcb92e17c661b` |
| `apps/desktop/src-tauri/tests/registered_commands_support/governor.rs` | `935e9958c02ed22e87927ee8674eafb61ea16adbaa3683799d10d768b54f8843` |
| `apps/desktop/src/bridge/client.ts` | `774b19621e48dd3cd8eff15b4edc7bb8ed0cdda560d9619d4b6e7f756e961126` |
| `apps/desktop/src/bridge/governor-contract.ts` | `c377c9fe272933235347ec1d117df2f5cc10ec8d8a623132e75a6a5b8d237c54` |
| `apps/desktop/src/bridge/types.ts` | `f4e4ce77470d4f2fa8fdffb81a08a6e17ac214374d7c9c0e14c020be0c5a244e` |
| `apps/desktop/src/dashboard/desktop-dashboard.tsx` | `94b5328b81140dfebeda0f5298c7093b190cc708769803329866502ca9298e0d` |
| `apps/desktop/src/dashboard/desktop-refresh-controller.ts` | `6e6a60ac739d3f7795010f0acc445b6589bb67330ca06f7489767e1ed1090582` |
| `apps/desktop/src/dashboard/desktop-refresh-state.ts` | `c54a9f3bfcdb38c1b0131bbf75478988549336df9dce7ad80ea5752d83628901` |
| `apps/desktop/src/dashboard/governor-master-switch.tsx` | `46513764fcbfac734df99af585644073762ea13efde71b0843b2cb8c0bb5351f` |
| `apps/desktop/src/dashboard/governor-messages.ts` | `089543ed88c0c8ceaf0ddf756a5df8ddfeecc44ec1ac4b437a2cf76178cac9b7` |
| `apps/desktop/src/dashboard/governor-switch-state.ts` | `55a8d02b49e7cdb29a0e5c52c83151632bf5bc1261446c8a91922495d492e803` |
| `apps/desktop/src/dashboard/job-policy-panel.tsx` | `f1f0cc44ad65d65a225aeb7bd1d9be64c68b438a28562187207a29f5867b666a` |
| `apps/desktop/src/dashboard/selected-job-panel.tsx` | `5dd2323f09406f4b086bf6f6900c13dbc21d326fbd327e542e9766cd62acaf03` |
| `apps/desktop/src/desktop-application.tsx` | `6e3d44efb26421aba28738745f45249fd1b8d87371932758b2815d970986e493` |
| `apps/desktop/tests/dashboard.test.ts` | `8ef7c5d6638222aec6c92d2bd69123848fec167eb26f02c0e0cf4476970bfe3b` |
| `apps/desktop/tests/fixtures/desktop-test-harness.ts` | `4eea9487c7bef3f0b9997611e52a5cd74d2c7228f1179936f4f0af2cc3c03e53` |
| `apps/desktop/tests/governor-contract.test.ts` | `0fcad9ce9ca63c20a1d9fdf000408c710c9b6269f14d7fc3c9bd680fb8bb6a36` |
| `apps/desktop/tests/governor.test.ts` | `9f3b1bd261bb4f852eb284ad09325e11c1624029737c66a13b5c51278cceb20d` |
| `apps/desktop/tests/lifecycle.test.ts` | `fd8762076868741f1746c8eb64c2aff20cfb73347bea5dabfc234859c4a71d01` |
| `crates/arc-cli/src/exec.rs` | `5b53c041f5eba8ba418aad8f759655f54340a554f424020e0c25c76bcd23b0f6` |
| `crates/arc-cli/src/exec/accounting.rs` | `6fa3210c6fd0a7fda91e899d9924a275bb1fed86037060515b28400dd3e52cae` |
| `crates/arc-cli/src/exec/governor.rs` | `4c903386647ff5acdac5b5ebd042c70a58e18eb3234db84e796a3d15a0092306` |
| `crates/arc-cli/src/exec/launch.rs` | `95e369c795f4ca9436bb0440098ea97b24e66fed3cb321fd79ea3a349a96a58a` |
| `crates/arc-cli/src/exec/launch/governor.rs` | `4e9d15d2b719e9ff434497b3a8289db18fbd52dc14d768f347393bcda9c00be0` |
| `crates/arc-cli/src/exec/traffic.rs` | `9f951fc72e1be32246bb92dae01c3144621468a4578c812c6c7df817274674bf` |
| `crates/arc-cli/src/governor.rs` | `e5f9e34006fd44e98c57d30f6b5029eae951d241615e75d9516f39cf6b60f68a` |
| `crates/arc-cli/src/main.rs` | `ebfc3827e7ce197eed61d790d00fdbc908ceefb66427df91ad298026f31990e5` |
| `crates/arc-cli/tests/governor.rs` | `dd3d18f7e61e43f9b6804935f7d1add0e0280a0e1a3c8f98b4b976b5bbe26a31` |
| `crates/arc-cli/tests/governor/execution.rs` | `33781e61d6d77c9b3b2f061a9e365cf41e01176d5b45b0a082d149ed3fb459d6` |
| `crates/arc-cli/tests/governor/support.rs` | `e786450050ce69edde9681a39fcc554ba8356fbdeeebf1b8a4bd087f1b8db6c4` |

Material handoff: `docs/requirements/governor-client-handoff.md` is nonignored and Git-visible. Ignored raw logs are not the delivery artifact. The Chief/task-8 can copy these compact check summaries into the canonical integration receipt after reconciling actual integrated bytes. No broader SRS completion counts are asserted by task-7.
