use arc_detector::{classify, classify_argv, ClassifyRequest};
use arc_traffic::{
    Active, BypassReason, Decision, Policy, PolicyError, Resources, TrafficClass, WaitReason,
    MAX_RESOURCE_VALUE,
};
use std::ffi::OsString;

fn direct(args: &[&str]) -> arc_detector::Classification {
    classify_argv(&args.iter().map(OsString::from).collect::<Vec<_>>())
}

fn decision(
    args: &[&str],
    active: Active,
    policy: &Policy,
    head: bool,
) -> Result<Decision, PolicyError> {
    arc_traffic::decide(direct(args), active, policy, head)
}

#[test]
fn direct_classes_have_independent_windows_and_fixed_reservations() {
    let policy = Policy::default();
    assert_eq!(policy.validate(), Ok(()));
    for (args, class) in [
        (vec!["cargo", "build"], TrafficClass::Build),
        (vec!["cargo", "test"], TrafficClass::Test),
        (vec!["bun", "test"], TrafficClass::Test),
        (vec!["cargo", "clippy"], TrafficClass::Lint),
        (vec!["cargo", "fmt"], TrafficClass::Lint),
    ] {
        assert_eq!(
            decision(&args, Active::default(), &policy, true),
            Ok(Decision::Admit {
                class,
                reservation: policy.class(class).reservation,
            })
        );
        let mut active = Active::default();
        match class {
            TrafficClass::Build => active.build = policy.build.window,
            TrafficClass::Test => active.test = policy.test.window,
            TrafficClass::Lint => active.lint = policy.lint.window,
            TrafficClass::Typecheck => active.typecheck = policy.typecheck.window,
        }
        assert_eq!(
            decision(&args, active, &policy, true),
            Ok(Decision::Wait(WaitReason::ClassWindow))
        );
    }
    let active = Active {
        build: 1,
        ..Active::default()
    };
    assert!(matches!(
        decision(&["cargo", "test"], active, &policy, true),
        Ok(Decision::Admit { .. })
    ));
}

#[test]
fn cargo_and_bun_share_test_window_and_each_global_budget() {
    for (first, second) in [
        (["cargo", "test"], ["bun", "test"]),
        (["bun", "test"], ["cargo", "test"]),
    ] {
        let mut policy = Policy::default();
        let reservation = policy.test.reservation;
        policy.build.reservation = reservation;
        let active = Active {
            test: 1,
            resources: reservation,
            ..Active::default()
        };
        assert!(matches!(
            decision(&first, Active::default(), &policy, true),
            Ok(Decision::Admit {
                class: TrafficClass::Test,
                ..
            })
        ));
        assert_eq!(
            decision(&second, active, &policy, true),
            Ok(Decision::Wait(WaitReason::ClassWindow))
        );
        policy.test.window = 2;
        policy.budget.cpu_millis = reservation.cpu_millis * 2 - 1;
        assert_eq!(
            decision(&second, active, &policy, true),
            Ok(Decision::Wait(WaitReason::CpuBudget))
        );
        policy.budget.cpu_millis = reservation.cpu_millis * 2;
        policy.budget.memory_bytes = reservation.memory_bytes * 2 - 1;
        assert_eq!(
            decision(&second, active, &policy, true),
            Ok(Decision::Wait(WaitReason::MemoryBudget))
        );
        assert!(matches!(
            decision(&second, Active::default(), &policy, true),
            Ok(Decision::Admit {
                class: TrafficClass::Test,
                ..
            })
        ));
        for args in [
            vec!["bun", "test", "--watch"],
            vec!["bun", "test", "--hot"],
            vec!["bun", "test", "--unknown"],
            vec!["bun", "run", "test"],
        ] {
            assert_eq!(
                decision(&args, active, &policy, false),
                Ok(Decision::Bypass(BypassReason::Unknown))
            );
        }
    }
}

#[test]
fn strict_fifo_blocks_later_candidates_even_if_their_class_fits() {
    let policy = Policy::default();
    let active = Active {
        build: 1,
        ..Active::default()
    };
    assert_eq!(
        decision(&["cargo", "build"], active, &policy, true),
        Ok(Decision::Wait(WaitReason::ClassWindow))
    );
    assert_eq!(
        decision(&["cargo", "test"], active, &policy, false),
        Ok(Decision::Wait(WaitReason::Fifo))
    );
    assert!(matches!(
        decision(&["cargo", "test"], active, &policy, true),
        Ok(Decision::Admit { .. })
    ));
}

