# Preservation baseline

Status: completed for task-1. Skill: oma-refactor, using the installed
refactor-engineer definition. No product file was edited. Implementation,
independent QA and final mission verification belong to later tasks.

Accepted revision: `a905653d557f4c5246a1f656042cb8d29e46c052`; live HEAD matches.
`baseline.json` is the entry point. `source-files.json` records accepted Git blob
IDs, SHA-256 byte hashes, sizes, newline counts and working-byte comparisons for
60 concrete paths. Every captured working file matches its accepted Git bytes.
The two actual original violations are:

| Path | Newlines | Excess over 500 |
| --- | ---: | ---: |
| crates/arc-daemon/tests/resident.rs | 901 | 401 |
| crates/arc-ipc/src/lib.rs | 551 | 51 |

Metric delta in this baseline task: zero source lines and zero test bodies
changed. Readability verdict: no restructuring performed yet. Named scenario
modules under one resident integration target and ordinary IPC child modules
fit the current Rust module conventions. No new dependency is needed.

## Inventory and mapping interface

The inventory comes from accepted Git bytes, not supplied Director counts.
`inventory.json` covers all 24 original tests and 27 helper functions/methods,
including nested `snapshot::visit`, the IPC `tests::hello` helper and shared
support functions. Each record has an original ID, inclusive line/body ranges,
qualified name, attributes, raw item/body hashes, literal-preserving body token
hash and owned assertion locations. `assertions.json` records 137 macro sites
(`assert*`, `panic!` and `unreachable!`) in tests and helpers. These are static
sites, not dynamic assertion execution counts. Nested helper sites belong to
their innermost helper. Unwrap/expect calls remain covered by full body hashes.

`attributes.json` records all 28 target/support attributes, including the IPC
module's inherited `cfg(test)`, resident macOS `cfg`, test, derive and support
`allow(dead_code)` attributes. `declarations.json` additionally inventories helper
types, constants, static counters/locks, imports and implementation blocks.
The lexical collector is scoped to these inspected sources; it is not a general
Rust parser. Final QA must compare actual source bodies and attributes as well
as the stored inventory.

`mapping-contract.json` defines the implementation handoff: one mapping entry
per inventory ID, original and destination path/name/ranges/hashes, attributes,
per-assertion location pairing and explicit mechanical differences. Nonfunction
declarations have their own coverage. Relocation, indentation, private module
wiring and narrowly scoped helper visibility are permitted; test semantics,
case arrays, loops, expected output and production code remain frozen.
If the probe moves, only its exact selector literal may change as documented
there; final discovery and ready/acquired handoff must prove the selector works.

## Fixture invariants from accepted source

All references below use accepted Git line numbers.

- `resident.rs:31-105`: one Fixture implementation owns its directory, database
  and socket paths. `spawn_configured:58-70` changes umask only in the child via
  `pre_exec`. Preserve configuration and ownership.
- `resident.rs:86-105`: gated children execute `/bin/sh` with
  `read -r gate; exec "$@"`. Both child handles exist before release. `exec`
  preserves the owned PID; cleanup must not become shell-orphan cleanup.
- `resident.rs:109-137`: Daemon owns `Child`, dereferences it for existing calls,
  and on Drop uses `try_wait`, kills a still-live owned child and always waits.
  Fixture Drop removes only its owned directory. Preserve kill/wait behavior
  on success and assertion failure; do not introduce broad process cleanup.
- `resident.rs:140-189`: readiness uses a valid hello response and bounded
  deadlines; wait_for_exit kills on deadline and the Daemon wrapper reaps it
  during unwinding. Preserve those helpers and constants.
- Resident scenarios take `support::serial()` before creating fixtures and
  children. Reverse local drop order keeps the lock until owned-child cleanup
  and fixture cleanup complete. Preserve declaration and explicit release order.
- `tests/support/mod.rs:9-23`: Serial owns `_file` then `_thread`. Releasing the
  process lock takes only the file; the thread mutex remains held. Keep one
  shared implementation rather than module-local locks.
- `tests/support/mod.rs:26-66`: one static thread mutex plus a UID-specific
  `arc-fixtures-{uid}.lock` under the native runtime root coordinates threads and
  integration binaries. Open flags include O_NOFOLLOW/O_NONBLOCK, mode 0600,
  regular-file/UID/mode assertions and a bounded try_lock loop. Preserve all.
