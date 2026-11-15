//! Executable-name stubs test routing only: no SwiftPM or other native tool runs.
use super::owned_tree::OwnedTree;
use super::*;
use std::{os::unix::fs::PermissionsExt, path::Path, process::Output};

pub(super) const STUB: &str = r#"#!/bin/sh
ARC_TEST_OWNER_HELPER=1 "$ARC_TEST_OWNER_REGISTER" --exact native_recovery::owned_tree_registration_tests::owned_tree_registration_helper --nocapture >/dev/null 2>&1 || exit 125
if [ "${ARC_ROUTE_HOLD_READY:-}" = 1 ]; then
    printf '%s\n' "$$" > "$ARC_ROUTE_EARLY"
    while [ ! -e "$ARC_ROUTE_RELEASE" ]; do /bin/sleep 30; done
fi
printf '%s\n' "$$" >> "$ARC_ROUTE_STARTS"
printf 'argc=%s\n' "$#"
for arg in "$@"; do printf 'arg=%s\n' "$arg"; done
if [ "${ARC_TRAFFIC_LINEAGE+x}" = x ]; then
    printf 'lineage=present:%s\n' "$ARC_TRAFFIC_LINEAGE"
else
    printf 'lineage=absent\n'
fi
printf 'sentinel=%s\n' "$ARC_ROUTE_SENTINEL"
count=0
while [ ! -e "$ARC_ROUTE_RELEASE" ]; do
    count=$((count + 1)); [ "$count" -lt 4000 ] || exit 125
    /bin/sleep 0.01
done
"#;

#[test]
fn native_environment_each_route_preserves_all_lineage_states_through_production_admission() {
    let _serial = serialization::serial();
    let mut daemon = Daemon::new();
    let root = &daemon.fixture.root;
    let bin = root.join("native-route-stubs");
    fs::create_dir(&bin).unwrap();
    for name in ["go", "swift", "cmake", "ctest"] {
        let path = bin.join(name);
        fs::write(&path, STUB).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let supplied = "d".repeat(64);
    for (name, args, reason, class) in [
        ("go", vec!["build"], "GO_BUILD", "BUILD"),
        ("go", vec!["test"], "GO_TEST", "TEST"),
        ("go", vec!["vet"], "GO_VET", "LINT"),
        ("swift", vec!["build"], "SWIFT_BUILD", "BUILD"),
        ("swift", vec!["test"], "SWIFT_TEST", "TEST"),
        ("cmake", vec!["--build", "."], "CMAKE_BUILD", "BUILD"),
        ("ctest", vec![], "CTEST", "TEST"),
    ] {
        for lineage in [None, Some(""), Some(supplied.as_str())] {
            let old: Vec<_> = daemon
                .rows("SELECT job_id FROM traffic_entries")
                .into_iter()
                .map(|row| row["job_id"].clone())
                .collect();
            let direct = run(
                &daemon,
                &bin.join(name),
                &args,
                lineage,
                false,
                &old,
                (reason, class),
            );
            assert_eq!(
                daemon.rows("SELECT job_id FROM traffic_entries").len(),
                old.len()
            );
            let managed = run(
                &daemon,
                &bin.join(name),
                &args,
                lineage,
                true,
                &old,
                (reason, class),
            );
            assert_eq!(managed.stdout, direct.stdout, "{reason} {lineage:?}");
            assert_eq!(managed.stderr, direct.stderr, "{reason} {lineage:?}");
            assert_eq!(managed.status.code(), direct.status.code());
        }
    }
    daemon.stop();
}

fn run(
    daemon: &Daemon,
    executable: &Path,
    args: &[&str],
    lineage: Option<&str>,
    managed: bool,
    old: &[Value],
    expected: (&str, &str),
) -> Output {
    let (reason, class) = expected;
    let root = &daemon.fixture.root;
    let starts = root.join("route-starts");
    let release = root.join("route-release");
    for path in [&starts, &release] {
        match fs::remove_file(path) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => panic!("route fixture reset: {error}"),
        }
    }
    let mut command = if managed {
        daemon.cli()
    } else {
        Command::new(executable)
    };
    if managed {
        command.args(["exec", "--"]).arg(executable);
    }
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env(arc_core::ARC_SOCKET_PATH, &daemon.fixture.socket)
        .env(arc_core::ARC_DB_PATH, &daemon.fixture.database)
        .env("ARC_ROUTE_STARTS", &starts)
        .env("ARC_ROUTE_RELEASE", &release)
        .env(super::owned_tree::REGISTRY_ENV, root.join("route-owners"))
        .env("ARC_ROUTE_SENTINEL", "literal $HOME ' 日本語")
        .env_remove("ARC_ROUTE_HOLD_READY")
        .env_remove("ARC_ROUTE_EARLY");
    if let Some(value) = lineage {
        command.env("ARC_TRAFFIC_LINEAGE", value);
    } else {
        command.env_remove("ARC_TRAFFIC_LINEAGE");
    }
    let mut child = OwnedTree::spawn(command, root);
    let mut job = None;
    eventually(|| {
        let Ok(log) = fs::read_to_string(&starts) else {
            return false;
        };
        let lines: Vec<_> = log.lines().collect();
        if lines.is_empty() {
            return false;
        }
        assert_eq!(lines.len(), 1);
        if !managed {
            return true;
        }
        let rows: Vec<_> = daemon.rows("SELECT t.job_id,t.state,l.child_pid,l.classification_reason,j.job_class,p.state AS permit_state FROM traffic_entries t JOIN job_lifecycle l USING(job_id) JOIN jobs j ON j.id=t.job_id JOIN permits p USING(job_id)")
            .into_iter().filter(|row| !old.contains(&row["job_id"])).collect();
        if rows.len() != 1 || rows[0]["state"] != "RUNNING" || !rows[0]["child_pid"].is_number() {
            return false;
        }
        assert_eq!(rows[0]["classification_reason"], reason);
        assert_eq!(rows[0]["job_class"], class);
        assert_eq!(rows[0]["permit_state"], "ACTIVE");
        assert_eq!(
            rows[0]["child_pid"].as_u64(),
            Some(lines[0].parse().unwrap())
        );
        job = Some(rows[0]["job_id"].clone());
        true
    });
    fs::write(release, "release").unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{reason}: {result:?}");
    assert_eq!(fs::read_to_string(starts).unwrap().lines().count(), 1);
    if let Some(job) = job {
        let rows = daemon.rows("SELECT t.job_id,t.state,p.state AS permit_state FROM traffic_entries t JOIN permits p USING(job_id)");
        let row = rows.iter().find(|row| row["job_id"] == job).unwrap();
        assert_eq!(row["state"], "COMPLETED");
        assert_eq!(row["permit_state"], "RELEASED");
    }
    result
}
