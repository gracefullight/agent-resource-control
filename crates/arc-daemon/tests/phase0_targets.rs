//! Phase 0 performance targets on production binaries (NFR-001, NFR-003,
//! NFR-005). Samples stay in memory; nothing is written beside the sources.
//!
//! One daemon may run per user, so every test that starts one holds the
//! fixture lock, the 30 s idle window of NFR-001 included. NFR-003 starts no
//! daemon (the hook never contacts one) and runs beside that window, which
//! keeps this binary well under its 60 s budget.
#[path = "phase0_targets/cpu.rs"]
mod cpu;
#[path = "phase0_targets/hook.rs"]
mod hook;
#[path = "support/journey.rs"]
mod journey;
#[path = "support/native.rs"]
mod native;
#[path = "support/percentile.rs"]
mod percentile;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
mod support;
#[path = "phase0_targets/watchdog.rs"]
mod watchdog;

use arc_core::meter::SamplingMode;
use arc_ipc::{
    locality::{ClassifyLocalityRequest, Command},
    Request, OPERATION_JOB_CLASSIFY, STATUS_OK,
};
use journey::Sandbox;
use percentile::nearest_rank;
use runtime::{child_watchdog, Fixture};
use serde_json::{json, Value};
use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

/// SRS NFR-001: idle daemon CPU below 0.5% of one core (CPU time, not an elapsed-time bound).
const NFR_001_CPU_PARTS_PER_100_000: u128 = 500;
/// SRS NFR-003: hook P95 minus reference P95 at most 50 ms. Worst loaded run (two series of five): 5.4 ms, margin 9.3x.
const NFR_003_OVERHEAD_NS: u64 = 50_000_000;
/// SRS NFR-005: classification round-trip P95 at most 50 ms. Worst loaded run (two series of five): 0.31 ms, margin 164x.
const NFR_005_P95_NS: u64 = 50_000_000;

/// A whole multiple of the 15,000 ms idle Meter cadence, so a tick falls inside.
const IDLE_WINDOW: Duration = Duration::from_secs(30);
const IDLE_SETTLE: Duration = Duration::from_secs(3);
const ORDINARY_COMMAND: &str = "cargo build";
const CLASSIFY_WARM_UP: usize = 200;
const CLASSIFY_SAMPLES: usize = 1_000;
const BYPASS_COMMAND: &str = "git status";
const HOOK_WARM_UP: usize = 10;
const HOOK_SAMPLES: usize = 300;

/// The elapsed-time targets of this binary are never sampled at the same time.
fn timing() -> MutexGuard<'static, ()> {
    static TIMING: Mutex<()> = Mutex::new(());
    TIMING.lock().unwrap_or_else(|error| error.into_inner())
}

/// The daemon fixture lock shared by every test binary of this user, with the
/// time this test waited for it. Waiting on another binary's daemon work is
/// not time this binary spends, so each test prints its wait and its hold.
struct Held {
    _guard: support::Serial,
    waited_ms: u64,
    since: Instant,
}

impl Held {
    fn held_ms(&self) -> u64 {
        self.since.elapsed().as_millis() as u64
    }
}

fn serial() -> Held {
    let start = Instant::now();
    let guard = support::serial();
    Held {
        _guard: guard,
        waited_ms: start.elapsed().as_millis() as u64,
        since: Instant::now(),
    }
}

fn milliseconds(nanoseconds: u64) -> f64 {
    nanoseconds as f64 / 1_000_000.0
}

