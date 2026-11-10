use super::*;

#[derive(Default)]
struct Intervals {
    active: usize,
    peak: usize,
}
impl Intervals {
    fn started(&mut self) {
        self.active += 1;
        self.peak = self.peak.max(self.active);
    }
    fn ended(&mut self) {
        self.active = self.active.checked_sub(1).expect("observed start");
    }
}

// The only socket opened by this queued wrapper is its admission poll. Observe
// separate socket lifetimes without stopping the process or proxying its peer ID.
fn queued_poll_sockets(pid: u32, project: &Path) -> usize {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut previous = false;
    let mut observed = 0;
    while observed < 2 {
        let mut fds: [libc::proc_fdinfo; 64] =
            std::array::from_fn(|_| unsafe { std::mem::zeroed() });
        // SAFETY: fds is an initialized buffer and its size is passed in bytes.
        let bytes = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr().cast(),
                std::mem::size_of_val(&fds) as i32,
            )
        };
        assert!(
            bytes > 0,
            "cannot observe queued wrapper descriptors: {}",
            std::io::Error::last_os_error()
        );
        assert!(
            (bytes as usize) < std::mem::size_of_val(&fds),
            "descriptor list truncated"
        );
        let socket = fds[..bytes as usize / std::mem::size_of::<libc::proc_fdinfo>()]
            .iter()
            .any(|fd| fd.proc_fdtype == libc::PROX_FDTYPE_SOCKET as u32);
        if socket && !previous {
            observed += 1;
        }
        previous = socket;
        assert!(
            !project.join("cargo-launches").exists(),
            "queued job spawned before release"
        );
        assert!(
            Instant::now() < deadline,
            "did not observe separate queued polls"
        );
        thread::sleep(Duration::from_micros(100));
    }
    observed
}

