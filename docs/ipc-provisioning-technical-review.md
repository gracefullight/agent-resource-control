# IPC provisioning technical assessment

Status: completed assessment; behavioral verification remains partial. Review result: **FAIL for the inspected baseline**, based on the source-level preservation violation below. This is input to implementation and independent QA, not acceptance of a repair.

## Scope and inspected revision

Used `oma-qa` and the installed `.agents/agents/qa-reviewer.md` definition. Only this report is owned and changed. HEAD was `e4adbb2ebd79ff21b2ffe2b2592c66b8d43951ea` (`AV task snapshot task-2`), immediately after accepted `f6971037ed1c3524f88e10732e8910909a4facd3`. The baseline-to-HEAD diff excluding inaccessible `av.yaml` was empty; the working-tree status with that exclusion was also empty before this report. References below describe those inspected bytes, not a later engineer revision.

Serena is configured in `.agents/oma-config.cue`, but discovery exposed no Serena/search tools. Review used scoped reads of the source paths already supplied by the assignment, not native project-source search. Unrestricted Git status/diff emitted `av.yaml: Operation not permitted`; no attempt was made to bypass it. `.agents/oma-config.yaml` was absent; the CUE configuration was readable.

## Source findings and initialization boundary

### HIGH — existing directories and symlink targets can be modified

`crates/arc-daemon/src/server.rs:48-49` executes `create_dir_all(parent)` and then unconditional `set_permissions(parent, 0700)`. Thus an existing 0755/0770 parent is changed rather than rejected. Directory symlinks are followed by these operations. No effective-UID check appears in `prepare_socket_directory` (`server.rs:29-50`). This is a confirmed static violation; no daemon reproduction was run in this assessment.

Remediation shape, not drop-in implementation:

```rust
// Check the directory entry without following its final symlink.
// Validate the traversal separately; final-component checks alone are insufficient.
let metadata = fs::symlink_metadata(parent)?;
validate_directory(&metadata, unsafe { libc::geteuid() })?;
// Existing entries: no chmod, chown, deletion, or content repair.
```

For missing entries, create privately and validate the resulting entry as described below. Remove the unconditional directory chmod; retain the distinct socket chmod at `server.rs:74`.

### Existing ordering to retain

`crates/arc-daemon/src/main.rs:35-40` resolves the socket, prepares its directory, then acquires `InstanceLock`. `main.rs:20-26` opens SQLite only after that succeeds. Database-path resolution at `main.rs:15` is configuration lookup, not initialization. Actual storage mutation begins with parent creation and `Connection::open` at `crates/arc-storage/src/lib.rs:34-40`, followed by WAL setup at lines 43-46. Unsafe socket directories must return before any lock acquisition or these storage operations.

`crates/arc-daemon/src/instance.rs:69-76` already uses `O_NOFOLLOW` for the lock file itself. The acquisition guard opens its directory using only `O_DIRECTORY` at lines 123-126; that does not protect ancestor traversal or prevent a directory symlink. Existing inode comparison before stale-lock removal at lines 145-158 protects that lock-file operation and is not directory-provisioning protection. Preserve the lock lifecycle and stale-holder semantics.

## Directory and path policy

| Boundary | Required treatment |
| --- | --- |
| Existing final socket directory | Require a real directory, effective-UID ownership and mode 0700. Inspect with no-follow metadata; reject 0755, 0770, foreign UID and other file types without mutation. Compare permission/special bits explicitly rather than allowing special bits accidentally. |
| Missing ARC directory components | Create each with requested mode 0700, then verify type, UID and effective mode. Every newly created nested component must be private. |
| Ordinary existing ancestors | Do not require every ancestor to be owned by ARC's user or mode 0700; do not chmod them. This preserves normal home and temporary-root traversal. |
| Symlink at the ARC directory or relevant intermediate entry | Reject, including dangling links and links to otherwise private targets, before creating children or opening the lock/database. `exists()` follows links and is unsuitable for deciding that an entry is absent. |
| Platform temporary-root aliases | Preserve established system aliases needed by the platform default, such as macOS temporary-root prefixes. A blanket ban on every symlink in an absolute path breaks that compatibility. Scope any exception to the established platform prefix; do not exempt arbitrary user-created symlinks just because `TMPDIR` or an override points through them. |