#[test]
fn aggregate_cpu_and_memory_are_checked_across_classes_with_exact_boundaries() {
    let policy = Policy::default();
    let test = policy.test.reservation;
    let mut active = Active {
        lint: 1,
        resources: Resources {
            cpu_millis: policy.budget.cpu_millis - test.cpu_millis,
            memory_bytes: policy.budget.memory_bytes - test.memory_bytes,
        },
        ..Active::default()
    };
    assert!(matches!(
        decision(&["cargo", "test"], active, &policy, true),
        Ok(Decision::Admit { .. })
    ));
    active.resources.cpu_millis += 1;
    assert_eq!(
        decision(&["cargo", "test"], active, &policy, true),
        Ok(Decision::Wait(WaitReason::CpuBudget))
    );
    active.resources.cpu_millis -= 1;
    active.resources.memory_bytes += 1;
    assert_eq!(
        decision(&["cargo", "test"], active, &policy, true),
        Ok(Decision::Wait(WaitReason::MemoryBudget))
    );
}

#[test]
fn bypass_is_immediate_even_with_a_full_window_and_fifo_waiter() {
    let policy = Policy::default();
    let active = Active {
        build: 1,
        test: 1,
        lint: 1,
        typecheck: 1,
        resources: policy.budget,
    };
    for (args, reason) in [
        (
            vec!["gh", "workflow", "run", "check.yml"],
            BypassReason::Remote,
        ),
        (
            vec!["ssh", "build.example", "cargo build"],
            BypassReason::Remote,
        ),
        (vec!["unknown-command"], BypassReason::Unknown),
    ] {
        assert_eq!(
            decision(&args, active, &policy, false),
            Ok(Decision::Bypass(reason))
        );
    }
    let compound = classify(&ClassifyRequest {
        command: "cargo build && cargo build".into(),
        shell_kind: "sh".into(),
    });
    assert_eq!(
        arc_traffic::decide(compound, active, &policy, false),
        Ok(Decision::Bypass(BypassReason::Unsupported))
    );
}

#[test]
fn invalid_configuration_and_overflow_fail_closed() {
    let mut policy = Policy::default();
    policy.budget.cpu_millis = 0;
    assert_eq!(policy.validate(), Err(PolicyError::InvalidBudget));
    assert_eq!(
        decision(&["cargo", "build"], Active::default(), &policy, true),
        Err(PolicyError::InvalidBudget)
    );
    policy = Policy::default();
    policy.test.window = 0;
    assert_eq!(policy.validate(), Err(PolicyError::InvalidClassPolicy));
    policy = Policy::default();
    policy.lint.reservation.memory_bytes = 0;
    assert_eq!(policy.validate(), Err(PolicyError::InvalidClassPolicy));
    policy = Policy::default();
    policy.build.reservation.memory_bytes = policy.budget.memory_bytes + 1;
    assert_eq!(
        policy.validate(),
        Err(PolicyError::ReservationExceedsBudget)
    );
    policy = Policy::default();
    policy.budget.memory_bytes = MAX_RESOURCE_VALUE + 1;
    assert_eq!(policy.validate(), Err(PolicyError::InvalidBudget));
    policy = Policy::default();
    for resources in [
        Resources {
            cpu_millis: u64::MAX,
            memory_bytes: 0,
        },
        Resources {
            cpu_millis: 0,
            memory_bytes: u64::MAX,
        },
        Resources {
            cpu_millis: MAX_RESOURCE_VALUE,
            memory_bytes: 0,
        },
    ] {
        assert_eq!(
            decision(
                &["cargo", "build"],
                Active {
                    resources,
                    ..Active::default()
                },
                &policy,
                true
            ),
            Err(PolicyError::ResourceOverflow)
        );
    }
    let mut invalid = direct(&["cargo", "build"]);
    invalid.locality = arc_detector::Locality::Remote;
    assert_eq!(
        arc_traffic::decide(invalid, Active::default(), &policy, true),
        Err(PolicyError::InvalidClassification)
    );
}