- `resident.rs:862-870`: root qualified name `fixture_serialization_probe` writes
  `ready` before acquiring the shared lock, then writes `acquired` while holding
  it, only when ARC_FIXTURE_COORDINATION_PROBE is set.
- `resident.rs:872-901`: the parent launches the current test executable with
  `--exact fixture_serialization_probe`, requires the child alive until `ready`,
  asserts `acquired` absent while the lock is held, releases only the process
  lock, waits for a successful child exit, then asserts `acquired` present.
  The `serial` guard outlives the owned-child wrapper. A zero-test child exit
  cannot satisfy these assertions. The baseline parent scenario passed.

## Frozen boundaries

`source-files.json` lists concrete hashes for every accepted crate file,
including every daemon production source, the shared fixture implementation,
all five crate manifests, root Cargo.toml/Cargo.lock, toolchain configuration,
`crates/arc-storage/migrations/0001_phase0.sql`, both harness scripts and harness
rule documents. Protected paths are listed in `baseline.json`; preservation
means byte equality, not a statement that each protected file was semantically
audited. IPC is the special mixed file: its production prefix, lines 1-354
before `#[cfg(test)] mod tests`, has a separate exact-byte hash. The private
`decode_object` remains private. No IPC production visibility change is needed.

`fingerprint.md` and `fingerprint-files.json` define one reproducible actual-byte
source identity that includes new untracked modules in later snapshots. The
baseline fingerprint is
`46ee27daa0957c86e794ac14ce917c81d4f026bb4364e9879aee3dc77835d9d5`.
Evidence is hashed separately to avoid a self-referential source identity.

## Native checks and prerequisites

On macOS 27.2 arm64, installed rustc/cargo are 1.95.0; rustfmt and clippy
components are installed. Node, Bun shim, C compiler and `/usr/sbin/lsof` are
present. Offline locked metadata succeeded for the five-crate workspace.
Discovery and focused checks ran serially with CARGO_NET_OFFLINE=true and
RUSTUP_AUTO_INSTALL=0. Only Cargo test's required compilation occurred; there
was no standalone build, installation, download, provider change or browser run.
Generated Cargo artifacts use the existing workspace `target/` directory.

| Command after `cargo test --offline --locked` | Exit | Evidence |
| --- | ---: | --- |
| `-p arc-ipc --lib -- --list` | 0; 13 tests | ipc-discovery.json |
| `-p arc-cli --test binary -- --list` | 0; 2 tests | cli-discovery.json |
| `-p arc-daemon --test resident -- --list` | 0; 11 tests | resident-discovery.json |
| `-p arc-ipc --lib` | 0; 13 passed | ipc-baseline.json |
| `-p arc-cli --test binary` | 0; 2 passed | cli-baseline.json |
| `-p arc-daemon --test resident` | 0; 11 passed | resident-baseline.json |

The resident result includes the macOS TMPDIR case, CLI status/doctor journeys,
lsof listener inspection, concurrent admission/recovery and cross-process
coordination. One passing baseline run establishes no measured flakiness rate,
coverage percentage or mutation strength. `commands.json` indexes all receipts;
ordered output parts retain raw command output and hashes without truncation.

The task used one local check process at a time, 120-second per-command limits
and owned process groups for later checks. CPU/address-space limits are not
finite on this host; open-file limits are recorded. No native OMA claim path or
run/session identity was injected. The exclusive baseline ownership requirement
was followed; no mission-state record or `.agents/` definition was changed.

## Limits and handoff

Serena is configured but no Serena tool was available after tool discovery.
The documented fallback used known-path reads and accepted Git path/object
metadata; no native recursive source text/symbol search was substituted.
Global Git status returned code 0 but emitted `av.yaml: Operation not permitted`.
That is an incomplete global inspection, not a clean-worktree proof. Scoped
staged/worktree diffs were empty; scoped untracked product inventory was empty.
No access-denied path was read through another mechanism.

Shell startup also emitted a mise tracked-config symlink warning with
`Operation not permitted`. Toolchain and test processes still succeeded. The
warning is recorded as a prerequisite observation, not silently repaired.

Deferred to the implementation and independent QA owners: perform both module
moves, fill the mapping, compare protected bytes and discovery, run the exact
required fmt/clippy/workspace tests and unchanged harness, and retain all actual
harness failures. No final conformance or harness-pass claim is made here.