#[test]
fn two_real_builds_queue_fifo_and_bypass_remote_and_unknown_while_full() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    install_cargo_observer(&f);
    let bypass = projects(&f.root, "bypass");
    let mut first = spawn_observed_build(&f, &bypass[0], false);
    let mut second = spawn_observed_build(&f, &bypass[1], false);
    eventually(|| bypass.iter().all(|path| path.join("started").exists()));
    assert!(cargo_pid(first.child.id()) && cargo_pid(second.child.id()));
    assert!(
        first.child.try_wait().unwrap().is_none() && second.child.try_wait().unwrap().is_none()
    );
    let mut bypass_intervals = Intervals::default();
    bypass_intervals.started();
    bypass_intervals.started();
    assert_eq!(cargo_launches(&bypass[0]), [first.child.id()]);
    assert_eq!(cargo_launches(&bypass[1]), [second.child.id()]);
    for project in &bypass {
        fs::write(project.join("release"), "release").unwrap();
    }
    finish(&mut first);
    bypass_intervals.ended();
    finish(&mut second);
    bypass_intervals.ended();
    let bypass_invocations = bypass.each_ref().map(|path| cargo_launches(path).len());
    assert_eq!(bypass_invocations, [1, 1]);
    assert_eq!(bypass_intervals.peak, 2);
    assert_eq!(bypass_intervals.active, 0);
    let independent = fs::canonicalize(&bypass[0]).unwrap()
        != fs::canonicalize(&bypass[1]).unwrap()
        && fs::canonicalize(bypass[0].join("target")).unwrap()
            != fs::canonicalize(bypass[1].join("target")).unwrap();
    assert!(independent);

    let scheduled = projects(&f.root, "scheduled");
    let mut first = spawn_observed_build(&f, &scheduled[0], true);
    eventually(|| {
        scheduled[0].join("started").exists()
            && entry(&f, first.child.id()).is_some_and(|e| e.1.is_some())
    });
    let first_entry = entry(&f, first.child.id()).unwrap();
    assert_eq!(first_entry.0, "RUNNING");
    assert!(cargo_pid(first_entry.1.unwrap()));
    assert_eq!(children(first.child.id()), vec![first_entry.1.unwrap()]);
    assert_eq!(cargo_launches(&scheduled[0]), [first_entry.1.unwrap()]);
    let mut scheduled_intervals = Intervals::default();
    scheduled_intervals.started();
    let mut second = spawn_observed_build(&f, &scheduled[1], true);
    eventually(|| {
        fs::read_to_string(scheduled[1].join("stderr"))
            .unwrap()
            .contains("QUEUED\nWaiting to start.")
            && entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED")
    });
    assert!(!scheduled[1].join("started").exists());
    eventually(|| children(second.child.id()).is_empty());
    assert!(first.child.try_wait().unwrap().is_none());
    let queued_id: String = database(&f)
        .query_row(
            "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
            [second.child.id()],
            |r| r.get(0),
        )
        .unwrap();
    for args in [
        vec!["job", "status"],
        vec!["job", "status", queued_id.as_str()],
    ] {
        let status = Command::new(binary("arcctl"))
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .output()
            .unwrap();
        assert!(status.status.success());
        assert!(String::from_utf8(status.stdout)
            .unwrap()
            .contains("State: queued\n"));
    }
    let observed_polls = queued_poll_sockets(second.child.id(), &scheduled[1]);
    let before = active(&f);
    assert_eq!(before.0, 2);
    assert_eq!(before.1, 1);

    for (program, args) in [
        ("gh", vec!["workflow", "run"]),
        ("unknown-tool", vec!["test"]),
    ] {
        symlink(binary("arc-exec-fixture"), f.root.join(program)).unwrap();
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--", program])
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env(
                "PATH",
                format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
            )
            .env("ARC_FIXTURE_MODE", "echo")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(17), "{output:?}");
        assert_eq!(active(&f), before);
    }
    assert!(!scheduled[1].join("cargo-launches").exists());
    assert!(entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED" && e.3.is_none()));
    fs::write(scheduled[0].join("release"), "release").unwrap();
    // The second launch marker persists, so observing it while the first
    // Cargo process is alive proves overlapping scheduled builds.
    loop {
        let first_running = first.child.try_wait().unwrap().is_none();
        if cargo_pid(first_entry.1.unwrap()) && scheduled[1].join("cargo-launches").exists() {
            panic!("second Cargo started while the first Cargo was running");
        }
        if !first_running {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    finish(&mut first);
    scheduled_intervals.ended();
    eventually(|| entry(&f, first.child.id()).is_some_and(|e| e.4.is_some()));
    let release = entry(&f, first.child.id()).unwrap().4.unwrap();
    eventually(|| entry(&f, second.child.id()).is_some_and(|e| e.3.is_some()));
    let grant = entry(&f, second.child.id()).unwrap().3.unwrap();
    assert!(grant >= release);
    eventually(|| {
        scheduled[1].join("started").exists()
            && entry(&f, second.child.id()).is_some_and(|e| e.1.is_some())
    });
    let second_entry = entry(&f, second.child.id()).unwrap();
    assert_eq!(second_entry.3, Some(grant));
    assert!(cargo_pid(second_entry.1.unwrap()));
    assert_eq!(children(second.child.id()), vec![second_entry.1.unwrap()]);
    assert_eq!(cargo_launches(&scheduled[1]), [second_entry.1.unwrap()]);
    let second_launch_ms = fs::metadata(scheduled[1].join("cargo-launches"))
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    assert!(
        second_launch_ms >= release as u128,
        "second Cargo launched before the committed release"
    );
    scheduled_intervals.started();
    let queue_wait = second_entry.3.unwrap() - second_entry.2.unwrap();
    fs::write(scheduled[1].join("release"), "release").unwrap();
    finish(&mut second);
    scheduled_intervals.ended();
    let scheduled_invocations = scheduled.each_ref().map(|path| cargo_launches(path).len());
    assert_eq!(scheduled_invocations, [1, 1]);
    assert_eq!(scheduled_intervals.peak, 1);
    assert_eq!(scheduled_intervals.active, 0);
    let independent = independent
        && fs::canonicalize(&scheduled[0]).unwrap() != fs::canonicalize(&scheduled[1]).unwrap()
        && fs::canonicalize(scheduled[0].join("target")).unwrap()
            != fs::canonicalize(scheduled[1].join("target")).unwrap();
    assert!(independent);
    let outcomes: Vec<(String, Option<i64>)> = {
        let db = database(&f);
        let mut stmt = db.prepare("SELECT t.state,j.exit_code FROM traffic_entries t JOIN jobs j ON j.id=t.job_id ORDER BY t.sequence").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        outcomes,
        vec![("COMPLETED".into(), Some(0)), ("COMPLETED".into(), Some(0))]
    );
    assert!(fs::read_to_string(scheduled[1].join("stderr"))
        .unwrap()
        .contains("STARTED\nCommand started."));
    let completion_outcomes: Vec<_> = outcomes
        .iter()
        .map(|(_, code)| format!("EXITED:{}", code.unwrap()))
        .collect();
    println!(
        "TRAFFIC_OBSERVATION {}",
        serde_json::json!({"platform":"macOS","independentDirectoriesVerified":independent,"bypass":{"peakConcurrentBuilds":bypass_intervals.peak,"invocations":bypass_invocations},"scheduled":{"peakConcurrentBuilds":scheduled_intervals.peak,"invocations":scheduled_invocations,"queuedBeforeFirstRelease":true,"secondStartedAfterRelease":true,"queuedPollSocketsObserved":observed_polls,"firstPermitReleasedAtMs":release,"secondGrantAtMs":grant,"secondCargoLaunchedAtMs":second_launch_ms,"queueWaitMs":queue_wait,"queueWaitRecorded":true,"completionOutcomes":completion_outcomes,"completionOutcomesRecorded":true}})
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}
