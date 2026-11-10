//! Preparation never owns execution. A lost acknowledgement cannot veto spawn.
use super::{signals::Signals, start_until, transport::Failure};
use arc_ipc::jobs::{JobId, Phase, ProcessIdentity, StartRequest, StartResult};
use std::{
    fmt,
    io::{self, Write},
    path::Path,
    sync::mpsc::{self, Sender},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tracking {
    Managed,
    Unavailable,
    Uncertain,
}
pub(super) fn prepare(
    socket: &Path,
    job_id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
    deadline: Instant,
    signals: &Signals,
) -> Result<Tracking, Failure> {
    let cancelled = || signals.interrupted();
    prepare_with(
        job_id,
        observer,
        classification,
        deadline,
        &cancelled,
        |request, deadline, cancelled| start_until(socket, request, deadline, cancelled),
    )
}

pub(super) fn prepare_with(
    job_id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    mut start: impl FnMut(StartRequest, Instant, &dyn Fn() -> bool) -> Result<StartResult, Failure>,
) -> Result<Tracking, Failure> {
    let register = start(
        StartRequest::Register {
            job_id: job_id.clone(),
            observer: observer.clone(),
            classification,
        },
        deadline,
        cancelled,
    );
    match register {
        Err(Failure::Unavailable) => return Ok(Tracking::Unavailable),
        Err(Failure::Uncertain | Failure::Busy) => return Ok(Tracking::Uncertain),
        Err(error) => return Err(error),
        Ok(r) if r.job.phase == Phase::Registered => (),
        Ok(_) => return Err(Failure::InvalidReply),
    }
    // REGISTER and CLAIM use exactly the same absolute deadline. REGISTER has
    // been confirmed, so any subsequent outage leaves persistence uncertain.
    match start(
        StartRequest::Claim {
            job_id: job_id.clone(),
            observer: observer.clone(),
        },
        deadline,
        cancelled,
    ) {
        Err(
            Failure::Unavailable | Failure::Uncertain | Failure::Busy | Failure::InvalidEndpoint,
        ) => Ok(Tracking::Uncertain),
        Err(error) => Err(error),
        Ok(r) if r.job.phase == Phase::Launching && r.job.classification == classification => {
            Ok(Tracking::Managed)
        }
        Ok(_) => Err(Failure::InvalidReply),
    }
}

pub(super) fn stopped(failure: Failure) -> std::process::ExitCode {
    let message = match failure {
        Failure::Cancelled => "NOT_STARTED\nExecution cancelled. No child was started.",
        Failure::Rejected => "NOT_STARTED\nARC rejected execution. No child was started.",
        Failure::Protocol => {
            "PROTOCOL_INCOMPATIBLE\nNOT_STARTED\nProtocol 1 is required. No child was started."
        }
        Failure::InvalidReply => {
            "IPC_ERROR\nNOT_STARTED\nInvalid lifecycle reply. No child was started."
        }
        Failure::Witness => {
            "SPAWN_FAILED\nNo child was started because its recovery witness could not be written."
        }
        _ => "NOT_STARTED\nLocal execution preparation failed. No child was started.",
    };
    best_effort(message);
    std::process::ExitCode::FAILURE
}

pub(super) fn best_effort(message: &str) {
    let mut diagnostics = Diagnostics::default();
    diagnostics.emit(format_args!("{message}"));
    diagnostics.finish();
}
pub(super) fn uncertain(diagnostics: &mut Diagnostics, job_id: &JobId) {
    diagnostics.emit(format_args!("TRACKING_UNCERTAIN\nCommand started. Lifecycle recording is unconfirmed.\nReference: {}\nAfter daemon recovery, inspect arcctl job status {}.\nA missing record does not show whether the command ran. Do not launch another copy to recover tracking.", job_id.as_str(), job_id.as_str()));
}
pub(super) fn started(tracking: Tracking, job_id: &JobId) -> Diagnostics {
    let mut diagnostics = Diagnostics::default();
    match tracking {
        Tracking::Managed => (),
        Tracking::Unavailable => diagnostics.emit(format_args!("TRACKING_UNAVAILABLE\nCommand started. ARC tracking is unavailable. A lifecycle record may be absent.\nDo not launch another copy to recover tracking.")),
        Tracking::Uncertain => uncertain(&mut diagnostics, job_id),
    }
    diagnostics
}

fn write_diagnostic(output: &mut impl Write, message: fmt::Arguments<'_>) {
    let line = format!("{message}\n");
    let _ = output.write_all(line.as_bytes());
}

enum Diagnostic {
    Text(String),
    Flushed(Sender<()>),
}

/// A blocked stderr may strand this worker, never the child supervisor.
#[derive(Default)]
pub(super) struct Diagnostics {
    sender: Option<Sender<Diagnostic>>,
}
impl Diagnostics {
    pub(super) fn emit(&mut self, message: fmt::Arguments<'_>) {
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::channel();
            let worker = super::signals::with_blocked_signals(|| {
                std::thread::Builder::new()
                    .name("arc-diagnostics".into())
                    .spawn(move || {
                        while let Ok(event) = receiver.recv() {
                            match event {
                                Diagnostic::Text(text) => write_diagnostic(
                                    &mut io::stderr().lock(),
                                    format_args!("{text}"),
                                ),
                                Diagnostic::Flushed(done) => {
                                    let _ = done.send(());
                                }
                            }
                        }
                    })
            });
            if matches!(worker, Ok(Ok(_))) {
                self.sender = Some(sender);
            }
        }
        if let Some(sender) = &self.sender {
            let _ = sender.send(Diagnostic::Text(message.to_string()));
        }
    }

    pub(super) fn finish(self) {
        if let Some(sender) = self.sender {
            let (done, received) = mpsc::channel();
            if sender.send(Diagnostic::Flushed(done)).is_ok() {
                // Give a writable sink time to receive its queued messages. A
                // blocked or broken sink is never claimed to have received one.
                let _ = received.recv_timeout(Duration::from_millis(50));
            }
        }
    }
}

/// The last admission check is adjacent to the sole native spawn expression.
pub(super) fn admit<T>(
    cancelled: impl FnOnce() -> bool,
    spawn: impl FnOnce() -> T,
) -> Result<T, Failure> {
    if cancelled() {
        Err(Failure::Cancelled)
    } else {
        Ok(spawn())
    }
}
#[cfg(test)]
mod tests {
    use super::super::transport::Failure;
    use super::admit;
    #[test]
    fn cancellation_at_spawn_admission_cannot_invoke_spawn() {
        let mut invoked = false;
        let result = admit(
            || true,
            || {
                invoked = true;
            },
        );
        assert_eq!(result, Err(Failure::Cancelled));
        assert!(!invoked);
        assert_eq!(admit(|| false, || 17), Ok(17));
    }
    #[test]
    fn nul_and_empty_native_arguments_are_invalid_before_metadata() {
        use std::os::unix::ffi::OsStringExt;
        assert_eq!(
            super::super::run(vec![std::ffi::OsString::from_vec(vec![b'x', 0])]),
            std::process::ExitCode::from(2)
        );
        assert_eq!(
            super::super::run(vec!["".into()]),
            std::process::ExitCode::from(2)
        );
    }

    #[test]
    fn diagnostic_writes_are_best_effort_and_preserve_writable_output() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        super::write_diagnostic(
            &mut Broken,
            format_args!("TRACKING_UNAVAILABLE\nCommand started."),
        );
        let mut output = Vec::new();
        super::write_diagnostic(&mut output, format_args!("message {}", 17));
        assert_eq!(output, b"message 17\n");
    }
}
