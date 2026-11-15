//! One ordered-pair scenario for all three lanes, using the existing Case and Run.
use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus};
use serde_json::json;
use std::fs;

pub fn pair<T: RunnerTools>(case: Case<T>, descriptor: Pair, first: Prepared, second: Prepared) {
    assert!(!case.injected, "contention must run the production daemon");
    assert_eq!(
        [first.form, second.form],
        [descriptor.first, descriptor.second]
    );
    assert_eq!(first.form.class(), second.form.class());
    let mut a = first.start(&case, None);
    let entered_a = entered(&case, &mut a, &first);
    assert_eq!(case.active()["permits"], 1);
    let limits = &entered_a.facts["diagnostic"]["limits"];
    let class = first.form.class().to_ascii_lowercase();
    let reservation = &limits[&class]["reservation"];
    assert_eq!(limits[&class]["window"], 1);
    // Both estimates fit simultaneously: only the matching class slot is full.
    for resource in ["cpu_millis", "memory_bytes"] {
        assert!(
            2 * reservation[resource].as_u64().unwrap()
                <= limits["budget"][resource].as_u64().unwrap()
        );
    }
    let mut b = second.start(&case, Some(&entered_a.job_id));
    let queued_b = queued(&case, &mut b, second.form);
    assert_ne!(entered_a.job_id, queued_b.job_id);
    assert!(a.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    assert_eq!(log(&second.project.join("launches")).lines().count(), 0);
    a.finish(0);
    let released_a = terminal(&case, &a, first.form, 0);
    let entered_b = entered(&case, &mut b, &second);
    assert_eq!(queued_b.job_id, entered_b.job_id);
    let row_a = case.row(&entered_a.job_id);
    let row_b = case.row(&entered_b.job_id);
    assert!(row_b["granted_at_ms"].as_u64().unwrap() >= row_a["released_at"].as_u64().unwrap());
    assert_eq!(row_a["permit_state"], "RELEASED");
    assert_eq!(row_b["permit_state"], "ACTIVE");
    assert_eq!(
        log(&second.project.join("release-witness")).trim(),
        "RELEASED"
    );
    assert_eq!(probe_live(&entered_a.child), IdentityStatus::Missing);
    b.finish(0);
    let released_b = terminal(&case, &b, second.form, 0);
    assert_eq!(probe_live(&entered_b.child), IdentityStatus::Missing);
    let permits = permit_history(&case, first.form.class(), 2, 1);
    assert_eq!(permits["matchingClassPermitHighWater"], 1);
    case.save(
        descriptor.id,
        json!({"pairId":descriptor.id,"productionDaemon":true,
        "firstEntry":entered_a,"queuedSecond":queued_b,"secondEntry":entered_b,
        "outcomes":[released_a,released_b],"queuedStartsBeforeRelease":0,"eachExecutedOnce":true,
        "allPermitsReleased":true,"adequateGlobalResources":true,"limits":limits,"permits":permits}),
    );
    // Run/OwnedChild retain the established process-tree cleanup through unwinding too.
    case.done();
}

/// An existing-class native Cargo peer: standard library only, no registry access.
/// The lane's observer must log once, check TBS_PREDECESSOR, then exec real Cargo.
pub fn cargo_peer<T>(case: &Case<T>, name: &str, form: Form, cargo: &Path) -> Prepared {
    assert!(matches!(
        form,
        Form::CargoBuild | Form::CargoTest | Form::CargoClippy
    ));
    let project = case.artifacts.join(name);
    fs::create_dir_all(project.join("src")).unwrap();
    write(
        &project.join("Cargo.toml"),
        "[package]\nname=\"native_tool_peer\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[workspace]\n",
    );
    write(
        &project.join("src/lib.rs"),
        include_str!("../traffic_python_tests/sources/lib.rs.txt"),
    );
    if form != Form::CargoTest {
        write(
            &project.join("build.rs"),
            include_str!("../traffic_python_tests/sources/build.rs.txt"),
        );
    }
    Prepared {
        form,
        project,
        // Cargo's native external-subcommand dispatch execs cargo-clippy in
        // place. The invocation remains exactly `cargo clippy` through PATH.
        executable: if form == Form::CargoClippy {
            cargo.with_file_name("cargo-clippy").canonicalize().unwrap()
        } else {
            cargo.to_owned()
        },
        argv: form.argv().iter().map(|s| (*s).to_owned()).collect(),
        ready: marker_ready,
        configure: configure_cargo,
    }
}
fn configure_cargo(project: &Path, command: &mut Command) {
    let cache = project.join("cargo-home");
    fs::create_dir_all(&cache).unwrap();
    command
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_HOME", cache)
        .env("CARGO_TARGET_DIR", project.join("target"));
}
