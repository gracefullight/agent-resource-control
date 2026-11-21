//! Identity-bound Reaper action for the single retained child.
//!
//! The daemon never signals a job child. It asks this wrapper, which is the
//! child's direct parent and sole reaper. Every request repeats the complete
//! identity check under the wait gate, so no consuming wait can run between
//! the check and the signal. Any unavailable or failed check sends no signal.
use super::{with_blocked_signals, CHILD, WAIT_GATE};
use arc_core::process_identity::{
    probe, probe_live, random_job_id, IdentityStatus, ProcessIdentity,
};
use arc_ipc::{
    client::{Continuity, ExchangeOptions},
    jobs::JobId,
    reaper::{
        valid_nonce, ActionOutcome, ActionRequest, ActionResult, ChildEvidence, Refusal,
        RegisterRequest, Signal, ACTION_KIND, OPERATION_REGISTER, RESULT_KIND,
    },
    Request,
};
use arc_platform::reaper_identity::{self, ChildIdentity};
use std::{
    fs, io,
    os::{
        fd::AsRawFd,
        unix::{fs::DirBuilderExt, net::UnixListener, net::UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) struct Service {
    directory: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

struct Context {
    socket: PathBuf,
    job_id: JobId,
    observer: ProcessIdentity,
    child: ProcessIdentity,
    endpoint: String,
    stop: Arc<AtomicBool>,
}

pub(super) fn start(
    socket: &Path,
    job_id: &JobId,
    observer: &ProcessIdentity,
    child: &ProcessIdentity,
) -> io::Result<Service> {
    let directory = std::env::temp_dir().join(format!("arc-r-{}", random_job_id()?));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    // From here the guard removes the private endpoint on every exit path.
    let mut service = Service {
        directory,
        stop: Arc::new(AtomicBool::new(false)),
        worker: None,
    };
    let path = service.directory.join("s");
    let endpoint = path
        .to_str()
        .ok_or_else(|| io::Error::other("non-UTF-8 action path"))?
        .to_owned();
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    let context = Context {
        socket: socket.to_owned(),
        job_id: job_id.clone(),
        observer: observer.clone(),
        child: child.clone(),
        endpoint,
        stop: service.stop.clone(),
    };
    // The action thread must never consume the native waiter's signals.
    let worker = with_blocked_signals(|| {
        thread::Builder::new()
            .name("arc-reaper-action".into())
            .spawn(move || serve(listener, context))
    })??;
    service.worker = Some(worker);
    Ok(service)
}

impl Drop for Service {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = UnixStream::connect(self.directory.join("s"));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_file(self.directory.join("s"));
        let _ = fs::remove_dir(&self.directory);
    }
}

pub(super) fn wire(own: &ChildIdentity) -> Option<ChildEvidence> {
    Some(ChildEvidence {
        process: own.process.clone(),
        parent_pid: own.parent_pid,
        executable: own.executable.to_str()?.to_owned(),
        executable_device: own.executable_device,
        executable_inode: own.executable_inode,
        real_uid: own.real_uid,
        effective_uid: own.effective_uid,
    })
}

fn native(evidence: &ChildEvidence) -> ChildIdentity {
    ChildIdentity {
        process: evidence.process.clone(),
        parent_pid: evidence.parent_pid,
        executable: PathBuf::from(&evidence.executable),
        executable_device: evidence.executable_device,
        executable_inode: evidence.executable_inode,
        real_uid: evidence.real_uid,
        effective_uid: evidence.effective_uid,
    }
}

/// The owning session is the process that launched this wrapper. An already
/// orphaned wrapper has no session to name and stays unregistered.
fn session() -> Option<ProcessIdentity> {
    let parent = unsafe { libc::getppid() };
    if parent <= 1 {
        return None;
    }
    let identity = ProcessIdentity::capture(parent as u32).ok()?;
    (probe_live(&identity) == IdentityStatus::VerifiedMatch && unsafe { libc::getppid() } == parent)
        .then_some(identity)
}

fn register(context: &Context, child: ChildEvidence, token: &str) -> Option<ProcessIdentity> {
    let mut request = Request::new(crate::request_id(), OPERATION_REGISTER);
    request.payload = serde_json::to_value(RegisterRequest {
        job_id: context.job_id.clone(),
        observer: context.observer.clone(),
        child,
        session: session()?,
        endpoint: context.endpoint.clone(),
        token: token.to_owned(),
    })
    .ok()?;
    let continuity = Continuity::default();
    arc_ipc::client::exchange_until(
        &context.socket,
        &request,
        Instant::now() + Duration::from_secs(2),
        &|| context.stop.load(Ordering::Acquire),
        &mut false,
        ExchangeOptions {
            continuity: Some(&continuity),
            retry_busy: true,
        },
    )
    .ok()?;
    // Later requests are accepted only from this same daemon instance.
    continuity.identity()
}

fn serve(listener: UnixListener, context: Context) {
    let Ok(own) = reaper_identity::capture(&context.child) else {
        return;
    };
    if own.parent_pid != std::process::id() {
        return;
    }
    let (Some(evidence), Ok(token)) = (wire(&own), random_job_id()) else {
        return;
    };
    let Some(daemon) = register(&context, evidence, &token) else {
        return;
    };
    while !context.stop.load(Ordering::Acquire) {
        let mut fd = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, 1000) } <= 0 {
            continue;
        }
        let Ok((mut stream, _)) = listener.accept() else {
            continue;
        };
        if context.stop.load(Ordering::Acquire) {
            break;
        }
        // Darwin can inherit O_NONBLOCK from the listener.
        if stream.set_nonblocking(false).is_err() {
            continue;
        }
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
        let authorized = arc_ipc::client::authenticated_identity(&stream)
            .is_ok_and(|peer| peer == daemon)
            && probe_live(&daemon) == IdentityStatus::VerifiedMatch;
        let Ok(request) = arc_ipc::read_message::<_, ActionRequest>(&mut stream) else {
            continue;
        };
        if request.kind != ACTION_KIND || !valid_nonce(&request.nonce) {
            continue;
        }
        let outcome = if authorized && request.token == token && request.job_id == context.job_id {
            act(&own, &request.child, request.signal)
        } else {
            ActionOutcome::Refused {
                reason: Refusal::Unauthorized,
            }
        };
        let _ = arc_ipc::write_message(
            &mut stream,
            &ActionResult {
                kind: RESULT_KIND.into(),
                nonce: request.nonce,
                outcome,
            },
        );
    }
}

