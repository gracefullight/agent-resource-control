# Resident slice review — attempt 3, independent opencode review

## Review Result: PASS for the slice code; BLOCKED for the operator fixed gate (environment, not product)

Status: completed, read-only except this report. The resident slice satisfies every task acceptance criterion when the three gates run on the toolchain the task authorizes. The literal, un-prefixed verification contract commands do **not** reach the compiler in this environment: bare `cargo` is a mise shim with no Rust version set, and all three exit 1 before doing any work. That is the only blocker, it is not caused by the slice, and this read-only reviewer cannot repair it.

Per the instruction "If any cargo command is nonzero … write the blocker and do not call the slice accepted", I record the blocker and stop short of declaring the mission operator gate passed. I do not call the slice rejected either: no product defect was found, and under the authorized invocation the evidence is clean.

Worktree HEAD: `a6a012646ab2f3129cedad230ea1a3ca37fdb5ba` (unchanged). Preceding changes, prior report, and untracked scratch were inspected first and preserved. Skill used: `oma-qa` (execution protocol + checklist). `oma-db` was not used; no schema change was made. No product, test, manifest, lockfile, toolchain, migration, or mission-state file was edited.

### The blocker and its exact remediation

From the workspace root, the three fixed-gate programs resolve through `~/.local/share/mise/shims/cargo`, and mise has no Rust version for that shim:

```text
$ cargo fmt --all -- --check        # BARE_FMT_EXIT=1
$ cargo clippy --workspace --all-targets -- -D warnings   # BARE_CLIPPY_EXIT=1
$ cargo test --workspace            # BARE_TEST_EXIT=1
mise ERROR No version is set for shim: cargo
Set a global default version with one of the following:
mise use -g rust@1.93.0
mise use -g rust@1.95.0
```

`~/.config/mise/config.toml` has no `rust` entry, there is no `.mise.toml`/`.tool-versions` in this worktree, and mise does not pick up `rust-toolchain.toml` by default. So the fixed contract (`program: cargo`) cannot pass until the environment resolves `cargo` to 1.95.0.

Two remedies exist; this task cannot apply either because it owns only this report and the goal forbids changing `rust-toolchain.toml` / the global mise shim:

1. Workspace-local pin (repo-scoped, no global change): add a root `.mise.toml` with `[tools]` / `rust = "1.95.0"`. I verified in a throwaway temp directory that bare `cargo --version` then prints `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` and exits 0. Caveat: the goal says "add only crates/arc-ipc and crates/arc-cli", so the Chief must decide whether a root tool config is in scope; if not, use remedy 2.
2. Harness PATH: run the fixed gate with `/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin` first on `PATH`, which is the remedy the task authorizes for this review.

Changing the toolchain file or running `mise use -g` would violate the assignment and was not done.

### Fresh command evidence

Bare invocations were attempted first and recorded (all exit 1 as shown above). The three exact argument lists were then run sequentially from the workspace root with only this process `PATH` prefixed by the existing toolchain:

```sh
PATH="/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH"
# rustc 1.95.0 (59807616e 2026-04-14); cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

| Command (exact argv) | Exit | Observed |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | no formatter diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile`; no warnings |
| `cargo test --workspace` | 0 | 31 passed, 0 failed, 0 ignored |

Test distribution: `arc-cli` binary 2; daemon units 9; `tests/resident.rs` 2; `tests/startup.rs` 1; `arc-ipc` units 13; `arc-storage` `tests/nfr010.rs` 4; remaining unit/doc targets 0. `resident_contract_holds_until_sigterm`, `commands_report_daemon_unavailable_when_absent`, `migration_failure_exits_nonzero_with_one_stderr_sentence`, and all four NFR-010 tests ran and passed. The historical `AF_UNIX` bind `EPERM` did not reproduce. Each command used the existing `target/` cache; no clean-target build was performed.

### Acceptance matrix

