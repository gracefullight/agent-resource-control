//! Controlled accounting workload used by the deterministic CLI tests.
use std::{
    fs::OpenOptions,
    io::Write,
    process::Command,
    time::{Duration, Instant},
};

pub fn run(mode: &str) {
    if let Some(marker) = std::env::var_os("ARC_METER_TEST_MARKER") {
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
    if let Some(ready) = std::env::var_os("ARC_METER_TEST_READY") {
        std::fs::write(ready, std::process::id().to_string()).unwrap();
    }
    match mode {
        "hold" => {
            let release = std::env::var_os("ARC_METER_TEST_RELEASE").unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while !std::path::Path::new(&release).exists() {
                assert!(Instant::now() < deadline, "owned hold fixture timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        "escaped" => {
            // A bounded owned descendant outlives its parent. Record identity
            // before exit so acceptance can prove it is missing from wait4.
            let mut child = Command::new(std::env::current_exe().unwrap())
                .env("ARC_FIXTURE_ACCOUNTING", "orphan-work")
                .env_remove("ARC_METER_TEST_MARKER")
                .spawn()
                .unwrap();
            let identity =
                arc_core::process_identity::ProcessIdentity::capture(child.id()).unwrap();
            std::fs::write(
                std::env::var_os("ARC_METER_TEST_ESCAPED").unwrap(),
                serde_json::to_vec(&identity).unwrap(),
            )
            .unwrap();
            // Dropping Child neither waits nor terminates it. It self-terminates
            // within 400ms, even if the acceptance observer fails.
            let _ = child.try_wait().unwrap();
            return;
        }
        "orphan-work" => {
            let start = Instant::now();
            while start.elapsed() < Duration::from_millis(400) {
                std::hint::black_box(start.elapsed());
            }
            return;
        }
        "nested" | "nested-scheduled" => {
            let mut child = Command::new(std::env::var_os("ARC_METER_TEST_CLI").unwrap());
            child.args(["exec", "--"]);
            if mode == "nested-scheduled" {
                child.args(["cargo", "build"]);
            } else {
                child.arg(std::env::current_exe().unwrap());
            }
            let status = child
                .env("ARC_FIXTURE_ACCOUNTING", "work")
                .status()
                .unwrap();
            std::process::exit(status.code().unwrap_or(1));
        }
        "stop" => {
            assert_eq!(unsafe { libc::raise(libc::SIGSTOP) }, 0);
        }
        "signal" => {
            unsafe { libc::raise(libc::SIGTERM) };
            unreachable!();
        }
        "quick" => std::process::exit(17),
        _ => (),
    }
    let start = Instant::now();
    let mut work = 0u64;
    while start.elapsed() < Duration::from_millis(120) {
        for _ in 0..1000 {
            work = std::hint::black_box(work.wrapping_mul(17).wrapping_add(1));
        }
    }
    std::hint::black_box(work);
    std::process::exit(if mode == "success" { 0 } else { 17 });
}
