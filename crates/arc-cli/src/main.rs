//! `arcctl`: a two-command client for the resident ARC daemon.
//!
//! It dials the owner-only Unix socket and never opens SQLite itself.

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
    /// Report daemon liveness and protocol
    Status,
    /// Run local daemon diagnostics
    Doctor,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{code}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), &'static str> {
    let socket = arc_core::socket_path().map_err(|_| DAEMON_UNAVAILABLE)?;
    match command {
        Command::Status => status(&socket),
        Command::Doctor => doctor(&socket),
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

fn doctor(socket: &std::path::Path) -> Result<(), &'static str> {
    let response = request(socket, OPERATION_DOCTOR)?;
    if response.status != STATUS_OK || response.protocol_version != PROTOCOL_VERSION {
        return Err(PROTOCOL_INCOMPATIBLE);
    }
    let result = response.result.ok_or(INTEGRITY_FAILED)?;
    if result["integrity_check"].as_str() != Some("ok")
        || result["schema_migrations"].as_i64() != Some(1)
    {
        return Err(INTEGRITY_FAILED);
    }
    println!(
        "ok. IPC owner-only. Tray not connected. Vendors not detected. \
         Command classification not checked. Sweeper off."
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