Socket resolution remains owned by `crates/arc-core/src/lib.rs:11-17`: a nonempty `ARC_SOCKET_PATH` wins, otherwise `env::temp_dir()/arc/arc.sock`. Do not silently fall back, change the override, or canonicalize the entire supplied path and thereby erase evidence of a redirected ARC entry. The lexical shared-root guard at `server.rs:53-55` alone cannot establish resolved identity; aliases and `.`/`..` must not bypass it. Preserve the refusal to use the shared temporary root itself as the ARC private directory.

Use `MetadataExt::uid()` and the effective UID (`libc::geteuid()`), not an environment username or real UID. GID is evidence to preserve, not an additional restriction on a valid owned 0700 directory.

## Creation, races and identity

Use existing standard Unix filesystem APIs and the declared `libc` dependency. `DirBuilderExt::mode(0o700)` supplies a private creation mode without a world-accessible interval followed by chmod. Umask can remove owner permissions; validate the actual result. If exact 0700 must be restored for an unusually restrictive mask, any permission operation must be confined to a verified descriptor for an entry this attempt actually created, never an `AlreadyExists` entry. Do not change process-global umask during daemon startup.

Distinguish absence from all other errors. Attempt creation without an `exists()` ownership decision. On `AlreadyExists`, inspect and validate the winning entry as existing, even if this process previously observed absence. Never infer ownership of a raced entry from an earlier lookup or repair its mode. Propagate permission and I/O errors. Leave existing entries and targets intact on every failure; do not perform recursive cleanup in production.

No-follow must cover relevant intermediate components as well as the final parent. Checking only `symlink_metadata(parent)` or using `O_NOFOLLOW` on a multi-component open does not cover intermediate links. For a component walk, directory-relative opens with `O_DIRECTORY | O_NOFOLLOW` and descriptor-relative creation through existing `libc` can anchor traversal without a filesystem dependency. Compare descriptor metadata `(dev, ino)` with the validated directory identity where a pathname is reopened for use. Do not normalize `..` lexically across an unchecked link.

**Unresolved verification question:** `prepare_socket_directory` currently returns `()` and subsequent lock, database and socket operations reopen pathnames. A preflight check alone is not proof against arbitrary concurrent ancestor replacement. The final reviewer must examine the implemented descriptor/identity strategy and any remaining check-to-use window. No concurrent exploit or race resistance was measured here. If an identity handoff requires an edit outside the engineer's assigned paths, report the exact path to the Chief for ownership coordination; this report creates no new public interface contract.

## Required deterministic evidence

Use a table-driven unit matrix plus the existing daemon/CLI subprocess harness, rather than a second lifecycle suite.

| Case | Evidence required from the engineer/final reviewer |
| --- | --- |
| Existing 0755 and 0770 | Unit and daemon rejection; compare before/after device/inode, mode, UID/GID, sorted entries and sentinel bytes. No readiness, new lock, database, WAL, SHM or socket. |
| Final and intermediate symlinks | Cover links to 0700 and permissive targets, dangling links and an intermediate link with missing descendants. Snapshot link identity/readlink plus target identity, mode, owner, entries and contents; rejection must leave both unchanged and create no descendant through the link. |
| Foreign ownership | Unit-test a small metadata-policy helper with a non-effective UID, without requiring privileged chown. This proves the policy branch, not an actual foreign-owned daemon run; state that distinction. Confirm by code review that this policy executes before lock/database access. |
| Missing and nested directories | Each newly created component ends at 0700; existing ordinary ancestor remains unchanged. Exercise a permissive child-process umask. Any additional restrictive-mask behavior must be explicit and tested without changing the parallel test runner's umask. |
| Creation loses an AlreadyExists race | Inject a deterministic interleaving/test seam or directly exercise the creation-result handler with safe, unsafe and symlink winners. Verify existing-winner validation and preservation; a timing-only stress loop is insufficient. |
| Existing owned 0700 | Startup, 0600 socket, status/doctor, duplicate rejection and graceful shutdown/reopen remain successful. Sentinel content and directory identity/mode/UID/GID remain intact; expected daemon artifacts are allowed on successful startup. |
| Resolution/platform aliases | Isolate child `TMPDIR`, remove inherited `ARC_SOCKET_PATH` for default coverage, then test explicit overrides. A supported platform temporary-root alias works, while an ARC link beneath it is rejected. Use short Unix socket paths. |

