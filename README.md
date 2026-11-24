# ARC

On macOS, `arcctl exec -- ...` runs commands directly and can use Traffic scheduling and lifecycle tracking when the daemon is available. See [managed execution and agent hook setup](docs/managed-exec.md) for supported commands, installation, removal and recovery.

Claude Code Bash, Codex Bash and Cursor Shell hooks support user and workspace installation. For local events they rewrite exactly `cargo build`, `bun run build`, `cargo test` and `bun run test`, and the few other literals listed in the guide, to ARC managed execution; a Bun command is admitted only when its `build` script is classified as BUILD or its `test` script as TEST. See the guide for setup and the supported execution surface.

Contributor checks: [Rust coverage setup, command and reports](docs/rust-coverage.md).

Codex's local `PreToolUse`/`Bash` hook can be installed alongside Claude Code:

```sh
arcctl hooks install codex --scope user
# Or, for one trusted workspace:
arcctl hooks install codex --scope workspace --workspace /absolute/path/to/project
```

User scope edits `$CODEX_HOME/hooks.json` (the directory must be absolute), or `~/.codex/hooks.json` when `CODEX_HOME` is unset; workspace scope edits `.codex/hooks.json`. Existing hooks are retained. In Codex, open `/hooks` and review and trust the new definition before it can run; a changed definition needs review again. The documented Codex rewrite response uses `permissionDecision: "allow"` with `updatedInput`; ARC does not install a `PermissionRequest` hook. The offline fixture exercises this response and does not demonstrate a live Codex approval prompt or trusted session. See the [official hook contract](https://learn.chatgpt.com/docs/hooks).

Remove the same scope with `arcctl hooks uninstall codex --scope user` or `arcctl hooks uninstall codex --scope workspace --workspace /absolute/path/to/project`. ARC removes its own entry and leaves other hooks intact. If installation reports ambiguous ownership or a changed file, review the named `hooks.json` and its `hooks.json.arc-codex-id` sidecar before retrying. Set `ARC_HOOKS_CODEX_DISABLED=1` (or `ARC_HOOKS_DISABLED=1` for all adapters) to leave commands unchanged. If tracking becomes unavailable after a command is launched, inspect the command and job status before retrying; do not launch another copy just to recover tracking.

Codex attribution is sent only to a daemon advertising Codex support. Older usage-summary readers retain their existing vendor vocabulary: Codex jobs appear in the legacy `none` group, and an attribution section containing Codex is omitted. A current CLI and daemon use the negotiated summary that identifies Codex.
