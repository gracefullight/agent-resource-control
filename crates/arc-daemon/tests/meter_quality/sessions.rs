//! Agent-session usage through the installed hooks (G-01 item 3): recorded
//! Claude Code and Cursor payloads re-addressed to fixture sessions, run by
//! the vendor's shell, and the `--by session` summary recomputed from SQLite.
use crate::{
    hook::{self, Vendor},
    journey::{output_with_input, starts, Sandbox},
    quality_support as support, recompute,
    runtime::Fixture,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

/// Raw session texts; each is at least 8 bytes, so the raw-id scan applies.
pub const SESSION_A: &str = "fixture-session-alpha-0001";
pub const SESSION_B: &str = "fixture-session-bravo-0002";

/// The digest the returned command carries, checking the option order.
#[track_caller]
fn session_option(command: &str, vendor: &str) -> String {
    let (_, keyed) = command
        .split_once(" hook-exec --arc-event ")
        .unwrap_or_else(|| panic!("not the keyed route: {command}"));
    let rest = &keyed[64..];
    let rest = rest
        .strip_prefix(&format!(" --arc-vendor {vendor} --arc-hook-overhead-ms "))
        .unwrap_or_else(|| panic!("no usage options: {command}"));
    let (_, session) = rest
        .split_once(" --arc-session ")
        .unwrap_or_else(|| panic!("no session digest: {command}"));
    let (digest, literal) = session.split_once(' ').unwrap();
    assert_eq!(literal, "-- cargo build", "{command}");
    assert!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    digest.to_owned()
}

fn run(f: &Fixture, s: &Sandbox, project: &Path, command: &str, marker: &Path) {
    let mut shell = s.shell(f, project, command);
    shell
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker);
    let before = starts(marker);
    let output = output_with_input(&mut shell, b"input\n", "returned hook command");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(starts(marker), before + 1, "the tool must run exactly once");
}

/// Two Claude Code sessions (one with two tool calls), a Cursor session
/// with the same session text as the first, and a direct `arcctl exec` with
/// no session. Returns the delivered events for the raw-id scan.
pub fn deliver(f: &Fixture, s: &Sandbox, project: &Path, marker: &Path) -> Vec<Value> {
    let claude = Vendor::Claude.install(f, s, project);
    let cursor = Vendor::Cursor.install(f, s, project);
    let mut events = Vec::new();
    let mut digests = Vec::new();
    for (n, (vendor, session)) in [
        (Vendor::Claude, SESSION_A),
        (Vendor::Claude, SESSION_A),
        (Vendor::Claude, SESSION_B),
        (Vendor::Cursor, SESSION_A),
    ]
    .into_iter()
    .enumerate()
    {
        let (mut event, installed, stored) = match vendor {
            Vendor::Claude => (
                hook::claude_event(project, "cargo build"),
                &claude,
                "claude-code",
            ),
            Vendor::Cursor => (Vendor::Cursor.recorded(), &cursor, "cursor"),
        };
        event["session_id"] = json!(session);
        event["tool_use_id"] = json!(format!("fixture-tool-call-{n:04}"));
        let command = vendor.returned_command(f, s, project, installed, &event);
        assert!(!command.contains(session), "raw session id in {command}");
        digests.push(session_option(&command, stored));
        run(f, s, project, &command, marker);
        events.push(event);
    }
    // One session's two calls share a digest; another session differs; the
    // same session text under another vendor differs.
    assert_eq!(digests[0], digests[1]);
    assert_ne!(digests[0], digests[2]);
    assert_ne!(digests[0], digests[3]);
    let mut direct = s.arcctl(f, project);
    direct
        .args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker);
    let output = output_with_input(&mut direct, b"input\n", "direct exec");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    events
}