Set valid fixtures explicitly to 0700: `crates/arc-daemon/tests/resident.rs:45` and `tests/startup.rs:25` currently rely on ambient creation permissions. Preserve the latter's migration-checksum failure assertion at lines 61-92 so it still reaches SQLite. Existing resident assertions cover 0600/0700 at lines 229-238, status/doctor at 282-302 and single-instance behavior at 304-318; reuse them. For unsafe cases, point `ARC_DB_PATH` at an absent location outside the rejected tree as well, proving early rejection does not initialize storage elsewhere. Snapshot before starting the daemon and again after CLI checks. `arcctl` must still report unavailable through its existing connection path (`crates/arc-cli/src/main.rs:96-106`).

## Stack, interfaces and command evidence

Keep the five crates (`Cargo.toml:1-8`), Tokio Unix listener, SQLite initialization, native locks and existing `libc` integration (`Cargo.toml:20-25`; `crates/arc-daemon/Cargo.toml:6-12`). Keep Rust 1.95.0 and its rustfmt/clippy components (`rust-toolchain.toml:1-4`). No new package, dependency, toolchain, general filesystem framework or unrelated architecture change is justified. Keep daemon/CLI outputs and error exits compatible; directory rejection should name the path and reason, state that the existing entry was left unchanged, and direct users to a separate private `ARC_SOCKET_PATH` directory. Preserve single-line sanitization (`main.rs:43-58`).

| Command/check performed here | Outcome |
| --- | --- |
| `git status --short -- . ':!av.yaml'` | Exit 0; initially empty. |
| `git diff --stat f6971037ed1c3524f88e10732e8910909a4facd3 HEAD -- . ':!av.yaml'` | Exit 0; empty. |
| `git log -4 --oneline` / `git rev-parse HEAD` | Exit 0; revision lineage recorded above. |
| Scoped `cat`, `sed`, `nl` reads | Source and skill evidence acquired; combined YAML configuration read exited 1 because that optional file was absent; CUE read succeeded. |
| Installed pinned `.../1.95.0-aarch64-apple-darwin/bin/rustc --version` | Exit 0: `rustc 1.95.0 (59807616e 2026-04-14)`. |
| Installed pinned `.../1.95.0-aarch64-apple-darwin/bin/cargo --version` | Exit 0: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`. |
| Report readback/content assertions through `/usr/bin/python3` | Exit 0; required topics, minimum size, final newline and absence of trailing whitespace verified. |
| `git diff --check -- docs/ipc-provisioning-technical-review.md` | Exit 0; the report is untracked, so the explicit readback above supplies its whitespace check. Final scoped Git status showed only this new report. |

No Cargo suite, linter, build, daemon run, browser check or installation was performed. This task explicitly avoids duplicating full-suite verification. Final acceptance still requires the unchanged commands, with the installed pinned toolchain on PATH: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Version probes do not establish those passes. Runtime behavior, cross-platform compatibility and race handling remain untested conclusions until the engineer and independent reviewer supply current evidence. Cost/latency savings from reuse are recommendations, not measured results.

## Assignment acceptance checklist

- [x] Source-backed trust boundaries, initialization ordering, creation races, identity risks and regression evidence documented.
- [x] Default-path/platform compatibility and the existing stack preserved in the recommendation.
- [x] Static findings, observed command outputs and untested runtime conclusions distinguished.
- [x] Only `docs/ipc-provisioning-technical-review.md` produced; product code, manifests, task instructions and mission state untouched.
