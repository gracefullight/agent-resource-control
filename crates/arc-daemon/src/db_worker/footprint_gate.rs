//! Opt-in fixture barriers for the real-daemon footprint race regression.
//! The ordinary daemon has no gate; only a daemon explicitly given the
//! fixture directory and a job-specific arm file performs this handshake.
use arc_ipc::jobs::JobId;
use arc_storage::footprint::{Point, Reading};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn root() -> Option<PathBuf> {
    std::env::var_os("ARC_FOOTPRINT_FIXTURE_GATE").map(PathBuf::from)
}

fn name(job: &JobId, suffix: &str) -> String {
    format!("{}.{}", job.as_str(), suffix)
}

pub(super) fn waiting(job: &JobId) {
    if let Some(root) = root() {
        if root.join(name(job, "active")).is_file() {
            let _ = std::fs::write(root.join(name(job, "waiting")), b"");
        }
    }
}

pub(super) async fn checkpoint(job: &JobId, point: Point, deadline: Instant) -> bool {
    let phase = match point {
        Point::Start => "start",
        Point::End => "end",
    };
    checkpoint_phase(job, phase, deadline).await
}

/// Pause only explicitly armed test jobs after their directory is captured.
pub(super) async fn after_capture(job: &JobId, point: Point, deadline: Instant) -> bool {
    let phase = match point {
        Point::Start => "start.capture",
        Point::End => "end.capture",
    };
    checkpoint_phase(job, phase, deadline).await
}

/// Publish only a specifically armed fixture walk's result for its test.
pub(super) fn measured(job: &JobId, point: Point, reading: Reading) {
    let Some(root) = root() else { return };
    let phase = match point {
        Point::Start => "start.capture",
        Point::End => "end.capture",
    };
    if !root.join(name(job, &format!("{phase}.arm"))).is_file() {
        return;
    }
    let value = match reading {
        Reading::Complete(bytes) => format!("complete:{bytes}"),
        Reading::Partial(bytes) => format!("partial:{bytes}"),
        Reading::Unavailable => "unavailable".to_owned(),
    };
    let temporary = root.join(name(job, &format!("{phase}.reading.tmp")));
    let final_path = root.join(name(job, &format!("{phase}.reading")));
    if std::fs::write(&temporary, value).is_ok() {
        let _ = std::fs::rename(temporary, final_path);
    }
}

async fn checkpoint_phase(job: &JobId, phase: &str, deadline: Instant) -> bool {
    let Some(root) = root() else { return true };
    if !root.join(name(job, &format!("{phase}.arm"))).is_file() {
        return true;
    }
    let active = root.join(name(job, "active"));
    let _ = std::fs::write(&active, phase);
    let _ = std::fs::write(root.join(name(job, &format!("{phase}.entered"))), b"");
    let release = root.join(name(job, &format!("{phase}.release")));
    loop {
        if release.is_file() {
            let _ = std::fs::write(root.join(name(job, &format!("{phase}.released"))), b"");
            let _ = std::fs::remove_file(active);
            return true;
        }
        if Instant::now() >= deadline {
            let _ = std::fs::remove_file(active);
            return false;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}