#[derive(Debug)]
struct Expected {
    jobs: u64,
    eligible: u64,
    cpu: (Option<f64>, u64),
    cpu_magnitude: f64,
    duration: (Option<f64>, u64),
    duration_magnitude: f64,
    peak: (Option<i64>, u64),
    read: (Option<i64>, u64),
    written: (Option<i64>, u64),
    footprint: (Option<i64>, u64),
}

type Groups = BTreeMap<(String, String, u64), Expected>;

/// Units of `n·ε·Σ|x|` allowed between two floating-point sums of the same
/// `n` terms. The daemon folds its rows in Rust while SQLite's SUM uses its
/// own compensated order, so the two differ by rounding only; the classic
/// bound for summing `n` terms in any order is `(n-1)·ε·Σ|x|`. The worst
/// difference observed (five consecutive runs beside another test binary)
/// was 0.29 of this unit; 8 units keeps more than five times that, and is
/// still far below one job's own contribution, so a missing, extra or
/// misattributed job still fails.
const SUM_ROUNDING_UNITS: f64 = 8.0;

#[track_caller]
fn assert_sum(label: &str, printed: Option<f64>, expected: Option<f64>, n: u64, magnitude: f64) {
    match (printed, expected) {
        (None, None) => {}
        (Some(printed), Some(expected)) => {
            let unit = n.max(1) as f64 * f64::EPSILON * magnitude;
            let deviation = (printed - expected).abs();
            assert!(
                deviation <= SUM_ROUNDING_UNITS * unit,
                "{label}: printed {printed}, SQLite {expected}, n {n}"
            );
        }
        other => panic!("{label}: {other:?}"),
    }
}