| Criterion | Evidence (file:line) | Result |
| --- | --- | --- |
| Exactly five members; only `arc-ipc` and `arc-cli` added | `Cargo.toml:1-8`; `crates/` contains exactly `arc-core, arc-storage, arc-ipc, arc-daemon, arc-cli` | PASS |
| `arc-ipc` has no tokio or rusqlite | `crates/arc-ipc/Cargo.toml` (serde + serde_json only); `cargo tree -p arc-ipc --offline` shows no other edge | PASS |
| u32 BE prefix + one UTF-8 JSON object; 1 MiB cap | `crates/arc-ipc/src/lib.rs:18`, `:212-242`, `:282-288`; array/text/number/null roots rejected | PASS |
| Oversize declared length rejected without storing body | `crates/arc-ipc/src/lib.rs:229-233` checks before `vec![0u8; declared]`; test asserts `reader.position() == 4` (`:459-470`); resident sends header only (`resident.rs:268-280`) | PASS |
| Operations `daemon.hello`, `daemon.health`, `diagnostics.doctor` | `crates/arc-ipc/src/lib.rs:313-349`; unknown op → `UNSUPPORTED_CAPABILITY` | PASS |
| Version ≠ 1 → `PROTOCOL_INCOMPATIBLE`, server survives | `crates/arc-ipc/src/lib.rs:309-311`; `resident.rs:243-265` sends 99 then succeeds on same and new connection | PASS |
| Owner-only socket, 0600; dir 0700 | `server.rs:29-51`, `:71-75`; `resident.rs:229-238` | PASS |
| No TCP LISTEN | `resident.rs:201-221`, `:239` | PASS |
| Default `TMPDIR/arc/arc.sock`, `ARC_SOCKET_PATH`, keep `ARC_DB_PATH` | `crates/arc-core/src/lib.rs:6-18` | PASS |
| Resident until SIGTERM, then `connection.close()` (no checkpoint), socket removal, exit 0 | `crates/arc-daemon/src/main.rs:14-33`; `server.rs:79-131`; `resident.rs:320-328` | PASS |
| Single instance: OS lock + pid + start time; second exits nonzero; stale recovery, no kill | `instance.rs:61-107`, `:134-165`; `platform.rs:70-113`; `resident.rs:304-318`; six unit tests | PASS |
| Zero `gc_operations` rows; no sweeper | `resident.rs:342-348`; no insert path outside the frozen migration | PASS |
| `arcctl` clap binary, only `status`/`doctor` | `crates/arc-cli/src/main.rs:21-41`; `tests/binary.rs` | PASS |
| Exact `status` line; exit 0 on hello v1 | `crates/arc-cli/src/main.rs:62-76`; `resident.rs:282-292` | PASS |
| Exact `doctor` line; exit 0 on up + integrity ok + 1 migration | `crates/arc-cli/src/main.rs:78-94`; `resident.rs:294-302` | PASS |
| Absent daemon → nonzero + `DAEMON_UNAVAILABLE`; never claims eleven healthy | `crates/arc-cli/src/main.rs:15`, `:97`, `:106`; `resident.rs:352-364`; `resident.rs:292,302` assert no `eleven` | PASS |
| Replace 10-second exit expectation while keeping the migration-failure test | `tests/startup.rs:61-93`; `tests/resident.rs` owns the resident contract | PASS |
| No forbidden surface / no SRS copy / no schema edit | see below | PASS |

### Frozen inputs and dependency invariants

`git diff --stat a6a0126 -- crates/arc-storage rust-toolchain.toml` is empty (`exit 0`). Byte-for-byte SHA-256 against the `a6a0126` blobs:

