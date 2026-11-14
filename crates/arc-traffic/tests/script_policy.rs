use arc_detector::{Classification, JobClass, Locality, Reason, Uncertainty};
use arc_traffic::{
    decide, Active, Decision, Policy, PolicyError, Resources, TrafficClass, WaitReason,
};

fn classification(class: JobClass, reason: Reason) -> Classification {
    Classification {
        classification: class,
        reason,
        locality: Locality::HostLocal,
        uncertainty: Uncertainty::None,
    }
}
#[test]
fn all_script_classes_use_their_existing_or_distinct_windows() {
    let p = Policy::default();
    for (class, reason, window) in [
        (JobClass::Build, Reason::BunScriptBuild, TrafficClass::Build),
        (JobClass::Test, Reason::BunScriptTest, TrafficClass::Test),
        (JobClass::Lint, Reason::BunScriptLint, TrafficClass::Lint),
        (
            JobClass::Typecheck,
            Reason::BunScriptTypecheck,
            TrafficClass::Typecheck,
        ),
    ] {
        let c = classification(class, reason);
        assert_eq!(
            decide(c, Active::default(), &p, true),
            Ok(Decision::Admit {
                class: window,
                reservation: p.class(window).reservation
            })
        );
        let mut a = Active::default();
        match window {
            TrafficClass::Build => a.build = 1,
            TrafficClass::Test => a.test = 1,
            TrafficClass::Lint => a.lint = 1,
            TrafficClass::Typecheck => a.typecheck = 1,
        }
        assert_eq!(
            decide(c, a, &p, true),
            Ok(Decision::Wait(WaitReason::ClassWindow))
        );
        assert_eq!(
            decide(c, a, &p, false),
            Ok(Decision::Wait(WaitReason::Fifo))
        );
    }
}
#[test]
fn typecheck_window_is_independent_but_cpu_and_memory_are_global() {
    let c = classification(JobClass::Typecheck, Reason::BunScriptTypecheck);
    let mut p = Policy::default();
    let a = Active {
        test: 1,
        lint: 1,
        ..Active::default()
    };
    assert!(matches!(decide(c, a, &p, true), Ok(Decision::Admit { .. })));
    let r = p.typecheck.reservation;
    let mut a = Active {
        build: 1,
        resources: Resources {
            cpu_millis: p.budget.cpu_millis - r.cpu_millis,
            memory_bytes: p.budget.memory_bytes - r.memory_bytes,
        },
        ..Active::default()
    };
    assert!(matches!(decide(c, a, &p, true), Ok(Decision::Admit { .. })));
    a.resources.cpu_millis += 1;
    assert_eq!(
        decide(c, a, &p, true),
        Ok(Decision::Wait(WaitReason::CpuBudget))
    );
    a.resources.cpu_millis -= 1;
    a.resources.memory_bytes += 1;
    assert_eq!(
        decide(c, a, &p, true),
        Ok(Decision::Wait(WaitReason::MemoryBudget))
    );
    p.typecheck.window = 0;
    assert_eq!(p.validate(), Err(PolicyError::InvalidClassPolicy));
    p.typecheck.window = 1;
    p.typecheck.reservation.cpu_millis = p.budget.cpu_millis + 1;
    assert_eq!(p.validate(), Err(PolicyError::ReservationExceedsBudget));
}

#[test]
fn typecheck_counter_and_resource_overflow_never_admit() {
    let c = classification(JobClass::Typecheck, Reason::BunScriptTypecheck);
    let p = Policy::default();
    assert_eq!(
        decide(
            c,
            Active {
                typecheck: u32::MAX,
                ..Active::default()
            },
            &p,
            true
        ),
        Ok(Decision::Wait(WaitReason::ClassWindow))
    );
    for resources in [
        Resources {
            cpu_millis: arc_traffic::MAX_RESOURCE_VALUE,
            memory_bytes: 1,
        },
        Resources {
            cpu_millis: 1,
            memory_bytes: arc_traffic::MAX_RESOURCE_VALUE,
        },
        Resources {
            cpu_millis: u64::MAX,
            memory_bytes: u64::MAX,
        },
    ] {
        assert_eq!(
            decide(
                c,
                Active {
                    resources,
                    ..Active::default()
                },
                &p,
                true
            ),
            Err(PolicyError::ResourceOverflow)
        );
    }
}