fn expected(db: &Connection, from: u64, to: u64) -> Groups {
    let sql = format!(
        "SELECT COALESCE(j.session_key,'none'),COALESCE(j.vendor,'none'),
                (l.registered_at_ms/{h})*{h},count(*),
                count(CASE WHEN COALESCE(m.relation,'unverified')!='included_in_root' THEN 1 END),
                SUM({cpu}),count({cpu}),TOTAL(ABS({cpu})),
                SUM({duration}),count({duration}),TOTAL(ABS({duration})),
                MAX({peak}),count({peak}), SUM({read}),count({read}),
                SUM({written}),count({written}), SUM({footprint}),count({footprint})
         FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
         LEFT JOIN job_meter_summaries m ON m.job_id=j.id
         WHERE l.registered_at_ms>=?1 AND l.registered_at_ms<?2
           AND j.job_class NOT IN ('REMOTE_CI','REMOTE_EXEC')
         GROUP BY 1,2,3",
        h = recompute::HOUR_MS,
        cpu = recompute::JOB_CPU,
        duration = "CASE WHEN m.relation='canonical_root' AND m.report_json IS NOT NULL THEN json_extract(m.report_json,'$.run_elapsed.seconds')*1000.0 END",
        peak = "CASE WHEN m.relation='canonical_root' THEN json_extract(m.process_tree_json,'$.memory_peak_bytes.value') END",
        read = "CASE WHEN m.relation='canonical_root' THEN json_extract(m.process_tree_json,'$.disk_read_bytes.value') END",
        written = "CASE WHEN m.relation='canonical_root' THEN json_extract(m.process_tree_json,'$.disk_write_bytes.value') END",
        footprint = "CASE WHEN m.relation='canonical_root' AND j.footprint_start_completeness='complete' AND j.footprint_end_completeness='complete' THEN j.footprint_end_bytes-j.footprint_start_bytes END"
    );
    db.prepare(&sql)
        .unwrap()
        .query_map([from as i64, to as i64], |r| {
            Ok((
                (r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64),
                Expected {
                    jobs: r.get::<_, i64>(3)? as u64,
                    eligible: r.get::<_, i64>(4)? as u64,
                    cpu: (r.get(5)?, r.get::<_, i64>(6)? as u64),
                    cpu_magnitude: r.get(7)?,
                    duration: (r.get(8)?, r.get::<_, i64>(9)? as u64),
                    duration_magnitude: r.get(10)?,
                    peak: (r.get(11)?, r.get::<_, i64>(12)? as u64),
                    read: (r.get(13)?, r.get::<_, i64>(14)? as u64),
                    written: (r.get(15)?, r.get::<_, i64>(16)? as u64),
                    footprint: (r.get(17)?, r.get::<_, i64>(18)? as u64),
                },
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The printed `--by session` summary equals its SQLite recomputation, no
/// raw session id is printed, and the fixture's sessions are separate.
pub fn assert_summary(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    db: &Connection,
    range: (u64, u64),
) {
    let printed = support::summary(f, s, project, "session", range);
    let text = printed.to_string();
    for raw in [SESSION_A, SESSION_B] {
        assert!(!text.contains(raw), "raw session id printed");
    }
    support::assert_all_flagged(&printed);
    let mut expected = expected(db, range.0, range.1);
    let mut by_vendor: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for group in printed["groups"].as_array().unwrap() {
        let key = (
            group["session"].as_str().unwrap().to_owned(),
            group["vendor"].as_str().unwrap().to_owned(),
            group["bucket_start_ms"].as_u64().unwrap(),
        );
        let local = &group["local"];
        let total = local["job.count"]["total"].as_u64().unwrap();
        let expected = expected.remove(&key).unwrap();
        assert_eq!(total, expected.jobs, "{key:?}");
        for (metric, (value, measured), magnitude) in [
            ("cpu_core_seconds", expected.cpu, expected.cpu_magnitude),
            (
                "job.duration_ms",
                expected.duration,
                expected.duration_magnitude,
            ),
        ] {
            assert_sum(
                &format!("{key:?} {metric}"),
                local[metric]["value"].as_f64(),
                value,
                measured,
                magnitude,
            );
            assert_eq!(local[metric]["measured_jobs"], measured, "{key:?} {metric}");
            assert_eq!(
                local[metric]["eligible_jobs"], expected.eligible,
                "{key:?} {metric}"
            );
        }
        for (metric, (value, measured)) in [
            ("memory_peak_bytes", expected.peak),
            ("disk_read_bytes", expected.read),
            ("disk_write_bytes", expected.written),
            ("footprint_delta_bytes", expected.footprint),
        ] {
            assert_eq!(local[metric]["value"].as_i64(), value, "{key:?} {metric}");
            assert_eq!(local[metric]["measured_jobs"], measured, "{key:?} {metric}");
            assert_eq!(
                local[metric]["eligible_jobs"], expected.eligible,
                "{key:?} {metric}"
            );
        }
        assert!(key.0 == "none" || key.0.len() == 64, "{key:?}");
        if key.0 != "none" {
            *by_vendor
                .entry(key.1.clone())
                .or_default()
                .entry(key.0.clone())
                .or_default() += total;
        }
    }
    assert!(expected.is_empty(), "not printed: {expected:?}");
    // Two Claude Code sessions (2 and 1 jobs) and one Cursor session.
    let mut claude: Vec<_> = by_vendor
        .remove("claude-code")
        .unwrap()
        .into_values()
        .collect();
    claude.sort_unstable();
    assert_eq!(claude, [1, 2]);
    assert_eq!(
        by_vendor
            .remove("cursor")
            .unwrap()
            .into_values()
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(by_vendor.is_empty(), "{by_vendor:?}");
    // The direct command has no session: `none`, never a digest.
    let missing: Vec<_> = printed["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["session"] == "none")
        .collect();
    assert!(!missing.is_empty() && missing.iter().all(|g| g["vendor"] == "none"));
    // The stored digests are exactly the printed ones.
    let stored: Vec<String> = db
        .prepare("SELECT DISTINCT session_key FROM jobs WHERE session_key IS NOT NULL ORDER BY 1")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(stored.len(), 3, "{stored:?}");
    for digest in &stored {
        assert!(printed["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group["session"] == *digest));
    }
}