| File | Current = baseline |
| --- | --- |
| `crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |
| `crates/arc-storage/src/lib.rs` | `fbbbbf0f9ae4f6628475d79e7f6dbd348691dfdcaf99567ef2f275e6eff2dcab` |
| `crates/arc-storage/tests/nfr010.rs` | `e90cf1e26f5f23d733ab7705fd1e770f18165245e5d3359b80e83cf4faa4e244` |
| `rust-toolchain.toml` | `7d21c66a15f41bad66b82ed06a289f5dc8a6e3751951d2696bbf20ce296f2b0f` |

`Cargo.lock` is additive only: `274 insertions(+), 0 deletions`. `rusqlite 0.41.1` (bundled), `tokio 1.54.1`, `sha2 0.11.1`, and `libc 0.2.191` are identical to `a6a0126`; additions are `clap 4.6.7`, `serde 1.0.229`, `serde_json 1.0.154`. No SRS copy exists in the repo.

### Forbidden-surface check

`grep -rniE "TcpListener|TcpStream|NamedPipe|localhost|127.0.0.1|0.0.0.0" crates` → no hits. The only `wal_checkpoint` hit is the pre-existing frozen comment `crates/arc-storage/tests/nfr010.rs:126`; there is no `PRAGMA wal_checkpoint` or call. No desktop, `package.json`, adapters, meter, traffic, sweeper implementation, Windows pipe, or SRS file exists. `git diff a6a0126 -- av.yaml` reports the file as deleted only because this sandbox denies reading `av.yaml`; it is AV mission metadata, still present on disk, not a product file, and was not touched.

### Findings

CRITICAL: none. HIGH: none. MEDIUM: none.

LOW (retained, not gate blockers):

- `crates/arc-daemon/src/server.rs:102` — one unbounded `std::thread::spawn` per accepted connection and no idle read timeout at `:140`. The 0600 socket plus the same-user peer check (`platform.rs:10-67`) confine this to the local account, and shutdown is not deadlocked (the DB mutex is not held while blocked in `read_frame`). Remediation: cap concurrent handlers and set a read timeout on the accepted stream.
- `crates/arc-daemon/src/server.rs:48-49` — `prepare_socket_directory` unconditionally chmods a pre-existing operator-chosen parent to 0700. It only tightens and satisfies the owner-only requirement, but could surprise an operator who points `ARC_SOCKET_PATH` at a shared existing directory. Remediation: tighten only a directory this call created, or refuse a pre-existing non-owner-only parent (the shared-root guard at `:38-46` already refuses `/tmp`).
- `crates/arc-cli/src/main.rs:89-92` — `doctor` prints the fixed honest lines without asserting the corresponding `ipc_owner_only` / `tray_connected` / `vendors_detected` / `command_classification` fields returned at `crates/arc-ipc/src/lib.rs:338-341`. Accurate for this daemon today; a future daemon change could make the printed claim stale. Remediation: read those fields and fail if they are not owner-only / disconnected / not-detected / not-checked.
- `crates/arc-daemon/tests/resident.rs:201-221` — the TCP assertion shells out to `lsof` and requires empty stderr; on a host without `lsof` the test fails rather than skips. `/usr/sbin/lsof` exists on this host and the test passed.

### Verification gaps

1. The literal fixed verification cannot execute (bare-cargo mise shim) — the blocker above.
2. No clean-target build; gates used the existing `target/` cache.
3. Only macOS ran. Linux `SO_PEERCRED` / `/proc/<pid>/stat` branches (`platform.rs:11-40`, `:100-113`) were not exercised; Windows Named Pipe and the two-OS tray DoD are out of this gate.
4. Serena/code-intelligence MCP tools were absent; review used scoped reads of the supplied paths, `git diff`, `cargo tree`, and `cargo metadata`-equivalent manifest inspection.

### Files changed by this review

- `docs/resident-slice-review.md` only (attempt-3 report prepended; the attempt-2 report is preserved verbatim below).

No crate, test, manifest, lockfile, toolchain file, migration, or mission-state file was edited. No external delivery, publish, or spend occurred. No revenue, conversion, churn, or field-test data is asserted.

---

# Resident slice review — attempt 2, independent Codex review (preserved)

## Review Result: PASS

Status: completed. The assigned slice is accepted against the specified local gate. All three freshly executed Cargo commands exited 0; all 31 tests passed. No new CRITICAL, HIGH, or MEDIUM defect was established. No required check is blocked. This is a review deliverable, not mission-state approval or an external delivery.

Reviewed on 2026-11-06, with evidence captured at 11:12–11:14 UTC, in the assigned mission worktree. HEAD remains `a6a012646ab2f3129cedad230ea1a3ca37fdb5ba`. Existing changes and the preceding report were inspected first and preserved. The earlier report is retained verbatim below as historical evidence; its manual probes and execution-harness claims are not fresh observations from attempt 2.

Skill used: `oma-qa`, including its execution protocol and applicable checklist. No implementation or database skill was used. Only `docs/resident-slice-review.md` was edited by this reviewer. Product files, tests, manifests, lockfile, toolchain, migration, saved assignment, and mission state were not edited.

## Fresh command evidence

All commands ran from the workspace root. Bare `cargo --version` first exited 1 with `mise ERROR No version is set for shim: cargo`. The permitted remedy was applied only to the command subprocess environment:

```sh
PATH="/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH"
```

No installation, global mise setting, or toolchain-file change occurred. Resolved versions were `rustc 1.95.0 (59807616e 2026-04-14)` and `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`.

The three exact command argument lists were executed sequentially with a 120-second timeout each; no timeout occurred. Wall times include command startup.

| Command | Exit | Wall time | Observed result |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | 0.697 s | No formatting diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0.698 s | Finished dev profile; no warnings |
| `cargo test --workspace` | 0 | 2.074 s | 31 passed, 0 failed, 0 ignored |

Test counts: arc-cli binary integration 2; daemon units 9; resident integration 2; migration startup integration 1; arc-ipc units 13; frozen storage/NFR-010 integration 4. Other unit/doc-test targets had zero tests and passed. In particular, `resident_contract_holds_until_sigterm`, `commands_report_daemon_unavailable_when_absent`, `migration_failure_exits_nonzero_with_one_stderr_sentence`, and `nfr010_wal_migration_survives_close_and_reopen_without_cleanup` all ran successfully. The historical AF_UNIX bind failure did not reproduce.

Additional read-only commands: `cargo metadata --no-deps --offline --format-version 1` (0), `cargo tree -p arc-ipc --offline` (0), `rustc --version` (0), `cargo --version` with the PATH prefix (0), `git rev-parse HEAD` (0), and `git diff --stat a6a0126 -- crates/arc-storage rust-toolchain.toml` (0, empty). Scoped file reads, manifest/lockfile parsing, and SHA-256 comparisons supplied the static evidence below. `git status --short` and the initial diff inspection also completed; Git emitted the existing `av.yaml: Operation not permitted` diagnostic. That file was not read or changed.

## Acceptance and interface evidence

| Criterion | Fresh evidence | Result |
| --- | --- | --- |
| Exactly five members; only two added crates | Manifest and Cargo metadata both list `arc-core`, `arc-storage`, `arc-ipc`, `arc-daemon`, `arc-cli`; root/crate inventory agrees | PASS |
| IPC dependency isolation | `arc-ipc/Cargo.toml` and `cargo tree` show only serde and serde_json as direct dependencies; no tokio or rusqlite edge | PASS |
| Bounded length-prefixed JSON object | `arc-ipc/src/lib.rs:222` reads a 4-byte big-endian header, rejects zero or greater than 1 MiB before allocating the body, then checks the JSON root is an object. Oversize unit test checks reader position is 4; resident test also sends only an oversized header | PASS |
| Protocol operations and compatibility | Dispatcher handles only `daemon.hello`, `daemon.health`, `diagnostics.doctor`; incompatible versions are rejected before handlers. Named-socket lifecycle sends version 99 and gets `PROTOCOL_INCOMPATIBLE`, then hello succeeds on the same connection and a later connection | PASS |
| Owner-only IPC; no TCP LISTEN | `resident.rs:221` starts the daemon with temporary `ARC_DB_PATH` and `ARC_SOCKET_PATH`, asserts socket type and 0600, directory 0700, and a successful nonempty `lsof` result without TCP LISTEN | PASS |
| Resident single instance and safe recovery | Lifecycle proves the second daemon exits nonzero with the duplicate-instance diagnostic while the first still answers hello. Six instance unit tests pass, including dead-pid and changed-start-time recovery. Source uses OS file locks, pid/start token, directory acquisition serialization, and inode comparison without terminating another process | PASS |
| SIGTERM shutdown and database state | Lifecycle sends SIGTERM, asserts successful process exit and socket removal, reopens SQLite, asserts `integrity_check = ok`, one migration, zero `gc_operations`, and closes it. Server explicitly closes its SQLite connection before removing the socket | PASS |
| Exact CLI contract | Lifecycle asserts `running. Socket: <path>. Protocol: 1. Sweeper off.` and `ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.`; both commands exit 0. Absence test proves both exit nonzero with `DAEMON_UNAVAILABLE` | PASS |
| Original failure and NFR-010 checks retained | `startup.rs:60` retains the exact migration-failure test and stderr sentence; all four unchanged storage tests pass | PASS |
| No prohibited product surface | Reviewed manifests, metadata targets, root inventory, and all changed/new Rust slice files contain no desktop, package.json, adapters, meter, traffic, sweeper implementation, TCP listener, Windows pipe, or explicit WAL-checkpoint operation; no SRS copy was added | PASS |

`arcctl` is a clap binary with exactly the `status` and `doctor` subcommands and performs socket exchanges without opening SQLite. `status` validates both response and hello-result protocol 1. `doctor` validates protocol, integrity `ok`, and exactly one migration. Neither claims vendor health. Default socket resolution remains `<TMPDIR>/arc/arc.sock`, with `ARC_SOCKET_PATH` and `ARC_DB_PATH` overrides preserved. Same-user peer validation is enforced by the daemon. Request payloads are ignored rather than logged or persisted; the slice adds no database writes beyond the existing migration and no collection of secrets, chat text, or cookies.

Locked `rusqlite 0.41.1`, `tokio 1.54.1`, `sha2 0.11.1`, and `libc 0.2.191` match baseline; bundled rusqlite remains enabled. IPC structs and CLI/daemon exchanges are compatible under the passing tests. No ownership or interface conflict was found.

## Frozen-file verification

The required `git diff --stat a6a0126 -- crates/arc-storage rust-toolchain.toml` was empty before and after the gates. Direct comparisons to `git show a6a0126:<path>` additionally confirmed byte equality:

| Path | Current and baseline SHA-256 |
| --- | --- |
| `crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` |
| `crates/arc-storage/src/lib.rs` | `fbbbbf0f9ae4f6628475d79e7f6dbd348691dfdcaf99567ef2f275e6eff2dcab` |
| `crates/arc-storage/tests/nfr010.rs` | `e90cf1e26f5f23d733ab7705fd1e770f18165245e5d3359b80e83cf4faa4e244` |
| `rust-toolchain.toml` | `7d21c66a15f41bad66b82ed06a289f5dc8a6e3751951d2696bbf20ce296f2b0f` |

## Findings, limits, and handoff

No new blocker or failed Cargo gate. The preceding review's LOW advisories are retained below, with their original provenance. The gate validates this macOS host; Linux-specific branches were not executed, and Windows Named Pipe and the two-OS tray DoD remain outside the assignment. A clean-target Cargo run was not performed; the commands used the existing target cache. No revenue or field-study evidence is claimed.

Serena/code-intelligence tools were absent from the available tool registry. Following the configured fallback, review used scoped reads of the file paths already supplied by the assignment/status, Git diffs, Cargo metadata/dependency output, and inventories; no substitute recursive native source search was performed. Consequently the forbidden-surface conclusion covers the actual slice and visible product inventory, not a global audit of ignored tooling/cache contents. Existing `.arc-verify/` and `.probe.txt` were left intact. Initial heredoc use failed because the shell could not create its temporary file (`operation not permitted`); inline Python execution worked without creating a helper file. Commands used the normal execution tool without the historical PTY workaround.

Changed path / produced artifact: **`docs/resident-slice-review.md` only**. Cargo produced ordinary ignored target artifacts and its tests exercised disposable fixtures. No external delivery, mission mutation, product repair, or delegation occurred. The Chief Director can use the fresh 0/0/0 gate and acceptance matrix above for coordination; preceding Actor changes remain untouched.

---

# Historical attempt 1 report (preserved)

Status: this run observed `cargo fmt` exit 0, `cargo clippy` exit 0, and `cargo test --workspace` exit 0.

Task: `task-review`, attempt 1. Reviewer role: Independent reviewer (read-only except this report). Workspace HEAD `a6a0126`, branch `av-operation/343fe8a1-5fb8-4e16-b6fb-0288b57f0f1f/41363ec9-cdd2-4ddf-bb52-c1b9f55ffde5`. Commands ran in this mission worktree on 2026-11-06 between `2026-11-06T10:46:16Z` and `2026-11-06T11:02:00Z`.

Skill used: `oma-qa` (`.agents/skills/oma-qa/SKILL.md`, `resources/execution-protocol.md`). `oma-db` was not used; no schema was touched. `oma-backend` was not used because this task implements nothing.

## Review Result: PASS

Verification completeness: **completed** for the macOS gate defined by the mission. Zero CRITICAL, HIGH, or MEDIUM findings. Three LOW findings and two verification gaps are recorded below; none blocks the slice.

The slice is accepted against the eight task acceptance criteria and the mission `verificationContract`. The previously reported blocker (`cargo test` exit 101, `AF_UNIX` bind `EPERM`) **did not reproduce in this run**: `resident_contract_holds_until_sigterm` passed, and an independent manual probe bound a Unix socket successfully in the same worktree.

## Commands and exits

All three gates ran from the workspace root. Each exit code below was recorded in this run, not copied from a prior log.

| Argv | Exit recorded in this run |
| --- | --- |
| `cargo fmt --all -- --check` | 0 (`FMT_EXIT=0`). No formatter diff printed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 (`CLIPPY_EXIT=0`). Finished dev profile in 7.24s. |
| `cargo test --workspace` | 0 (`TEST_EXIT=0`). Finished test profile in 5.69s; whole run ~9.5s, inside the 120000 ms contract budget. |

### Environment and PATH prefix

Bare `cargo` is a mise shim with no version set. It fails before compile:

```
mise ERROR No version is set for shim: cargo
```

Per the task instruction, only this process's `PATH` was prefixed with the already-installed rustup toolchain. Nothing was installed, `mise use -g` was not run, the global shim was not changed, and `rust-toolchain.toml` was not edited.

```sh
export PATH="/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin:$PATH"
```

Recorded toolchain path: `/Users/gracefullight/.rustup/toolchains/1.95.0-aarch64-apple-darwin/bin/cargo`.
Resolved versions: `rustc 1.95.0 (59807616e 2026-04-14)`, `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`. `rust-toolchain.toml` still reads `channel = "1.95.0"`, `profile = "minimal"`, `components = ["rustfmt", "clippy"]`.

Host execution note: the foreground shell tool fails in this sandbox with `grantpt failed: Operation not permitted` (PTY allocation). Background-mode shell execution works and was used for every command; exit codes were read from the managed task status records. This is an environment limitation of the review harness, not a product defect.

### `cargo test --workspace` results

| Target | Tests | Result |
| --- | --- | --- |
| `arc-cli` `tests/binary.rs` | `arcctl_binary_is_built_next_to_the_test_harness`, `no_arguments_names_both_commands_and_exits_nonzero` | 2 passed |
| `arc-daemon` unit (`src/main.rs`) | 6 `instance::tests`, 3 `server::tests` | 9 passed |
| `arc-daemon` `tests/resident.rs` | `resident_contract_holds_until_sigterm`, `commands_report_daemon_unavailable_when_absent` | 2 passed |
| `arc-daemon` `tests/startup.rs` | `migration_failure_exits_nonzero_with_one_stderr_sentence` | 1 passed |
| `arc-ipc` unit | 13 framing / protocol / object-root tests | 13 passed |
| `arc-storage` `tests/nfr010.rs` | `migration_failure_rolls_back_all_new_tables`, `foreign_keys_and_unique_resource_identity_are_enforced`, `nfr010_wal_migration_survives_close_and_reopen_without_cleanup`, `mismatched_checksum_is_rejected_without_reapplying_migration` | 4 passed |
| `arc-core`, `arc-storage`, `arcctl` unit + all doc-tests | 0 tests | 0 failed |

Total: 31 passed, 0 failed, 0 ignored. The NFR-010 test is present and green.

## Frozen inputs and scope

`git diff --stat a6a0126 -- crates/arc-storage rust-toolchain.toml` printed nothing (`DIFF_EXIT=0`), both before and after the gates. Byte-for-byte SHA-256 comparison against the `a6a0126` blobs:

| File | Baseline `a6a0126` | Worktree | Verdict |
| --- | --- | --- | --- |
| `crates/arc-storage/migrations/0001_phase0.sql` | `046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384` | identical | unchanged |
| `crates/arc-storage/src/lib.rs` | `fbbbbf0f9ae4f6628475d79e7f6dbd348691dfdcaf99567ef2f275e6eff2dcab` | identical | unchanged |
| `crates/arc-storage/tests/nfr010.rs` | `e90cf1e26f5f23d733ab7705fd1e770f18165245e5d3359b80e83cf4faa4e244` | identical | unchanged |
| `rust-toolchain.toml` | `7d21c66a15f41bad66b82ed06a289f5dc8a6e3751951d2696bbf20ce296f2b0f` | identical | unchanged |

`git diff a6a0126 -- crates/arc-storage rust-toolchain.toml | wc -c` = `0`.

Members: `cargo metadata --no-deps --offline` reports exactly `['arc-cli', 'arc-core', 'arc-daemon', 'arc-ipc', 'arc-storage']`. `Cargo.toml` lists the same five. Only `crates/arc-ipc` and `crates/arc-cli` were added; `crates/` contains no other crate directory.

Dependency isolation: `cargo tree -p arc-ipc --offline` (exit 0) shows `arc-ipc` → `serde 1.0.229` and `serde_json 1.0.154` only, with their transitive proc-macro/itoa/memchr/zmij edges. No `tokio`, `rusqlite`, `axum`, `sqlx`, `nix`, or `fs2` edge exists. `crates/arc-ipc/Cargo.toml` declares only `serde` and `serde_json`.

Locked versions are unchanged: `rusqlite 0.41.1` (bundled), `tokio 1.54.1`, `sha2 0.11.1`, `libc 0.2.191`; additions are `clap 4.6.7`, `serde 1.0.229`, `serde_json 1.0.154`. `git diff --stat a6a0126 -- Cargo.lock` is `274 insertions(+)`, 0 deletions — purely additive, no version bump.

Forbidden surfaces: a content search of `crates/` for `wal_checkpoint|TcpListener|TcpStream|package.json|NamedPipe|named_pipe` returns exactly one hit, the pre-existing frozen comment `crates/arc-storage/tests/nfr010.rs:126` (`close without wal_checkpoint: use SQLite's normal connection close path.`). There is no `PRAGMA wal_checkpoint` and no `wal_checkpoint(` call. No desktop, `package.json`, adapters, meter, traffic, sweeper, TCP listener, or Windows pipe exists. The only root JSON file is the pre-existing `.mcp.json`. No SRS copy is present in the repo.

