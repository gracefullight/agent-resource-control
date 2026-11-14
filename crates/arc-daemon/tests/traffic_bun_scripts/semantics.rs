//! Native comparison, signals and nested accounting; no rewritten invocation.
use super::{projects::*, *};
use std::os::unix::process::ExitStatusExt;
fn native_records(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("NATIVE:"))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
pub fn invocation(read: Read) {
    let case = Case::new("invocation", read, false);
    let p = project(&case, "root", Kind::Test, false);
    let nested = p.join("package");
    fs::create_dir(&nested).unwrap();
    // All reachable lifecycle leaves are real Bun tests of the same class.
    write(
        &p.join("package.json"),
        json!({"scripts":{"misleading":"bun run --cwd package unexpected"}}).to_string(),
    );
    write(&nested.join("package.json"),json!({"scripts":{"preunexpected":"bun test pre.test.ts","unexpected":"bun test semantics.test.ts","postunexpected":"bun test post.test.ts"}}).to_string());
    for (file, event) in [
        ("pre.test.ts", "preunexpected"),
        ("post.test.ts", "postunexpected"),
    ] {
        write(&nested.join(file),format!("import {{test,expect}} from 'bun:test';test('hook',()=>{{expect(process.env.npm_lifecycle_event).toBe('{event}');console.log('NATIVE:'+JSON.stringify({{event:process.env.npm_lifecycle_event,cwd:process.cwd(),args:process.argv.slice(2)}}));}});\n"));
    }
    write(
        &nested.join("semantics.test.ts"),
        include_str!("sources/semantics.test.ts"),
    );
    let args = [
        "run",
        "misleading",
        "--",
        "--test-name-pattern",
        "literal arguments",
    ];
    let baseline_dir = p.join("baseline");
    fs::create_dir(&baseline_dir).unwrap();
    let mut direct = Command::new(&case.tools.bun);
    direct
        .env("TBS_PROJECT", &baseline_dir)
        .args(args)
        .current_dir(&p)
        .env("TBS_VALUE", "literal $HOME ' value")
        .env("TBS_CODE", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut direct = direct.spawn().unwrap();
    direct
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    let baseline = direct.wait_with_output().unwrap();
    assert_eq!(baseline.status.code(), Some(0), "{baseline:?}");
    let expected = native_records(&String::from_utf8(baseline.stdout).unwrap());
    assert_eq!(expected.len(), 3);
    let mut argv = vec!["bun"];
    argv.extend(args);
    let mut run = Run::start_with(&case, &p, &argv, |cmd| {
        cmd.env("TBS_VALUE", "literal $HOME ' value")
            .env("TBS_CODE", "0")
            .stdin(Stdio::piped());
    });
    run.wrapper
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    run.finish(0);
    let actual = native_records(&log(&p.join("stdout")));
    assert_eq!(actual, expected);
    assert_eq!(actual[1]["input"], json!(b"stdin\0bytes\n".to_vec()));
    assert_eq!(actual[1]["value"], "literal $HOME ' value");
    assert_eq!(actual[1]["cwd"], json!(nested.canonicalize().unwrap()));
    assert!(log(&p.join("stderr")).contains("NATIVE_STDERR"));
    assert_eq!(
        case.rows("SELECT count(*) AS n FROM job_lifecycle")[0]["n"],
        1
    );
    // Two native `bun run` entries are the original and literal nested script,
    // while admission remains one job and one root reservation.
    assert_eq!(run.count("bun"), 2);
    let success = case.terminal(&run, "TEST", "BUN_SCRIPT_TEST", 0);
    let e = project(&case, "exit17", Kind::Test, false);
    write(&e.join("evidence.test.ts"),"import {test} from 'bun:test';test('exit17',()=>{console.log('EXIT17_BODY');process.exit(17)});\n");
    let mut exit = Run::start(&case, &e, &Kind::Test.argv());
    exit.finish(17);
    assert_eq!(exit.count("bun"), 1);
    assert_eq!(log(&e.join("stdout")).matches("EXIT17_BODY").count(), 1);
    let failure = case.terminal(&exit, "TEST", "BUN_SCRIPT_TEST", 17);
    case.save("invocation",json!({"originalArgv":argv,"nativeBaseline":expected,"managed":actual,"lifecycleOrder":["preunexpected","unexpected","postunexpected"],"singleGraphJob":success,"exit17":failure,"originalStarts":[1,1],"noArgumentExpansion":true}));
    case.done();
}
pub fn signals(read: Read) {
    let case = Case::new("signals", read, false);
    let p = project(&case, "running", Kind::Test, true);
    let mut run = Run::start(&case, &p, &Kind::Test.argv());
    let entered = run.entered(&case, false);
    let id = entered["jobId"].as_str().unwrap();
    assert_eq!(
        unsafe { libc::kill(run.wrapper.child.id() as i32, libc::SIGTERM) },
        0
    );
    let status = run.wait();
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    eventually(|| case.row(id)["state"] == "COMPLETED");
    let done = case.reconcile(id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(done["sqlite"]["outcome_kind"], "SIGNALED");
    assert_eq!(done["sqlite"]["signal"], libc::SIGTERM);
    assert_eq!(
        done["diagnostic"]["admission"]["outcome"]["kind"],
        "SIGNALED"
    );
    assert_eq!(run.count("bun"), 1);
    assert_eq!(case.active()["permits"], 0);
    let meter = case.cli(&["meter", "summary", "--job", id, "--json"]);
    assert!(meter.status.success());
    let meter: Value = serde_json::from_slice(&meter.stdout).unwrap();
    assert_eq!(
        meter,
        (case.read)(&case.fixture.socket, "meter_summary", json!({"job_id":id}))
    );
    assert_eq!(
        meter["accounting"]["report"]["outcome"]["signal"],
        libc::SIGTERM
    );
    case.save("signal-running-cancel",json!({"entry":entered,"terminal":done,"meter":meter,"wrapperNativeSignal":status.signal(),"starts":1}));
    case.done();
}
pub fn nested(read: Read) {
    let case = Case::new("nested", read, false);
    let p = project(&case, "parent", Kind::Test, true);
    let q = project(&case, "nested-child", Kind::Test, true);
    write(
        &p.join("evidence.test.ts"),
        include_str!("sources/nested.test.ts"),
    );
    let mut run = Run::start_with(&case, &p, &Kind::Test.argv(), |cmd| {
        cmd.env("TBS_INNER", &q);
    });
    let entered = run.entered(&case, false);
    let root_id = entered["jobId"].as_str().unwrap();
    eventually(|| !log(&q.join("bodies")).is_empty());
    let mut nested_id = None;
    eventually(|| {
        nested_id = case
            .rows("SELECT job_id,root_job_id,state FROM traffic_entries")
            .into_iter()
            .find(|r| r["root_job_id"] == root_id && r["state"] == "RUNNING")
            .and_then(|r| r["job_id"].as_str().map(str::to_owned));
        nested_id.is_some()
    });
    let nested_id = nested_id.unwrap();
    assert_eq!(case.active()["permits"], 1);
    assert_eq!(case.rows("SELECT count(*) AS n FROM permits")[0]["n"], 1);
    let child_running = case.reconcile(&nested_id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(
        child_running["diagnostic"]["admission"]["root_job_id"],
        root_id
    );
    write(&q.join("release"), "release");
    eventually(|| case.row(&nested_id)["state"] == "COMPLETED");
    assert!(run.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    let inner = case.reconcile(&nested_id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(
        inner["diagnostic"]["admission"]["permit_active"], true,
        "completed nested work still shares the live root permit"
    );
    let meter = (case.read)(
        &case.fixture.socket,
        "meter_summary",
        json!({"job_id":nested_id}),
    );
    assert_eq!(
        meter["accounting"]["report"]["accounting_relation"],
        json!({"kind":"included_in_root","root_job_id":root_id})
    );
    run.finish(0);
    let outer = case.terminal(&run, "TEST", "BUN_SCRIPT_TEST", 0);
    assert_eq!(
        outer["meter"]["accounting"]["report"]["accounting_relation"],
        json!({"kind":"canonical_root","root_job_id":root_id})
    );
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    assert_eq!(log(&q.join("bodies")).lines().count(), 1);
    let after = case.reconcile(&nested_id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(after["diagnostic"]["admission"]["permit_active"], false);
    case.save("nested-meter",json!({"entry":entered,"childRunning":child_running,"childCompletedWhileParentLive":inner,"nestedMeter":meter,"outer":outer,"childAfterRootRelease":after,"rootPermitCount":1,"bodyCounts":[1,1]}));
    case.done();
}
