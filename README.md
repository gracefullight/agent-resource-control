# ARC

On macOS, `arcctl exec -- ...` runs commands directly and can use Traffic scheduling and lifecycle tracking when the daemon is available. See [managed execution and agent hook setup](docs/managed-exec.md) for supported commands, installation, removal and recovery.

Claude Code Bash and Cursor Shell hooks support user and workspace installation. For local events they rewrite exactly `cargo build`, `bun run build`, `cargo test` and `bun run test`, and the few other literals listed in the guide, to ARC managed execution; a Bun command is admitted only when its `build` script is classified as BUILD or its `test` script as TEST. See the guide for setup and the supported execution surface.

Contributor checks: [Rust coverage setup, command and reports](docs/rust-coverage.md).