#[test]
fn nfr_001_idle_daemon_cpu_below_half_percent_of_one_core() {
    let lock = serial();
    // The counters must mean nanoseconds before an idle reading can be trusted:
    // a unit error would shrink any consumption below the target.
    let calibration = cpu::calibrate();
    assert!(
        calibration.rusage_ns >= 50_000_000,
        "calibration child burned too little CPU to compare: {} ns",
        calibration.rusage_ns
    );
    assert!(
        calibration.counters_ns.abs_diff(calibration.rusage_ns) <= calibration.rusage_ns / 10,
        "process CPU counters {} ns disagree with wait4 rusage {} ns",
        calibration.counters_ns,
        calibration.rusage_ns
    );

    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("");
    let idle = cpu::idle(&fixture, daemon.child.id(), IDLE_SETTLE, IDLE_WINDOW);
    daemon.stop(libc::SIGTERM);
    let report = fixture.report();
    let window_ns = idle.window.as_nanos();
    eprintln!(
        "phase0-target NFR-001: {}",
        json!({"cpu_ns":idle.cpu_ns,"window_ms":idle.window.as_millis() as u64,
            "percent_of_one_core":idle.cpu_ns as f64 * 100.0 / window_ns as f64,
            "meter_ticks_in_window":idle.ticks,"meter_mode":idle.mode,
            "meter_interval_ms":idle.interval_ms,"settle_ms":IDLE_SETTLE.as_millis() as u64,
            "calibration":{"counters_ns":calibration.counters_ns,"rusage_ns":calibration.rusage_ns},
            "frames_completed":report["metrics"]["frames_completed"],
            "fixture_lock_wait_ms":lock.waited_ms,"fixture_lock_held_ms":lock.held_ms()})
    );
    assert!(idle.window >= IDLE_WINDOW, "{:?}", idle.window);
    // No job was ever registered, so the Meter never left its unloaded cadences.
    assert_ne!(idle.mode, SamplingMode::Active);
    assert!(
        idle.ticks >= 1,
        "no Meter sampling tick fell inside the idle window"
    );
    assert!(
        u128::from(idle.cpu_ns) * 100_000 < window_ns * NFR_001_CPU_PARTS_PER_100_000,
        "SRS NFR-001 idle CPU target failed: {} ns of CPU in {:?}",
        idle.cpu_ns,
        idle.window
    );
}

