//! Native child fixture; never contacts external services.
#[cfg(target_os = "macos")]
mod frozen_traffic_v1;
mod hold_unattached;
mod hook_capture;
mod job_meter;
mod pipeline;
// Compile the same execution implementation into this test fixture so the
// accounting tests can supply a clock without adding a production CLI switch.
#[cfg(not(test))]
#[allow(dead_code, unused_imports)]
#[path = "../../src/exec"]
mod exec {
    mod accounting;
    mod capture;
    mod ci_context;
    mod descriptors;
    mod endpoint_context;
    mod entry;
    mod event_key;
    mod governor;
    mod launch;
    mod lifecycle;
    mod lineage;
    mod metadata;
    mod native_environment;
    pub(crate) mod policy;
    mod preparation;
    mod project_key;
    mod proof;
    mod run;
    mod signals;
    mod stages;
    mod terminal;
    mod traffic;
    pub(crate) mod transport;
    mod witness;
    pub(crate) use capture::capture_argv;
    pub(crate) use entry::{run, run_hook, run_with_environment, run_with_environment_and_notice};
    use lifecycle::start_until;
    pub(crate) use preparation::invalid;
    pub(crate) use preparation::{write_notice_bounded, write_notice_for_fixture};

    pub(crate) fn synthetic_endpoint_exec(
        argv: Vec<std::ffi::OsString>,
        synthetic_local: bool,
    ) -> std::process::ExitCode {
        let context: arc_detector::EndpointContext = serde_json::from_value(serde_json::json!({
            "captured_at_ms":10000,
            "facts":[
                {"kind":"DOCKER_CONTEXT","selector":"prod","endpoints":["tcp://192.0.2.40:2376"]},
                {"kind":"DOCKER_BUILDER","selector":"prod","endpoints":["tcp://192.0.2.40:2376"]},
                {"kind":"SSH_DESTINATION","selector":"@local-addresses","endpoints":["192.0.2.5"]}
            ]
        }))
        .unwrap();
        run::run_with_classifier(
            argv,
            false,
            &std::time::Instant::now,
            &std::time::Instant::now,
            &|args| {
                if synthetic_local {
                    // Exercise negotiated local admission independently of
                    // whether endpoint discovery can prove local compute.
                    arc_detector::Classification {
                        classification: arc_detector::JobClass::Build,
                        locality: arc_detector::Locality::HostLocal,
                        reason: arc_detector::Reason::LocalEndpointBuild,
                        uncertainty: arc_detector::Uncertainty::None,
                    }
                } else {
                    metadata::classify_with_context(args, &context, || 10000)
                }
            },
        )
    }

