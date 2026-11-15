//! Shared production-daemon scenarios. Desktop supplies the actual registered Router callback.
//! Family modules alone own tool readiness, offline projects and private caches.
pub mod cmake;
mod descriptors;
mod fixtures;
pub mod go;
mod scenarios;
#[path = "../traffic_bun_scripts/mod.rs"]
pub mod shared;
pub mod swift;
mod witnesses;

pub use descriptors::{Form, Pair, NATIVE_FORMS, ORDERED_PAIRS};
pub use fixtures::{observer, writable_cache};
pub use scenarios::{cargo_peer, pair};
use serde_json::{json, Value};
pub use shared::{
    binary, eventually, exchange, executable, log, root, sha, write, Case, Read, Run, RunnerTools,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
pub use witnesses::{
    entered, marker_ready, permit_history, queued, terminal, QueueWitness, ReadyWitness,
    ReleaseWitness,
};

/// A prepared real tool invocation, not a shell string or command language.
/// ready must witness an explicit native barrier; it cannot rely on elapsed time.
pub struct Prepared {
    pub form: Form,
    pub project: PathBuf,
    pub executable: PathBuf,
    pub argv: Vec<String>,
    pub ready: fn(&mut Run) -> bool,
    pub configure: fn(&Path, &mut Command),
}
impl Prepared {
    pub fn start<T: RunnerTools>(&self, case: &Case<T>, predecessor: Option<&str>) -> Run {
        let argv: Vec<_> = self.argv.iter().map(String::as_str).collect();
        Run::start_with(case, &self.project, &argv, |cmd| {
            (self.configure)(&self.project, cmd);
            if let Some(id) = predecessor {
                cmd.env("TBS_PREDECESSOR", id);
            }
        })
    }
}

/// Native diagnostics have their own negotiated vocabulary. Meter stays unchanged.
pub fn direct_read(socket: &Path, command: &str, args: Value) -> Value {
    let operation = match command {
        "traffic_job_diagnostics" => {
            assert_eq!(
                exchange(
                    socket,
                    arc_ipc::traffic::native::OPERATION_DIAGNOSTICS_CAPABILITIES,
                    Value::Null
                ),
                json!({"capability":arc_ipc::traffic::native::DIAGNOSTICS_CAPABILITY})
            );
            arc_ipc::traffic::native::OPERATION_DIAGNOSTICS_JOB
        }
        "meter_summary" => "meter.summary",
        _ => panic!("unsupported native-tools fixture read: {command}"),
    };
    exchange(socket, operation, args)
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    #[test]
    fn shared_registration_is_exact_and_family_signatures_compile() {
        let semantics: [fn(Read); 3] = [go::semantics, swift::semantics, cmake::semantics];
        let pairs: [fn(Read); 3] = [go::pairs, swift::pairs, cmake::pairs];
        assert_eq!(semantics.len(), pairs.len());
        assert_eq!(NATIVE_FORMS.len(), 7);
        let mut ids = std::collections::BTreeSet::new();
        for pair in ORDERED_PAIRS {
            assert!(ids.insert(pair.id));
            assert_eq!(pair.first.class(), pair.second.class());
            assert_ne!(pair.first.is_native(), pair.second.is_native());
            assert_eq!(
                pair.id,
                format!("{}_then_{}", pair.first.id(), pair.second.id())
            );
        }
        assert_eq!(ids.len(), 14);
        for form in NATIVE_FORMS {
            assert_eq!(
                ORDERED_PAIRS
                    .iter()
                    .filter(|p| p.first == form || p.second == form)
                    .count(),
                2
            );
        }
    }
}
