//! Usage summaries by vendor, project and period from recorded jobs, the
//! hook's vendor and overhead fields and `traffic.queue.depth`, through the
//! installed hook, built `arcctl` and a real daemon with offline stubs (lane
//! B: MTR-004, MTR-008, MTR-018, MET-013, MET-015, MET-017, MET-026). The
//! recorded payloads stand for the vendors; no live vendor session is run.
//! Every wait is on observed state; the 60 s deadlines are watchdogs only.
#[allow(dead_code)] // Only the installed hook and its returned command are used here.
#[path = "hook_event_identity/hook.rs"]
mod hook;
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)] // Only the permit ledger of the agent journey is used here.
#[path = "agent_test_journey/ledger.rs"]
mod ledger;
#[path = "usage_summary/recompute.rs"]
mod recompute;
#[allow(dead_code)] // Only the raw-id scan is used here.
#[path = "hook_event_identity/record.rs"]
mod record;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[allow(dead_code)] // Only the running command of the agent journey is used here.
#[path = "agent_test_journey/stage.rs"]
mod stage;
mod support;

use hook::Vendor;
use journey::{output_with_input, starts, Sandbox};
use recompute::{DAY_MS, HOUR_MS};
use runtime::Fixture;
use serde_json::{json, Value};
use std::path::Path;

/// The installed Claude Code hook still returns the keyed managed route for
/// `cargo build`, and the vendor's shell executing it records one BUILD job
/// carrying its event key.
#[test]
fn seam_installed_hook_keeps_keyed_cargo_build_rewrite() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    s.stub("cargo");
    let marker = f.root.join("cargo-starts");
    let mut daemon = f.spawn_isolated("");
    let installed = Vendor::Claude.install(&f, &s, &project);

    let event = hook::claude_event(&project, "cargo build");
    let command = Vendor::Claude.returned_command(&f, &s, &project, &installed, &event);
    let (route, _) = command
        .split_once(" hook-exec --arc-event ")
        .unwrap_or_else(|| panic!("not the keyed route: {command}"));
    hook::quoted_arcctl(route, "");
    assert!(command.ends_with(" -- cargo build"), "{command}");
    assert_eq!(starts(&marker), 0, "the hook itself ran a command");

    let mut shell = s.shell(&f, &project, &command);
    shell
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker);
    let output = output_with_input(&mut shell, b"input\n", "returned hook command");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(starts(&marker), 1, "offline cargo must run exactly once");
    let db = journey::database(&f);
    let recorded: (String, i64, bool) = db
        .query_row(
            "SELECT j.job_class,j.exit_code,l.event_key IS NOT NULL FROM jobs j \
             JOIN job_lifecycle l ON l.job_id=j.id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(recorded, ("BUILD".into(), 17, true));
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// The hook's own options of a returned keyed command: the vendor it names
/// and its measured overhead, between the event key and `--`.
#[track_caller]
fn hook_overhead(command: &str, vendor: &str) -> u64 {
    let (_, keyed) = command
        .split_once(" hook-exec --arc-event ")
        .unwrap_or_else(|| panic!("not the keyed route: {command}"));
    let (key, rest) = keyed.split_at(64);
    assert!(key
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    let (overhead, literal) = rest
        .strip_prefix(&format!(" --arc-vendor {vendor} --arc-hook-overhead-ms "))
        .and_then(|rest| rest.split_once(" -- "))
        .unwrap_or_else(|| panic!("no usage options: {command}"));
    assert_eq!(literal, "cargo build", "{command}");
    overhead.parse().unwrap()
}

/// Run `command` to its exit with the offline tool echoing its input; the
/// tool was started once with exactly `args`.
#[track_caller]
fn echo(mut command: std::process::Command, marker: &Path, args: &[&str]) {
    command
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker);
    let before = starts(marker);
    let output = output_with_input(&mut command, b"input\n", "managed command");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(starts(marker), before + 1, "the tool must run exactly once");
    let child: Value =
        serde_json::from_slice(output.stdout.strip_prefix(b"input\n").unwrap()).unwrap();
    let args: Vec<Vec<u8>> = args.iter().map(|a| a.as_bytes().to_vec()).collect();
    assert_eq!(child["args"], json!(args), "{output:?}");
}

/// `arcctl meter summary --by <by> ... --json`, parsed.
fn summary(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    by: &str,
    range: (u64, u64),
    bucket: &str,
) -> Value {
    let output = runtime::child_watchdog::output(
        s.arcctl(f, cwd).args([
            "meter",
            "summary",
            "--by",
            by,
            "--from",
            &range.0.to_string(),
            "--to",
            &range.1.to_string(),
            "--bucket",
            bucket,
            "--json",
        ]),
        "arcctl meter summary",
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1, "{value}");
    assert_eq!(
        value["query"],
        json!({"by":by,"from_ms":range.0,"to_ms":range.1,"bucket":bucket})
    );
    assert!(value["generated_at_ms"].as_u64().unwrap() >= range.0);
    value
}

/// The aggregate of `group` (by vendor or project) and `side` in one bucket.
fn aggregate<'a>(summary: &'a Value, by: &str, group: &str, side: &str) -> &'a Value {
    let groups: Vec<&Value> = summary["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g[by] == group)
        .collect();
    assert_eq!(groups.len(), 1, "{by} {group}: {summary}");
    &groups[0][side]
}

