// These tests use a real Cargo or Bun parent, a real Cargo child, and either the
// current CLI or the explicitly labeled frozen protocol-1 client fixture.
#[test]
fn frozen_old_client_reuses_new_cargo_and_bun_parent_permits() {
    let _serial = support::serial();
    for parent in ["cargo", "bun"] {
        run_nested_recovery_case(parent, "cargo", true, false, false, None);
    }
}

#[test]
fn attached_cargo_and_bun_children_keep_lineage_after_wrapper_death_and_restart() {
    let _serial = support::serial();
    for parent in ["cargo", "bun"] {
        for child in ["cargo", "bun"] {
            for restart in [false, true] {
                run_nested_recovery_case(parent, child, false, true, restart, None);
            }
        }
    }
}

#[test]
fn old_daemon_style_claim_lookup_works_for_both_clients_and_wrapper_death() {
    let _serial = support::serial();
    for frozen in [false, true] {
        for kill_wrapper in [false, true] {
            run_nested_recovery_case("cargo", "cargo", frozen, kill_wrapper, false, None);
        }
    }
}

#[test]
fn caller_lineage_is_preserved_and_live_sidecar_takes_precedence() {
    let _serial = support::serial();
    for value in [
        "caller-owned-not-a-lineage-token",
        "b0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
    ] {
        for kill_wrapper in [false, true] {
            run_nested_recovery_case("cargo", "cargo", false, kill_wrapper, false, Some(value));
        }
    }
}