#[test]
fn nfr_003_installed_hook_bypass_p95_overhead_within_50_ms() {
    let _timing = timing();
    let started = Instant::now();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    let installed = hook::install(&f, &s, &project);
    let invoke = |command: &str| {
        hook::timed(
            &mut s.shell(&f, &project, &installed),
            &hook::event(&project, command),
            "installed hook",
        )
    };
    // The measured command is the working hook: it rewrites a managed literal.
    let (_, managed) = invoke("cargo build");
    assert!(managed.status.success(), "{managed:?}");
    let managed: Value = serde_json::from_slice(&managed.stdout).unwrap();
    let rewritten = managed["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    assert!(
        rewritten.ends_with(" hook-exec -- cargo build"),
        "{managed}"
    );

    let hook_run = || {
        let (elapsed, output) = invoke(BYPASS_COMMAND);
        // Pass-through: success and no response, so the vendor runs the command unchanged.
        assert!(output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        elapsed
    };
    // The reference is a no-op started exactly as the vendor starts the hook:
    // one command through its shell tool. A no-op started without that shell
    // is sampled beside it, so the cost of the shell itself stays visible.
    let no_op = format!("'{}'", hook::no_op().to_str().unwrap());
    let reference_run = || {
        let mut command = s.shell(&f, &project, &no_op);
        let (elapsed, output) = hook::timed(&mut command, &[], "reference no-op");
        assert!(output.status.success(), "{output:?}");
        elapsed
    };
    let direct_run = || {
        let mut command = s.command(&f, hook::no_op(), &project);
        let (elapsed, output) = hook::timed(&mut command, &[], "direct no-op");
        assert!(output.status.success(), "{output:?}");
        elapsed
    };
    for _ in 0..HOOK_WARM_UP {
        hook_run();
        reference_run();
        direct_run();
    }
    let (mut hooks, mut references, mut directs) = (Vec::new(), Vec::new(), Vec::new());
    for index in 0..HOOK_SAMPLES {
        // Interleaved in rotating order, so every sample set sees the same host.
        for turn in 0..3 {
            match (index + turn) % 3 {
                0 => hooks.push(hook_run()),
                1 => references.push(reference_run()),
                _ => directs.push(direct_run()),
            }
        }
    }
    let (hook_p95, reference_p95) = (nearest_rank(&hooks, 95), nearest_rank(&references, 95));
    let direct_p95 = nearest_rank(&directs, 95);
    let overhead = hook_p95.saturating_sub(reference_p95);
    eprintln!(
        "phase0-target NFR-003: {}",
        json!({"hook_samples":hooks.len(),"reference_samples":references.len(),
            "direct_samples":directs.len(),
            "hook_p95_ms":milliseconds(hook_p95),"reference_p95_ms":milliseconds(reference_p95),
            "overhead_p95_ms":milliseconds(overhead),
            "direct_spawn_p95_ms":milliseconds(direct_p95),
            "overhead_over_direct_spawn_p95_ms":milliseconds(hook_p95.saturating_sub(direct_p95)),
            "hook_p50_ms":milliseconds(nearest_rank(&hooks, 50)),
            "hook_max_ms":milliseconds(nearest_rank(&hooks, 100)),
            "reference_p50_ms":milliseconds(nearest_rank(&references, 50)),
            "payload":BYPASS_COMMAND,"test_elapsed_ms":started.elapsed().as_millis() as u64})
    );
    assert!(hooks.len() >= 200 && references.len() >= 200);
    assert!(
        overhead <= NFR_003_OVERHEAD_NS,
        "SRS NFR-003 hook overhead target failed: hook P95 {hook_p95} ns, reference P95 {reference_p95} ns"
    );
}

#[test]
fn nfr_005_cached_ordinary_classification_p95_within_50_ms() {
    let lock = serial();
    let _timing = timing();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    let mut daemon = f.spawn_isolated("");
    // Path proof, untimed: the built CLI classifies the same command through this daemon.
    for _ in 0..3 {
        let output = child_watchdog::output(
            s.arcctl(&f, &project)
                .args(["classify", "--json", ORDINARY_COMMAND]),
            "arcctl classify",
        )
        .unwrap();
        assert!(output.status.success(), "{output:?}");
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["classification"], "BUILD", "{result}");
        assert_eq!(result["locality"], "HOST_LOCAL", "{result}");
    }

    // The protocol-1 request, and the endpoint-aware request the CLI sends
    // with a context captured for each request.
    let contextual = serde_json::to_value(ClassifyLocalityRequest {
        command: Command::Shell {
            command: ORDINARY_COMMAND.into(),
            shell_kind: "sh".into(),
        },
        context: Default::default(),
    })
    .unwrap();
    let requests = [
        (OPERATION_JOB_CLASSIFY, json!({"command":ORDINARY_COMMAND})),
        (arc_ipc::locality::OPERATION_CLASSIFY, contextual),
    ];
    let mut stream = f.connect();
    let mut measured = Vec::new();
    for (operation, payload) in requests {
        let mut samples = Vec::with_capacity(CLASSIFY_SAMPLES);
        for index in 0..CLASSIFY_WARM_UP + CLASSIFY_SAMPLES {
            let mut request = Request::new(format!("classify-{index}"), operation);
            request.payload = payload.clone();
            if let Some(context) = request.payload.get_mut("context") {
                context["captured_at_ms"] = json!(arc_ipc::locality::now_ms().unwrap());
            }
            let start = Instant::now();
            let response = arc_ipc::exchange(&mut stream, &request).unwrap();
            let elapsed = u64::try_from(start.elapsed().as_nanos()).unwrap();
            assert_eq!(response.status, STATUS_OK, "{response:?}");
            assert_eq!(response.request_id, request.request_id);
            let result = response.result.unwrap();
            assert_eq!(result["classification"], "BUILD", "{result}");
            assert_eq!(result["locality"], "HOST_LOCAL", "{result}");
            if index >= CLASSIFY_WARM_UP {
                samples.push(elapsed);
            }
        }
        measured.push((operation, samples));
    }
    drop(stream);
    daemon.stop(libc::SIGTERM);
    let report = f.report();
    let frames = report["metrics"]["frames_completed"].as_u64().unwrap();
    // Every timed round trip, and at least one more per CLI run, was served by this daemon.
    assert!(frames >= 2 * (CLASSIFY_WARM_UP + CLASSIFY_SAMPLES) as u64 + 3);
    for (operation, samples) in &measured {
        let p95 = nearest_rank(samples, 95);
        eprintln!(
            "phase0-target NFR-005: {}",
            json!({"operation":operation,"samples":samples.len(),"warm_up":CLASSIFY_WARM_UP,
                "p50_ms":milliseconds(nearest_rank(samples, 50)),"p95_ms":milliseconds(p95),
                "max_ms":milliseconds(nearest_rank(samples, 100)),"command":ORDINARY_COMMAND,
                "fixture_lock_wait_ms":lock.waited_ms,"fixture_lock_held_ms":lock.held_ms()})
        );
        assert!(samples.len() >= 1_000);
        assert!(
            p95 <= NFR_005_P95_NS,
            "SRS NFR-005 classification target failed for {operation}: P95 {p95} ns"
        );
    }
}

