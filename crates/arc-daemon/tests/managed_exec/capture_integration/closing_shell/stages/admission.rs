//! Stage-by-stage admission of one mixed compound on one production route
//! (AT-TRF-12 items 1 and 2, DET-016 items 1 and 2).
use super::super::super::{child_watchdog, support, Fixture};
use super::harness::*;
use serde_json::json;
use std::{fs, io::Write, process::Stdio};

/// `Job: <id>` blocks of `arcctl job status` whose state is `queued`.
fn queued_in_listing(listing: &str) -> Vec<String> {
    listing
        .split("Job: ")
        .skip(1)
        .filter(|block| block.lines().nth(1) == Some("State: queued"))
        .map(|block| block.lines().next().unwrap().to_owned())
        .collect()
}

pub(super) fn stage_by_stage(route: Route, source: &str) {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = sandbox(&f);
    let mut daemon = daemon(&f);
    let make = source == MAKE;
    let exec = route == Route::Exec;

    // The same shell without ARC: the reference for stream bytes and status.
    let mut reference = native(route, &s, &f, source);
    reference
        .env("ARC_FIXTURE_MODE", "success")
        .env("ARC_FIXTURE_MARKER", f.root.join("native-starts"));
    let reference = output_with_input(&mut reference, b"", "native compound");
    assert!(reference.status.success(), "{reference:?}");
    assert_eq!(rows(&f), [], "a native run registered a job");

    // Separate real CLIs own one Build permit and the one TEST permit.
    let build = hold(&s, &f, "build");
    let test = hold(&s, &f, "test");
    let held = rows(&f).len();

    let stage = Barrier::new(&f, "stage");
    let (mut command, hook_request) = managed(route, &s, &f, source);
    stage.install(&mut command);
    let (out, err) = (f.root.join("compound-out"), f.root.join("compound-err"));
    command
        .env("ARC_FIXTURE_MODE", "success")
        .stdin(Stdio::null())
        .stdout(fs::File::create(&out).unwrap())
        .stderr(fs::File::create(&err).unwrap());
    let mut compound = child_watchdog::Owned::spawn(&mut command).unwrap();

    // The Test stage is recorded TEST/HOST_LOCAL and waits; nothing started.
    let waiting = eventually("queued Test stage", || {
        rows(&f)
            .into_iter()
            .find(|row| row.entry.as_deref() == Some("QUEUED"))
    });
    let notice = eventually("QUEUED notice", || {
        fs::read_to_string(&err)
            .ok()
            .filter(|text| text.ends_with('\n') && text.contains("Reference: "))
    });
    assert_waiting_notice(&notice, &waiting.id);
    assert_eq!(stage.starts(), [] as [u64; 0], "a stage started while held");
    assert_eq!(
        (waiting.class.as_str(), waiting.locality.as_str()),
        ("TEST", "HOST_LOCAL")
    );
    assert_eq!(waiting.permit, None, "{waiting:?}");
    let recorded = status(&f, &waiting.id);
    assert_eq!(
        recorded["classification"],
        json!({"classification":"TEST","locality":"HOST_LOCAL",
            "reason": if make { "CARGO_TEST" } else { "BUN_TEST" },"uncertainty":"NONE"}),
        "{recorded}"
    );
    // The waiting stage is found and read through `arcctl job status`: it is
    // the queued TEST stage, not the MIXED request and not a bypass.
    assert_eq!(
        queued_in_listing(&job_status(&s, &f, None)),
        std::slice::from_ref(&waiting.id)
    );
    let single = job_status(&s, &f, Some(&waiting.id));
    assert!(
        single.starts_with(&format!("Job: {}\nState: queued\n", waiting.id)),
        "{single}"
    );
    let all = rows(&f);
    assert_eq!(all.len(), held + 1 + usize::from(exec), "{all:?}");
    assert!(all.iter().all(|row| row.class != "REMOTE_CI"), "{all:?}");
    let request = match hook_request {
        Some(request) => request,
        None => {
            // The bash child stays the request record: one MIXED job.
            let shell = &all[held];
            assert_eq!(
                (shell.class.as_str(), shell.locality.as_str()),
                ("MIXED", "MIXED")
            );
            assert_eq!((&shell.entry, &shell.permit), (&None, &None), "{shell:?}");
            eventually("request record child", || {
                status(&f, &shell.id)["child"]["pid"].as_u64()
            });
            shell.id.clone()
        }
    };

    // Releasing the separate TEST permit admits the stage: exactly one start.
    test.release(&f);
    let (mut run_test, test_stub) = stage.accept();
    assert_eq!(test_stub["args"], json!([b"test".to_vec()]), "{test_stub}");
    assert_eq!(stage.starts(), [test_stub["pid"].as_u64().unwrap()]);
    let admitted = running(&f, &waiting.id);
    assert_eq!(admitted.permit.as_deref(), Some("ACTIVE"), "{admitted:?}");
    let stage_child = eventually("Test stage child", || {
        status(&f, &waiting.id)["child"]["pid"].as_u64()
    });
    // `bun` is the stage's own child; `cargo test` is the recipe make started.
    assert_eq!(
        test_stub[if make { "ppid" } else { "pid" }],
        stage_child,
        "{test_stub}"
    );
    assert_eq!(linked(&f, &request), std::slice::from_ref(&waiting.id));
    run_test.write_all(b"x").unwrap();

    // The gh stage starts while the Build permit is still held, after the Test
    // stage released its own permit, with no queue entry and no reservation.
    let (mut run_remote, remote_stub) = stage.accept();
    assert_eq!(
        remote_stub["args"],
        json!([b"workflow".to_vec(), b"run".to_vec(), b"ci.yml".to_vec()]),
        "{remote_stub}"
    );
    assert_eq!(
        stage.starts(),
        [
            test_stub["pid"].as_u64().unwrap(),
            remote_stub["pid"].as_u64().unwrap()
        ]
    );
    assert_eq!(row(&f, &build.id).permit.as_deref(), Some("ACTIVE"));
    let released = row(&f, &waiting.id);
    assert_eq!(
        (released.entry.as_deref(), released.permit.as_deref()),
        (Some("COMPLETED"), Some("RELEASED")),
        "{released:?}"
    );
    assert_eq!(status(&f, &waiting.id)["outcome"]["code"], 0);
    let all = rows(&f);
    assert_eq!(all.len(), held + 2 + usize::from(exec), "{all:?}");
    let remote = all.last().unwrap();
    assert_eq!(
        (remote.class.as_str(), remote.locality.as_str()),
        ("REMOTE_CI", "REMOTE")
    );
    assert_eq!(
        (&remote.entry, &remote.permit, remote.scheduled),
        (&None, &None, false),
        "the gh stage took local admission state: {remote:?}"
    );
    assert_eq!(
        all.iter().filter(|row| row.entry.is_some()).count(),
        3,
        "only the two holds and the Test stage were queued: {all:?}"
    );
    assert_eq!(
        linked(&f, &request),
        [waiting.id.clone(), remote.id.clone()]
    );
    let remote_record = eventually("gh stage child", || {
        Some(status(&f, &remote.id)).filter(|job| job["child"]["pid"].is_u64())
    });
    assert_eq!(remote_record["child"]["pid"], remote_stub["pid"]);
    assert_eq!(
        remote_record["classification"]["classification"],
        "REMOTE_CI"
    );
    run_remote.write_all(b"x").unwrap();

    // Shell semantics: the status, stdout and child stderr bytes are native.
    let status_code = compound.wait("staged compound").unwrap();
    assert_eq!(status_code.code(), reference.status.code());
    assert_eq!(fs::read(&out).unwrap(), reference.stdout);
    let (child_stderr, queued, started) = without_notices(&fs::read(&err).unwrap());
    assert_eq!(child_stderr.as_bytes(), reference.stderr);
    assert_eq!(
        (queued, started),
        (1, 1),
        "one notice pair for the waiting Test stage, none for gh"
    );
    assert_eq!(stage.starts().len(), 2);
    assert_eq!(status(&f, &remote.id)["outcome"]["code"], 0);
    if exec {
        let shell = status(&f, &request);
        assert_eq!(shell["classification"]["classification"], "MIXED");
        assert_eq!(shell["outcome"]["code"], 0, "{shell}");
        assert_eq!(linked(&f, &request).len(), 2, "the request is no stage");
    }
    build.release(&f);

    // The gh stage's own process is native too: the Test stub only succeeds,
    // so the one capturing stub is gh.
    quiet_stub(&s, if make { "cargo" } else { "bun" });
    let before = rows(&f).len();
    let (expected, _) = observe(&f, native(route, &s, &f, source), "native gh stage");
    assert_eq!(rows(&f).len(), before, "a native run registered a job");
    let (command, _) = managed(route, &s, &f, source);
    let (actual, pid) = observe(&f, command, "managed gh stage");
    assert_eq!(actual, expected);
    assert_eq!(actual.args, remote_stub["args"]);
    assert_eq!((actual.starts, actual.code), (1, Some(17)));
    let all = rows(&f);
    assert_eq!(all.len(), before + 2 + usize::from(exec), "{all:?}");
    let remote = status(&f, &all.last().unwrap().id);
    assert_eq!(remote["classification"]["classification"], "REMOTE_CI");
    assert_eq!(remote["child"]["pid"].as_u64(), pid);
    assert_eq!(remote["outcome"]["code"], 17);

    daemon.stop(libc::SIGTERM);
    f.report();
}
