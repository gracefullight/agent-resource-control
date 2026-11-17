# ARC

On macOS, `arcctl exec -- ...` runs commands directly and can use Traffic scheduling and lifecycle tracking when the daemon is available. See [managed execution and agent hook setup](docs/managed-exec.md) for supported commands, installation, removal and recovery.

Claude Code Bash and Cursor Shell hook installers support user and workspace settings. The production hook handlers currently pass events through unchanged and do not schedule commands. Isolated fixtures exercise proposed rewrites of `cargo build` and one exact Bun build-script invocation described in the guide; live vendor compatibility has not been verified.

Contributor checks: [Rust coverage setup, command and reports](docs/rust-coverage.md).