/// Deliver at most one signal to the retained child, or refuse with a reason.
pub(super) fn act(own: &ChildIdentity, requested: &ChildEvidence, signal: Signal) -> ActionOutcome {
    let refused = |reason| ActionOutcome::Refused { reason };
    // The supervising thread holds this gate across each consuming wait and
    // until it clears CHILD. Holding it here means the PID read below is still
    // this wrapper's unreaped child through the signal: it cannot be recycled.
    let _gate = WAIT_GATE.lock().unwrap_or_else(|e| e.into_inner());
    let pid = CHILD.load(Ordering::SeqCst);
    if pid <= 0 || pid as u32 != own.process.pid || requested.process.pid != own.process.pid {
        return refused(Refusal::NotSupervised);
    }
    // The daemon's record must equal the evidence captured at this child's birth.
    if native(requested) != *own {
        return refused(Refusal::IdentityMismatch);
    }
    let foreground = reaper_identity::foreground(&own.process);
    // Last check before the signal: direct parent, both UIDs, start time,
    // executable path and file identity, all read fresh.
    if !reaper_identity::matches(own, std::process::id()) {
        let changed = reaper_identity::capture(&own.process).is_ok()
            || probe(&own.process) == IdentityStatus::Mismatch;
        return refused(if changed {
            Refusal::IdentityMismatch
        } else {
            Refusal::IdentityUnavailable
        });
    }
    // An unreadable terminal state is not evidence of a background process.
    if !matches!(foreground, Ok(false)) {
        return refused(Refusal::Foreground);
    }
    let number = match signal {
        Signal::Term => libc::SIGTERM,
        Signal::Kill => libc::SIGKILL,
    };
    if unsafe { libc::kill(pid, number) } != 0 {
        return refused(Refusal::SignalFailed);
    }
    ActionOutcome::Signaled {}
}