## Acceptance criteria checklist

| # | Criterion | Verdict | Evidence |
| --- | --- | --- | --- |
| 1 | `arc-ipc` is a member without tokio or rusqlite | PASS | `Cargo.toml` members; `cargo tree -p arc-ipc` exit 0, serde/serde_json only |
| 2 | Oversize length is rejected without storing the body | PASS | `crates/arc-ipc/src/lib.rs:222-238` validates `declared > MAX_FRAME_BYTES` before `vec![0u8; declared]`; test `oversize_declared_length_is_rejected_without_reading_the_body` asserts `reader.position() == 4`; runtime probe sent only the 4-byte header for 1 MiB+1 and received `FRAME_TOO_LARGE` |
| 3 | `0001_phase0.sql` and `nfr010.rs` stay unmodified | PASS | SHA-256 identical to `a6a0126`; empty diff; NFR-010 4/4 green |
| 4 | Daemon stays up until SIGTERM, then closes SQLite, removes the socket, exits 0 | PASS | `server.rs:78-131`; runtime `SIGTERM_EXIT=0`, `SOCKET_REMOVED`, `integrity_check=ok`; `connection.close()` with no checkpoint |
| 5 | `status` and `doctor` use the exact lines and print `DAEMON_UNAVAILABLE` when absent | PASS | exact-string assertions in `resident.rs:286-302`; runtime stdout matched verbatim; absent socket gave `DAEMON_UNAVAILABLE` and exit 1 for both |
| 6 | `resident.rs` covers the lifecycle and the migration-failure stderr test remains | PASS | 11511 bytes; all contract tokens present; `startup.rs:60` `migration_failure_exits_nonzero_with_one_stderr_sentence` green |
| 7 | Report records the three cargo exit codes and any blockers | PASS | this file; fmt 0 / clippy 0 / test 0; no blocker |
| 8 | Confirms the frozen migration, five members, and no forbidden surface | PASS | sections above |

