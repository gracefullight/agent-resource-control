//! Local fault injection tests the credential boundary without a runtime bypass.
use super::{
    call_until, call_with, endpoint_failure, peer, ConnectFailure, DeadlineStream, Failure,
    TrackedStream,
};
use crate::exec::preparation::{prepare_with, Tracking};
use arc_ipc::{
    jobs::{
        IdentityObservation, JobId, JobSnapshot, LifecycleReason, ObservationStatus,
        ObservationTarget, Phase, ProcessIdentity, StartDisposition, StartRequest, StartResult,
    },
    Request, Response,
};
use serde_json::Value;
use std::{
    cell::Cell,
    ffi::OsString,
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStringExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Disconnect(bool);
impl Read for Disconnect {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}
impl Write for Disconnect {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0 |= !b.is_empty();
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl TrackedStream for Disconnect {
    fn sent(&self) -> bool {
        self.0
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}

#[test]
fn malformed_addresses_are_invalid_endpoints_in_preparation() {
    let paths = [
        PathBuf::from("x".repeat(200)),
        PathBuf::from(OsString::from_vec(b"socket\0suffix".to_vec())),
    ];
    let id = JobId::try_from("0123456789abcdef0123456789abcdef".to_string()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["fixture".into()]);
    for path in paths {
        let authenticated = Cell::new(false);
        assert!(matches!(
            DeadlineStream::connect_with_auth(&path, deadline(), &|| false, |_| {
                authenticated.set(true);
                Ok(())
            }),
            Err(ConnectFailure::InvalidEndpoint)
        ));
        assert!(!authenticated.get());
        assert_eq!(
            call_until(&path, "REGISTER", Value::Null, deadline(), &|| false),
            Err(Failure::InvalidEndpoint)
        );
        let mut calls = 0;
        let result = prepare_with(
            &id,
            &observer,
            classification,
            deadline(),
            &|| false,
            |request, end, cancelled| {
                calls += 1;
                crate::exec::start_until(&path, request, end, cancelled)
            },
        );
        assert_eq!(calls, 1);
        assert_eq!(result, Err(Failure::InvalidEndpoint));
    }
}

#[test]
fn sent_request_then_invalid_endpoint_keeps_uncertainty() {
    for errno in [
        libc::ENOTDIR,
        libc::ENOTSOCK,
        libc::ELOOP,
        libc::ENAMETOOLONG,
        libc::EPROTOTYPE,
    ] {
        let attempts = Cell::new(0);
        let result = call_with("REGISTER", Value::Null, deadline(), &|| false, |_| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Ok(Disconnect::default())
            } else {
                Err(endpoint_failure(io::Error::from_raw_os_error(errno)))
            }
        });
        assert_eq!(result, Err(Failure::Uncertain), "errno {errno}");
        assert_eq!(attempts.get(), 2);
    }
}

#[test]
fn local_setup_failure_after_send_still_stops() {
    let attempts = Cell::new(0);
    let result = call_with("REGISTER", Value::Null, deadline(), &|| false, |_| {
        attempts.set(attempts.get() + 1);
        if attempts.get() == 1 {
            Ok(Disconnect::default())
        } else {
            Err(ConnectFailure::Local)
        }
    });
    assert_eq!(result, Err(Failure::Local));
}

fn registered(
    id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
) -> StartResult {
    let reason = LifecycleReason::RegisteredNotClaimed;
    StartResult {
        disposition: StartDisposition::Created,
        job: JobSnapshot {
            job_id: id.clone(),
            phase: Phase::Registered,
            classification,
            observer: observer.clone(),
            observer_verified_at_ms: 1,
            child: None,
            child_verified_at_ms: None,
            observation: IdentityObservation {
                target: ObservationTarget::Observer,
                status: ObservationStatus::VerifiedMatch,
                observed_at_ms: 1,
                reason,
            },
            reason,
            outcome: None,
            registered_at_ms: 1,
            launch_claimed_at_ms: None,
            completed_at_ms: None,
            updated_at_ms: 1,
            revision: 1,
        },
    }
}

#[test]
fn confirmed_register_then_invalid_endpoint_is_uncertain_but_local_failure_stops() {
    let id = JobId::try_from("0123456789abcdef0123456789abcdef".to_string()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["fixture".into()]);
    for failure in [
        Failure::InvalidEndpoint,
        Failure::Local,
        Failure::Cancelled,
        Failure::Rejected,
        Failure::InvalidReply,
    ] {
        let mut calls = 0;
        let result = prepare_with(
            &id,
            &observer,
            classification,
            deadline(),
            &|| false,
            |request, _, _| {
                calls += 1;
                match request {
                    StartRequest::Register { .. } => Ok(registered(&id, &observer, classification)),
                    StartRequest::Claim { .. } => Err(failure),
                    _ => unreachable!(),
                }
            },
        );
        assert_eq!(calls, 2);
        assert_eq!(
            result,
            if failure == Failure::InvalidEndpoint {
                Ok(Tracking::Uncertain)
            } else {
                Err(failure)
            }
        );
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn same_user_os_peer_credentials_are_accepted() {
    let (a, b) = UnixStream::pair().unwrap();
    assert!(peer::ensure_same_user(&a).is_ok());
    assert!(peer::ensure_same_user(&b).is_ok());
    let uid = unsafe { libc::geteuid() };
    assert!(peer::check_uid(uid, uid).is_ok());
    assert_eq!(
        peer::check_uid(uid.wrapping_add(1), uid)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    struct InvalidDescriptor;
    impl std::os::fd::AsRawFd for InvalidDescriptor {
        fn as_raw_fd(&self) -> std::os::fd::RawFd {
            -1
        }
    }
    assert!(peer::ensure_same_user(&InvalidDescriptor).is_err());
}

fn untrusted_result(kind: io::ErrorKind, disconnect_first: bool) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "arc-peer-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let requests = Arc::new(AtomicUsize::new(0));
    let received = requests.clone();
    let processed = Arc::new(AtomicUsize::new(0));
    let observed = processed.clone();
    let server = thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(5);
        while !done.load(Ordering::SeqCst) && Instant::now() < until {
            let (mut stream, _) = match listener.accept() {
                Ok(value) => value,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => panic!("accept: {e}"),
            };
            stream.set_nonblocking(false).unwrap();
            let mut header = [0u8; 4];
            if stream.read_exact(&mut header[..1]).is_ok() {
                // Count the first byte, even if the rest is incomplete. No
                // lifecycle prefix may reach an untrusted connection.
                let number = received.fetch_add(1, Ordering::SeqCst);
                if !disconnect_first || number > 0 {
                    stream.read_exact(&mut header[1..]).unwrap();
                    let mut body = vec![0; u32::from_be_bytes(header) as usize];
                    stream.read_exact(&mut body).unwrap();
                    let request: Request = serde_json::from_slice(&body).unwrap();
                    let _ = arc_ipc::write_response(
                        &mut stream,
                        &Response::error(request.request_id, "INVALID_REQUEST"),
                    );
                }
            }
            observed.fetch_add(1, Ordering::SeqCst);
        }
    });
    let attempts = Cell::new(0);
    let result = call_with("REGISTER", Value::Null, deadline(), &|| false, |end| {
        attempts.set(attempts.get() + 1);
        DeadlineStream::connect_with_auth(&path, end, &|| false, |stream| {
            if disconnect_first && attempts.get() == 1 {
                peer::ensure_same_user(stream)
            } else {
                Err(io::Error::new(kind, "injected credential failure"))
            }
        })
    });
    let end = Instant::now() + Duration::from_secs(2);
    while processed.load(Ordering::SeqCst) < attempts.get() && Instant::now() < end {
        thread::sleep(Duration::from_millis(1));
    }
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        result,
        Err(if disconnect_first {
            Failure::Uncertain
        } else {
            Failure::Unavailable
        })
    );
    assert_eq!(
        requests.load(Ordering::SeqCst),
        usize::from(disconnect_first),
        "untrusted peer received a request"
    );
    assert_eq!(attempts.get(), 3);
    assert_eq!(
        processed.load(Ordering::SeqCst),
        3,
        "all injected peers were observed"
    );
}

#[test]
fn untrusted_peer_receives_no_request_and_cannot_reject() {
    // PermissionDenied models a foreign UID; Other models query failure; the
    // unsupported case uses the same failure path on unsupported platforms.
    for kind in [
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::Other,
        io::ErrorKind::Unsupported,
    ] {
        untrusted_result(kind, false);
        untrusted_result(kind, true);
    }
}

#[test]
fn cancellation_overrides_invalid_endpoint_after_send() {
    let cancel = Cell::new(false);
    let attempts = Cell::new(0);
    let result = call_with(
        "REGISTER",
        Value::Null,
        deadline(),
        &|| cancel.get(),
        |_| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Ok(Disconnect::default())
            } else {
                cancel.set(true);
                Err(endpoint_failure(io::Error::from_raw_os_error(
                    libc::ENOTSOCK,
                )))
            }
        },
    );
    assert_eq!(result, Err(Failure::Cancelled));
}
