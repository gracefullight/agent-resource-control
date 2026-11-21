//! Authenticate complete legacy requests across a local byte relay.
use arc_ipc::Request;
use std::{
    cell::RefCell,
    fs, io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::DirBuilderExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

thread_local! {
    // The managed exec thread owns this scope through child completion.
    static SCOPE: RefCell<Option<Option<Service>>> = const { RefCell::new(None) };
}

pub(in crate::exec) struct Scope;
impl Scope {
    pub(in crate::exec) fn enter() -> Self {
        SCOPE.with(|scope| {
            let old = scope.replace(Some(None));
            assert!(old.is_none(), "nested managed relay scope");
        });
        Self
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let service = SCOPE.with(|scope| scope.borrow_mut().take()).flatten();
        drop(service);
    }
}

/// A relayed request can arrive after its first response was lost. Keep its
/// exact request available while the original CLI still owns the managed job.
pub(super) fn bind(request: &mut Request) -> io::Result<()> {
    SCOPE.with(|scope| {
        let mut scope = scope.borrow_mut();
        let slot = scope
            .as_mut()
            .ok_or_else(|| io::Error::other("no managed relay scope"))?;
        if slot.is_none() {
            *slot = Some(Service::start()?);
        }
        slot.as_mut().expect("installed service").bind(request)
    })
}

struct Service {
    directory: PathBuf,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Service {
    fn start() -> io::Result<Self> {
        let directory = std::env::temp_dir().join(format!(
            "arc-p-{}",
            arc_core::process_identity::random_job_id()?
        ));
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let path = directory.join("s");
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) => {
                let _ = fs::remove_dir(&directory);
                return Err(error);
            }
        };
        if let Err(error) = listener.set_nonblocking(true) {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_dir(&directory);
            return Err(error);
        }
        let requests = Arc::new(Mutex::new(Vec::new()));
        let expected = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        // Do not let the callback thread consume the native waiter's signals.
        let worker = match super::super::signals::with_blocked_signals(|| {
            thread::Builder::new()
                .name("arc-relay-proof".into())
                .spawn(move || {
                    while !stopped.load(Ordering::Acquire) {
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
                        if stopped.load(Ordering::Acquire) {
                            break;
                        }
                        // Darwin can inherit O_NONBLOCK from the listener. The
                        // daemon authenticates this endpoint before writing,
                        // so an immediate read must wait for its frame.
                        if stream.set_nonblocking(false).is_err() {
                            continue;
                        }
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                        if let Ok(challenge) = arc_ipc::read_message::<_, serde_json::Value>(&mut stream) {
                            let nonce = challenge.get("nonce").and_then(serde_json::Value::as_str);
                            let request = challenge.get("request").cloned()
                                .and_then(|value| serde_json::from_value::<Request>(value).ok());
                            let matched = challenge.as_object().is_some_and(|value| value.len() == 3)
                                && challenge.get("kind").and_then(serde_json::Value::as_str) == Some("arc.relay.challenge.v1")
                                && nonce.is_some_and(|value| value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()))
                                && request.as_ref().is_some_and(|request| expected.lock().unwrap_or_else(|e| e.into_inner()).contains(request));
                            if matched {
                                let response = serde_json::json!({"kind":"arc.relay.response.v1", "nonce": nonce});
                                let _ = arc_ipc::write_message(&mut stream, &response);
                            }
                        }
                    }
                })
        })
        .and_then(|result| result)
        {
            Ok(worker) => worker,
            Err(error) => {
                let _ = fs::remove_file(&path);
                let _ = fs::remove_dir(&directory);
                return Err(error);
            }
        };
        Ok(Self {
            directory,
            requests,
            stop,
            worker: Some(worker),
        })
    }

    fn bind(&mut self, request: &mut Request) -> io::Result<()> {
        let path = self.directory.join("s");
        let path = path
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF-8 proof path"))?;
        let mut requests = self.requests.lock().unwrap_or_else(|e| e.into_inner());
        if requests.len() >= 16 {
            return Err(io::Error::other("relay proof request limit"));
        }
        request.request_id = format!(
            "arc-proof:{path}#{}",
            arc_core::process_identity::random_job_id()?
        );
        requests.push(request.clone());
        Ok(())
    }
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