Mission `verificationContract` file check on `crates/arc-daemon/tests/resident.rs`: 11511 bytes (≥ 400) and all required substrings present — `0600` (L233), `0700` (L237), `PROTOCOL_INCOMPATIBLE` (L5, L256), `DAEMON_UNAVAILABLE` (L360), `Vendors not detected` (L299), `Sweeper off` (L288, L300), `IPC owner-only` (L299), `integrity_check` (L332), `gc_operations` (L344), `SIGTERM` (L2, L320-323).

## Runtime verification results

Independent of the cargo test, the built binaries were driven manually against a fresh `mktemp -d` directory outside the repo (`ARC_DB_PATH` and `ARC_SOCKET_PATH` set to that directory). The scratch directory was removed afterwards; no repo file was written.

| Check | Method | Expected | Actual | Status |
| --- | --- | --- | --- | --- |
| Socket mode | `stat -f '%Lp'` on the socket | 0600 | `SOCKET_MODE=600` | PASS |
| Directory mode | `stat -f '%Lp'` on the parent | 0700 | `DIR_MODE_AFTER=700` | PASS |
| No TCP listener | `lsof -nP -p <pid>` | 0 TCP `(LISTEN)` rows | `TCP_LISTEN_COUNT=0` | PASS |
| Length-prefixed `daemon.hello` | raw `u32be` + JSON over `AF_UNIX` | `status ok`, `protocol_version 1` | `{"protocol_version":1,...,"result":{"daemon":"running","protocol_version":1,"sweeper":"off"},"status":"ok"}` | PASS |
| `protocol_version` 99 | same framing | `PROTOCOL_INCOMPATIBLE`, server survives | `{"error":{"code":"PROTOCOL_INCOMPATIBLE"},...,"status":"error"}` then `HELLO_AFTER_V99` ok | PASS |
| 1 MiB+1 declaration, header only | send 4 bytes, read reply | `FRAME_TOO_LARGE`, no body waited for | `{"error":{"code":"FRAME_TOO_LARGE"},...}` then `HELLO_AFTER_OVERSIZE` ok | PASS |
| `daemon.health` | framed request | ok, no DB needed | `status ok`, `sweeper off` | PASS |
| `diagnostics.doctor` | framed request | integrity ok, 1 migration, honest negatives | `integrity_check:"ok"`, `schema_migrations:1`, `ipc_owner_only:true`, `tray_connected:false`, `vendors_detected:false`, `command_classification:"not_checked"`, `sweeper:"off"` | PASS |
| Non-object JSON root | framed `[1,"r7","daemon.hello",null]` | rejected, connection usable | `INVALID_REQUEST`; later requests ok | PASS |
| Unknown operation | `gc.run` | `UNSUPPORTED_CAPABILITY` | `{"error":{"code":"UNSUPPORTED_CAPABILITY"}}` | PASS |
| `arcctl status` | run binary | exit 0, exact line | `running. Socket: <path>. Protocol: 1. Sweeper off.` / `STATUS_EXIT=0` | PASS |
| `arcctl doctor` | run binary | exit 0, exact line | `ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.` / `DOCTOR_EXIT=0` | PASS |
| Second daemon | spawn again on same socket | non-zero, first still answers | `SECOND_DAEMON_EXIT=1`, stderr `socket already served by another daemon: <path>`; `STATUS_AFTER_SECOND_EXIT=0` | PASS |
| SIGTERM shutdown | `kill -TERM`, `wait` | exit 0, socket removed | `SIGTERM_EXIT=0`, `SOCKET_REMOVED` | PASS |
| Reopen database | `sqlite3` after shutdown | `ok`, 1 migration, 0 gc rows | `ok` / `1` / `0`, `SQLITE_EXIT=0` | PASS |
| Absent daemon | point `ARC_SOCKET_PATH` at a non-existent socket | non-zero + `DAEMON_UNAVAILABLE` | `ABSENT_STATUS_EXIT=1`, `ABSENT_DOCTOR_EXIT=1`, both printed `DAEMON_UNAVAILABLE` | PASS |
| No-args CLI | `arcctl` | non-zero, names both commands, no color | exit 2, lists `status` and `doctor`, mentions SIGTERM, no ANSI | PASS |

