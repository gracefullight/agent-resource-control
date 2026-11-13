//! Explicit desired-state controls. No inversion or mutation replay after loss.
use arc_ipc::{client::Session, governor::*, Request};
use clap::{Args, Subcommand};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Subcommand)]
pub(crate) enum DaemonCommand {
    /// Pause scheduling; running children and measurement continue
    Pause(MutationOptions),
    /// Restore the existing admission rules
    Resume(MutationOptions),
    /// Read the authoritative Governor state and an optional mutation receipt
    Governor {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        mutation_id: Option<String>,
    },
}
#[derive(Debug, Args)]
pub(crate) struct MutationOptions {
    #[arg(long)]
    json: bool,
    /// CAS base for a stable explicit intent; requires --mutation-id
    #[arg(long, requires = "mutation_id")]
    expected_revision: Option<u64>,
    /// Revision-bound identity; requires --expected-revision
    #[arg(long, requires = "expected_revision")]
    mutation_id: Option<String>,
}
fn exchange(socket: &Path, operation: &str, payload: Value) -> Result<Value, String> {
    let mut session = Session::connect_until(
        socket,
        Instant::now() + Duration::from_secs(2),
        &|| false,
        None,
    )
    .map_err(|_| "GOVERNOR_UNAVAILABLE".to_owned())?;
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    session
        .exchange(&request, &|| false)
        .map_err(|_| "GOVERNOR_UNCONFIRMED".to_owned())
}
fn read(
    socket: &Path,
    query: &GetGovernorSwitchRequest,
) -> Result<GovernorSwitchReadResult, String> {
    decode_get_switch(&json!(query)).map_err(str::to_owned)?;
    let value = exchange(socket, OPERATION_GET_SWITCH, json!(query))?;
    decode_switch_read_result(&value, query).map_err(|_| "INVALID_GOVERNOR_REPLY".into())
}
fn show_read(result: &GovernorSwitchReadResult, json_output: bool) {
    if json_output {
        println!("{}", json!(result));
    } else {
        println!(
            "{} · revision {}",
            if result.switch.enabled {
                "Active"
            } else {
                "Paused"
            },
            result.switch.revision
        );
    }
}
pub(crate) fn run(socket: &Path, command: DaemonCommand) -> Result<(), String> {
    let (enabled, options) = match command {
        DaemonCommand::Governor { json, mutation_id } => {
            let query = GetGovernorSwitchRequest {
                mutation_id,
                ..Default::default()
            };
            show_read(&read(socket, &query)?, json);
            return Ok(());
        }
        DaemonCommand::Pause(options) => (false, options),
        DaemonCommand::Resume(options) => (true, options),
    };
    let expected_revision = match options.expected_revision {
        Some(revision) => revision,
        None => {
            read(socket, &GetGovernorSwitchRequest::default())?
                .switch
                .revision
        }
    };
    let mutation_id = match options.mutation_id {
        Some(id) => id,
        None => format!(
            "{expected_revision}-{}",
            arc_core::process_identity::random_job_id().map_err(|_| "LOCAL_PREPARATION_FAILED")?
        ),
    };
    let request = SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled,
        expected_revision,
        mutation_id,
    };
    if !request.is_valid() {
        return Err("INVALID_REQUEST".into());
    }
    let response = exchange(
        socket,
        if enabled {
            OPERATION_RESUME
        } else {
            OPERATION_PAUSE
        },
        json!(request),
    )
    .and_then(|value| {
        decode_mutation_result(&value, &request).map_err(|_| "INVALID_GOVERNOR_REPLY".into())
    });
    let result = match response {
        Ok(result) => result,
        Err(_) => {
            eprintln!("GOVERNOR_UNCONFIRMED\nChange not confirmed. Checking Governor…\nMutation: {}\nExpected revision: {}", request.mutation_id, request.expected_revision);
            let query = GetGovernorSwitchRequest {
                mutation_id: Some(request.mutation_id.clone()),
                ..Default::default()
            };
            let current = read(socket, &query)?;
            // A current-state match alone never attributes a possibly delayed write.
            if let Some(receipt) = &current.mutation_result {
                if receipt.is_valid_for(&request) {
                    show_read(&current, options.json);
                    return Ok(());
                }
            }
            show_read(&current, options.json);
            return Err("GOVERNOR_UNCONFIRMED: retain the mutation identity and use daemon governor --mutation-id for readback".into());
        }
    };
    if matches!(
        result,
        GovernorMutationResult::Committed { replayed: true, .. }
    ) {
        let query = GetGovernorSwitchRequest {
            mutation_id: Some(request.mutation_id.clone()),
            ..Default::default()
        };
        let mut current = read(socket, &query)?;
        // The replay proves a historical outcome, never the current switch.
        // Keep both pieces of evidence and never rebase/resend the old intent.
        current.mutation_result = Some(result);
        if !current.is_valid_for(&query) {
            return Err("INVALID_GOVERNOR_REPLY".into());
        }
        if !options.json {
            let receipt = current.mutation_result.as_ref().unwrap();
            println!(
                "Mutation {} committed previously at revision {}. Current Governor:",
                request.mutation_id,
                receipt.switch().revision
            );
        }
        show_read(&current, options.json);
        return Ok(());
    }
    if options.json {
        println!("{}", json!(result));
    } else {
        println!(
            "{} · revision {}",
            if result.switch().enabled {
                "Active"
            } else {
                "Paused"
            },
            result.switch().revision
        );
    }
    if matches!(result, GovernorMutationResult::Committed { .. }) {
        Ok(())
    } else {
        Err("GOVERNOR_WRITE_REJECTED".into())
    }
}
