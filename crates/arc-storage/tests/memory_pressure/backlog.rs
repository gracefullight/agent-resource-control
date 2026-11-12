use super::{admission::*, observation::*};
use arc_core::meter::MemoryPressure;
use arc_policy::*;
use arc_storage::jobs::traffic::*;
use arc_traffic::Policy;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

struct Observed {
    provider: Provider,
    calls: AtomicUsize,
}
impl ObservationProvider for Observed {
    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.provider.with_observation(consume);
    }
}
#[test]
fn critical_published_during_writer_backlog_replaces_prebacklog_normal() {
    let dir = std::env::temp_dir().join(format!(
        "pressure-backlog-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let path = dir.join("db.sqlite3");
    let mut lock = arc_storage::open_and_migrate(&path).unwrap();
    let mut writer = arc_storage::open_and_migrate(&path).unwrap();
    writer.busy_timeout(Duration::from_secs(5)).unwrap();
    let source = Arc::new(Observed {
        provider: Provider::new(MemoryPressure::Normal),
        calls: AtomicUsize::new(0),
    });
    let tx = lock
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let (entered, started) = mpsc::sync_channel(1);
    let owned = Arc::clone(&source);
    let handle = std::thread::spawn(move || {
        entered.send(()).unwrap();
        let reply = enqueue_observed(
            &mut writer,
            &request(1, "CARGO_BUILD"),
            &OwnerEvidence::gather(&owner()),
            &Policy::default(),
            &context(1),
            None,
            1000,
            owned.as_ref(),
        )
        .unwrap();
        (writer, reply)
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        source.calls.load(Ordering::SeqCst),
        0,
        "cache cannot be read before IMMEDIATE is acquired"
    );
    source.provider.set(MemoryPressure::Critical);
    tx.commit().unwrap();
    let (writer, reply) = handle.join().unwrap();
    assert!(matches!(reply, arc_ipc::traffic::JobResult::Queued { .. }));
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts(&writer), (0, 0));
    drop(writer);
    drop(lock);
    let mut reopened = arc_storage::open_and_migrate(&path).unwrap();
    assert!(matches!(
        poll_job(&mut reopened, 1, &source.provider),
        arc_ipc::traffic::JobResult::Queued { .. }
    ));
    source.provider.set(MemoryPressure::Normal);
    assert!(matches!(
        poll_job(&mut reopened, 1, &source.provider),
        arc_ipc::traffic::JobResult::Granted { .. }
    ));
    assert_eq!(counts(&reopened), (1, 1));
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}