No output claimed eleven vendors are healthy. `resident.rs:292,302` additionally assert stdout does not contain `eleven`. No revenue, conversion, churn, or field-test data was invented or is asserted anywhere in this review.

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

- `crates/arc-daemon/src/server.rs:49` — `prepare_socket_directory` unconditionally applies `fs::set_permissions(parent, 0o700)` to the socket's parent even when that directory already existed and was chosen by the operator through `ARC_SOCKET_PATH`. Reproduced: a directory created `0755` before daemon start read `700` afterwards (`PREEXISTING_DIR_MODE_BEFORE=755`, `DIR_MODE_AFTER=700`). The change only ever tightens permissions, so it cannot expose data, and it satisfies the owner-only directory requirement; the risk is surprising availability loss for anything else sharing an operator-chosen parent (for example `ARC_SOCKET_PATH=$HOME/arc.sock` removes group/other traverse from `$HOME`). The shared-root guard at `server.rs:38-46` already refuses `/tmp`, `/var/tmp`, and `env::temp_dir()` rather than tightening them; the same restraint is not applied to other pre-existing directories. Remediation: tighten only a directory this call created, or refuse a pre-existing parent that is not already owner-only.

```rust
// server.rs, inside prepare_socket_directory, replacing the unconditional chmod
let created = !parent.exists();
fs::create_dir_all(parent)?;
if created {
    fs::set_permissions(parent, Permissions::from_mode(0o700))?;
} else {
    let mode = fs::metadata(parent)?.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not owner-only; choose a private socket directory", parent.display()),
        ));
    }
}
```