/// Claude Code and Cursor deliver recorded `cargo build` events through the
/// installed hooks, and a direct `arcctl exec -- cargo build` and a gh
/// REMOTE_CI `arcctl exec` run, in two projects. The printed vendor, project
/// and period aggregates equal their recomputation from SQLite; the remote job
/// is in its own group and adds nothing to local totals; `job.duration_ms`
/// is the recorded run duration (and `null` with its reason for a job whose
/// bounded best-effort report was not recorded); `arc.hook.overhead_ms` is
/// recorded, non-negative and the value the hook wrote for the hook jobs only.
#[test]
fn mtr_004_vendor_project_and_period_summaries_match_recorded_jobs() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let (alpha, beta) = (s.project("alpha"), s.project("beta"));
    s.stub("cargo");
    s.stub("gh");
    // Offline GitHub facts, so `gh workflow run` is REMOTE_CI (SRS 7.3).
    std::fs::write(
        s.home.join("gh/hosts.yml"),
        "github.com:\n  user: fixture\n",
    )
    .unwrap();
    std::fs::write(
        beta.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    )
    .unwrap();
    let marker = f.root.join("tool-starts");
    let mut daemon = f.spawn_isolated("");

    // Claude Code in alpha, Cursor in beta, each through its installed hook.
    let claude = hook::claude_event(&alpha, "cargo build");
    let installed = Vendor::Claude.install(&f, &s, &alpha);
    let claude_command = Vendor::Claude.returned_command(&f, &s, &alpha, &installed, &claude);
    let claude_overhead = hook_overhead(&claude_command, "claude-code");
    let cursor = Vendor::Cursor.recorded();
    let installed = Vendor::Cursor.install(&f, &s, &beta);
    let cursor_command = Vendor::Cursor.returned_command(&f, &s, &beta, &installed, &cursor);
    let cursor_overhead = hook_overhead(&cursor_command, "cursor");
    assert_eq!(starts(&marker), 0, "a hook ran a command");
    echo(s.shell(&f, &alpha, &claude_command), &marker, &["build"]);
    echo(s.shell(&f, &beta, &cursor_command), &marker, &["build"]);
    // A direct command in alpha and remote CI in beta.
    let mut direct = s.arcctl(&f, &alpha);
    direct.args(["exec", "--", "cargo", "build"]);
    echo(direct, &marker, &["build"]);
    let mut remote = s.arcctl(&f, &beta);
    remote.args(["exec", "--", "gh", "workflow", "run", "build.yml"]);
    echo(remote, &marker, &["workflow", "run", "build.yml"]);

    let db = journey::database(&f);
    type Recorded = (String, Option<String>, Option<i64>, Option<String>, String);
    let jobs: Vec<Recorded> = db
        .prepare(
            "SELECT j.job_class,j.vendor,j.hook_overhead_ms,j.project_key,j.state
             FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
             ORDER BY l.registered_at_ms,l.rowid",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let classes: Vec<&str> = jobs.iter().map(|j| j.0.as_str()).collect();
    assert_eq!(
        classes,
        ["BUILD", "BUILD", "BUILD", "REMOTE_CI"],
        "{jobs:?}"
    );
    assert!(jobs.iter().all(|j| j.4 == "FAILED"), "{jobs:?}");
    // MET-026: vendor and overhead only on the hook jobs, as the hook wrote them.
    let usage: Vec<_> = jobs.iter().map(|j| (j.1.as_deref(), j.2)).collect();
    assert_eq!(
        usage,
        [
            (Some("claude-code"), Some(claude_overhead as i64)),
            (Some("cursor"), Some(cursor_overhead as i64)),
            (None, None),
            (None, None),
        ]
    );
    // Every job names its project; the two projects differ.
    let projects: Vec<&str> = jobs.iter().map(|j| j.3.as_deref().unwrap()).collect();
    assert_eq!((projects[0], projects[1]), (projects[2], projects[3]));
    assert_ne!(projects[0], projects[1]);
    // A recorded canonical-root report value of one job, or `None` when the
    // wrapper's bounded best-effort report was not accepted.
    let recorded = |sql: &str, path: &str| -> Option<f64> {
        db.query_row(
            &format!(
                "SELECT CASE WHEN m.relation='canonical_root' AND m.report_json IS NOT NULL \
                 THEN json_extract(m.report_json,'{path}') END FROM jobs j \
                 JOIN job_meter_summaries m ON m.job_id=j.id WHERE {sql}"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    let local_jobs = [
        ("claude-code", "j.vendor='claude-code'"),
        ("cursor", "j.vendor='cursor'"),
        ("none", "j.vendor IS NULL AND j.job_class='BUILD'"),
    ];
    let reports = local_jobs
        .iter()
        .filter(|(_, sql)| recorded(sql, "$.run_elapsed.seconds").is_some())
        .count();
    // The duration claims below are about recorded reports, so at least one
    // must exist; the wrapper sends each one once, right after its child.
    assert!(reports > 0, "no job Meter report was recorded");

    let (first, last): (i64, i64) = db
        .query_row(
            "SELECT min(registered_at_ms),max(registered_at_ms) FROM job_lifecycle",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let (first, last) = (first as u64, last as u64);
    let hours = (first - first % HOUR_MS, last - last % HOUR_MS + HOUR_MS);
    let days = (first - first % DAY_MS, last - last % DAY_MS + DAY_MS);
    for (by, range, bucket) in [
        ("vendor", hours, "hour"),
        ("project", hours, "hour"),
        ("period", hours, "hour"),
        ("vendor", days, "day"),
        ("period", days, "day"),
    ] {
        let printed = summary(&f, &s, &alpha, by, range, bucket);
        assert_eq!(recompute::assert_matches(&db, &printed), 4, "{by} {bucket}");
    }

    // MTR-004/MTR-008 in one day bucket: three vendor groups, `none` holding
    // the direct job locally and the remote job separately with no CPU.
    let by_vendor = summary(&f, &s, &alpha, "vendor", days, "day");
    let vendors: Vec<&Value> = by_vendor["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| &g["vendor"])
        .collect();
    assert_eq!(vendors, ["claude-code", "cursor", "none"]);
    let none_local = aggregate(&by_vendor, "vendor", "none", "local");
    let none_remote = aggregate(&by_vendor, "vendor", "none", "remote");
    assert_eq!(none_local["job.count"]["total"], 1);
    assert_eq!(
        none_local["job.count"]["by_class_outcome"],
        json!([{"class":"BUILD","outcome":"FAILED","count":1}])
    );
    assert_eq!(none_remote["job.count"]["total"], 1);
    assert_eq!(
        none_remote["job.count"]["by_class_outcome"],
        json!([{"class":"REMOTE_CI","outcome":"FAILED","count":1}])
    );
    assert_eq!(none_remote["cpu_core_seconds"]["value"], Value::Null);
    assert_eq!(
        none_remote["cpu_core_seconds"]["reason"],
        "not_metered_locally"
    );
    // MET-013 and the local CPU of each group: the recorded values of that
    // job alone, `null` with a reason when its report was not recorded.
    for (vendor, sql) in local_jobs {
        let local = aggregate(&by_vendor, "vendor", vendor, "local");
        for (metric, path, scale) in [
            ("job.duration_ms", "$.run_elapsed.seconds", 1000.0),
            ("cpu_core_seconds", "$.cpu_core_seconds.total", 1.0),
        ] {
            let expected = recorded(sql, path).map(|value| value * scale);
            let printed = &local[metric];
            recompute::same(
                &format!("{vendor} {metric}"),
                printed["value"].as_f64(),
                expected,
            );
            if expected.is_none() {
                assert_eq!(printed["reason"], "no_recorded_report", "{vendor} {metric}");
            }
        }
        let overhead = &local["arc.hook.overhead_ms"];
        match vendor {
            "none" => assert_eq!(*overhead, json!({"sum":null,"max":null,"jobs":0})),
            "claude-code" => assert_eq!(overhead["sum"], claude_overhead),
            _ => assert_eq!(overhead["sum"], cursor_overhead),
        }
    }
    // The remote job adds nothing to the local totals of any grouping.
    let by_period = summary(&f, &s, &alpha, "period", days, "day");
    let local = &by_period["groups"][0]["local"];
    assert_eq!(local["job.count"]["total"], 3);
    assert_eq!(by_period["groups"][0]["remote"]["job.count"]["total"], 1);
    let local_cpu = local_jobs
        .iter()
        .filter_map(|(_, sql)| recorded(sql, "$.cpu_core_seconds.total"))
        .reduce(|a, b| a + b);
    recompute::same(
        "local cpu",
        local["cpu_core_seconds"]["value"].as_f64(),
        local_cpu,
    );
    drop(db);

    // Raw session and tool-call ids reached neither SQLite nor any stream.
    record::assert_no_raw_ids(&f, &claude);
    record::assert_no_raw_ids(&f, &cursor);
    daemon.stop(libc::SIGTERM);
    f.report();
    record::assert_no_raw_ids(&f, &claude);
}

/// `arcctl meter snapshot --json`, parsed.
fn snapshot(f: &Fixture, s: &Sandbox, cwd: &Path) -> Option<Value> {
    let output = runtime::child_watchdog::output(
        s.arcctl(f, cwd).args(["meter", "snapshot", "--json"]),
        "arcctl meter snapshot",
    )
    .unwrap();
    output
        .status
        .success()
        .then(|| serde_json::from_slice(&output.stdout).unwrap())
}

fn queued(db: &rusqlite::Connection) -> u64 {
    db.query_row(
        "SELECT count(*) FROM traffic_entries WHERE state='QUEUED'",
        [],
        |r| r.get::<_, i64>(0),
    )
    .unwrap() as u64
}

/// One `arcctl exec -- cargo build` that runs until its input is closed.
fn hold(f: &Fixture, s: &Sandbox, project: &Path, marker: &Path, name: &str) -> stage::Run {
    let mut command = s.arcctl(f, project);
    command
        .args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker);
    stage::Run::start(&mut command, &f.root, name)
}

/// While one `cargo build` holds the single Build permit, two more requests
/// are QUEUED. `arcctl meter snapshot --json` then reports
/// `traffic.queue.depth` 2 from daemon state, beside unchanged v1 fields; a
/// protocol-1 read of the same daemon gets no `traffic` key. After the
/// release the depth is 0 again.
#[test]
fn met_017_snapshot_reports_traffic_queue_depth_while_build_permit_is_held() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    s.stub("cargo");
    let marker = f.root.join("cargo-starts");
    let mut daemon = f.spawn_isolated("");
    let db = journey::database(&f);
    db.busy_timeout(std::time::Duration::from_secs(60)).unwrap();

    let mut holder = hold(&f, &s, &project, &marker, "holder");
    holder.until("the held Build permit", || {
        let entries = ledger::entries(&db);
        entries.len() == 1
            && entries[0].state == "RUNNING"
            && entries[0].permit.as_deref() == Some("ACTIVE")
    });
    let mut waiting = Vec::new();
    for name in ["second", "third"] {
        let mut run = hold(&f, &s, &project, &marker, name);
        let expected = waiting.len() as u64 + 1;
        run.until("the queued request", || queued(&db) == expected);
        waiting.push(run);
    }
    // One permit, held by the running entry; the queued two hold none.
    let entries = ledger::entries(&db);
    let states: Vec<_> = entries
        .iter()
        .map(|e| (e.state.as_str(), e.permit.as_deref()))
        .collect();
    assert_eq!(
        states,
        [
            ("RUNNING", Some("ACTIVE")),
            ("QUEUED", None),
            ("QUEUED", None)
        ]
    );

    let mut traffic = None;
    journey::eventually("a Meter snapshot", || {
        traffic = snapshot(&f, &s, &project);
        traffic.is_some()
    });
    let traffic = traffic.unwrap();
    assert_eq!(traffic["traffic"]["queue_depth"], 2, "{traffic}");
    assert_eq!(traffic["traffic"]["queue_depth"], queued(&db));
    assert!(traffic["traffic"]["observed_at_ms"].as_u64().unwrap() > 0);
    let mut v1 = traffic.clone();
    v1.as_object_mut().unwrap().remove("traffic");
    arc_ipc::meter::decode_snapshot(&v1).unwrap();
    // A protocol-1 read: the strict v1 decoder accepts only the v1 fields.
    arc_ipc::meter::snapshot_until(
        &f.socket,
        "protocol-1",
        std::time::Instant::now() + std::time::Duration::from_secs(60),
        &|| false,
    )
    .unwrap();

    holder.input(b"input\n");
    let done = holder.finish("holder");
    assert_eq!(done.status.code(), Some(17));
    for (name, mut run) in ["second", "third"].into_iter().zip(waiting) {
        run.input(b"input\n");
        assert_eq!(run.finish(name).status.code(), Some(17));
    }
    assert_eq!(starts(&marker), 3);
    assert_eq!(queued(&db), 0);
    let after = snapshot(&f, &s, &project).unwrap();
    assert_eq!(after["traffic"]["queue_depth"], 0, "{after}");
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}
