//! Measured startup gaps and inherited instrumentation, using resident fixtures.
use super::*;

fn child_receipt(binary: &str, pid: u32) -> serde_json::Value {
    // wait/try_wait must have reaped this owned child, not merely sent a signal.
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    let profiles = if let Some(template) = env::var_os("LLVM_PROFILE_FILE") {
        let template = PathBuf::from(template);
        let name = template.file_name().unwrap().to_str().unwrap();
        assert!(
            name.contains("%p"),
            "child profiles need distinct PIDs: {name}"
        );
        assert!(
            name.contains("%m"),
            "child profiles need binary identities: {name}"
        );
        let expanded = name.replace("%p", &pid.to_string());
        let (prefix, suffix) = expanded.split_once("%m").unwrap();
        let profiles: Vec<_> = fs::read_dir(template.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                let name = path.file_name().unwrap().to_str().unwrap();
                name.starts_with(prefix) && name.ends_with(suffix)
            })
            .collect();
        assert!(
            !profiles.is_empty(),
            "no inherited profile for {binary} PID {pid}"
        );
        for profile in &profiles {
            assert!(fs::metadata(profile).unwrap().len() > 0);
        }
        profiles
    } else {
        Vec::new()
    };
    serde_json::json!({"binary":binary,"pid":pid,"reaped":true,"profiles":profiles})
}

fn cli_receipt(fixture: &Fixture, operation: &str) -> serde_json::Value {
    let child = Command::new(wait_for_binary("arcctl"))
        .arg(operation)
        .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.is_empty());
    child_receipt("arcctl", pid)
}

#[test]
fn startup_failures_release_admission_then_sigint_reaps_instrumented_children() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut children = Vec::new();

    // Missing default database configuration fails before touching local data.
    let mut command = fixture.command(false);
    command
        .env_remove(arc_core::ARC_DB_PATH)
        .env_remove("HOME")
        .env_remove("XDG_DATA_HOME");
    let mut rejected = Daemon(command.spawn().unwrap());
    assert_eq!(wait_for_exit(&mut rejected).code(), Some(1));
    let mut error = String::new();
    rejected
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    assert_eq!(error, "Database startup failed: HOME is not set.\n");
    assert!(!fixture.database.exists());
    assert!(!fixture.socket.exists());
    children.push(child_receipt("arc-daemon", rejected.id()));

    // This report has no private parent. Startup must unwind worker/socket/lock
    // ownership so the next real daemon can use the same database and endpoint.
    let mut command = fixture.command(false);
    command.env("ARC_RUNTIME_REPORT", "report-without-parent");
    let mut rejected = Daemon(command.spawn().unwrap());
    assert_eq!(wait_for_exit(&mut rejected).code(), Some(1));
    let mut error = String::new();
    rejected
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    assert!(
        error.contains("report needs a private parent directory"),
        "{error}"
    );
    assert!(!fixture.socket.exists());
    children.push(child_receipt("arc-daemon", rejected.id()));

    let report_path = fixture.directory.join("shutdown.json");
    let mut command = fixture.command(false);
    command.env("ARC_RUNTIME_REPORT", &report_path);
    let mut daemon = Daemon(command.spawn().unwrap());
    wait_until_ready(&mut daemon, &fixture.socket);
    for operation in ["status", "doctor"] {
        children.push(cli_receipt(&fixture, operation));
    }
    assert_eq!(unsafe { libc::kill(daemon.id() as i32, libc::SIGINT) }, 0);
    assert!(wait_for_exit(&mut daemon).success());
    children.push(child_receipt("arc-daemon", daemon.id()));
    assert!(!fixture.socket.exists());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
    assert_eq!(report["reason"], "SIGINT");
    for field in [
        "accounted",
        "handlers_reaped",
        "worker_joined",
        "connection_closed",
        "gauges_zero",
        "socket_removed",
        "deadline_met",
    ] {
        assert_eq!(report[field], true, "{field}: {report}");
    }
    assert_eq!(report["handlers_remaining"], 0);
    let receipt = serde_json::json!({"children":children,"shutdown":report,
        "inheritedProfileTemplate":env::var_os("LLVM_PROFILE_FILE")});
    if let Some(directory) = env::var_os("ARC_TEST_EVIDENCE_DIR") {
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            PathBuf::from(directory).join("coverage-lifecycle.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
    eprintln!("coverage-lifecycle: {receipt}");
}
