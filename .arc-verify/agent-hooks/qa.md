# Independent agent-hook lane review

**No current blocker reproduced; verification partial.** Skill: `oma-qa`.

## Scope

Baseline `f29db0b5330ba16f3e21f1f504d43d517651a0aa`, tree `0d4a8a9e69041800366b3062429b425069f98924`. Earlier review snapshot `b3514e845fc5b3ee1d12e3a707e882d19a3150b3`, tree `11f9e7d9cff3e38f5bac3a8f0db858afdceb736c`, included an overlay absent from baseline. Current review HEAD/snapshot `981bedfd2a0341d773c1d947c6c1908310549275`, tree `d45e225e0465b6286d4348a3e4ef406bdb9c16a8`, matches the mission's staged tree; its HEAD remains the baseline. Task-evidence refs did not resolve here. Full status reports `av.yaml: Operation not permitted`; scoped status was clean. Serena unavailable. Installer files match Actor snapshot `c5b70a17e290e0cb1431874d8717770665629205`, tree `11375f12c89b6469b29bb4682122bd38a6a49cd3`. Handoffs were read at `250ae908fa08c0d8686a04d04b82c52147a815f8`; four withdrawn recovery files are absent.

## Earlier integration finding

Paths under `crates/arc-cli/src/`; `claude/`, `cursor/` under `agent_hooks/`; `binary/` under `crates/arc-cli/tests/binary/`. Reviewer owns only `qa.md`.

At `b3514e8`, `hooks/event.rs:39-40,64-66` rejected valid `1e400` mutations and all four vendor/literal pairs passed through. At `981bedf`, `hooks/event.rs:38-71,98-123` preserves raw numbers; built-hook probes below confirm rewrites for all four pairs. The mutations are not observed vendor fields.

## Five installer/watchdog dispositions

1. **Settings/symlinks/sidecars — remains.** Cursor checks the owner snapshot (`cursor/installer.rs:261-264`, `cursor/lock.rs:73-86,136-140`); checkpoint (`cursor/installer_tests.rs:100,144`) and symlink tests passed. Failed temporary unlink records ownership for retry/uninstall (`cursor/installer_tests.rs:39`). Injected empty/partial owner writes stop later operations unchanged and name both files and marker for manual resolution (`:184`); automatic recovery was withdrawn.
2. **Other/privileged user then owner — unverified; remains.** Locks use invoking identity (`claude/transaction.rs:23`, `cursor/lock.rs:12`). `/usr/bin/sudo -n -u nobody /usr/bin/id` was denied, exit 127, `operation not permitted`; real two-identity testing is needed.
3. **Owner-denying umask/ACL — partial; remains.** Existing-directory `umask 0777` repeat tests passed. Independently, absent vendor directories became mode `000` and both installs exited 1, unable to open locks. The inherited owner-deny ACL test failed safely with settings unchanged but an unreadable lock (`binary/installer_hardening.rs:312-368`). Other ACLs remain unverified.
4. **Nested watchdog — addressed for tested tree.** `hooks/fixture_owned_installer_tests.rs:5-98` passed: injected expiry kills observed detached helper and descendant and reaps root. Escape before observation or loss of marker remains unverified (`hooks/fixture_owned.rs:189`).
5. **Unwritten matcher/schema forms — bounded; remains manual.** `binary/installer_hardening.rs:162-212` passed: unrelated alternate matchers/extra fields survive; modified marked entries stop. Forms are not exhaustive.

## Installer hardening follow-ups and amendment 21

The 22:15Z ruling classifies concurrent-writer and interrupted-write cases as follow-ups: Cursor settings can change after `cursor/installer.rs:362-364` and before rename at `:411`; owner record after `:373` and before `cursor/lock.rs:165-169`; Claude identity after its check and before unlink (`claude/installer.rs:94,112-123,167-169,425-429`). Symlink/identity checks and publication are separate (`:157-165`). Recovery can require manual resolution; last-check-to-write changes are not always detected.

