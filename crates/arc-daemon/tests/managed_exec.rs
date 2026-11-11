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

#[path = "managed_exec/native_python.rs"]
mod native_python;
#[path = "managed_exec/retries.rs"]
mod retries;
#[path = "managed_exec/scenarios.rs"]
mod scenarios;

#[path = "managed_exec/historical.rs"]
mod historical;

#[path = "managed_exec/fail_open.rs"]
mod fail_open;
#[path = "managed_exec/fault.rs"]
mod fault;

#[path = "managed_exec/fallback_pipeline.rs"]
mod fallback_pipeline;
#[path = "managed_exec/fallback_terminal.rs"]
mod fallback_terminal;

#[path = "managed_exec/fallback_descriptors.rs"]
mod fallback_descriptors;

#[path = "managed_exec/fallback_sigstate.rs"]
mod fallback_sigstate;

#[path = "managed_exec/fallback_environment.rs"]
mod fallback_environment;

#[path = "managed_exec/connect_access.rs"]
mod connect_access;

#[cfg(target_os = "macos")]
#[path = "managed_exec/invalid_socket.rs"]
mod invalid_socket;

#[cfg(target_os = "macos")]
#[path = "managed_exec/malformed_endpoint.rs"]
mod malformed_endpoint;

#[cfg(target_os = "macos")]
#[path = "managed_exec/broken_stderr.rs"]
mod broken_stderr;

#[cfg(target_os = "macos")]
#[path = "managed_exec/fallback_sigpipe.rs"]
mod fallback_sigpipe;

#[cfg(target_os = "macos")]
#[path = "managed_exec/full_stderr.rs"]
mod full_stderr;

#[cfg(target_os = "macos")]
#[path = "managed_exec/replaced_endpoint.rs"]
mod replaced_endpoint;
