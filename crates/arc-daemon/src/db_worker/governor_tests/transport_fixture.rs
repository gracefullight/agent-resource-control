//! Paused Governor router fixture; actual writer and one-shot lease owners.
use super::*;

// A native writer runs on another thread. Keep paused time from auto-advancing
// while it works; only the test explicitly advances the launch timer.
pub(super) struct ManualTime(
    Option<std::sync::mpsc::Sender<()>>,
    tokio::task::JoinHandle<()>,
);
impl ManualTime {
    pub(super) fn new() -> Self {
        let (release, wait) = std::sync::mpsc::channel();
        Self(
            Some(release),
            tokio::task::spawn_blocking(move || {
                let _ = wait.recv_timeout(Duration::from_secs(60));
            }),
        )
    }
    pub(super) async fn finish(mut self) {
        self.0.take().unwrap().send(()).unwrap();
        (&mut self.1).await.unwrap();
    }
}

// The actual framed Governor dispatcher and production writer, with its short
// transport clock paused and its publication/control owners frozen. The writer
// ticket has a fixture watchdog; the real one-shot lease remains unchanged.
pub(super) async fn connect(
    f: &Fixture,
) -> (
    tokio::net::UnixStream,
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    struct Dropped(Option<tokio::sync::oneshot::Sender<()>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }
    let (expired, observed) = tokio::sync::oneshot::channel();
    let expired = Arc::new(std::sync::Mutex::new(Some(expired)));
    let db = f.client.clone();
    let handler: connection::policy_dispatch::Handler = Arc::new(move |input, peer, _| {
        let (db, expired) = (db.clone(), expired.clone());
        Box::pin(async move {
            let mut value = crate::db_worker::policy::handle(
                db,
                input,
                peer,
                Instant::now() + Duration::from_secs(60),
            )
            .await?;
            if let crate::connection::policy_dispatch::Value::Governor(Value::Launch {
                fence: Some((_, guard)),
                ..
            }) = &mut value
            {
                let owned = std::mem::replace(guard, Box::new(()));
                *guard = Box::new((owned, Dropped(expired.lock().unwrap().take())));
            }
            Ok(value)
        })
    });
    let (server, client) = tokio::net::UnixStream::pair().unwrap();
    let task = tokio::spawn(async move {
        let limits = RuntimeLimits::default();
        let metrics = Arc::new(RuntimeMetrics::new(&limits));
        loop {
            let end = tokio::time::Instant::now() + Duration::from_secs(60);
            let size = match crate::transport::read_header(&server, end).await {
                Ok(size) => size,
                Err(arc_ipc::FrameError::Closed) => break,
                Err(e) => panic!("controlled router read: {e}"),
            };
            let bytes = crate::transport::read_body(&server, size, end)
                .await
                .unwrap();
            let request = Request::decode(&bytes).unwrap();
            assert!(
                crate::connection::governor_dispatch::dispatch_with_clock(
                    &server,
                    &request,
                    &limits,
                    Some(&handler),
                    &metrics,
                    || tokio::time::Instant::now().into_std()
                )
                .await
            );
        }
    });
    (client, task, observed)
}