The 00:50Z ruling also classifies these as follow-ups: extra fields in ARC's Claude identity or Cursor lock record can be lost on rewrite (`claude/installer.rs:144-150,360-397`, `cursor/installer.rs:369-378`); duplicate record keys are not detected (`claude/installer.rs:360-362`, `cursor/lock.rs:143-151`); and a missing Claude marked entry is reinstalled when its identity remains, the intended idempotent behavior (`claude/installer.rs:174-195`). Earlier disposable probes exercised the Claude extra-field, Cursor duplicate-key, and missing-entry cases.

Amendment 21 C10 requires preservation, idempotence, exact ownership or ambiguity stops, and atomic replacement preserving mode/ACL. The Actor reported metadata and idempotence tests passing; C3 and later rulings define the review boundary.

## Vendor and execution-surface evidence

Operator `vendor-docs/README.md` binds pages retrieved 2026-11-18T13:56:18Z to Claude `2.1.327` and cursor-agent `2026.11.13-eacade1`. Recomputed SHA-256 matched the three full hashes and URLs in `contract.md`. All 10 fixtures matched `recorded/{claude-code,cursor}/sanitized` (provenance: `recorded/README.md`); no raw recording was read. Claude accepted complete `updatedInput` without `permissionDecision`; Cursor accepted `updated_input` without `permission`, and later `beforeShellExecution` saw the rewrite. The operator rules neither decides permission, preserving normal evaluation. Sessions used bypass/force and echo rewrites: interactive prompts and live ARC-hook acceptance remain unverified, neither a success criterion.

`recorded/execution-surface/README.md` records zsh, unchanged argv, project cwd and tool-shell PATH distinct from hook PATH; the operator binds this surface for both vendors/literals. Accepted shell differences: cargo/Bun aliases and functions, command hashes, no-shebang executable text, `command_not_found`, `precmd`, `preexec` and DEBUG hooks, and shell lookup options. Other shells, remote/cloud and Windows remain unverified; recognized remote markers pass through.

## Checks and gaps

- Earlier mission `target/debug/arcctl`, SHA-256 `9ba6f70bee7afd3bc61ff307f1018bfc5cee768a6559f46cf9f96859aa13c72c`: CLI/core/IPC sources and manifests matched `b3514e8` and predated the binary. Disposable-workspace probes reproduced the three installer observations, the earlier integration finding and the absent-directory umask result; both recorded literals and top-level `1.25` mutations rewrote for both vendors; `cargo build --release` passed through.
- Existing `target/debug/deps/binary-9110e985c07b74f9`: 10 `--exact` tests passed for owner-write failure, temporary unlink, two owner-snapshot cases, watchdog, settings edits, symlink retarget, matcher variants, umask and ACL. No build, browser or UI check ran. Baseline-to-snapshot `git diff --check` excluding `av.yaml`, three document hashes and 10 sanitized fixture comparisons passed.
- Mission runner `.agent-valley/integration-checks/2026-11-18T22-30-16-770Z.log`: two workspace tests passed (741.3 s, 627.1 s), 22-file Bun passed 370 tests, built-hook/daemon contention tests passed, and four architecture checks passed with coverage skipped. Two coverage collections failed when `llvm-profdata merge` exited via SIGTRAP. Runner ended `INTEGRATION CHECK FAILED` for `final-gates(second-run,duration,traceability,coverage,harness)` despite individually passing workspace runs. An earlier 20:45Z run failed `native_launch_races` at `apps/desktop/src-tauri/tests/memory_pressure/native_sequence.rs:70`; inputs changed since then.

The later runner log `.agent-valley/integration-checks/2026-11-18T23-46-47-265Z.log` reports two passing workspace runs, Bun (371 tests across 22 files), desktop, fresh coverage and the full harness, ending `integration check passed`; its staged product tree matches `981bedf`. Current mission `target/debug/arcctl`, SHA-256 `a95cbf1290c7a72b7bab53f6eefd0364909a7669976b63306fd326b93f55bd8b`, postdates the matched source. Direct non-building probes of both literals for both vendors preserved raw `1e400`, emitted rewrites and omitted permission decisions. `av.yaml` was unreadable; that denial does not block acceptance. No live ARC-hook or interactive approval was verified.
