//! `arcctl`: diagnostics and literal command classification over owner-only IPC.
//!
//! It dials the owner-only Unix socket and never opens SQLite itself.

mod classify;
mod doctor;
mod exec;
mod job;
mod meter;

use arc_ipc::{
    self, Request, Response, OPERATION_DOCTOR, OPERATION_HELLO, PROTOCOL_VERSION, STATUS_OK,
};
use clap::{Parser, Subcommand};
use std::{
    os::unix::net::UnixStream,
    process::ExitCode,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DAEMON_UNAVAILABLE: &str = "DAEMON_UNAVAILABLE";
const PROTOCOL_INCOMPATIBLE: &str = "PROTOCOL_INCOMPATIBLE";
const INTEGRITY_FAILED: &str = "INTEGRITY_FAILED";
const IPC_ERROR: &str = "IPC_ERROR";
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Parser)]
#[command(
    name = "arcctl",
    version,
    about = "Inspect the resident ARC daemon. Stop it with SIGTERM.",
    color = clap::ColorChoice::Never,
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect host resources, daemon overhead or one persisted job summary
    Meter {
        #[command(subcommand)]
        command: meter::MeterCommand,
    },
    /// Run a program and its arguments directly.
    #[command(override_usage = "arcctl exec -- <PROGRAM> [ARG...]")]
    #[command(long_about = "Run a program and its arguments directly.
Fallback applies to tracking outages. Invalid protocol replies and local preparation failures stop execution before a child starts.")]
    #[command(after_help = "Examples:
  arcctl exec -- cargo test
  arcctl exec -- bun test
  arcctl exec -- printf '%s\\n' 'two words'
  arcctl exec -- '/path/with spaces/tool' '' '--help' '|'

Your shell handles quoting before ARC receives arguments.
ARC does not interpret pipes, redirections, substitutions or shell operators.
If the daemon is unavailable or tracking is unconfirmed due to an outage or exhausted DB_BUSY retries, ARC still attempts the command once.
Each admission exchange has a bounded outage budget; a healthy queued job waits for a permit.
Direct local cargo build, cargo test, cargo clippy, cargo fmt and one-shot bun test jobs use the default Traffic windows and resource reservations when the daemon supports them. Bun watch, hot and unresolved forms bypass scheduling.
Cancellation, invalid arguments, invalid protocol replies, local preparation failures and explicit authenticated rejection do not start a child.
Tracking diagnostics go to stderr; the child keeps its native streams, exit status and terminal behavior.
Stopping the daemon does not stop the running child.
Use arcctl job status to list last-known lifecycle records.
Use arcctl job status <JOB_ID> to observe a record after daemon recovery; this may update the lifecycle record.
Use arcctl meter summary --job <JOB_ID> --json for persisted CPU used, run elapsed, queue wait and accounting quality.
A reference does not confirm a saved record. A missing record does not show whether the command ran.
Daemon recovery never replays the command. Do not launch another copy to recover tracking.
Desktop integration, native Windows acceptance, vendor hooks and other scheduling policies remain outstanding.")]
    Exec {
        #[arg(required = true, num_args = 1.., last = true, value_name = "PROGRAM")]
        argv: Vec<std::ffi::OsString>,
    },
    /// Inspect persistent lifecycle records without launching work
    Job {
        #[command(subcommand)]
        command: JobCommand,
    },
    /// Report daemon liveness and protocol
    Status,
    /// Check database integrity and schema compatibility through the daemon
    #[command(
        after_help = "If the daemon failed before startup, inspect its stderr for the cause."
    )]
    Doctor,
    /// Analyze literal command text without executing it
    #[command(
        after_help = "Pass the entire command as one quoted argument. In POSIX shells, single quotes preserve literal text. Your shell processes quoting and substitutions before ARC receives input. Command text is analyzed, not executed.

