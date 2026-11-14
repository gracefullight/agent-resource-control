//! Deterministic CLI transport/native-child regression tests. Real-tool and
//! production-daemon acceptance are owned by the downstream native task.
#![cfg(unix)]
#[allow(dead_code)]
#[path = "../../arc-ipc/tests/fixtures/memory_pressure.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "governor/support.rs"]
mod support;
#[path = "traffic_scripts/mod.rs"]
mod traffic_scripts;