fn run_nested_recovery_case(
    parent: &str,
    child: &str,
    frozen: bool,
    kill_wrapper: bool,
    restart: bool,
    caller_lineage: Option<&str>,
) {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let cargo_root = f.root.join("cargo_parent");
    let cargo_child = f.root.join("cargo_child");
    let bun_root = f.root.join("bun_parent");
    let bun_child = f.root.join("bun_child");
    for (dir, name) in [
        (&cargo_root, "traffic_lineage_parent"),
        (&cargo_child, "traffic_lineage_child"),
    ] {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2021'\n"),
        )
        .unwrap();
    }
    fs::create_dir(&bun_root).unwrap();
    fs::create_dir(&bun_child).unwrap();
    fs::write(cargo_child.join("src/lib.rs"), r#"
#[test] fn child() {
    use std::{fs::OpenOptions, io::Write, thread, time::Duration};
    writeln!(OpenOptions::new().append(true).create(true).open(std::env::var("CHILD_MARKER").unwrap()).unwrap(), "child").unwrap();
    while !std::path::Path::new(&std::env::var("CHILD_RELEASE").unwrap()).exists() { thread::sleep(Duration::from_millis(10)); }
}
"#).unwrap();
    fs::write(
        bun_child.join("child.test.ts"),
        r#"
import {test} from 'bun:test';
import {appendFileSync, existsSync} from 'node:fs';
test('child', async () => {
  appendFileSync(process.env.CHILD_MARKER!, 'child\n');
  while (!existsSync(process.env.CHILD_RELEASE!)) await Bun.sleep(10);
}, 60000);
"#,
    )
    .unwrap();
    fs::write(cargo_root.join("src/lib.rs"), r#"
#[test] fn parent() {
    use std::{fs::{self, OpenOptions}, io::Write, process::Command, thread, time::Duration};
    writeln!(OpenOptions::new().append(true).create(true).open(std::env::var("ROOT_MARKER").unwrap()).unwrap(), "root").unwrap();
    while !std::path::Path::new(&std::env::var("START_NESTED").unwrap()).exists() { thread::sleep(Duration::from_millis(10)); }
    let frozen = std::env::var_os("FROZEN_MODE").is_some();
    let mut cmd = Command::new(std::env::var(if frozen {"FROZEN_CLIENT"} else {"TEST_ARCCTL"}).unwrap());
    if frozen { cmd.env("ARC_FIXTURE_FROZEN_TRAFFIC_V1", "1").env("FROZEN_DROP_ENQUEUE_REPLY", "1"); }
    else if std::env::var("CHILD_TOOL").unwrap() == "cargo" { cmd.args(["exec", "--", "cargo", "test", "--offline"]); }
    else { cmd.args(["exec", "--", "bun", "test", "--timeout=60000"]); }
    let status = cmd.current_dir(std::env::var("NESTED_DIR").unwrap()).env("CARGO_TARGET_DIR", std::env::var("CHILD_TARGET").unwrap()).status().unwrap();
    assert!(status.success(), "nested exit: {status}");
    fs::write(std::env::var("ROOT_SUCCESS").unwrap(), "success\n").unwrap();
}
"#).unwrap();
    fs::write(bun_root.join("parent.test.ts"), r#"
import {test, expect} from 'bun:test';
import {appendFileSync, existsSync, writeFileSync} from 'node:fs';
test('parent', async () => {
  appendFileSync(process.env.ROOT_MARKER!, 'root\n');
  while (!existsSync(process.env.START_NESTED!)) await Bun.sleep(10);
  const frozen = !!process.env.FROZEN_MODE;
  const args = frozen ? [process.env.FROZEN_CLIENT!] : process.env.CHILD_TOOL === 'cargo'
    ? [process.env.TEST_ARCCTL!, 'exec', '--', 'cargo', 'test', '--offline']
    : [process.env.TEST_ARCCTL!, 'exec', '--', 'bun', 'test', '--timeout=60000'];
  const env = {...process.env, CARGO_TARGET_DIR:process.env.CHILD_TARGET!, ...(frozen ? {ARC_FIXTURE_FROZEN_TRAFFIC_V1:'1', FROZEN_DROP_ENQUEUE_REPLY:'1'} : {})};
  const child = Bun.spawn(args, {cwd:process.env.NESTED_DIR!, env, stdout:'inherit', stderr:'inherit'});
  expect(await child.exited).toBe(0);
  writeFileSync(process.env.ROOT_SUCCESS!, 'success\n');
}, 60000);
"#).unwrap();

    let root_marker = f.root.join("root-marker");
    let start_nested = f.root.join("start-nested");
    let child_marker = f.root.join("child-marker");
    let child_release = f.root.join("child-release");
    let root_success = f.root.join("root-success");
    let mut command = Command::new(binary("arcctl"));
    command.args(["exec", "--", parent, "test"]);
    if parent == "cargo" {
        command.arg("--offline");
    } else {
        command.arg("--timeout=60000");
    }
    command
        .current_dir(if parent == "cargo" {
            &cargo_root
        } else {
            &bun_root
        })
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("CARGO_TARGET_DIR", cargo_root.join("target"))
        .env("CHILD_TARGET", cargo_child.join("target"))
        .env("TEST_ARCCTL", binary("arcctl"))
        .env("FROZEN_CLIENT", binary("arc-exec-fixture"))
        .env("FROZEN_CARGO", env!("CARGO"))
        .env("FROZEN_CARGO_DIR", &cargo_child)
        .env("CHILD_TOOL", child)
        .env(
            "NESTED_DIR",
            if child == "cargo" {
                &cargo_child
            } else {
                &bun_child
            },
        )
        .env("ROOT_MARKER", &root_marker)
        .env("ROOT_SUCCESS", &root_success)
        .env("START_NESTED", &start_nested)
        .env("CHILD_MARKER", &child_marker)
        .env("CHILD_RELEASE", &child_release)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            File::create(f.root.join("parent-out")).unwrap(),
        ))
        .stderr(Stdio::from(
            File::create(f.root.join("parent-err")).unwrap(),
        ));
    if let Some(value) = caller_lineage {
        command.env("ARC_TRAFFIC_LINEAGE", value);
    }
    if frozen {
        command.env("FROZEN_MODE", "1");
    } else {
        command.env_remove("FROZEN_MODE");
    }
    let mut wrapper = BuildChild::new(OwnedChild::new(command.spawn().unwrap()), &f, true);
    eventually(|| {
        root_marker.exists()
            && entry(&f, wrapper.child.id()).is_some_and(|e| e.0 == "RUNNING" && e.1.is_some())
    });
    let (root_id, root_context, root_generation, native_pid): (String, String, i64, u32) = database(&f).query_row(
        "SELECT t.job_id,t.context,t.generation,l.child_pid FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id WHERE l.observer_pid=?1 AND t.state='RUNNING'",
        [wrapper.child.id()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    let native = ProcessIdentity::capture(native_pid).unwrap();
    assert_eq!(probe_live(&native), IdentityStatus::VerifiedMatch);
    assert_eq!(active(&f).1, 1);
    let root_witness_path = f.root.join(format!(".traffic-witness-{root_id}"));
    let root_witness = fs::read(&root_witness_path).unwrap();
    assert_eq!(&root_witness[..8], b"ARCWIT03");
    assert_eq!(
        u64::from_be_bytes(root_witness[24..32].try_into().unwrap()),
        u64::from(native.pid)
    );
    assert_eq!(
        u64::from_be_bytes(root_witness[32..40].try_into().unwrap()).to_string(),
        native.start_token
    );
    assert_eq!(&root_witness[104..136], root_id.as_bytes());
    if kill_wrapper {
        wrapper.child.kill().unwrap();
        assert!(!wrapper.child.wait().unwrap().success());
        assert_eq!(probe_live(&native), IdentityStatus::VerifiedMatch);
        assert_eq!(active(&f).1, 1);
        assert_eq!(fs::read(&root_witness_path).unwrap(), root_witness);
    }
    if restart {
        daemon.stop(libc::SIGTERM);
        assert_eq!(probe_live(&native), IdentityStatus::VerifiedMatch);
        fs::rename(
            f.root.join("runtime.json"),
            f.root.join("prior-runtime.json"),
        )
        .unwrap();
        daemon = f.spawn("test");
        assert_eq!(probe_live(&native), IdentityStatus::VerifiedMatch);
        assert_eq!(active(&f).1, 1);
        assert_eq!(fs::read(&root_witness_path).unwrap(), root_witness);
    }
    fs::write(&start_nested, "start").unwrap();
    eventually(|| {
        child_marker.exists()
            && database(&f)
                .query_row(
                    "SELECT count(*) FROM traffic_entries WHERE root_job_id=?1 AND state='RUNNING'",
                    [&root_id],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap()
                == 1
    });
    let db = database(&f);
    let (nested_id, nested_generation, inherited): (String, i64, Option<String>) = db.query_row(
        "SELECT job_id,generation,inherited_context FROM traffic_entries WHERE root_job_id=?1 AND state='RUNNING'", [&root_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(nested_generation, root_generation);
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    let claim = inherited.unwrap();
    let expected_context = caller_lineage.is_none() || !kill_wrapper;
    if expected_context {
        assert_eq!(
            claim, root_context,
            "nested request must carry the root claim"
        );
        // The accepted daemon resolves candidates by this claim before checking
        // native ancestry. Check that the same lookup finds the active root.
        let key = arc_ipc::traffic::LineageContext::try_from(claim.clone()).unwrap();
        let candidates = arc_storage::jobs::traffic::lineage_snapshots(&db, &key).unwrap();
        assert!(candidates.iter().any(|candidate| {
            candidate.permit_job_id.as_str() == root_id
                && candidate.generation == root_generation as u64
                && candidate.root.child.as_ref() == Some(&native)
        }));
    } else if let Some(value) = caller_lineage.filter(|value| value.len() == 64) {
        assert_eq!(
            claim, value,
            "a dead sidecar owner must fall back to the caller value"
        );
        assert_ne!(claim, root_context);
    } else {
        assert_ne!(claim, root_context);
    }
    if !expected_context {
        let key = arc_ipc::traffic::LineageContext::try_from(claim.clone()).unwrap();
        assert!(arc_storage::jobs::traffic::lineage_snapshots(&db, &key)
            .unwrap()
            .is_empty());
    }
    let witness = fs::read(f.root.join(format!(".traffic-witness-{nested_id}"))).unwrap();
    assert_eq!(&witness[40..104], claim.as_bytes());
    assert_eq!(&witness[104..136], nested_id.as_bytes());
    drop(db);
    assert_eq!(active(&f).1, 1);
    fs::write(&child_release, "release").unwrap();
    eventually(|| root_success.exists());
    if kill_wrapper {
        eventually(|| {
            matches!(
                probe_live(&native),
                IdentityStatus::Missing | IdentityStatus::Mismatch
            )
        });
        let id = arc_ipc::jobs::JobId::try_from(root_id.clone()).unwrap();
        eventually(|| {
            let _ = traffic(
                &f,
                &arc_ipc::traffic::JobRequest::Status { job_id: id.clone() },
            );
            active(&f).1 == 0
        });
    } else {
        finish(&mut wrapper);
        assert_eq!(active(&f).1, 0);
    }
    assert_eq!(fs::read_to_string(root_marker).unwrap().lines().count(), 1);
    assert_eq!(fs::read_to_string(child_marker).unwrap().lines().count(), 1);
    assert_eq!(fs::read_to_string(root_success).unwrap(), "success\n");
    let native_output = fs::read_to_string(f.root.join(if parent == "cargo" {
        "parent-out"
    } else {
        "parent-err"
    }))
    .unwrap();
    if parent == "cargo" {
        assert!(
            native_output.contains("test result: ok. 1 passed"),
            "{native_output}"
        );
    } else {
        assert!(
            native_output.contains("1 pass") && native_output.contains("0 fail"),
            "{native_output}"
        );
    }
    let nested_state: String = database(&f)
        .query_row(
            "SELECT state FROM traffic_entries WHERE job_id=?1",
            [&nested_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(nested_state, "COMPLETED");
    daemon.stop(libc::SIGTERM);
    f.report();
}
