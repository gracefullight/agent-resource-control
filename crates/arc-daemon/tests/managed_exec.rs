//! Native lifecycle acceptance through real authenticated IPC and SQLite.
#[path = "managed_exec/cli.rs"]
mod cli;
#[path = "support/mod.rs"]
mod coordination;
#[path = "managed_exec/crash_recovery.rs"]
mod crash_recovery;
#[path = "managed_exec/current_user_ownership.rs"]
mod current_user_ownership;
#[path = "managed_exec/pipeline.rs"]
mod pipeline;
#[path = "managed_exec/protocol.rs"]
mod protocol;
#[path = "managed_exec/reaper_escalation.rs"]
mod reaper_escalation;
#[path = "managed_exec/reaper_fixture.rs"]
mod reaper_fixture;
#[path = "managed_exec/reaper_safety.rs"]
mod reaper_safety;
#[path = "managed_exec/relay.rs"]
mod relay;
#[allow(dead_code)] // Shared fixture helpers also serve older acceptance suites.
#[path = "support/runtime.rs"]
mod runtime;
// The accepted historical client is built and validated independently of the
// per-user daemon. Prepare it before taking the daemon lock so unrelated
// lifecycle cases can run while the offline compatibility fixture is built.
mod support {
    pub fn serial() -> super::coordination::Serial {
        let current = std::thread::current();
        if matches!(
            current.name(),
            Some(
                "cli::spawn_failure_quick_exit_help_and_status_have_static_diagnostics"
                    | "fail_open::historical_arcctl_fails_live_fallback_assertions"
                    | "retries::cli_has_one_child_under_before_after_commit_reply_loss_and_retains_exit17"
            )
        ) {
            super::historical::negative_control_binary();
        }
        super::coordination::serial()
    }
}
#[path = "managed_exec/terminal.rs"]
mod terminal;

#[path = "managed_exec/native_python.rs"]
mod native_python;
#[path = "managed_exec/retries.rs"]
mod retries;
#[path = "managed_exec/scenarios.rs"]
mod scenarios;

#[path = "managed_exec/remote_contract.rs"]
mod remote_contract;

#[path = "managed_exec/remote_ci.rs"]
mod remote_ci;
#[path = "managed_exec/remote_endpoints.rs"]
mod remote_endpoints;
#[path = "managed_exec/remote_unknown.rs"]
mod remote_unknown;

#[path = "managed_exec/capture_endpoints.rs"]
mod capture_endpoints;
#[path = "managed_exec/capture_git.rs"]
mod capture_git;
#[path = "managed_exec/capture_integration.rs"]
mod capture_integration;
#[path = "managed_exec/shared_execution_contract.rs"]
mod shared_execution_contract;

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
