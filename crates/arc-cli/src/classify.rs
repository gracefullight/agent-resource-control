use arc_ipc::{
    Classification, ClassifyRequest, Request, OPERATION_JOB_CLASSIFY, PROTOCOL_VERSION,
    STATUS_ERROR, STATUS_OK,
};
use std::{os::unix::net::UnixStream, path::Path};

pub(crate) fn run(
    socket: &Path,
    command: String,
    shell: Option<String>,
    json: bool,
) -> Result<(), &'static str> {
    let input = ClassifyRequest {
        command,
        shell_kind: shell.unwrap_or_else(|| "sh".into()),
    };
    let mut request = Request::new(super::request_id(), OPERATION_JOB_CLASSIFY);
    request.payload = serde_json::to_value(input).map_err(|_| super::IPC_ERROR)?;
    let mut stream = UnixStream::connect(socket).map_err(|_| super::DAEMON_UNAVAILABLE)?;
    stream
        .set_read_timeout(Some(super::TIMEOUT))
        .map_err(|_| super::DAEMON_UNAVAILABLE)?;
    stream
        .set_write_timeout(Some(super::TIMEOUT))
        .map_err(|_| super::DAEMON_UNAVAILABLE)?;
    let response = arc_ipc::exchange(&mut stream, &request).map_err(|_| super::IPC_ERROR)?;
    if response.protocol_version != PROTOCOL_VERSION {
        return Err(super::PROTOCOL_INCOMPATIBLE);
    }
    if response.request_id != request.request_id {
        return Err(super::IPC_ERROR);
    }
    if response.status == STATUS_ERROR {
        if response.result.is_some() {
            return Err(super::IPC_ERROR);
        }
        return Err(
            match response.error.as_ref().map(|error| error.code.as_str()) {
                Some(arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE) => super::PROTOCOL_INCOMPATIBLE,
                Some(arc_ipc::ERROR_INVALID_REQUEST) => arc_ipc::ERROR_INVALID_REQUEST,
                Some(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) => {
                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
                }
                _ => super::IPC_ERROR,
            },
        );
    }
    if response.status != STATUS_OK || response.error.is_some() {
        return Err(super::IPC_ERROR);
    }
    let result: Classification = serde_json::from_value(response.result.ok_or(super::IPC_ERROR)?)
        .map_err(|_| super::IPC_ERROR)?;
    if !result.is_consistent() {
        return Err(super::IPC_ERROR);
    }
    let value = serde_json::to_value(result).map_err(|_| super::IPC_ERROR)?;
    if json {
        println!("{value}");
    } else {
        for (label, field) in [
            ("Classification", "classification"),
            ("Locality", "locality"),
            ("Reason", "reason"),
            ("Uncertainty", "uncertainty"),
        ] {
            println!(
                "{label}: {}",
                value[field].as_str().ok_or(super::IPC_ERROR)?
            );
        }
        println!("Command not executed.");
    }
    Ok(())
}

/// Static recovery text only; never use the daemon's message or request input.
pub(crate) fn recovery(code: &str) -> &'static str {
    match code {
        arc_ipc::ERROR_INVALID_REQUEST => "Pass exactly one command string.",
        super::PROTOCOL_INCOMPATIBLE => "Use matching arcctl and daemon protocol versions.",
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY => "Use a daemon that supports job.classify.",
        super::DAEMON_UNAVAILABLE => "Start the ARC daemon, then submit again.",
        _ => "Check the daemon connection, then submit again.",
    }
}
