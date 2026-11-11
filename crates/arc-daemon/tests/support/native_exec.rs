//! Native execution barrier and bounded child wait shared with desktop acceptance.
use serde_json::Value;
use std::{
    os::unix::net::{UnixListener, UnixStream},
    thread,
    time::{Duration, Instant},
};

pub fn accept(listener: &UnixListener) -> (UnixStream, Value) {
    listener.set_nonblocking(true).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(s) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < end, "child never reached barrier");
                thread::sleep(Duration::from_millis(1));
            }
            Err(e) => panic!("{e}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let value = serde_json::from_slice(&arc_ipc::read_frame(&mut stream).unwrap()).unwrap();
    (stream, value)
}
pub fn wait(child: &mut std::process::Child) -> std::process::ExitStatus {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < end, "child wait expired");
        thread::sleep(Duration::from_millis(1));
    }
}
