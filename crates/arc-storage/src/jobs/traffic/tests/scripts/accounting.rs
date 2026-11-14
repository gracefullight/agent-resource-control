use super::*;

#[test]
fn all_class_counts_and_resource_sums_reject_overflow_without_partial_updates() {
    use arc_traffic::{Active, Resources, MAX_RESOURCE_VALUE};
    for class in ["BUILD", "TEST", "LINT", "TYPECHECK"] {
        let mut active = Active {
            build: u32::MAX,
            test: u32::MAX,
            lint: u32::MAX,
            typecheck: u32::MAX,
            resources: Resources::default(),
        };
        let before = active;
        assert_eq!(
            store::accumulate(&mut active, class, 1, 1),
            Err(JobError::StorageFailure)
        );
        assert_eq!(active, before);
    }
    for (cpu, memory) in [(MAX_RESOURCE_VALUE, 1), (1, MAX_RESOURCE_VALUE)] {
        let mut active = Active {
            resources: Resources {
                cpu_millis: cpu,
                memory_bytes: memory,
            },
            ..Active::default()
        };
        let before = active;
        assert_eq!(
            store::accumulate(&mut active, "TYPECHECK", 1, 1),
            Err(JobError::StorageFailure)
        );
        assert_eq!(active, before);
    }
    for (class, cpu, memory) in [
        ("UNRECOGNIZED", 1, 1),
        ("TYPECHECK", 0, 1),
        ("TYPECHECK", 1, 0),
    ] {
        assert_eq!(
            store::accumulate(&mut Active::default(), class, cpu, memory),
            Err(JobError::StorageFailure)
        );
    }
}