- `crates/arc-daemon/src/server.rs:102` — one unbounded `std::thread::spawn` per accepted connection, and `handle_connection` (`server.rs:140`) blocks in `read_frame` with no idle read timeout. The 0600 socket plus the same-user peer check (`platform.rs:7-66`) confine the exposure to the local account, so this is not a remote attack surface, but a local client can still hold threads open indefinitely. Shutdown is not deadlocked by this: the database mutex is not held while blocked in `read_frame`, and the manual SIGTERM probe exited 0. Remediation: bound concurrent handlers and set a read timeout.

```rust
// server.rs, in the accept arm before spawning
static CONNECTIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
if CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 64 {
    CONNECTIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    eprintln!("IPC connection limit reached.");
    continue;
}
// and in handle_connection, before the read loop:
let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
// decrement CONNECTIONS when the handler returns
```

- `crates/arc-daemon/src/server.rs:176` — `diagnostics.doctor` runs a full `PRAGMA integrity_check` on every call while holding the single-writer mutex. Correct for Phase 0 (the database is tiny and the probe returned `ok` immediately), but the cost is a whole-database scan that grows with the ledger and serializes concurrent doctor calls behind the one writer. Remediation, if doctor becomes frequent: keep `integrity_check` for the explicit doctor path but avoid calling it from any future high-frequency health path, or time-bound it.

