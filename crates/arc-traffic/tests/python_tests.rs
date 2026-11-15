use arc_detector::{classify_argv, JobClass, Locality, Reason, Uncertainty};
use arc_traffic::{decide, Active, BypassReason, Decision, Policy, TrafficClass, WaitReason};

#[test]
fn python_uses_the_same_test_window_and_global_resources_as_cargo_and_bun() {
    let policy = Policy::default();
    for argv in [
        vec!["pytest"],
        vec![".venv/bin/python", "-m", "pytest"],
        vec!["python3", "-m", "unittest", "discover"],
    ] {
        let class = classify_argv(&argv.into_iter().map(Into::into).collect::<Vec<_>>());
        assert_eq!(
            decide(class, Active::default(), &policy, true),
            Ok(Decision::Admit {
                class: TrafficClass::Test,
                reservation: policy.test.reservation
            })
        );
        assert_eq!(
            decide(
                class,
                Active {
                    test: 1,
                    resources: policy.test.reservation,
                    ..Default::default()
                },
                &policy,
                true
            ),
            Ok(Decision::Wait(WaitReason::ClassWindow))
        );
        assert_eq!(
            decide(
                class,
                Active {
                    resources: policy.budget,
                    ..Default::default()
                },
                &policy,
                true
            ),
            Ok(Decision::Wait(WaitReason::CpuBudget))
        );
        assert_eq!(
            decide(class, Active::default(), &policy, false),
            Ok(Decision::Wait(WaitReason::Fifo))
        );
        assert_eq!(
            decide(class.legacy(), Active::default(), &policy, true),
            Ok(Decision::Bypass(BypassReason::Unknown))
        );
        assert_eq!(
            (class.classification, class.locality, class.uncertainty),
            (JobClass::Test, Locality::HostLocal, Uncertainty::None)
        );
        assert!(matches!(
            class.reason,
            Reason::PythonPytest | Reason::PythonUnittest
        ));
    }
    for argv in [
        vec!["pytest", "--watch"],
        vec!["python", "-I", "-m", "pytest"],
    ] {
        let class = classify_argv(&argv.into_iter().map(Into::into).collect::<Vec<_>>());
        assert_eq!(
            decide(class, Active::default(), &policy, true),
            Ok(Decision::Bypass(BypassReason::Unknown))
        );
    }
}
