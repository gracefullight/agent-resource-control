use super::*;

pub(super) fn kubectl_without_placement_proof_stays_observed_without_starting_a_probe() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::super::endpoints::install(&s);
    let kube = s.home.join("kube.yaml");
    let kube = kube.to_str().unwrap();
    let cases: [(&str, &[&str]); 5] = [
        (
            "exec with selected context and namespace",
            &[
                "kubectl",
                "--kubeconfig",
                kube,
                "--context",
                "second",
                "--namespace",
                "builds",
                "exec",
                "remote-pod",
                "--",
                "make",
            ],
        ),
        (
            "run",
            &[
                "kubectl",
                "--kubeconfig",
                kube,
                "run",
                "remote-pod",
                "--image=fixture",
            ],
        ),
        (
            "copy",
            &[
                "kubectl",
                "--kubeconfig",
                kube,
                "cp",
                "remote-pod:/tmp/input",
                "./output",
            ],
        ),
        (
            "debug",
            &[
                "kubectl",
                "--kubeconfig",
                kube,
                "debug",
                "remote-pod",
                "--image=fixture",
            ],
        ),
        (
            "attach",
            &["kubectl", "--kubeconfig", kube, "attach", "remote-pod"],
        ),
    ];
    let mut daemon = f.spawn_isolated("test");
    for (index, (name, argv)) in cases.iter().enumerate() {
        let before = admission_counts(&f);
        // Sandbox::classify checks that the kubectl stub was not started.
        let inspection = s.classify(&f, argv, &[]);
        assert_eq!(s.calls(), index, "{name}: classification launched a tool");
        // Sandbox::exec checks native argv/streams/status and exactly one stub
        // call, so a hidden placement query during preparation also fails.
        let (job, _) = s.exec(&f, argv, &[]);
        assert!(!assert_routes(name, &inspection, &job, "UNKNOWN"));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(s.calls(), index + 1, "{name}: extra tool execution");
        assert_eq!(admission_counts(&f), before, "{name}: local admission");
    }
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let queued_or_reserved: i64 = db
        .query_row(
            "SELECT count(*) FROM jobs WHERE queue_at IS NOT NULL OR reserved_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(queued_or_reserved, 0);
    drop(db);
    eprintln!(
        "Kubernetes without placement proof: denominator 5, UNKNOWN 5, false-local 0 per route"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}
