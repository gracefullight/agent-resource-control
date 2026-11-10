use super::{
    cli::{accept, child_command},
    runtime::{Fixture, OwnedChild},
    support,
};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{net::UnixListener, process::ExitStatusExt},
    },
    process::Stdio,
};
struct Pty {
    master: File,
    slave: File,
    reader: Option<std::thread::JoinHandle<()>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    bytes: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}
impl Pty {
    fn open() -> Self {
        let (mut master, mut slave) = (-1, -1);
        unsafe {
            assert_eq!(
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
            libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
            libc::fcntl(slave, libc::F_SETFD, libc::FD_CLOEXEC);
            let master = File::from_raw_fd(master);
            let slave = File::from_raw_fd(slave);
            libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let bytes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let mut input = master.try_clone().unwrap();
            let stopped = stop.clone();
            let output = bytes.clone();
            let reader = std::thread::spawn(move || {
                let mut buffer = [0; 4096];
                while !stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    if let Ok(n) = input.read(&mut buffer) {
                        output.lock().unwrap().extend_from_slice(&buffer[..n]);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                while let Ok(n) = input.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    output.lock().unwrap().extend_from_slice(&buffer[..n]);
                }
            });
            Self {
                master,
                slave,
                reader: Some(reader),
                stop,
                bytes,
            }
        }
    }
    fn modes(&self) -> Result<Vec<u64>, i32> {
        unsafe {
            let mut term = std::mem::zeroed();
            if libc::tcgetattr(self.slave.as_raw_fd(), &mut term) != 0 {
                return Err(std::io::Error::last_os_error().raw_os_error().unwrap());
            }
            let mut values = vec![
                term.c_iflag as u64,
                term.c_oflag as u64,
                term.c_cflag as u64,
                term.c_lflag as u64,
            ];
            values.extend(term.c_cc.iter().map(|v| *v as u64));
            Ok(values)
        }
    }
    fn output(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.bytes.lock().unwrap())
    }
    fn finish(&mut self) -> Vec<u8> {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.reader.take().unwrap().join().unwrap();
        self.output()
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        if self.reader.is_some() {
            self.finish();
        }
    }
}

