# ARC

On macOS, `arcctl exec -- ...` runs commands directly and can use Traffic scheduling and lifecycle tracking when the daemon is available. See [managed execution and agent hook setup](docs/managed-exec.md) for supported commands, installation, removal and recovery.

Claude Code Bash and Cursor Shell hooks support user and workspace installation. For local events they rewrite exactly `cargo build` and `bun run build` to ARC managed execution; Bun admission depends on its build script being classified as BUILD. See the guide for setup and the supported execution surface.

Contributor checks: [Rust coverage setup, command and reports](docs/rust-coverage.md).
