//! Barrier-controlled native pipeline participants. The peer reads /dev/tty.
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::atomic::{AtomicI32, AtomicUsize, Ordering},
};
static COUNT: AtomicUsize = AtomicUsize::new(0);
static CODE: AtomicI32 = AtomicI32::new(0);
static SENDER: AtomicI32 = AtomicI32::new(0);
extern "C" fn signal(_: i32, info: *mut libc::siginfo_t, _: *mut libc::c_void) {
    // Only lock-free atomics and siginfo reads in the fixture's handler.
    COUNT.fetch_add(1, Ordering::SeqCst);
    unsafe {
        CODE.store((*info).si_code, Ordering::SeqCst);
        SENDER.store((*info).si_pid, Ordering::SeqCst);
    }
}
fn send(stream: &mut UnixStream, value: serde_json::Value) {
    arc_ipc::write_frame(stream, &serde_json::to_vec(&value).unwrap()).unwrap();
}
pub fn run() {
    let role = std::env::var("ARC_FIXTURE_PIPE_ROLE").unwrap();
    let mut continuation: libc::sigaction = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::sigaction(libc::SIGCONT, std::ptr::null(), &mut continuation) },
        0
    );
    if role == "producer" {
        if let Some(marker) = std::env::var_os("ARC_FIXTURE_MARKER") {
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(marker)
                    .unwrap(),
                "{}",
                std::process::id()
            )
            .unwrap();
        }
    }
    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .unwrap();
    for number in [libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGHUP] {
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = signal as *const () as usize;
            action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            assert_eq!(libc::sigaction(number, &action, std::ptr::null_mut()), 0);
        }
    }
    let mut stream =
        UnixStream::connect(std::env::var_os("ARC_FIXTURE_PIPE_CONTROL").unwrap()).unwrap();
    let identity =
        arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
    send(
        &mut stream,
        serde_json::json!({"role":role,"identity":identity,"sigcont_ignored":continuation.sa_sigaction == libc::SIG_IGN,
        "pgid":unsafe {libc::getpgrp()}, "foreground":unsafe {libc::tcgetpgrp(2)}}),
    );
    loop {
        let mut command = [0];
        if stream.read_exact(&mut command).is_err() {
            break;
        }
        match command[0] {
            b'p' => {
                let data = b"producer-data\n";
                if role == "producer" {
                    std::io::stdout().write_all(data).unwrap();
                } else {
                    let mut actual = vec![0; data.len()];
                    std::io::stdin().read_exact(&mut actual).unwrap();
                    assert_eq!(actual, data);
                }
                send(&mut stream, serde_json::json!({"pipeline":true}));
            }
            b't' => {
                let mut input = Vec::new();
                loop {
                    let mut byte = [0];
                    tty.read_exact(&mut byte).unwrap();
                    input.push(byte[0]);
                    if byte[0] == b'\n' {
                        break;
                    }
                }
                send(
                    &mut stream,
                    serde_json::json!({"input":input,
                    "pgid":unsafe {libc::getpgrp()},"foreground":unsafe {libc::tcgetpgrp(2)}}),
                );
            }
            b'c' => send(
                &mut stream,
                serde_json::json!({"signals":COUNT.load(Ordering::SeqCst),
                "code":CODE.load(Ordering::SeqCst),"sender":SENDER.load(Ordering::SeqCst)}),
            ),
            b'x' => std::process::exit(if role == "producer" { 17 } else { 0 }),
            _ => panic!("unknown fixture control"),
        }
    }
}
