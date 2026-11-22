//! Preparation never owns execution. A lost acknowledgement cannot veto spawn.
use super::{signals::Signals, start_until, traffic, transport::Failure};
use arc_ipc::jobs::{JobId, Outcome, Phase, ProcessIdentity, StartRequest, StartResult};
use std::{
    fmt,
    io::{self, Write},
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{ExitCode, ExitStatus},
    sync::mpsc::{self, Sender},
    time::{Duration, Instant},
};

pub(super) fn deadline(now: Instant) -> Instant {
    now + Duration::from_millis(1800)
}

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
            usage: super::usage_fields::registration(),
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
        Ok(r)
            if r.job.phase == Phase::Launching
                && (r.job.classification == classification
                    || r.job.classification == classification.legacy()) =>
        {
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

pub(super) fn started_for_mode(tracking: &traffic::Mode, job_id: &JobId) -> Diagnostics {
    match tracking {
        traffic::Mode::Legacy(mode) => started(*mode, job_id),
        traffic::Mode::Uncertain { .. } => started(Tracking::Uncertain, job_id),
        // A duplicate delivery adds nothing to the child's streams.
        traffic::Mode::Duplicate => Diagnostics::default(),
        traffic::Mode::Scheduled { waited, .. } | traffic::Mode::Unreserved { waited, .. } => {
            let mut diagnostics = Diagnostics::default();
            if *waited {
                diagnostics.emit(format_args!("STARTED\nCommand started."));
            }
            diagnostics
        }
    }
}

pub(super) fn outcome_from_status(status: &ExitStatus) -> Outcome {
    match status.code() {
        Some(code) => Outcome::Exited { code: code as u8 },
        None => Outcome::Signaled {
            signal: status.signal().unwrap_or(libc::SIGTERM) as u8,
            core_dumped: status.core_dumped(),
        },
    }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NoticeWrite {
    NotWritten,
    Complete,
    Uncertain,
}

fn notice_writer_exit(status: libc::c_int) -> NoticeWrite {
    if libc::WIFEXITED(status) {
        match libc::WEXITSTATUS(status) {
            0 => NoticeWrite::Complete,
            1 => NoticeWrite::NotWritten,
            _ => NoticeWrite::Uncertain,
        }
    } else {
        NoticeWrite::Uncertain
    }
}

type NoticeWriter = fn(&str) -> NoticeWrite;
thread_local! {
    static NOTICE_WRITER: std::cell::Cell<Option<NoticeWriter>> = const { std::cell::Cell::new(None) };
}

pub(super) fn with_notice_writer<T>(writer: NoticeWriter, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<NoticeWriter>);
    impl Drop for Reset {
        fn drop(&mut self) {
            NOTICE_WRITER.with(|slot| slot.set(self.0));
        }
    }
    let previous = NOTICE_WRITER.with(|slot| slot.replace(Some(writer)));
    let _reset = Reset(previous);
    run()
}

pub(super) fn write_notice(message: &str) -> NoticeWrite {
    let override_writer = NOTICE_WRITER.with(|slot| slot.get());
    override_writer.unwrap_or(write_notice_bounded)(message)
}

pub(crate) fn write_notice_bounded(message: &str) -> NoticeWrite {
    write_notice_controlled(message, 1, &Instant::now, || {
        std::thread::sleep(Duration::from_millis(1));
    })
}

// The isolated native fixture drives the same writer and waiter with a frozen
// clock. Its helper alarm is disabled; production input cannot select this.
#[allow(dead_code)]
pub(crate) fn write_notice_for_fixture(message: &str) -> NoticeWrite {
    let frozen = Instant::now();
    write_notice_controlled(message, 0, &|| frozen, std::thread::yield_now)
}

fn write_notice_controlled(
    message: &str,
    helper_alarm: libc::c_uint,
    now: &dyn Fn() -> Instant,
    pause: impl FnMut(),
) -> NoticeWrite {
    write_notice_with_wait(message, helper_alarm, |pid| {
        wait_notice_with(pid, now, native_wait, native_kill, pause)
    })
}

fn write_notice_with_wait(
    message: &str,
    timeout_seconds: libc::c_uint,
    wait: impl FnOnce(libc::pid_t) -> NoticeWrite,
) -> NoticeWrite {
    // A duplicate descriptor shares O_NONBLOCK with the caller and siblings.
    // A short-lived writer instead performs one blocking write; it is reaped
    // before returning, so a delayed notice cannot follow the target spawn.
    // The helper also owns a timeout in case its parent is killed mid-write.
    unsafe {
        let mut all = std::mem::zeroed();
        let mut original = std::mem::zeroed();
        libc::sigfillset(&mut all);
        if libc::pthread_sigmask(libc::SIG_BLOCK, &all, &mut original) != 0 {
            return NoticeWrite::NotWritten;
        }
        let pid = libc::fork();
        if pid == 0 {
            // Only async-signal-safe operations may run between fork and exit.
            // Restore the default action even if the wrapper inherited a custom
            // SIGALRM handler, and unblock only this helper's timeout signal.
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = libc::SIG_DFL;
            let mut timeout_signal = std::mem::zeroed();
            if libc::sigemptyset(&mut action.sa_mask) != 0
                || libc::sigaction(libc::SIGALRM, &action, std::ptr::null_mut()) != 0
                || libc::sigemptyset(&mut timeout_signal) != 0
                || libc::sigaddset(&mut timeout_signal, libc::SIGALRM) != 0
                || libc::sigprocmask(libc::SIG_UNBLOCK, &timeout_signal, std::ptr::null_mut()) != 0
            {
                libc::_exit(1);
            }
            libc::alarm(timeout_seconds);
            let written = libc::write(libc::STDERR_FILENO, message.as_ptr().cast(), message.len());
            libc::_exit(if written == message.len() as isize {
                0
            } else if written <= 0 {
                1
            } else {
                2
            });
        }
        libc::pthread_sigmask(libc::SIG_SETMASK, &original, std::ptr::null_mut());
        if pid < 0 {
            return NoticeWrite::NotWritten;
        }
        wait(pid)
    }
}

fn native_wait(pid: libc::pid_t, status: &mut i32, flags: i32) -> io::Result<i32> {
    let result = unsafe { libc::waitpid(pid, status, flags) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

fn native_kill(pid: libc::pid_t) {
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
}

// The production waiter and the fixture use the same state machine. Tests
// drive its clock and wait results without relying on process scheduling.
fn wait_notice_with(
    pid: libc::pid_t,
    now: &dyn Fn() -> Instant,
    mut wait: impl FnMut(libc::pid_t, &mut i32, i32) -> io::Result<i32>,
    mut kill: impl FnMut(libc::pid_t),
    mut pause: impl FnMut(),
) -> NoticeWrite {
    let deadline = now() + Duration::from_millis(50);
    let mut status = 0;
    loop {
        match wait(pid, &mut status, libc::WNOHANG) {
            Ok(result) if result == pid => return notice_writer_exit(status),
            Err(error) if error.raw_os_error() != Some(libc::EINTR) => {
                return NoticeWrite::Uncertain
            }
            _ => (),
        }
        if now() >= deadline {
            kill(pid);
            loop {
                match wait(pid, &mut status, 0) {
                    Ok(result) if result == pid => return notice_writer_exit(status),
                    Err(error) if error.raw_os_error() != Some(libc::EINTR) => {
                        return NoticeWrite::Uncertain
                    }
                    _ => (),
                }
            }
        }
        pause();
    }
}

pub(crate) fn invalid() -> ExitCode {
    eprintln!("INVALID_REQUEST\nUse arcctl exec -- <PROGRAM> [ARG...]. No child was started.");
    ExitCode::from(2)
}

pub(super) fn completion_notice(job_id: &JobId, status: &std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    let observed = status.code().map_or_else(
        || {
            format!(
                "The child terminated by signal {}.",
                status.signal().unwrap_or(0)
            )
        },
        |code| format!("The child exited with code {code}."),
    );
    format!("COMPLETION_UNCONFIRMED\nJob: {}\n{observed} Recording is unconfirmed. Inspect arcctl job status for this job. Do not launch another copy.", job_id.as_str())
}

pub(super) fn valid_argv(argv: &[std::ffi::OsString]) -> bool {
    !argv.is_empty()
        && !argv[0].is_empty()
        && !argv.iter().any(|s| s.as_encoded_bytes().contains(&0))
}

#[cfg(test)]
mod tests;