Examples:
  arcctl classify 'cargo test'
  arcctl classify --json 'gh workflow run ci.yml'
  arcctl classify 'cargo test && gh workflow run ci.yml'
  arcctl classify 'cargo test -- \"gh workflow run ci.yml\"'
  arcctl classify --shell bash -- 'cargo test'
  arcctl classify -- ''
  arcctl classify --shell powershell -- 'cargo test'"
    )]
    Classify {
        /// Shell syntax subset (sh, bash, zsh); other values return UNKNOWN
        #[arg(long)]
        shell: Option<String>,
        /// Emit only the typed four-field JSON result
        #[arg(long)]
        json: bool,
        /// Entire command as one literal UTF-8 argument
        command: String,
    },
}

#[derive(Debug, Subcommand)]
enum JobCommand {
    /// Show one job or the 50 most recent last-known records
    Status { job_id: Option<String> },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            use clap::error::ErrorKind;
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp
                    | ErrorKind::DisplayVersion
                    | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) {
                let success = !error.use_stderr();
                let _ = error.print();
                return if success {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                };
            }
            if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("exec")) {
                return exec::invalid();
            }
            if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("job")) {
                return job::invalid();
            }
            if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("meter")) {
                return meter::invalid();
            }
            // Preserve legacy usage errors outside the new classification command.
            if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new("classify")) {
                error.exit();
            }
            // Clap's default unknown-argument errors echo the supplied value.
            eprintln!(
                "INVALID_REQUEST\nPass exactly one command string. Use arcctl classify --help."
            );
            return ExitCode::from(2);
        }
    };
    let command = match cli.command {
        Command::Exec { argv } => return exec::run(argv),
        Command::Job {
            command: JobCommand::Status { job_id },
        } => return job::run(job_id),
        command => command,
    };
    let classification = matches!(&command, Command::Classify { .. });
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{code}");
            if classification {
                eprintln!("{}", classify::recovery(&code));
            }
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    let socket = arc_core::socket_path().map_err(|_| DAEMON_UNAVAILABLE)?;
    match command {
        Command::Exec { .. } | Command::Job { .. } => unreachable!("handled before diagnostics"),
        Command::Status => status(&socket).map_err(str::to_owned),
        Command::Meter {
            command: meter::MeterCommand::Snapshot { .. },
        } => meter::run(&socket),
        Command::Meter {
            command: meter::MeterCommand::Summary { job, .. },
        } => meter::summary(&socket, &job),
        Command::Doctor => doctor::run(&socket),
        Command::Classify {
            command,
            shell,
            json,
        } => classify::run(&socket, command, shell, json).map_err(str::to_owned),
    }
}

fn status(socket: &std::path::Path) -> Result<(), &'static str> {
    let response = request(socket, OPERATION_HELLO)?;
    if response.status != STATUS_OK || response.protocol_version != PROTOCOL_VERSION {
        return Err(PROTOCOL_INCOMPATIBLE);
    }
    let result = response.result.ok_or(PROTOCOL_INCOMPATIBLE)?;
    if result["protocol_version"].as_u64() != Some(u64::from(PROTOCOL_VERSION)) {
        return Err(PROTOCOL_INCOMPATIBLE);
    }
    println!(
        "running. Socket: {}. Protocol: 1. Sweeper off.",
        socket.display()
    );
    Ok(())
}

fn request(socket: &std::path::Path, operation: &str) -> Result<Response, &'static str> {
    let mut stream = UnixStream::connect(socket).map_err(|_| DAEMON_UNAVAILABLE)?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|_| DAEMON_UNAVAILABLE)?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|_| DAEMON_UNAVAILABLE)?;

    let request = Request::new(request_id(), operation);
    let response = arc_ipc::exchange(&mut stream, &request).map_err(|_| DAEMON_UNAVAILABLE)?;
    if response.protocol_version != PROTOCOL_VERSION {
        return Err(PROTOCOL_INCOMPATIBLE);
    }
    if response.request_id != request.request_id {
        return Err(IPC_ERROR);
    }
    if operation == OPERATION_DOCTOR {
        return Ok(response);
    }
    if let Some(error) = &response.error {
        return Err(match error.code.as_str() {
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE => PROTOCOL_INCOMPATIBLE,
            arc_ipc::ERROR_INTEGRITY_FAILED => INTEGRITY_FAILED,
            _ => IPC_ERROR,
        });
    }
    Ok(response)
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("arcctl-{}-{nanos}", std::process::id())
}
