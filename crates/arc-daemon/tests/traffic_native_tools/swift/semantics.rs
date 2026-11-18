//! Success/failure baselines and managed comparison; shared recovery remains shared.
use super::{projects::*, tools::Tools, *};
use crate::suite::{entered, permit_history, terminal, Run};
use serde_json::json;
use std::{
    io::Write,
    path::Path,
    process::{Output, Stdio},
};

pub fn offline() {
    let tools = Tools::installed();
    let facts = std::thread::scope(|scope| {
        let mut jobs = Vec::new();
        for form in [Form::SwiftBuild, Form::SwiftTest] {
            for fail in [false, true] {
                let tools = &tools;
                jobs.push(scope.spawn(move || {
                    let prepared = prepare(
                        &tools.directory,
                        tools,
                        &format!("{}-{fail}", form.id()),
                        form,
                        false,
                        fail,
                    );
                    let output = direct(tools, &prepared);
                    assert_result(&output, form, fail);
                    super::streams::validate_form(form, &output.stdout, &output.stderr);
                    let record = record(&prepared.project);
                    let facts = json!({"form":form.id(),"fail":fail,"exitCode":output.status.code(),
                        "record":record,"argv":prepared.argv,"offline":true,"executions":1,
                        "nativeChild":super::evidence::observation(&prepared.project),
                        "stdinSupplied":INPUT,"privateCacheWritesVerified":true});
                    tools.release_success(&prepared.project);
                    facts
                }));
            }
        }
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>()
    });
    println!(
        "SWIFT_OFFLINE_READINESS {}",
        json!({"tools":tools.receipt(),"cases":facts})
    );
}

pub fn managed(read: Read) {
    let tools = Tools::installed();
    let receipt = tools.receipt();
    let observers = tools.observers.clone();
    let case = Case::with_tools("swift-semantics", read, false, tools, receipt, observers);
    compare_all(
        |form, fail| {
            let baseline = prepare(
                &case.artifacts,
                &case.tools,
                &format!("direct-{}-{fail}", form.id()),
                form,
                false,
                fail,
            );
            let expected = direct(&case.tools, &baseline);
            assert_result(&expected, form, fail);
            // The direct case is independently complete even if a sibling or
            // its later managed comparison fails.
            let baseline_record = record(&baseline.project);
            let argv = baseline.argv.clone();
            case.tools.release_baseline(&baseline.project);
            (argv, baseline_record, expected)
        },
        |form, fail, (argv, baseline_record, expected)| {
            managed_one(&case, form, fail, &argv, &baseline_record, &expected);
        },
    );
    let permits = permit_history(&case, "BUILD", 4, 1);
    case.save("swift-semantics-permits", permits);
    case.done();
}

