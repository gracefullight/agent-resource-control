//! One bounded authenticated read; never launches work or fabricates a snapshot.
use arc_core::meter::{MeterSnapshot, MAX_SNAPSHOT_BYTES};
use arc_ipc::client::Failure;
use clap::Subcommand;
use std::{
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Subcommand)]
pub(crate) enum MeterCommand {
    /// Read current host and ARC daemon measurements (no job/vendor attribution)
    Snapshot {
        /// Emit one typed snapshot with units, observation times and availability
        #[arg(long, required = true)]
        json: bool,
    },
}

pub(crate) fn invalid() -> std::process::ExitCode {
    eprintln!("INVALID_REQUEST\nUse arcctl meter snapshot --json.");
    std::process::ExitCode::from(2)
}
pub(crate) fn run(socket: &Path) -> Result<(), String> {
    let snapshot = arc_ipc::meter::snapshot_until(
        socket,
        super::request_id(),
        Instant::now() + Duration::from_secs(2),
        &|| false,
    )
    .map_err(|error| failure(error).to_owned())?;
    let bytes = render(&snapshot).map_err(str::to_owned)?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|_| "IPC_ERROR\nCould not write the Meter snapshot.".to_owned())
}
fn render(snapshot: &MeterSnapshot) -> Result<Vec<u8>, &'static str> {
    if !snapshot.is_valid() {
        return Err("IPC_ERROR\nInvalid Meter reply.");
    }
    let bytes = serde_json::to_vec(snapshot).map_err(|_| "IPC_ERROR\nInvalid Meter reply.")?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("IPC_ERROR\nInvalid Meter reply.");
    }
    Ok(bytes)
}
fn failure(error: Failure) -> &'static str {
    match error {
        Failure::Unsupported => "UNSUPPORTED_CAPABILITY\nThis daemon does not support meter.snapshot. Use a daemon with Meter support.",
        Failure::Unavailable | Failure::Uncertain | Failure::Busy => "METER_UNAVAILABLE\nMeter is unavailable. Check the daemon connection and try another snapshot.",
        Failure::Protocol => "PROTOCOL_INCOMPATIBLE\nUse matching arcctl and daemon protocol versions.",
        Failure::InvalidEndpoint => "IPC_ERROR\nThe daemon endpoint could not be authenticated.",
        Failure::Cancelled => "CANCELLED\nThe Meter read was cancelled.",
        _ => "IPC_ERROR\nThe daemon did not return a valid Meter snapshot.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parsing_accepts_only_the_fixed_json_read_without_client_selected_scope() {
        assert!(matches!(
            super::super::Cli::try_parse_from(["arcctl", "meter", "snapshot", "--json"])
                .unwrap()
                .command,
            super::super::Command::Meter {
                command: MeterCommand::Snapshot { json: true }
            }
        ));
        for args in [
            vec!["meter"],
            vec!["meter", "snapshot"],
            vec!["meter", "snapshot", "--json", "--pid", "1"],
            vec!["meter", "snapshot", "--json", "/"],
            vec!["meter", "history", "--json"],
        ] {
            assert!(
                super::super::Cli::try_parse_from(std::iter::once("arcctl").chain(args)).is_err()
            );
        }
    }
    #[test]
    fn serialization_preserves_unknowns_real_zero_units_and_bounded_shape() {
        let mut snapshot: MeterSnapshot = serde_json::from_str(include_str!(
            "../../arc-core/tests/fixtures/meter-snapshot.json"
        ))
        .unwrap();
        let bytes = render(&snapshot).unwrap();
        assert!(bytes.len() < MAX_SNAPSHOT_BYTES);
        assert_eq!(
            arc_ipc::meter::decode_snapshot(&serde_json::from_slice(&bytes).unwrap()).unwrap(),
            snapshot
        );
        snapshot.host.cpu_interval = None;
        snapshot.host.cpu_utilization.reading = arc_core::meter::Reading::FirstInterval {};
        snapshot.host.swap_used_bytes.reading = arc_core::meter::Reading::Available { value: 0 };
        let wire: serde_json::Value = serde_json::from_slice(&render(&snapshot).unwrap()).unwrap();
        assert_eq!(
            wire["host"]["cpu_utilization"]["reading"],
            serde_json::json!({"availability":"first_interval"})
        );
        assert_eq!(wire["host"]["swap_used_bytes"]["reading"]["value"], 0);
        snapshot.schema_version = 999;
        assert!(render(&snapshot).is_err());
    }
    #[test]
    fn static_errors_do_not_invent_zero_values_or_echo_peer_diagnostics() {
        for error in [
            Failure::Unavailable,
            Failure::Uncertain,
            Failure::Busy,
            Failure::Rejected,
            Failure::Protocol,
            Failure::InvalidReply,
            Failure::InvalidEndpoint,
            Failure::Local,
            Failure::Witness,
            Failure::Cancelled,
            Failure::Unsupported,
        ] {
            let text = failure(error);
            assert!(!text.is_empty());
            assert!(!text.contains("{\""));
        }
        assert!(failure(Failure::Unsupported).starts_with("UNSUPPORTED_CAPABILITY\n"));
    }
}
