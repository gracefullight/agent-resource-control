use crate::{
    native,
    runtime::{self, Fixture},
    support,
};
use arc_ipc::{Request, OPERATION_HEALTH, OPERATION_HELLO, STATUS_OK};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier,
    },
    thread,
    time::{Duration, Instant},
};

fn timed_samples(fixture: &Fixture, count: usize) -> serde_json::Value {
    let setup = Instant::now();
    let mut stream = fixture.connect();
    let setup_ns = setup.elapsed().as_nanos();
    let mut samples = Vec::with_capacity(count);
    let mut sizes = Vec::new();
    for index in 0..count {
        let request = Request::new(
            "small",
            if index % 2 == 0 {
                OPERATION_HELLO
            } else {
                OPERATION_HEALTH
            },
        );
        let start = Instant::now();
        let response = arc_ipc::exchange(&mut stream, &request).unwrap();
        let elapsed = start.elapsed().as_nanos() as u64;
        assert_eq!(response.status, STATUS_OK);
        if index < 2 {
            sizes.push(serde_json::json!({"operation":request.operation,
            "request_body_bytes":serde_json::to_vec(&request).unwrap().len(),
            "response_body_bytes":serde_json::to_vec(&response).unwrap().len()}));
        }
        samples.push(elapsed);
    }
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    serde_json::json!({"count":count,"failed":0,"clock":"std::time::Instant","unit":"nanoseconds",
        "connection_setup_ns":setup_ns,"p50_ns":sorted[count.div_ceil(2)-1],
        "p95_ns":sorted[(count*95).div_ceil(100)-1],"max_ns":sorted[count-1],
        "percentile":"nearest rank ceil(0.95*N)-1","framing_bytes_per_message":4,
        "sizes":sizes,"samples_ns":samples})
}

fn save_raw(name: &str, value: &serde_json::Value) {
    if let Some(root) = std::env::var_os("ARC_TEST_EVIDENCE_DIR") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
}