// Separate direct baselines can compile concurrently. Managed comparisons use
// the same daemon and wait for each preceding terminal reconciliation before
// starting the next, so different class releases cannot race for its launch
// fence. The ordered-pair scenarios separately exercise actual queuing.
fn compare_all<T: Send>(
    baseline: impl Fn(Form, bool) -> T + Sync,
    mut managed: impl FnMut(Form, bool, T),
) {
    let cases = [
        (Form::SwiftBuild, false),
        (Form::SwiftBuild, true),
        (Form::SwiftTest, false),
        (Form::SwiftTest, true),
    ];
    let baselines = std::thread::scope(|scope| {
        let jobs: Vec<_> = cases
            .into_iter()
            .map(|(form, fail)| {
                let baseline = &baseline;
                scope.spawn(move || (form, fail, baseline(form, fail)))
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>()
    });
    for (form, fail, baseline) in baselines {
        managed(form, fail, baseline);
    }
}

fn managed_one(
    case: &Case<Tools>,
    form: Form,
    fail: bool,
    baseline_argv: &[String],
    baseline_record: &serde_json::Value,
    expected: &Output,
) {
    let prepared = prepare(
        &case.artifacts,
        &case.tools,
        &format!("managed-{}-{fail}", form.id()),
        form,
        true,
        fail,
    );
    assert_eq!(baseline_argv[..5], prepared.argv[..5]);
    let argv: Vec<_> = prepared.argv.iter().map(String::as_str).collect();
    let mut run = Run::start_with(case, &prepared.project, &argv, |command| {
        configure(&prepared.project, command);
        command.stdin(Stdio::piped());
    });
    run.wrapper
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(INPUT)
        .unwrap();
    let entry = entered(case, &mut run, &prepared);
    let code = i32::from(fail);
    crate::suite::finish_swift(&mut run, code);
    let actual = record(&prepared.project);
    assert_eq!(&actual, baseline_record);
    assert_streams(form, expected, &prepared.project);
    assert_eq!(super::evidence::body_count(&prepared.project), 1);
    let terminal = terminal(case, &run, form, code);
    case.save(
        &format!("{}-{}", form.id(), if fail { "failure" } else { "success" }),
        json!({"originalArgv":prepared.argv,"baseline":actual,"outcome":terminal,
                    "entry":entry,"nativeChild":super::evidence::observation(&prepared.project),
                    "offline":true,"eachExecutedOnce":true,"originalExecutionSemantics":true}),
    );
    // Preserve this completed comparison for a later failure, but do not hold
    // its build tree while the remaining comparisons wait for their leases.
    case.tools.release_baseline(&prepared.project);
}

fn assert_streams(form: Form, expected: &Output, project: &Path) {
    super::streams::compare(
        form,
        [&expected.stdout, &expected.stderr],
        [
            &std::fs::read(project.join("stdout")).unwrap(),
            &std::fs::read(project.join("stderr")).unwrap(),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    };
    use std::time::Duration;

    #[test]
    fn baselines_finish_before_managed_comparisons_and_each_launch_is_reconciled() {
        const WATCHDOG: Duration = Duration::from_secs(60);
        let (baseline_started, starts) = mpsc::channel();
        let (baseline_release, release_baseline) = mpsc::channel();
        let release_baseline = Mutex::new(release_baseline);
        let (baseline_finished, finishes) = mpsc::channel();
        let (managed_started, managed_starts) = mpsc::channel();
        let (managed_release, release_managed) = mpsc::channel();
        let (managed_finished, managed_finishes) = mpsc::channel();
        let (done, completed) = mpsc::channel();
        let finished = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicUsize::new(0));
        let worker = std::thread::spawn(move || {
            compare_all(
                |form, fail| {
                    baseline_started.send((form, fail)).unwrap();
                    release_baseline
                        .lock()
                        .unwrap()
                        .recv_timeout(WATCHDOG)
                        .expect("baseline was never released");
                    finished.fetch_add(1, Ordering::SeqCst);
                    baseline_finished.send((form, fail)).unwrap();
                    (form, fail)
                },
                |form, fail, baseline| {
                    assert_eq!(baseline, (form, fail));
                    assert_eq!(finished.load(Ordering::SeqCst), 4);
                    assert_eq!(running.fetch_add(1, Ordering::SeqCst), 0);
                    managed_started.send((form, fail)).unwrap();
                    release_managed
                        .recv_timeout(WATCHDOG)
                        .expect("managed comparison was never reconciled");
                    assert_eq!(running.fetch_sub(1, Ordering::SeqCst), 1);
                    managed_finished.send((form, fail)).unwrap();
                },
            );
            done.send(()).unwrap();
        });

        let cases = [
            (Form::SwiftBuild, false),
            (Form::SwiftBuild, true),
            (Form::SwiftTest, false),
            (Form::SwiftTest, true),
        ];
        let mut observed = Vec::new();
        for _ in cases {
            observed.push(
                starts
                    .recv_timeout(WATCHDOG)
                    .expect("baseline never started"),
            );
        }
        for case in cases {
            assert!(observed.contains(&case));
        }
        for _ in 0..3 {
            baseline_release.send(()).unwrap();
            finishes
                .recv_timeout(WATCHDOG)
                .expect("baseline never finished");
        }
        assert!(matches!(
            managed_starts.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        baseline_release.send(()).unwrap();
        finishes
            .recv_timeout(WATCHDOG)
            .expect("last baseline never finished");
        for case in cases {
            assert_eq!(
                managed_starts
                    .recv_timeout(WATCHDOG)
                    .expect("managed comparison never started"),
                case
            );
            managed_release.send(()).unwrap();
            assert_eq!(
                managed_finishes
                    .recv_timeout(WATCHDOG)
                    .expect("managed comparison never finished"),
                case
            );
        }
        completed
            .recv_timeout(WATCHDOG)
            .expect("comparison worker never exited");
        worker.join().unwrap();
    }
}
