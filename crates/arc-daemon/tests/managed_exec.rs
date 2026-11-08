//! Native lifecycle acceptance through real authenticated IPC and SQLite.
#[path = "managed_exec/cli.rs"]
mod cli;
#[path = "managed_exec/pipeline.rs"]
mod pipeline;
#[path = "managed_exec/protocol.rs"]
mod protocol;
#[allow(dead_code)] // Shared fixture helpers also serve older acceptance suites.
#[path = "support/runtime.rs"]
mod runtime;
mod support;
#[path = "managed_exec/terminal.rs"]
mod terminal;

#[path = "managed_exec/retries.rs"]
mod retries;
#[path = "managed_exec/scenarios.rs"]
mod scenarios;
