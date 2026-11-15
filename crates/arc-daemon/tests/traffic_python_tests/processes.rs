//! Native signal forwarding and verified nested TEST/Meter lineage.
use super::*;
use std::os::unix::process::ExitStatusExt;

pub fn signals(read: Read) {
    let case = case("signals", read, false);
    let p = projects::project(&case, "forwarded-sigterm", Kind::Pytest, true);
    let mut run = start(&case, &p, &Kind::Pytest.argv());
    let entry = entered(&case, &mut run, Kind::Pytest);
    let id = entry["jobId"].as_str().unwrap();
    assert_eq!(
        unsafe { libc::kill(run.wrapper.child.id() as i32, libc::SIGTERM) },
        0
    );
    let status = run.wait();
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    eventually(|| case.row(id)["state"] == "COMPLETED");
    let result = case.reconcile(id, "TEST", "PYTHON_PYTEST");
    assert_eq!(result["sqlite"]["outcome_kind"], "SIGNALED");
    assert_eq!(result["sqlite"]["signal"], libc::SIGTERM);
    let meter_cli = case.cli(&["meter", "summary", "--job", id, "--json"]);
    assert!(meter_cli.status.success());
    let meter: Value = serde_json::from_slice(&meter_cli.stdout).unwrap();
    assert_eq!(
        meter,
        (case.read)(&case.fixture.socket, "meter_summary", json!({"job_id":id}))
    );
    assert_eq!(
        meter["accounting"]["report"]["outcome"]["signal"],
        libc::SIGTERM
    );
    let identity: ProcessIdentity = serde_json::from_value(entry["child"].clone()).unwrap();
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
    assert_eq!(log(&p.join("launches")).lines().count(), 1);
    // A child-originated native signal is also preserved, not converted to exit 1.
    let q = projects::project(&case, "native-sigterm", Kind::Unittest, false);
    let mut native = Command::new(case.tools.shims.join("python"));
    native.args(["-m", "unittest"]).current_dir(&q);
    projects::configure(&case, &q, &mut native);
    let expected = native.env("TBS_SIGNAL", "1").output().unwrap();
    assert_eq!(expected.status.signal(), Some(libc::SIGTERM));
    write(&q.join("bodies"), "");
    write(&q.join("launches"), "");
    let mut child = start_with(&case, &q, &Kind::Unittest.argv(), |cmd| {
        cmd.env("TBS_SIGNAL", "1");
    });
    assert_eq!(child.wait().signal(), expected.status.signal());
    let child_id = case.job(child.wrapper.child.id());
    eventually(|| case.row(&child_id)["state"] == "COMPLETED");
    let child_result = case.reconcile(&child_id, "TEST", "PYTHON_UNITTEST");
    assert_eq!(child_result["sqlite"]["signal"], libc::SIGTERM);
    let interrupt = interrupt(&case);
    let history = permit_history(&case, 3, 1);
    case.save("signals", json!({"entry":entry,"terminal":result,"meter":meter,"originalSignal":status.signal(),"childOriginated":child_result,"pytestSigint":interrupt,"permits":history}));
    case.done();
}

