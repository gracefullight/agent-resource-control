//! Built-CLI fallback against a peer predating the workspace capability.
use crate::{
    journey::{output_with_input, Sandbox},
    runtime::Fixture,
    workspace::{members, result},
};
use std::{
    fs,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

struct Peer {
    done: Arc<AtomicBool>,
    worker: Option<JoinHandle<Vec<String>>>,
    socket: PathBuf,
}
impl Peer {
    fn stop(&mut self) -> Vec<String> {
        self.done.store(true, Ordering::Release);
        let operations = self
            .worker
            .take()
            .map(|worker| worker.join().unwrap())
            .unwrap_or_default();
        let _ = fs::remove_file(&self.socket);
        operations
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn unsupported_compound(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    command: &str,
    expected_members: &[(String, String, String)],
    projection: &[String; 4],
) {
    let socket = f.root.join("legacy-compound.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let stopped = done.clone();
    let worker = thread::spawn(move || {
        let mut operations = Vec::new();
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(client) => client,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("legacy listener: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(60)))
                .unwrap();
            let request: arc_ipc::Request =
                serde_json::from_slice(&arc_ipc::read_frame(&mut stream).unwrap()).unwrap();
            if request.operation == arc_ipc::workspace_compound::OPERATION {
                assert_eq!(request.payload, serde_json::json!({"action":"probe"}));
            }
            operations.push(request.operation.clone());
            let response = if request.operation == arc_ipc::locality::OPERATION_CLASSIFY {
                arc_ipc::locality::classify_response_at(
                    &request,
                    arc_ipc::locality::now_ms().unwrap(),
                )
            } else {
                arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
            };
            arc_ipc::write_message(&mut stream, &response).unwrap();
        }
        operations
    });
    let mut peer = Peer {
        done,
        worker: Some(worker),
        socket: socket.clone(),
    };
    let mut cli = s.arcctl(f, cwd);
    cli.env(arc_core::ARC_SOCKET_PATH, socket).args([
        "classify",
        "--json",
        "--diagnostics",
        "--",
        command,
    ]);
    let output = output_with_input(&mut cli, b"", "legacy compound classify");
    let operations = peer.stop();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(&result(&value), projection);
    assert_eq!(members(&value), expected_members);
    assert!(value["analysis"].get("compound").is_none(), "{value}");
    assert!(
        operations
            .iter()
            .any(|operation| operation == arc_ipc::locality::OPERATION_CLASSIFY),
        "{operations:?}"
    );
    assert!(
        operations
            .iter()
            .any(|operation| operation == arc_ipc::workspace_compound::OPERATION),
        "{operations:?}"
    );
}
