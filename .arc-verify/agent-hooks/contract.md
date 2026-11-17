# Agent hook contract — shared handoff

The installers are connected; production handlers pass through. Live compatibility is unverified. Specifications: [Claude hooks](https://code.claude.com/docs/en/hooks), [Cursor hooks](https://cursor.com/docs/hooks).

## Entrypoints and ownership

`arcctl hooks run claude|cursor` reads at most 1 MiB of JSON from stdin. Claude's no-change response is no output and exit 0. Cursor's current no-change handler returns no output and exit 1: Cursor documents nonzero exits other than 2 as fail-open; installations must not set `failClosed`. Invalid/oversized input takes the same path. Cursor says invalid/schema-invalid JSON on a permission hook blocks; do not emit speculative pass-through JSON. A rewrite emits exactly one JSON object on stdout and exits 0. No hook starts the requested command.

`arcctl hooks install|uninstall claude|cursor --scope user|workspace [--workspace ABSOLUTE_DIR]` dispatches installers. Workspace requires an existing absolute directory; user scope forbids `--workspace`. Vendor `configure(action, scope, workspace, context)` owns implementation; `context.executable` is the current arcctl path. User scope resolves config under isolated HOME (including Claude's `CLAUDE_CONFIG_DIR`); workspace uses the explicit directory. Install and uninstall preserve unrelated settings and hook order, remove only their own entry, and are repeatable. Test isolated configs.

Shared: `crates/arc-cli/src/hooks.rs`, `exec.rs`, `exec/run.rs`, `exec/native_environment.rs`, `main.rs`, `crates/arc-ipc/src/client.rs`, common fixtures and focused tests. Claude owns `crates/arc-cli/src/agent_hooks/claude.rs` and its test/config fixtures; Cursor owns `crates/arc-cli/src/agent_hooks/cursor.rs` and its test/config fixtures. Neither edits shared files, the other adapter, manifests or lockfiles. Lane 1 owns performance/reliability and coverage, including `crates/arc-daemon/tests/support/`, selected test fixtures, `scripts/coverage/` and hygiene tests. The runner owns integration, desktop/bridge, traceability, verification, coverage and harness. Report shared-file needs to the Chief.

## Event, proof and rewrite interfaces

`handle(&Value, &HookContext) -> Option<Value>` is the production entry. `handle_verified(&Value, &HookContext, Option<&VerifiedSurface>) -> Option<Value>` exposes the transformation; `None` means no change. Fixtures can source-include `hooks.rs` and call `#[cfg(test)] dispatch_verified`, as in `tests/binary.rs`; production has no proof bypass. `VerifiedSurface` carries the command shell's PATH; constructing it is an evidence-backed assertion, not proof. `rewrite_literal(command, context, Some(surface))` applies shared checks.

The only accepted UTF-8 text is exactly `cargo build` or `bun run build`. Quotes, extra whitespace, options, assignments, operators, redirections, newlines, substitutions, path-qualified tools and already wrapped commands are unsupported. The wrapper is a safely single-quoted absolute arcctl path, `hook-exec --`, and the original literal. Without running a program, check arcctl and the PATH-resolved tool are executable regular files and every PATH component is absolute. Pass through for disabled adapters, unavailable executables, relative PATH, unknown events or malformed input. `ARC_HOOKS_DISABLED=1` disables both; `ARC_HOOKS_CLAUDE_DISABLED=1` and `ARC_HOOKS_CURSOR_DISABLED=1` disable their respective adapter.

Before supplying `VerifiedSurface`, establish for the actual call: local macOS, enabled integration, shell PATH and cwd, matching `_` behavior, and no altered resolution from startup, imported functions, aliases, hashes or functions for either tool or wrapper. An environment marker, absent `CURSOR_CODE_REMOTE`, workspace location or hook process shell is not proof. Cursor project hooks also run in cloud agents; `CURSOR_CODE_REMOTE=true` passes through. Unverified locality or resolution passes through. A fixture can use fresh non-login `/bin/sh -c`, no startup variables or imported functions, an absolute PATH and the transformed literal. This proves only that invocation.

`arcctl hook-exec -- cargo build` and `arcctl hook-exec -- bun run build` validate the exact argv before entering the existing `exec` path. Direct `arcctl exec` retains its legacy-lineage behavior. The hook route preserves environment bytes, including absent, empty, non-UTF-8 and supplied `ARC_TRAFFIC_LINEAGE`. When `_` names the wrapper, it restores the PATH-resolved tool spelling used by supported shells. Both routes retain cwd, standard descriptors, exit handling, daemon outage budget, refusal behavior and owned-child lifecycle. ARC queue/outage diagnostics may add stderr text; preserve the child's streams. Do not re-run on uncertain recording. The detector sees native Cargo/Bun argv. Later contention tests must use a `bun run build` project whose bounded resolver classifies it as BUILD.

## Vendor event and response fixtures

The Git-visible common fixtures are `claude-pre-tool-use.json` and `cursor-pre-tool-use.json` under `crates/arc-cli/tests/fixtures/agent_hooks/`. The Cursor event includes `model`, `user_email` and `transcript_path`. Adapters match only `PreToolUse`/`Bash` and `preToolUse`/`Shell`; they clone the entire tool input and change only `command`. Claude's proposed response is `{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{...}}}` with no permission decision; Claude may evaluate permissions on changed input and auto mode may reject it. Cursor documents `permission` (`allow` or `deny`) on `preToolUse`; an `allow` must not be assumed to preserve the normal approval flow, and invalid JSON can block. **Cursor production must pass through** until a supported runtime establishes an accepted rewrite response that preserves normal approval, as well as a local surface proof. A provisional `{"updated_input":{...}}` response without permission may be tested only as a fixture transformation; it is not an approved production response. Unsupported, remote, disabled, unverified or repeated inputs produce no update.

Test both literals, malformed or unsupported events, unknown-field preservation, no permission grant or double wrap, unavailable executables, disabling, uncertain shell/locality and no hook-side execution. Install/remove tests use independent temporary homes and workspaces; passing cases clean their own directory, failures retain it. Vendor lifecycle, session attribution, broader commands and Windows remain open.

## Evidence boundary

Skill: `oma-backend`; Serena unavailable. Vendor contention is fixture-level, not live compatibility.

Production lineage tests include a poisoned environment; queue tests observe attachment and one execution. Notice-writer and IPC deadline gaps remain.

Recent-status repair: a test-only macOS clock interposer drives the actual `arcctl job status` against the production daemon. The 50 outcomes and a two-record, one-lookup deadline passed individually; final runner checks remain pending.

The controlled daemon and execution fixtures establish a 5-second queue interval and a separate 7-second native interval. A separate synchronized production `arc-daemon`/`arcctl` queue case observes attachment before release, zero queued starts, one execution per child, completed queue entries and matching SQLite, CLI and registered-bridge reports. Its initial nonblocking readiness read failed on macOS; setting the accepted socket to blocking fixed it.

Prior focused checks and runner results are historical; final-input workspace, coverage, harness and integration checks belong to the runner.

## Outstanding scope

The prior runner workspace run took 1,047.615 seconds against the under-360-second goal; speed is deferred. Production handlers pass through, so the two literal rewrites are not delivered. Cursor's approval-preserving rewrite response remains unproven. Vendor lifecycle, session attribution, broader commands and Windows remain open.