#[test]
fn native_pty_foreground_input_modes_and_terminal_signals_match_direct_child() {
    let _serial = support::serial();
    let f = Fixture::new();

    for mode in [
        "tty",
        "interrupt-count",
        "interrupt-exit",
        "quit-exit",
        "ignored-int",
        "stopped",
        "stop-killed",
    ] {
        let mut outputs = Vec::new();
        let mut after_modes = Vec::new();
        for managed in [false, true] {
            let mut pty = Pty::open();
            let before = pty.modes();
            let control = f.root.join(format!("{mode}-{managed}.sock"));
            let listener = UnixListener::bind(&control).unwrap();
            eprintln!("PTY {mode} managed={managed}: launching");
            let target = child_command(&f, managed);
            let restored = f.root.join(format!("restored-{mode}-{managed}"));
            let mut command = std::process::Command::new(super::cli::binary("arc-exec-fixture"));
            command
                .arg(target.get_program())
                .args(target.get_args())
                .env("ARC_FIXTURE_RESTORED", &restored)
                .env("ARC_FIXTURE_LAUNCH", mode)
                .env("PRIVATE_ENV", "environment-secret-sentinel")
                .env(arc_core::ARC_SOCKET_PATH, &f.socket)
                .current_dir(&f.root)
                .env("ARC_FIXTURE_CONTROL", &control)
                .stdin(Stdio::from(pty.slave.try_clone().unwrap()))
                .stdout(Stdio::from(pty.slave.try_clone().unwrap()))
                .stderr(Stdio::from(pty.slave.try_clone().unwrap()));
            let mut child = OwnedChild::new(command.spawn().unwrap());
            let (mut barrier, msg) = accept(&listener);
            eprintln!("PTY {mode} managed={managed}: child ready {msg}");

            if managed {
                assert_eq!(
                    msg["pgid"].as_i64().unwrap(),
                    unsafe { libc::getpgid(msg["ppid"].as_i64().unwrap() as i32) } as i64,
                    "child must retain the caller job group"
                );
            }
            assert_eq!(pty.modes(), before);
            assert_eq!(msg["tty"], true);
            assert_eq!(msg["foreground"], true);
            let mut output = Vec::new();

            if matches!(mode, "tty" | "ignored-int" | "stopped" | "stop-killed") {
                barrier.write_all(b"x").unwrap();
                if mode == "tty" {
                    pty.master.write_all(b"hello\n").unwrap();
                }
                if matches!(mode, "stopped" | "stop-killed") {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    loop {
                        let mut status = 0;
                        let pid = unsafe {
                            libc::waitpid(
                                child.child.id() as i32,
                                &mut status,
                                libc::WUNTRACED | libc::WNOHANG,
                            )
                        };
                        if pid > 0 {
                            assert!(libc::WIFSTOPPED(status));
                            assert_eq!(libc::WSTOPSIG(status), libc::SIGSTOP);
                            break;
                        }
                        assert!(std::time::Instant::now() < deadline, "stop not observed");
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    if mode == "stop-killed" {
                        let identity: arc_core::process_identity::ProcessIdentity =
                            serde_json::from_value(msg["identity"].clone()).unwrap();
                        assert_eq!(
                            arc_core::process_identity::probe_live(&identity),
                            arc_core::process_identity::IdentityStatus::VerifiedMatch
                        );
                        unsafe {
                            assert_eq!(libc::kill(identity.pid as i32, libc::SIGKILL), 0);
                        }
                    }
                    unsafe {
                        libc::kill(child.child.id() as i32, libc::SIGCONT);
                    }
                }
            } else if mode == "interrupt-count" {
                pty.master.write_all(&[3]).unwrap();
                let mut ack = [0];
                barrier.set_nonblocking(true).unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    output.extend(pty.output());
                    match barrier.read(&mut ack) {
                        Ok(1) => break,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "signal not delivered: {:?}",
                                output
                            );
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        value => panic!("signal acknowledgement failed: {value:?}"),
                    }
                }
                barrier.set_nonblocking(false).unwrap();
                assert_eq!(&ack, b"s");
                barrier.write_all(b"x").unwrap();
            } else {
                pty.master
                    .write_all(&[if mode == "quit-exit" { 28 } else { 3 }])
                    .unwrap();
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let status = loop {
                output.extend(pty.output());
                if let Some(status) = child.child.try_wait().unwrap() {
                    break status;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "PTY child timeout: {}",
                    String::from_utf8_lossy(&output)
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            eprintln!("PTY {mode} managed={managed}: exited");
            if matches!(mode, "interrupt-exit" | "quit-exit" | "stop-killed") {
                assert_eq!(
                    status.signal(),
                    Some(match mode {
                        "quit-exit" => libc::SIGQUIT,
                        "stop-killed" => libc::SIGKILL,
                        _ => libc::SIGINT,
                    })
                );
            } else if matches!(mode, "tty" | "ignored-int" | "stopped") {
                assert_eq!(status.code(), Some(17));
            } else {
                assert_eq!(status.code(), Some(0));
            }
            assert_eq!(std::fs::read(restored).unwrap(), b"foreground-restored");
            after_modes.push(pty.modes());
            output.extend(pty.finish());
            outputs.push(if managed {
                super::fail_open::arc_stderr(&output)
            } else {
                output
            });
        }
        assert_eq!(
            after_modes[0], after_modes[1],
            "terminal restoration differs: {mode}"
        );
        assert_eq!(outputs[0], outputs[1], "PTY behavior differs: {mode}");
        if mode == "interrupt-count" {
            assert!(
                String::from_utf8_lossy(&outputs[1]).contains("signals=1"),
                "{:?}",
                outputs[1]
            );
        }
    }
    super::fail_open::receipt(
        "terminal-1",
        serde_json::json!({"scenario":"terminal","passed":true,"invocations":1}),
    );
}

#[test]
fn native_pty_spawn_failure_restores_foreground_without_child_output() {
    let _serial = support::serial();
    let f = Fixture::new();

    let mut pty = Pty::open();
    let restored = f.root.join("failure-restored");
    let mut command = std::process::Command::new(super::cli::binary("arc-exec-fixture"));
    command
        .arg(super::cli::binary("arcctl"))
        .args(["exec", "--", "/missing/private-executable"])
        .env("ARC_FIXTURE_LAUNCH", "spawn-failed")
        .env("ARC_FIXTURE_RESTORED", &restored)
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .stdin(Stdio::from(pty.slave.try_clone().unwrap()))
        .stdout(Stdio::from(pty.slave.try_clone().unwrap()))
        .stderr(Stdio::from(pty.slave.try_clone().unwrap()));
    let mut child = OwnedChild::new(command.spawn().unwrap());
    assert_eq!(super::cli::wait(&mut child.child).code(), Some(1));
    assert_eq!(std::fs::read(restored).unwrap(), b"foreground-restored");
    assert_eq!(pty.finish(),b"SPAWN_FAILED\r\nNo child was started. Check the executable and execution permissions.\r\n");
    super::fail_open::receipt(
        "terminal-2",
        serde_json::json!({"scenario":"terminal","passed":true,"invocations":0}),
    );
}

#[test]
fn native_background_terminal_read_can_be_foregrounded_and_resumed() {
    let _serial = support::serial();
    let f = Fixture::new();

    let mut outputs = Vec::new();
    for managed in [false, true] {
        let mut pty = Pty::open();
        let control = f.root.join(format!("bg-{managed}.sock"));
        let listener = UnixListener::bind(&control).unwrap();
        let restored = f.root.join(format!("bg-restored-{managed}"));
        let target = child_command(&f, managed);
        let mut command = std::process::Command::new(super::cli::binary("arc-exec-fixture"));
        command
            .arg(target.get_program())
            .args(target.get_args())
            .env("ARC_FIXTURE_LAUNCH", "background-tty")
            .env("ARC_FIXTURE_CONTROL", &control)
            .env("ARC_FIXTURE_RESTORED", &restored)
            .env("PRIVATE_ENV", "environment-secret-sentinel")
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .current_dir(&f.root)
            .stdin(Stdio::from(pty.slave.try_clone().unwrap()))
            .stdout(Stdio::from(pty.slave.try_clone().unwrap()))
            .stderr(Stdio::from(pty.slave.try_clone().unwrap()));
        let mut child = OwnedChild::new(command.spawn().unwrap());
        let (mut barrier, msg) = accept(&listener);
        assert_eq!(msg["foreground"], false);
        barrier.write_all(b"x").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            let mut status = 0;
            let pid = unsafe {
                libc::waitpid(
                    child.child.id() as i32,
                    &mut status,
                    libc::WUNTRACED | libc::WNOHANG,
                )
            };
            if pid > 0 {
                assert!(libc::WIFSTOPPED(status));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        unsafe {
            libc::kill(child.child.id() as i32, libc::SIGCONT);
        }
        pty.master.write_all(b"hello\n").unwrap();
        assert_eq!(super::cli::wait(&mut child.child).code(), Some(17));
        assert_eq!(std::fs::read(restored).unwrap(), b"foreground-restored");
        let output = pty.finish();
        outputs.push(if managed {
            super::fail_open::arc_stderr(&output)
        } else {
            output
        });
    }
    assert_eq!(outputs[0], outputs[1]);
    assert!(String::from_utf8_lossy(&outputs[1]).contains("input=hello"));
    super::fail_open::receipt(
        "terminal-3",
        serde_json::json!({"scenario":"terminal","passed":true,"invocations":1}),
    );
}

#[test]
fn native_session_leader_with_controlling_terminal_keeps_foreground_input() {
    let _serial = support::serial();
    let f = Fixture::new();

    let mut outputs = Vec::new();
    for managed in [false, true] {
        let mut pty = Pty::open();
        let control = f.root.join(format!("session-tty-{managed}.sock"));
        let listener = UnixListener::bind(&control).unwrap();
        let target = child_command(&f, managed);
        let mut command = std::process::Command::new(super::cli::binary("arc-exec-fixture"));
        command
            .arg(target.get_program())
            .args(target.get_args())
            .env("ARC_FIXTURE_LAUNCH", "session-exec-tty")
            .env("ARC_FIXTURE_CONTROL", &control)
            .env("PRIVATE_ENV", "environment-secret-sentinel")
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .current_dir(&f.root)
            .stdin(Stdio::from(pty.slave.try_clone().unwrap()))
            .stdout(Stdio::from(pty.slave.try_clone().unwrap()))
            .stderr(Stdio::from(pty.slave.try_clone().unwrap()));
        let mut wrapper = OwnedChild::new(command.spawn().unwrap());
        let (mut barrier, msg) = accept(&listener);
        let identity: arc_core::process_identity::ProcessIdentity =
            serde_json::from_value(msg["identity"].clone()).unwrap();
        let foreground = msg["foreground"] == true;
        if !foreground {
            // Keep RED cleanup bounded and target only a verified owned child.
            assert_eq!(
                arc_core::process_identity::probe_live(&identity),
                arc_core::process_identity::IdentityStatus::VerifiedMatch
            );
            unsafe {
                libc::kill(identity.pid as i32, libc::SIGKILL);
            }
            super::cli::wait(&mut wrapper.child);
            panic!("session leader target lost foreground terminal access");
        }
        barrier.write_all(b"x").unwrap();
        pty.master.write_all(b"hello\n").unwrap();
        assert_eq!(super::cli::wait(&mut wrapper.child).code(), Some(17));
        let output = pty.finish();
        outputs.push(if managed {
            super::fail_open::arc_stderr(&output)
        } else {
            output
        });
    }
    assert_eq!(outputs[0], outputs[1]);
    assert!(String::from_utf8_lossy(&outputs[1]).contains("input=hello"));
    super::fail_open::receipt(
        "terminal-4",
        serde_json::json!({"scenario":"terminal","passed":true,"invocations":1}),
    );
}