pub fn run() {
    let _serial = support::serial();
    // Small reversible probe before admitting the full native workload.
    let probe = Fixture::new();
    let mut child = probe.spawn("");
    let native_probe = native::sample(child.child.id()).unwrap();
    let probe_latency = timed_samples(&probe, 200);
    let (exit_us, peak) = child.stop(libc::SIGTERM);
    let probe_report = probe.report();
    assert_eq!(probe_report["metrics"]["frames_completed"], 200);
    runtime::receipt(
        "native-probe",
        &serde_json::json!({"rss_bytes":native_probe.0,
        "threads":native_probe.1,"lifetime_peak_rss_bytes":peak,"p95_ns":probe_latency["p95_ns"],
        "signal_to_exit_us":exit_us,"attempts":200,"completed":200,"report":probe_report}),
    );

    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    // Disclosed settling/idle intervals measure ordinary idle RSS rather than
    // interpreting structural buffer reservations as allocated resident memory.
    thread::sleep(Duration::from_secs(5));
    let mut idle = Vec::new();
    let start = Instant::now();
    for _ in 0..100 {
        let (rss, threads) = native::sample(child.child.id()).unwrap();
        idle.push(serde_json::json!({"elapsed_us":start.elapsed().as_micros(),"rss_bytes":rss,"threads":threads}));
        thread::sleep(Duration::from_millis(100));
    }
    let idle_rss_max = idle
        .iter()
        .map(|v| v["rss_bytes"].as_u64().unwrap())
        .max()
        .unwrap();
    let _warmup = timed_samples(&fixture, 200);
    let latency = timed_samples(&fixture, 10_000);
    save_raw("small-message-samples.json", &latency);

    let running = Arc::new(AtomicBool::new(true));
    let sampler_running = Arc::clone(&running);
    let pid = child.child.id();
    let sampler = thread::spawn(move || {
        let start = Instant::now();
        let mut observations = Vec::new();
        while sampler_running.load(Ordering::Acquire) {
            let (rss, threads) = native::sample(pid).unwrap();
            observations.push(serde_json::json!({"elapsed_us":start.elapsed().as_micros(),"rss_bytes":rss,"threads":threads}));
            thread::sleep(Duration::from_millis(10));
        }
        observations
    });
    let barrier = Arc::new(Barrier::new(5));
    let mut workers = Vec::new();
    for worker in 0..4 {
        let socket = fixture.socket.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            let mut stream = runtime::connect(&socket);
            barrier.wait();
            let mut completed = 0u64;
            for index in 0..25_000 {
                let request = Request::new(
                    format!("{worker}-{index}"),
                    if index % 2 == 0 {
                        OPERATION_HELLO
                    } else {
                        OPERATION_HEALTH
                    },
                );
                let response = arc_ipc::exchange(&mut stream, &request).unwrap();
                assert_eq!(response.status, STATUS_OK);
                assert_eq!(response.request_id, request.request_id);
                completed += 1;
            }
            completed
        }));
    }
    let start = Instant::now();
    barrier.wait();
    let completed: u64 = workers.into_iter().map(|w| w.join().unwrap()).sum();
    let load_us = start.elapsed().as_micros();
    running.store(false, Ordering::Release);
    let load_samples = sampler.join().unwrap();
    assert!(!load_samples.is_empty());
    assert_eq!(completed, 100_000);
    let recovered_latency = timed_samples(&fixture, 200);
    fixture.assert_cli();
    let (exit_us, lifetime_peak_rss_bytes) = child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["profiles"]["ipc"], "default");
    assert_eq!(report["metrics"]["connection_rejected"], 0);
    assert_eq!(report["metrics"]["request_rejected"], 0);
    assert_eq!(report["metrics"]["db_rejected"], 0);
    assert!(report["metrics"]["frames_completed"].as_u64().unwrap() >= completed + 10_400);
    let sampled_thread_peak = idle
        .iter()
        .chain(load_samples.iter())
        .map(|v| v["threads"].as_u64().unwrap())
        .max()
        .unwrap();
    let p95 = latency["p95_ns"].as_u64().unwrap();
    let summary = serde_json::json!({"host":native::host(),"configuration":"default",
        "scope":"Actor local snapshot plus owned uncommitted task-5 tests; source manifest binds exact bytes",
        "nativeMeasurementsRecorded":true,"srsComparisonComplete":true,
        "idle":{"settle_seconds":5,"window_seconds":10,"cadence_ms":100,"max_rss_bytes":idle_rss_max,
            "rss_mb":idle_rss_max as f64/1_000_000.0,"rss_mib":idle_rss_max as f64/1_048_576.0},
        "lifetime_peak_rss_bytes":lifetime_peak_rss_bytes,"sampled_native_thread_peak":sampled_thread_peak,
        "load_sampling_cadence_ms":10,"load_elapsed_us":load_us,
        "attempts":{"attempted":100000,"admitted":completed,"completed":completed,"rejected":0,"timed_out":0,"shutdown_cancelled":0},
        "small_message":{"count":10000,"warmup":200,"failed":0,"p95_ns":p95,"p50_ns":latency["p50_ns"],"max_ns":latency["max_ns"],"connection_setup_ns":latency["connection_setup_ns"]},
        "after_load_p95_ns":recovered_latency["p95_ns"],"signal_to_exit_us":exit_us,
        "srs":{"source_sha256":"a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e",
            "NFR-002":{"idle_rss_target_bytes":50000000,"passed":idle_rss_max<50_000_000},
            "NFR-004":{"small_message_p95_target_ns":20000000,"passed":p95<=20_000_000}},
        "report":report});
    save_raw(
        "native-observations.json",
        &serde_json::json!({"idle":idle,"load":load_samples}),
    );
    runtime::receipt("native-measurements", &summary);
    // SRS targets remain fixed. A comparison failure is a real failing check.
    assert!(
        idle_rss_max < 50_000_000,
        "SRS NFR-002 idle RSS target failed: {summary}"
    );
    assert!(
        p95 <= 20_000_000,
        "SRS NFR-004 P95 target failed: {summary}"
    );
}
