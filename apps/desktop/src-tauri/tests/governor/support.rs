use super::native_support::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::{json, Value};
use std::{
    fs,
    io::BufRead,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
};

pub fn save_case(id: &str, value: Value) {
    let root = std::env::var_os("ARC_GOVERNOR_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/governor/task8/native")
        });
    fs::create_dir_all(&root).unwrap();
    let value = json!({"case":id,"platform":std::env::consts::OS,
        "pressureSource":"explicit injection through production Meter worker; other host fields use SystemSource",
        "transport":"production owner-authenticated socket, arcctl and registered Tauri router; MockRuntime shell only",
        "observations":value});
    fs::write(
        root.join(format!("{id}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
    eprintln!("governor-native-observation={value}");
}
pub fn intent(base: u64, enabled: bool, nonce: u32) -> Value {
    json!({"key":"governor.enabled","enabled":enabled,"expected_revision":base,
        "mutation_id":format!("{base}-{nonce:032x}")})
}
pub fn cli_set(case: &Case, request: &Value) -> Value {
    let base = request["expected_revision"].to_string();
    let output = case.cli(&[
        "daemon",
        if request["enabled"] == true {
            "resume"
        } else {
            "pause"
        },
        "--json",
        "--expected-revision",
        &base,
        "--mutation-id",
        request["mutation_id"].as_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}
pub fn bridge_set(case: &Case, request: &Value) -> Value {
    case.router
        .call("governor_set_switch", json!({"request":request}))
        .unwrap()["data"]
        .clone()
}
pub fn switches(case: &Case, enabled: bool, revision: u64, mutation_id: Option<&str>) -> Value {
    let query = json!({"key":"governor.enabled","mutation_id":mutation_id});
    let bridge = case
        .router
        .call("governor_get_switch", json!({"query":query}))
        .unwrap();
    let output = case.cli(&["daemon", "governor", "--json"]);
    assert!(output.status.success(), "{output:?}");
    let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
    let state = &bridge["data"]["switch"];
    assert_eq!(cli["switch"], *state);
    assert_eq!(state["enabled"], enabled);
    assert_eq!(state["revision"], revision);
    let sqlite = case.rows("SELECT key,enabled,policy_revision,updated_at FROM policy_states WHERE key='governor.enabled'");
    if revision == 0 && sqlite.is_empty() {
        assert!(enabled);
        assert!(state["updated_at_ms"].is_null());
    } else {
        assert_eq!(sqlite.len(), 1);
        assert_eq!(sqlite[0]["enabled"], u8::from(enabled));
        assert_eq!(sqlite[0]["policy_revision"], revision);
        assert_eq!(sqlite[0]["updated_at"], state["updated_at_ms"]);
    }
    json!({"sqlite":sqlite,"cli":cli,"registeredBridge":bridge})
}
pub fn permits(case: &Case, total: usize, active: usize) -> Value {
    let rows=case.rows("SELECT job_id,generation,state,cpu_millis_est,memory_bytes_est,reserved_at,released_at FROM permits ORDER BY generation");
    assert_eq!(rows.len(), total, "{rows:?}");
    assert_eq!(
        rows.iter().filter(|p| p["state"] == "ACTIVE").count(),
        active,
        "{rows:?}"
    );
    for p in &rows {
        assert!(p["generation"].as_u64().is_some_and(|v| v > 0));
        if p["state"] == "RELEASED" {
            assert!(p["released_at"].as_u64().unwrap() >= p["reserved_at"].as_u64().unwrap());
        }
    }
    json!(rows)
}
pub fn queued(case: &Case, run: &mut Run) -> String {
    let id = case.job(run.wrapper.child.id());
    eventually(|| {
        case.rows("SELECT job_id,state FROM traffic_entries")
            .iter()
            .any(|r| r["job_id"] == id && r["state"] == "QUEUED")
    });
    assert_eq!(run.count(), 0);
    assert!(run.wrapper.child.try_wait().unwrap().is_none());
    id
}
pub fn alive(identity: &ProcessIdentity) {
    assert_eq!(probe_live(identity), IdentityStatus::VerifiedMatch);
}
pub fn job(case: &Case, id: &str, run: &Run, child: &ProcessIdentity, terminal: bool) -> Value {
    eventually(|| {
        case.rows("SELECT job_id,child_pid FROM job_lifecycle")
            .iter()
            .any(|r| r["job_id"] == id && r["child_pid"] == child.pid)
    });
    let rows=case.rows("SELECT l.job_id,l.child_pid,l.child_start_token,l.child_boot_id,l.outcome_kind,j.exit_code,l.revision,t.state,t.generation,g.mode,g.release_id,g.previous_generation FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id JOIN traffic_entries t USING(job_id) JOIN governor_jobs g USING(job_id)");
    let row = rows.into_iter().find(|r| r["job_id"] == id).unwrap();
    assert_eq!(row["child_pid"], child.pid);
    assert_eq!(row["child_start_token"], child.start_token);
    assert_eq!(row["child_boot_id"], child.boot_id);
    let markers: Vec<u32> = fs::read_to_string(&run.marker)
        .unwrap()
        .lines()
        .map(|p| p.parse().unwrap())
        .collect();
    assert_eq!(markers, vec![child.pid]);
    let bridge = case.router.call("job_by_id", json!({"job_id":id})).unwrap();
    // Registered desktop projections intentionally omit private process fields.
    // Bind the independent reads by job identity and durable lifecycle revision;
    // native handshake + read-only SQLite independently establish child identity.
    assert_eq!(bridge["data"]["job"]["job_id"], id);
    assert!(
        bridge["data"]["job"]["revision"].as_u64().unwrap() >= row["revision"].as_u64().unwrap()
    );
    let after = case.rows("SELECT job_id,revision FROM job_lifecycle");
    assert!(
        after.iter().find(|r| r["job_id"] == id).unwrap()["revision"]
            .as_u64()
            .unwrap()
            >= bridge["data"]["job"]["revision"].as_u64().unwrap()
    );
    let cli = case.cli(&["job", "status", id]);
    assert!(cli.status.success(), "{cli:?}");
    let cli = String::from_utf8(cli.stdout).unwrap();
    assert!(cli.contains(&format!("Job: {id}")));
    let meter = case
        .router
        .call("meter_summary", json!({"job_id":id}))
        .unwrap();
    let cli_meter = case.cli(&["meter", "summary", "--job", id, "--json"]);
    assert!(cli_meter.status.success(), "{cli_meter:?}");
    let cli_meter: Value = serde_json::from_slice(&cli_meter.stdout).unwrap();
    assert_eq!(cli_meter, meter["data"]);
    if terminal {
        assert_eq!(row["state"], "COMPLETED");
        assert_eq!(row["outcome_kind"], "EXITED");
        assert_eq!(row["exit_code"], 17);
        assert_eq!(
            bridge["data"]["job"]["outcome"],
            json!({"kind":"EXITED","code":17})
        );
        assert!(cli.contains("State: failed\nExit code: 17"), "{cli}");
        let reports = case.rows("SELECT job_id,report_json FROM job_meter_summaries");
        let report = reports.iter().find(|r| r["job_id"] == id).unwrap();
        let report: Value = serde_json::from_str(
            report["report_json"]
                .as_str()
                .expect("owned wait4 report must survive OFF"),
        )
        .unwrap();
        assert_eq!(report["child"], json!(child));
        assert_eq!(report, meter["data"]["accounting"]["report"]);
        assert_eq!(report["outcome"], json!({"kind":"EXITED","code":17}));
        assert!(report["run_elapsed"]["seconds"]
            .as_f64()
            .is_some_and(|v| v > 0.0));
        assert!(report["cpu_core_seconds"]["total"]
            .as_f64()
            .is_some_and(|v| v >= 0.0));
    }
    json!({"sqlite":row,"registeredBridge":bridge,"cli":cli,"meter":meter,"cliMeter":cli_meter,"markers":markers,
        "stdout":fs::read_to_string(&run.stdout).unwrap(),"stderr":fs::read_to_string(&run.stderr).unwrap()})
}
pub fn checkpoint(listener: &UnixListener) -> (UnixStream, Value) {
    listener.set_nonblocking(true).unwrap();
    let mut got = None;
    eventually(|| match listener.accept() {
        Ok((stream, _)) => {
            got = Some(stream);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(e) => panic!("{e}"),
    });
    let stream = got.unwrap();
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(&stream)
        .read_line(&mut line)
        .unwrap();
    (stream, serde_json::from_str(&line).unwrap())
}
pub fn restart(case: &mut Case) -> Value {
    let first = case.daemon.child.id();
    case.stop();
    fs::rename(
        case.fixture.root.join("runtime.json"),
        case.fixture.root.join("runtime-before-restart.json"),
    )
    .unwrap();
    let disconnected = case.router.call(
        "governor_get_switch",
        json!({"query":{"key":"governor.enabled"}}),
    );
    assert!(
        disconnected.is_err(),
        "offline must not become Paused/Active evidence"
    );
    let mut command = case.fixture.command();
    command.env("ARC_PRESSURE_CONTROL", case.fixture.root.join("pressure"));
    case.daemon = super::native::OwnedChild::new(command.spawn().unwrap());
    let second = case.daemon.child.id();
    assert_ne!(first, second);
    eventually(|| case.router.call("daemon_health", json!({})).is_ok());
    json!({"daemonPids":[first,second],"disconnectedRead":disconnected,"controlledFixtureOnly":true})
}