fn interrupt(case: &Case) -> Value {
    let p = projects::project(case, "native-and-forwarded-sigint", Kind::Pytest, true);
    let mut command = Command::new(case.tools.shims.join("python"));
    command
        .args(["-m", "pytest"])
        .current_dir(&p)
        .stdin(Stdio::null())
        .stdout(File::create(p.join("direct.stdout")).unwrap())
        .stderr(File::create(p.join("direct.stderr")).unwrap());
    projects::configure(case, &p, &mut command);
    let mut direct = native::OwnedChild::new(command.spawn().unwrap());
    eventually(|| log(&p.join("direct.stderr")).contains("PYTHON_STDERR"));
    assert_eq!(
        unsafe { libc::kill(direct.child.id() as i32, libc::SIGINT) },
        0
    );
    let expected = direct.wait_with_output().unwrap();
    assert_eq!(expected.status.code(), Some(2));
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    write(&p.join("bodies"), "");
    write(&p.join("launches"), "");
    let mut run = start(case, &p, &Kind::Pytest.argv());
    let entry = entered(case, &mut run, Kind::Pytest);
    eventually(|| log(&p.join("stderr")).contains("PYTHON_STDERR"));
    assert_eq!(
        unsafe { libc::kill(run.wrapper.child.id() as i32, libc::SIGINT) },
        0
    );
    assert_eq!(run.wait().code(), expected.status.code());
    assert_eq!(
        fs::read(p.join("stdout")).unwrap(),
        fs::read(p.join("direct.stdout")).unwrap()
    );
    assert_eq!(
        fs::read(p.join("stderr")).unwrap(),
        fs::read(p.join("direct.stderr")).unwrap()
    );
    let outcome = terminal(case, &run, Kind::Pytest, 2);
    json!({"entry":entry,"directAndWrappedExit":2,"exactStreams":true,"result":outcome})
}

pub fn nested(read: Read) {
    let case = case("nested", read, false);
    let p = projects::project(&case, "root", Kind::Pytest, true);
    let q = projects::project(&case, "nested", Kind::Pytest, true);
    let mut run = start_with(&case, &p, &Kind::Pytest.argv(), |cmd| {
        cmd.env("TBS_INNER", &q);
    });
    let outer_entry = entered(&case, &mut run, Kind::Pytest);
    let root = outer_entry["jobId"].as_str().unwrap();
    eventually(|| log(&q.join("bodies")).lines().count() == 1);
    let mut nested = String::new();
    eventually(|| {
        if let Some(row) = case
            .rows("SELECT job_id,root_job_id,state FROM traffic_entries")
            .into_iter()
            .find(|r| r["root_job_id"] == root && r["state"] == "RUNNING")
        {
            nested = row["job_id"].as_str().unwrap().to_owned();
            true
        } else {
            false
        }
    });
    assert_eq!(case.active()["permits"], 1);
    assert_eq!(case.rows("SELECT count(*) AS n FROM permits")[0]["n"], 1);
    let before = case.reconcile(&nested, "TEST", "PYTHON_PYTEST");
    assert_eq!(before["diagnostic"]["admission"]["root_job_id"], root);
    let inner_identity =
        ProcessIdentity::capture(before["sqlite"]["child_pid"].as_u64().unwrap() as u32).unwrap();
    run.wrapper.own_process(inner_identity.clone());
    write(&q.join("release"), "release");
    eventually(|| case.row(&nested)["state"] == "COMPLETED");
    assert!(run.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    let inner = case.reconcile(&nested, "TEST", "PYTHON_PYTEST");
    assert_eq!(inner["diagnostic"]["admission"]["permit_active"], true);
    let meter = (case.read)(
        &case.fixture.socket,
        "meter_summary",
        json!({"job_id":nested}),
    );
    let cli = case.cli(&["meter", "summary", "--job", &nested, "--json"]);
    assert!(cli.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&cli.stdout).unwrap(), meter);
    assert_eq!(
        meter["accounting"]["report"]["accounting_relation"],
        json!({"kind":"included_in_root","root_job_id":root})
    );
    run.finish(0);
    let outer = terminal(&case, &run, Kind::Pytest, 0);
    assert_eq!(
        outer["meter"]["accounting"]["report"]["accounting_relation"],
        json!({"kind":"canonical_root","root_job_id":root})
    );
    assert_eq!(log(&q.join("launches")).lines().count(), 1);
    assert_eq!(probe_live(&inner_identity), IdentityStatus::Missing);
    let after = case.reconcile(&nested, "TEST", "PYTHON_PYTEST");
    assert_eq!(after["diagnostic"]["admission"]["permit_active"], false);
    let history = permit_history(&case, 1, 1);
    case.save("nested", json!({"root":outer_entry,"innerRunning":before,"innerCompletedRootLive":inner,"innerMeter":meter,"rootCompleted":outer,"innerAfterRoot":after,"permits":history}));
    case.done();
}