    // Exercise the production run path with an injected preparation clock.
    // The synthetic elapsed interval surrounds actual endpoint classification.
    pub(crate) fn delayed_capture_exec(argv: Vec<std::ffi::OsString>) -> std::process::ExitCode {
        use std::{
            cell::Cell,
            time::{Duration, Instant},
        };
        let captured = Cell::new(false);
        let clock = || {
            if captured.get() {
                Instant::now()
            } else {
                Instant::now() - Duration::from_secs(3)
            }
        };
        run::run_with_classifier_and_preparation_clock(
            argv,
            false,
            &Instant::now,
            &Instant::now,
            &|args| {
                let before = clock();
                let classification = metadata::classify(args);
                captured.set(true);
                assert!(clock().duration_since(before) >= Duration::from_secs(3));
                classification
            },
            &clock,
        )
    }
}
#[cfg(not(test))]
#[path = "../../src/event_digest.rs"]
mod event_digest;
#[cfg(not(test))]
#[allow(dead_code)]
#[path = "../../src/hooks.rs"]
mod hooks;
// Exercise the actual CLI status implementation with a controlled clock. The
// production arcctl entrypoint has no fixture switch or clock-file input.
#[cfg(not(test))]
#[allow(dead_code)]
#[path = "../../src/job.rs"]
mod job;
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
    #[cfg(not(test))]
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--delayed-capture-exec"))
    {
        let result = exec::delayed_capture_exec(std::env::args_os().skip(2).collect());
        std::process::exit(if result == std::process::ExitCode::from(17) {
            17
        } else {
            1
        });
    }
    #[cfg(not(test))]
    if std::env::args_os()
        .next()
        .and_then(|program| {
            std::path::Path::new(&program)
                .file_name()
                .map(|s| s.to_owned())
        })
        .as_deref()
        == Some(std::ffi::OsStr::new("docker"))
        && std::env::var_os("ARC_FIXTURE_ENV_REPORT").is_some()
    {
        let environment: std::collections::BTreeMap<_, _> = std::env::vars().collect();
        println!(
            "{}",
            serde_json::json!({
                "environment":environment,
                "pid":std::process::id(),
                "argv":std::env::args().collect::<Vec<_>>(),
                "cwd":std::env::current_dir().unwrap()
            })
        );
        std::io::stdout().flush().unwrap();
        let mut release = String::new();
        std::io::stdin().read_line(&mut release).unwrap();
        std::process::exit(if release == "release\n" { 23 } else { 2 });
    }
    #[cfg(not(test))]
    {
        let mode = std::env::args_os().nth(1);
        let synthetic_local =
            mode.as_deref() == Some(std::ffi::OsStr::new("--synthetic-local-endpoint-exec"));
        if synthetic_local
            || mode.as_deref() == Some(std::ffi::OsStr::new("--synthetic-endpoint-exec"))
        {
            let key = std::env::var_os("ARC_FIXTURE_ENDPOINT_KEY")
                .and_then(|path| std::fs::read(path).ok());
            if key.as_deref() != Some(b"arc-synthetic-endpoint-v1") {
                std::process::exit(2);
            }
            let code = exec::synthetic_endpoint_exec(
                std::env::args_os().skip(2).collect(),
                synthetic_local,
            );
            std::process::exit(if code == std::process::ExitCode::from(23) {
                23
            } else {
                i32::from(code != std::process::ExitCode::SUCCESS)
            });
        }
    }
    #[cfg(not(test))]
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--clocked-fixture-job-status"))
    {
        let key =
            std::env::var_os("ARC_FIXTURE_CLOCK_KEY").and_then(|path| std::fs::read(path).ok());
        if key.as_deref() != Some(b"arc-fixture-clock-v1") {
            std::process::exit(2);
        }
        let clock = std::env::var_os("ARC_FIXTURE_CLOCK").expect("fixture clock path");
        let origin = std::time::Instant::now();
        let now = || {
            let millis: u64 = std::fs::read_to_string(&clock).unwrap().parse().unwrap();
            origin + std::time::Duration::from_millis(millis)
        };
        let status = job::run_with_clock(None, &now);
        std::process::exit(i32::from(status != std::process::ExitCode::SUCCESS));
    }
    #[cfg(not(test))]
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--clocked-fixture-exec"))
    {
        let key =
            std::env::var_os("ARC_FIXTURE_CLOCK_KEY").and_then(|path| std::fs::read(path).ok());
        if key.as_deref() != Some(b"arc-fixture-clock-v1") {
            std::process::exit(2);
        }
        let clock = std::env::var_os("ARC_FIXTURE_CLOCK").expect("fixture clock path");
        let args = std::env::args_os().skip(2).collect();
        let origin = std::time::Instant::now();
        let now = || {
            let seconds: u64 = std::fs::read_to_string(&clock).unwrap().parse().unwrap();
            origin + std::time::Duration::from_secs(seconds)
        };
        let writer = if std::env::var_os("ARC_FIXTURE_CONTROLLED_NOTICE").is_some() {
            exec::write_notice_for_fixture
        } else {
            // The normal fixture path retains the production bounded writer.
            exec::write_notice_bounded
        };
        let result = exec::run_with_environment_and_notice(args, false, &now, writer);
        let status = if result == std::process::ExitCode::SUCCESS {
            0
        } else if result == std::process::ExitCode::from(17) {
            17
        } else {
            1
        };
        std::process::exit(status);
    }
    if let Ok(mode) = std::env::var("ARC_FIXTURE_ACCOUNTING") {
        job_meter::run(&mode);
        return;
    }
    hook_capture::run_if_requested();
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
    // A target that survives graceful termination, for escalation fixtures.
    if mode == "ignored-term" {
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
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

#[cfg(not(test))]
fn request_id() -> String {
    format!(
        "fixture-{}",
        arc_core::process_identity::random_job_id().unwrap()
    )
}
