use super::*;
use crate::{
    db_worker::{DbConfig, DbWorker},
    limits::RuntimeLimits,
    runtime_metrics::RuntimeMetrics,
};
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};
#[allow(dead_code)]
#[path = "../../../arc-meter/src/fixtures.rs"]
mod native_fixture;
struct InjectedSource {
    level: Arc<AtomicU8>,
    epoch: Instant,
}
impl arc_meter::NativeSource for InjectedSource {
    fn sample(&mut self) -> arc_platform::NativeSample {
        let mut sample = native_fixture::sample(self.epoch.elapsed().as_millis() as u64);
        sample.memory_pressure = match self.level.load(Ordering::SeqCst) {
            0 => arc_core::meter::Reading::Available {
                value: arc_core::meter::MemoryPressure::Normal,
            },
            1 => arc_core::meter::Reading::Available {
                value: arc_core::meter::MemoryPressure::Critical,
            },
            _ => arc_core::meter::Reading::Failed {
                diagnostic: arc_core::meter::Diagnostic::bounded("explicit injected failure"),
            },
        };
        sample
    }
}
pub(crate) struct Fixture {
    pub worker: DbWorker,
    pub meter: arc_meter::MeterWorker,
    pub client: DbClient,
    pub owner: arc_core::process_identity::ProcessIdentity,
    level: Arc<AtomicU8>,
    root: std::path::PathBuf,
}
impl Fixture {
    pub async fn new(hooks: Option<crate::db_worker::WorkerTestHooks>) -> Self {
        let owner =
            arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
        let root = arc_core::runtime_root().unwrap().join(format!(
            "pressure-policy-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let connection = arc_storage::open_and_migrate(root.join("db.sqlite3")).unwrap();
        let limits = RuntimeLimits::default();
        let metrics = Arc::new(RuntimeMetrics::new(&limits));
        let config = DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        };
        let worker = match hooks {
            Some(hooks) => {
                crate::db_worker::start_with_test_hooks(connection, config, metrics, hooks)
            }
            None => DbWorker::start(connection, config, metrics),
        }
        .unwrap();
        let level = Arc::new(AtomicU8::new(0));
        let epoch = Instant::now();
        let meter = arc_meter::MeterWorker::start(
            InjectedSource {
                level: Arc::clone(&level),
                epoch,
            },
            owner.clone(),
            epoch,
            arc_meter::SamplingPolicy::new(10_000, 1000, 1000).unwrap(),
        )
        .unwrap();
        meter.activity().set_active(true);
        let client = worker.client();
        client.bind_observation(meter.reader());
        let fixture = Self {
            worker,
            meter,
            client,
            owner,
            level,
            root,
        };
        fixture.wait_level(0).await;
        fixture
    }
    pub fn path(&self) -> std::path::PathBuf {
        self.root.join("db.sqlite3")
    }
    pub fn sampler(&self, level: u8) -> arc_meter::MeterWorker {
        let epoch = Instant::now();
        let worker = arc_meter::MeterWorker::start(
            InjectedSource {
                level: Arc::new(AtomicU8::new(level)),
                epoch,
            },
            self.owner.clone(),
            epoch,
            arc_meter::SamplingPolicy::new(10_000, 1000, 1000).unwrap(),
        )
        .unwrap();
        worker.activity().set_active(true);
        worker
    }
    async fn wait_level(&self, level: u8) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let matches = self.meter.reader().snapshot().is_some_and(|s| {
                match &s.host.memory_pressure.reading {
                    arc_core::meter::Reading::Available { value } => {
                        level < 2
                            && *value
                                == if level == 0 {
                                    arc_core::meter::MemoryPressure::Normal
                                } else {
                                    arc_core::meter::MemoryPressure::Critical
                                }
                    }
                    arc_core::meter::Reading::Failed { .. } => level == 2,
                    _ => false,
                }
            });
            if matches {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "injected observation publication deadline"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
    pub async fn level(&self, level: u8) {
        self.level.store(level, Ordering::SeqCst);
        self.wait_level(level).await;
    }
    pub fn id(&self, n: u32) -> JobId {
        JobId::try_from(format!("{n:032x}")).unwrap()
    }
    pub async fn enqueue(&self, n: u32) -> arc_ipc::traffic::JobResult {
        let classification=serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap();
        self.traffic(arc_ipc::traffic::JobRequest::Enqueue {
            job_id: self.id(n),
            observer: self.owner.clone(),
            classification,
            lineage_context: None,
        })
        .await
    }
    pub async fn poll(&self, n: u32) -> arc_ipc::traffic::JobResult {
        self.traffic(arc_ipc::traffic::JobRequest::Poll {
            job_id: self.id(n),
            observer: self.owner.clone(),
        })
        .await
    }
    pub async fn traffic(
        &self,
        request: arc_ipc::traffic::JobRequest,
    ) -> arc_ipc::traffic::JobResult {
        serde_json::from_value(
            traffic::handle(
                self.client.clone(),
                request,
                self.owner.pid,
                Instant::now() + Duration::from_secs(5),
            )
            .await
            .unwrap(),
        )
        .unwrap()
    }
    pub async fn policy(&self, input: Input) -> Value {
        handle(
            self.client.clone(),
            input,
            self.owner.pid,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap()
    }
    pub async fn shutdown(mut self) {
        self.worker
            .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
        self.meter
            .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
