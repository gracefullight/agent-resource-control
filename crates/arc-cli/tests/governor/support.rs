use arc_ipc::{Request, Response};
use serde_json::Value;
use std::{
    fs,
    os::unix::{
        fs::DirBuilderExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

pub struct Socket {
    pub root: PathBuf,
    pub listener: UnixListener,
}
impl Socket {
    pub fn new() -> Self {
        let root = PathBuf::from(format!(
            "/tmp/arc-gv-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let listener = UnixListener::bind(root.join("s")).unwrap();
        Self { root, listener }
    }
    pub fn cli(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .env("ARC_SOCKET_PATH", self.root.join("s"))
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
pub fn next(listener: &UnixListener) -> (UnixStream, Request) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                return {
                    let request = read(&mut stream);
                    (stream, request)
                };
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "missing client request");
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("fixture accept: {error}"),
        }
    }
}
pub fn read(stream: &mut UnixStream) -> Request {
    arc_ipc::read_message(stream).unwrap()
}
pub fn reply(stream: &mut UnixStream, request: &Request, value: Value) {
    arc_ipc::write_response(stream, &Response::ok(&request.request_id, value)).unwrap();
}
pub fn output(mut command: Command) -> Output {
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let result = child.wait_with_output().unwrap();
            panic!("CLI deadline: {}", String::from_utf8_lossy(&result.stderr));
        }
        thread::sleep(Duration::from_millis(2));
    }
    child.wait_with_output().unwrap()
}
pub fn state(enabled: bool, revision: u64) -> Value {
    serde_json::json!({"key":"governor.enabled","enabled":enabled,"revision":revision,"updated_at_ms":100})
}
