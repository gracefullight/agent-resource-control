use arc_core::{meter::*, process_identity::ProcessIdentity};
use arc_platform::{CpuCounters, NativeFilesystem, NativeSample};

pub fn identity(token: &str) -> ProcessIdentity {
    ProcessIdentity {
        pid: 123,
        start_token: token.into(),
        boot_id: "00000000-0000-0000-0000-000000000001".into(),
    }
}
pub fn sample(monotonic: u64) -> NativeSample {
    NativeSample {
        observed_at_ms: Some(10_000 + monotonic),
        monotonic_ms: monotonic,
        host_cpu: Reading::Available {
            value: CpuCounters {
                user: monotonic / 20,
                system: monotonic / 20,
                nice: 0,
                idle: monotonic / 10,
                counter_bits: 32,
                logical_cpu_count: 2,
            },
        },
        memory_available_bytes: Reading::Available { value: 1024 },
        memory_pressure: Reading::Available {
            value: MemoryPressure::Normal,
        },
        swap_used_bytes: Reading::Available { value: 0 },
        filesystem: NativeFilesystem {
            identity: Some(FilesystemIdentity {
                path: "/".into(),
                mount_point: "/".into(),
                filesystem_type: "apfs".into(),
                volume_id: "1:2".into(),
                is_local: true,
            }),
            capacity_bytes: Reading::Available { value: 1000 },
            free_bytes: Reading::Available { value: 200 },
            available_bytes: Reading::Available { value: 150 },
        },
        daemon_cpu_core_seconds: Reading::Available {
            value: monotonic as f64 / 100_000.0,
        },
        daemon_rss_bytes: Reading::Available { value: 4096 },
    }
}
