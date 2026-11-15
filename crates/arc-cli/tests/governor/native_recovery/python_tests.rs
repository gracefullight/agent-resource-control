//! Launch-fence regressions using real stdlib unittest and production admission.
use super::*;

#[test]
fn python_pause_after_reservation_keeps_job_witness_and_executes_once() {
    pause_after_reservation(false);
}

#[test]
fn python_pause_resume_before_release_repolls_same_job_and_executes_once() {
    pause_after_reservation(true);
}

fn pause_after_reservation(resume: bool) {
    let _serial = serialization::serial();
    let interpreter = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .expect("installed Python 3 is required for the launch regression");
    assert!(interpreter.status.success(), "{interpreter:?}");
    let interpreter = String::from_utf8(interpreter.stdout).unwrap();
    let interpreter = fs::canonicalize(interpreter.trim()).unwrap();
    let mut daemon = Daemon::new();
    let root = &daemon.fixture.root;
    fs::create_dir_all(root.join(".venv/bin")).unwrap();
    symlink(interpreter, root.join(".venv/bin/python")).unwrap();
    fs::write(
        root.join("test_route.py"),
        r#"import json, os, socket, struct, sys, unittest

class Route(unittest.TestCase):
    def test_exec(self):
        with open(os.environ['ARC_ROUTE_STARTS'], 'a') as starts:
            starts.write(str(os.getpid()) + '\n')
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as channel:
            channel.connect(os.environ['ARC_ROUTE_CONTROL'])
            frame = json.dumps({'pid': os.getpid(), 'ppid': os.getppid(),
                                'argv': sys.argv, 'executable': sys.executable}).encode()
            channel.sendall(struct.pack('>I', len(frame)) + frame)
            self.assertEqual(channel.recv(1), b'x')
        print('route-stdout', flush=True)
        print('route-stderr', file=sys.stderr, flush=True)
"#,
    )
    .unwrap();
    let target_listener = UnixListener::bind(root.join("target.sock")).unwrap();
    let barrier = UnixListener::bind(root.join("checkpoint.sock")).unwrap();
    let starts = root.join("target.starts");
    let mut command = daemon.cli();
    command
        .args([
            "exec",
            "--",
            ".venv/bin/python",
            "-m",
            "unittest",
            "test_route.Route.test_exec",
            "-v",
        ])
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .env("ARC_ROUTE_STARTS", &starts)
        .env("ARC_ROUTE_CONTROL", root.join("target.sock"))
        .env("ARC_PRESSURE_ACCEPTANCE_STAGE", "reserved,spawned")
        .env(
            "ARC_PRESSURE_ACCEPTANCE_BARRIER",
            root.join("checkpoint.sock"),
        )
        .stdout(fs::File::create(root.join("wrapper.out")).unwrap())
        .stderr(fs::File::create(root.join("wrapper.err")).unwrap());
    let mut wrapper = native::OwnedChild::new(command.spawn().unwrap());
    let (mut reserved_barrier, reserved) = checkpoint(&barrier);
    assert_eq!(reserved["mode"], "RESERVED");
    let id = reserved["request"]["job_id"].as_str().unwrap();
    let original_generation = reserved["request"]["permit_generation"].as_u64().unwrap();
    let witness = root.join(format!(".traffic-witness-{id}"));
    let inode = fs::metadata(&witness).unwrap().ino();
    let original_key = fs::read(&witness).unwrap()[40..104].to_vec();
    assert!(!starts.exists());
    assert_eq!(
        daemon.rows("SELECT state FROM permits"),
        vec![json!({"state":"ACTIVE"})]
    );
    assert_eq!(daemon.control("pause")["switch"]["enabled"], false);
    assert_eq!(
        daemon.rows("SELECT state FROM permits"),
        vec![json!({"state":"RELEASED"})]
    );
    if resume {
        assert_eq!(daemon.control("resume")["switch"]["enabled"], true);
    }
    assert!(!starts.exists(), "a reservation is not a native start");
    reserved_barrier.write_all(b"x").unwrap();
    drop(reserved_barrier);

    let (mut spawned_barrier, spawned) = checkpoint(&barrier);
    assert_eq!(spawned["job_id"], id);
    let (mut target, hello) = native_process::accept(&target_listener);
    let pid = hello["pid"].as_u64().unwrap() as u32;
    wrapper.own_process(ProcessIdentity::capture(pid).unwrap());
    assert_eq!(spawned["child_pid"], pid);
    assert_eq!(hello["ppid"], wrapper.child.id());
    assert_eq!(hello["argv"][1], "test_route.Route.test_exec");
    assert_eq!(hello["argv"][2], "-v");
    assert_eq!(fs::metadata(&witness).unwrap().ino(), inode);
    assert_eq!(fs::read_to_string(&starts).unwrap().lines().count(), 1);
    let rows = daemon.rows("SELECT l.job_id,l.child_pid,l.classification_reason,j.job_class,g.mode,g.release_id,t.generation AS permit_generation FROM job_lifecycle l JOIN governor_jobs g USING(job_id) JOIN traffic_entries t USING(job_id) JOIN jobs j ON j.id=l.job_id");
    assert_eq!(rows.len(), 1, "retries retain the original job");
    assert_eq!(rows[0]["job_id"], id);
    assert_eq!(rows[0]["job_class"], "TEST");
    assert_eq!(rows[0]["classification_reason"], "PYTHON_UNITTEST");
    assert!(
        rows[0]["child_pid"].is_null(),
        "spawn barrier precedes attachment"
    );
    let bytes = fs::read(&witness).unwrap();
    assert_eq!(
        u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
        pid as u64
    );
    if resume {
        assert_eq!(rows[0]["mode"], "SCHEDULED");
        assert!(rows[0]["release_id"].is_null());
        assert!(rows[0]["permit_generation"].as_u64().unwrap() > original_generation);
        assert_eq!(&bytes[40..104], original_key);
        assert_eq!(
            daemon.rows("SELECT state FROM permits"),
            vec![json!({"state":"ACTIVE"})]
        );
    } else {
        assert_eq!(rows[0]["mode"], "UNRESERVED");
        let release = rows[0]["release_id"].as_str().unwrap();
        assert_eq!(&bytes[40..104], format!("{release}{release}").as_bytes());
        assert_eq!(
            daemon.rows("SELECT state FROM permits"),
            vec![json!({"state":"RELEASED"})]
        );
    }
    spawned_barrier.write_all(b"x").unwrap();
    drop(spawned_barrier);
    eventually(|| daemon.rows("SELECT state FROM traffic_entries")[0]["state"] == "RUNNING");
    assert_eq!(
        daemon.rows("SELECT child_pid FROM job_lifecycle")[0]["child_pid"],
        pid
    );
    target.write_all(b"x").unwrap();
    let exit = native_process::wait(&mut wrapper.child);
    let stderr = fs::read_to_string(root.join("wrapper.err")).unwrap();
    assert!(exit.success(), "{exit:?}: {stderr}");
    assert!(
        stderr.contains("route-stderr") && stderr.contains("OK"),
        "{stderr}"
    );
    assert!(fs::read_to_string(root.join("wrapper.out"))
        .unwrap()
        .contains("route-stdout"));
    let terminal = daemon.rows("SELECT l.job_id,l.child_pid,l.outcome_kind,j.exit_code,t.state FROM job_lifecycle l JOIN traffic_entries t USING(job_id) JOIN jobs j ON j.id=l.job_id");
    assert_eq!(
        terminal,
        vec![
            json!({"job_id":id,"child_pid":pid,"outcome_kind":"EXITED","exit_code":0,"state":"COMPLETED"})
        ]
    );
    assert_eq!(
        daemon.rows("SELECT state FROM permits"),
        vec![json!({"state":"RELEASED"})]
    );
    assert_eq!(fs::read_to_string(&starts).unwrap().lines().count(), 1);
    let status = daemon.status(id);
    assert!(status.status.success(), "{status:?}");
    let status_text = String::from_utf8_lossy(&status.stdout);
    assert!(status_text.contains(id));
    assert!(
        status_text.contains("State: succeeded\nExit code: 0\n"),
        "{status:?}"
    );
    eprintln!(
        "python-launch-regression={}",
        json!({"resumeBeforeRelease":resume,"jobId":id,"before":rows,"terminal":terminal,"executions":1,"permitsReleased":true})
    );
    daemon.stop();
}