#[test]
fn nearest_rank_is_the_ceiling_rank_member_of_the_sample_set() {
    let hundred: Vec<u64> = (1..=100).rev().collect();
    assert_eq!(nearest_rank(&hundred, 95), 95);
    assert_eq!(nearest_rank(&hundred, 50), 50);
    assert_eq!(nearest_rank(&hundred, 100), 100);
    assert_eq!(nearest_rank(&hundred, 1), 1);
    // ceil(0.95 * 10) = 10 and ceil(0.95 * 21) = 20: never interpolated.
    assert_eq!(nearest_rank(&[7, 3, 9, 1, 5, 2, 8, 4, 6, 10], 95), 10);
    let odd: Vec<u64> = (1..=21).map(|n| n * 10).collect();
    assert_eq!(nearest_rank(&odd, 95), 200);
    assert_eq!(nearest_rank(&[42], 95), 42);
}

#[test]
fn seam_real_daemon_classification_round_trips_feed_the_percentile_helper() {
    let lock = serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("");
    let mut stream = fixture.connect();
    let mut samples = Vec::new();
    for index in 0..16 {
        let mut request = Request::new(format!("classify-{index}"), OPERATION_JOB_CLASSIFY);
        request.payload = serde_json::json!({"command":"cargo build"});
        let start = Instant::now();
        let response = arc_ipc::exchange(&mut stream, &request).unwrap();
        samples.push(u64::try_from(start.elapsed().as_nanos()).unwrap());
        assert_eq!(response.status, STATUS_OK, "{response:?}");
        assert_eq!(response.request_id, request.request_id);
        let result = response.result.unwrap();
        assert_eq!(result["classification"], "BUILD", "{result}");
        assert_eq!(result["locality"], "HOST_LOCAL", "{result}");
    }
    // No elapsed-time bound here: the helper selects observed samples.
    let (p50, p95, max) = (
        nearest_rank(&samples, 50),
        nearest_rank(&samples, 95),
        nearest_rank(&samples, 100),
    );
    for selected in [p50, p95, max] {
        assert!(samples.contains(&selected));
    }
    assert!(p50 <= p95 && p95 <= max);
    assert_eq!(Some(&max), samples.iter().max());
    drop(stream);
    daemon.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["frames_completed"], 16);
    eprintln!(
        "phase0-target seam: {}",
        json!({"fixture_lock_wait_ms":lock.waited_ms,"fixture_lock_held_ms":lock.held_ms()})
    );
}