```rust
// Keep the authoritative check for doctor; do not reuse it for liveness.
// OPERATION_HEALTH already answers without touching the database (arc-ipc/src/lib.rs:322-330).
```

## Verification gaps

These are missing checks, not defects, and they do not change the PASS verdict for the defined gate.

1. **Linux branches unexecuted.** `crates/arc-daemon/src/platform.rs:11-40` (`SO_PEERCRED`) and `:100-113` (`/proc/<pid>/stat` field 22) compile only under `cfg(target_os = "linux")`. This host is `aarch64-apple-darwin`, so only the macOS `getpeereid` and `proc_pidinfo` branches ran. Next action: run the same three gates on a Linux host before claiming cross-platform behavior. The mission gate is macOS-scoped, and Windows Named Pipe plus the two-OS Phase 0 tray DoD are explicitly out of this gate.
2. **Foreground shell PTY unavailable.** `run_shell_command` in the foreground fails with `grantpt failed: Operation not permitted`; the `monitor` probe produced no usable events. All commands therefore ran through background-mode shells, with exits read from the managed status records. Alternative checks completed: all three gates, the frozen-file hashes, the dependency tree, and the full manual runtime probe. No required check was skipped.

## Residual risks and notes for the Chief Director

- Untracked scratch from earlier attempts remains in the worktree root: `.arc-verify/` (logs plus `probe.py`, a harmless `AF_UNIX` bind probe with no TCP) and `.probe.txt` (a captured `git status`). Neither is a product file or a forbidden surface, and neither is owned by this review, so both were left untouched. A broad `git add -A` would sweep them into a commit; decide whether to remove or ignore them before any commit. `/target/` is gitignored, so the cargo builds did not dirty the tree.
- `git status` prints `av.yaml: Operation not permitted` in this sandbox. Git still completes and the rest of the status is readable; this is a pre-existing host restriction, not a slice defect. Mission state was not mutated.
- Single-instance recovery is sound under the tested paths: the lock is an OS file lock plus pid and start time, acquisition is serialized by a directory lock (`instance.rs:113-131`), a stale inode is unlinked only while the pathname still names it (`instance.rs:145-165`), and no code path sends a signal to another process. The only signal sent anywhere is the test's `libc::kill(..., SIGTERM)` to its own child. A residual, operator-induced edge remains: if someone deletes `<socket>.lock` while a daemon is live, a second daemon can lock a new inode and `bind_socket` (`server.rs:57-76`) will replace the live socket. That requires deliberate interference with the lock file and is outside the tested contract.
- The daemon also exits 0 on SIGINT (`server.rs:80,90`). The goal names SIGTERM; the extra handler is harmless and isn't a criterion violation.
- Messages and the database carry no secrets, chat text, or cookies. `Request.payload` is a free-form `Value` that the daemon never reads, echoes, or persists; `DoctorReport` carries only `integrity_check` and `schema_migrations`. `arc-storage/src/lib.rs` remains the single writer and `arcctl` never calls `open_and_migrate` (`crates/arc-cli/src/main.rs` dials the socket only).

## Files changed by this review

- `docs/resident-slice-review.md` (this report, new).

No crate, test, manifest, lockfile, toolchain file, migration, or mission-state file was edited. Post-review `git status --short` matches the pre-review set exactly, and `git diff --stat a6a0126 -- crates/arc-storage rust-toolchain.toml` is still empty (`DIFF_EXIT=0`).
