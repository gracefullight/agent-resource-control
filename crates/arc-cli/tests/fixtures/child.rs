//! Native child fixture; never contacts external services.
#[cfg(target_os = "macos")]
mod frozen_traffic_v1;
mod hold_unattached;
mod pipeline;
use std::{
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, net::UnixStream},
    sync::atomic::{AtomicI32, AtomicUsize, Ordering},
};
static ACK: AtomicI32 = AtomicI32::new(-1);
static SIGNALS: AtomicUsize = AtomicUsize::new(0);
extern "C" fn observed(_: i32) {
    SIGNALS.fetch_add(1, Ordering::SeqCst);
    unsafe {
        libc::write(ACK.load(Ordering::SeqCst), b"s".as_ptr().cast(), 1);
    }
}
fn main() {
    if std::env::var_os("ARC_FIXTURE_HOLD_UNATTACHED").is_some() {
        hold_unattached::run();
        return;
    }
    #[cfg(target_os = "macos")]
    if std::env::var_os("ARC_FIXTURE_FROZEN_TRAFFIC_V1").is_some() {
        frozen_traffic_v1::run();
        return;
    }
    if std::env::var_os("ARC_FIXTURE_PIPE_ROLE").is_some() {
        pipeline::run();
        return;
    }
    if let Ok(mode) = std::env::var("ARC_FIXTURE_LAUNCH") {
        use std::os::unix::process::CommandExt;
        unsafe {
            let prior = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
            assert!(libc::setsid() >= 0);
            assert!(libc::ioctl(0, libc::TIOCSCTTY as _, 0) >= 0);
            assert!(libc::tcsetpgrp(0, libc::getpgrp()) >= 0);
            libc::signal(libc::SIGTTOU, prior);
        }
        if mode == "ignored-int" {
            unsafe {
                libc::signal(libc::SIGINT, libc::SIG_IGN);
            }
        }
        std::env::remove_var("ARC_FIXTURE_LAUNCH");
        std::env::set_var(
            "ARC_FIXTURE_MODE",
            if mode == "session-exec-tty" {
                "tty"
            } else {
                &mode
            },
        );
        let mut args = std::env::args_os().skip(1);
        let mut command = std::process::Command::new(args.next().unwrap());
        command.args(args);
        if mode == "session-exec-tty" {
            let _ = command.exec();
            std::process::exit(1);
        }
        if mode == "background-tty" {
            command.process_group(0);
        }
        // This native session owner remains alive to observe foreground
        // restoration before its controlling terminal is revoked on exit.
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_IGN);
            libc::signal(libc::SIGQUIT, libc::SIG_IGN);
        }
        let ignored = mode == "ignored-int";
        unsafe {
            command.pre_exec(move || {
                libc::signal(
                    libc::SIGINT,
                    if ignored {
                        libc::SIG_IGN
                    } else {
                        libc::SIG_DFL
                    },
                );
                libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                Ok(())
            });
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => std::process::exit(1),
        };
        loop {
            let mut info = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    &mut info,
                    libc::WEXITED | libc::WSTOPPED | libc::WNOWAIT,
                )
            };
            assert_eq!(result, 0);
            if info.si_code != libc::CLD_STOPPED {
                break;
            }
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    &mut info,
                    libc::WSTOPPED | libc::WNOHANG,
                );
            }
            assert_eq!(
                unsafe { libc::tcgetpgrp(0) },
                unsafe { libc::getpgrp() },
                "foreground must be restored on suspend"
            );
            unsafe {
                libc::raise(libc::SIGSTOP);
                if mode == "background-tty" {
                    assert_eq!(libc::tcsetpgrp(0, child.id() as i32), 0);
                }
                libc::kill(
                    if mode == "background-tty" {
                        -(child.id() as i32)
                    } else {
                        child.id() as i32
                    },
                    libc::SIGCONT,
                );
            }
        }
        let status = child.wait().unwrap();
        if mode == "background-tty" {
            unsafe {
                let prior = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
                assert_eq!(libc::tcsetpgrp(0, libc::getpgrp()), 0);
                libc::signal(libc::SIGTTOU, prior);
            }
        }
        assert_eq!(
            unsafe { libc::tcgetpgrp(0) },
            unsafe { libc::getpgrp() },
            "foreground must be restored on exit"
        );
        if let Some(path) = std::env::var_os("ARC_FIXTURE_RESTORED") {
            std::fs::write(path, b"foreground-restored").unwrap();
        }
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
                libc::raise(signal);
            }
        }
        std::process::exit(status.code().unwrap_or(1));
    }

    if std::env::var_os("ARC_FIXTURE_MODE").is_none() {
        return;
    }
    if let Some(marker) = std::env::var_os("ARC_FIXTURE_MARKER") {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(marker)
            .unwrap();
        writeln!(file, "{}", std::process::id()).unwrap();
    }
    let args: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(|s| s.as_bytes().to_vec())
        .collect();
    let mode = std::env::var("ARC_FIXTURE_MODE").unwrap();
    if mode == "interrupt-count" {
        unsafe {
            libc::signal(libc::SIGINT, observed as *const () as usize);
        }
    }
    if let Some(control) = std::env::var_os("ARC_FIXTURE_CONTROL") {
        let mut stream = UnixStream::connect(control).unwrap();
        use std::os::fd::AsRawFd;
        ACK.store(stream.as_raw_fd(), Ordering::SeqCst);
        let identity =
            arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
        let mut action = unsafe { std::mem::zeroed() };
        let mut continuation: libc::sigaction = unsafe { std::mem::zeroed() };
        let mut mask = unsafe { std::mem::zeroed() };
        unsafe {
            assert_eq!(
                libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action),
                0
            );
            assert_eq!(
                libc::sigaction(libc::SIGCONT, std::ptr::null(), &mut continuation),
                0
            );
            assert_eq!(
                libc::sigprocmask(libc::SIG_SETMASK, std::ptr::null(), &mut mask),
                0
            );
        }
        let msg = serde_json::to_vec(&serde_json::json!({"identity":identity,"args":args,
            "sigcont_ignored":continuation.sa_sigaction == libc::SIG_IGN,
            "sigchld_ignored":action.sa_sigaction == libc::SIG_IGN,
            "sigchld_blocked":unsafe {libc::sigismember(&mask,libc::SIGCHLD)} == 1,
            "foreground":unsafe {libc::tcgetpgrp(0)} == unsafe {libc::getpgrp()},
            "tty":unsafe {libc::isatty(0)} == 1,"pgid":unsafe{libc::getpgrp()},"pid":std::process::id(),"ppid":unsafe{libc::getppid()}}))
        .unwrap();
        arc_ipc::write_frame(&mut stream, &msg).unwrap();
        let mut action = [0];
        stream.read_exact(&mut action).unwrap();
    }
    if mode == "interrupt-count" {
        println!("signals={}", SIGNALS.load(Ordering::SeqCst));
        return;
    }
    if mode == "ignored-int" {
        unsafe {
            libc::raise(libc::SIGINT);
        }
    }
    if mode == "stopped" || mode == "stop-killed" {
        unsafe {
            libc::raise(libc::SIGSTOP);
        }
    }
    if mode == "term" {
        unsafe {
            libc::raise(libc::SIGTERM);
        }
    }
    if mode == "tty" || mode == "background-tty" {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        println!("input={}", line.trim_end());
    } else if mode == "echo" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        std::io::stdout().write_all(&bytes).unwrap();
    }
    let metadata = serde_json::json!({"args":args,"cwd":std::env::current_dir().unwrap().as_os_str().as_bytes(),
        "environment":std::env::var_os("PRIVATE_ENV").map(|s|s.as_bytes().to_vec()),
        "stdin_open":unsafe {libc::fcntl(0,libc::F_GETFD)} >= 0});
    println!("{metadata}");
    eprintln!("fixture-stderr");
    if mode == "success" {
        return;
    }
    std::process::exit(17);
}
