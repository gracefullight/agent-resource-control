use arc_ipc::{
    locality::{ClassifyLocalityRequest, Command, OPERATION_CLASSIFY},
    Classification, ClassifyRequest, JobClass, Locality, Reason, Request, Uncertainty,
    OPERATION_JOB_CLASSIFY, PROTOCOL_VERSION, STATUS_ERROR, STATUS_OK,
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
    let context = super::exec::capture_shell(&input);
    let result = classify(socket, &input, context, arc_ipc::locality::now_ms)?;
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

fn classify(
    socket: &Path,
    input: &ClassifyRequest,
    context: Option<arc_detector::EndpointContext>,
    now_ms: impl Fn() -> Option<u64>,
) -> Result<Classification, &'static str> {
    let mut stream = UnixStream::connect(socket).map_err(|_| super::DAEMON_UNAVAILABLE)?;
    stream
        .set_read_timeout(Some(super::TIMEOUT))
        .map_err(|_| super::DAEMON_UNAVAILABLE)?;
    stream
        .set_write_timeout(Some(super::TIMEOUT))
        .map_err(|_| super::DAEMON_UNAVAILABLE)?;

    // Oversized or unsupported inputs already have a definite legacy UNKNOWN.
    // Keep the original protocol-1 path for these cases, including inputs that
    // exceed the smaller locality payload bound.
    if input.command.len() > arc_detector::MAX_INPUT_BYTES
        || !matches!(input.shell_kind.as_str(), "sh" | "bash" | "zsh")
    {
        let result = exchange(&mut stream, OPERATION_JOB_CLASSIFY, input)?;
        return Ok(verified_result(input, context.as_ref(), &now_ms, result));
    }

    let contextual = ClassifyLocalityRequest {
        command: Command::Shell {
            command: input.command.clone(),
            shell_kind: input.shell_kind.clone(),
        },
        // An invalid timestamp makes capture failure explicit to a new peer.
        context: context.as_ref().map_or_else(Default::default, |context| {
            let mut public = context.clone();
            // The manifest digest is only for the CLI's post-response
            // revalidation. The daemon needs the finite case marker, not
            // the fingerprint or the script body.
            for fact in &mut public.facts {
                if fact.kind == arc_detector::EndpointKind::PnpmScript {
                    fact.endpoints.truncate(1);
                }
            }
            public
        }),
    };
    match exchange(&mut stream, OPERATION_CLASSIFY, &contextual) {
        Ok(result) => Ok(verified_result(input, context.as_ref(), &now_ms, result)),
        Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) => {
            // Use the same peer and the unchanged protocol-1 request shape.
            // No malformed, lost, mismatched or otherwise rejected reply is a
            // capability negotiation signal.
            let result = exchange(&mut stream, OPERATION_JOB_CLASSIFY, input)?;
            Ok(verified_result(input, context.as_ref(), &now_ms, result))
        }
        Err(arc_ipc::ERROR_INVALID_REQUEST)
            if context.as_ref().is_some_and(|context| {
                context.is_valid_at(context.captured_at_ms)
                    && context.facts.iter().any(|fact| {
                        matches!(
                            fact.kind,
                            arc_detector::EndpointKind::GitPushDefault
                                | arc_detector::EndpointKind::Makefile
                                | arc_detector::EndpointKind::PnpmScript
                                | arc_detector::EndpointKind::ShellStartup
                                | arc_detector::EndpointKind::ActLocal
                        )
                    })
            }) =>
        {
            // Older locality-v1 schemas predate these fact kinds.
            // Probe the same request with only the new kinds removed: an
            // unrelated INVALID_REQUEST must still be returned to the user.
            let mut older = contextual;
            older.context.facts.retain(|fact| {
                !matches!(
                    fact.kind,
                    arc_detector::EndpointKind::GitPushDefault
                        | arc_detector::EndpointKind::Makefile
                        | arc_detector::EndpointKind::PnpmScript
                        | arc_detector::EndpointKind::ShellStartup
                        | arc_detector::EndpointKind::ActLocal
                )
            });
            exchange(&mut stream, OPERATION_CLASSIFY, &older)?;
            // A peer that could not receive the selected destination cannot
            // prove the result of this request, even if its probe is confident.
            Ok(unknown())
        }
        Err(error) => Err(error),
    }
}

fn exchange(
    stream: &mut UnixStream,
    operation: &str,
    input: &impl serde::Serialize,
) -> Result<Classification, &'static str> {
    let mut request = Request::new(super::request_id(), operation);
    request.payload = serde_json::to_value(input).map_err(|_| super::IPC_ERROR)?;
    let response = arc_ipc::exchange(stream, &request).map_err(|_| super::IPC_ERROR)?;
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
    Ok(result)
}

fn verified_result(
    input: &ClassifyRequest,
    context: Option<&arc_detector::EndpointContext>,
    now_ms: &impl Fn() -> Option<u64>,
    result: Classification,
) -> Classification {
    // A failed capture must not obscure a definitive UNKNOWN diagnostic
    // behind the contextual operation's invalid timestamp.
    if context.is_none() {
        let diagnostic = arc_detector::classify(input);
        if diagnostic.classification == JobClass::Unknown
            && matches!(
                diagnostic.reason,
                Reason::EmptyCommand
                    | Reason::UnsupportedShell
                    | Reason::UnsupportedSyntax
                    | Reason::UnresolvedCommand
                    | Reason::UnresolvedScript
                    | Reason::UnresolvedExpansion
                    | Reason::InputLimit
                    | Reason::TokenLimit
                    | Reason::InvocationLimit
                    | Reason::NestingLimit
                    | Reason::WorkLimit
            )
        {
            return diagnostic;
        }
    }
    if result.classification == JobClass::Unknown {
        return result;
    }
    // Locality-v1 peers may implement older endpoint precedence. Accept a
    // confident reply only when the current detector agrees on fresh facts.
    if let (Some(context), Some(now)) = (context, now_ms()) {
        if context.is_valid_at(now)
            && super::exec::revalidate_context(context)
            && now_ms().is_some_and(|now| {
                context.is_valid_at(now)
                    && arc_detector::classify_with_context(input, context, now) == result
            })
        {
            return result;
        }
    }
    unknown()
}

fn unknown() -> Classification {
    Classification {
        classification: JobClass::Unknown,
        locality: Locality::Unknown,
        reason: Reason::UnresolvedCommand,
        uncertainty: Uncertainty::Observe,
    }
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

#[cfg(test)]
mod tests;
