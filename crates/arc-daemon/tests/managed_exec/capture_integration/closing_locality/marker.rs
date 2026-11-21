use crate::{cli, protocol, runtime::Fixture};
use arc_ipc::jobs::{StatusRequest, OPERATION_JOB_STATUS_LOCALITY};
use serde_json::json;
use std::{fs, path::Path};

pub(super) fn check(f: &Fixture, _name: &str, _args: &[&str], marker: &Path) {
    let recorded = protocol::call(
        f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById {
            job_id: cli::recent(f)
        }),
    );
    let pid = recorded.result.unwrap()["job"]["child"]["pid"]
        .as_u64()
        .unwrap()
        .to_string();
    let markers = fs::read_to_string(marker).unwrap();
    let lines: Vec<_> = markers.lines().collect();
    // Endpoint capture never invokes the user's tool as a probe.
    assert_eq!(lines.len(), 1);
    assert_eq!(lines.last().copied(), Some(pid.as_str()));
    assert_eq!(lines.iter().filter(|line| **line == pid).count(), 1);
}
