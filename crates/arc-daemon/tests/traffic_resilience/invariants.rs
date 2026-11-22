//! The permit invariants of NFR-009, read from SQLite.
//!
//! `observe` holds at every instant and is called after every step and while
//! a step waits. `owners_alive` holds once the daemon has reconciled, and
//! `finished` holds when every job of a run has ended.
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_traffic::{Policy, TrafficClass};
use std::collections::{BTreeMap, BTreeSet};

/// One grant of a job's permit as last read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Grant {
    state: String,
    generation: i64,
    released_at: Option<i64>,
}

pub struct Monitor {
    policy: Policy,
    grants: BTreeMap<String, Grant>,
    /// Most permits ACTIVE in one read, and how many reads were made.
    pub high_water: usize,
    pub reads: usize,
}

pub fn class(job_class: &str) -> TrafficClass {
    match job_class {
        "BUILD" => TrafficClass::Build,
        "TEST" => TrafficClass::Test,
        "LINT" => TrafficClass::Lint,
        "TYPECHECK" => TrafficClass::Typecheck,
        other => panic!("a permit of class {other}"),
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            policy: Policy::default(),
            grants: BTreeMap::new(),
            high_water: 0,
            reads: 0,
        }
    }

    /// Whether the daemon's policy admits one more job of `class` beside the
    /// permits ACTIVE now.
    pub fn admits(&self, db: &rusqlite::Connection, wanted: TrafficClass) -> bool {
        let (mut same, mut cpu, mut memory) = (0, 0, 0);
        for (job_class, cpu_millis, memory_bytes) in active(db) {
            same += u32::from(class(&job_class) == wanted);
            cpu += cpu_millis;
            memory += memory_bytes;
        }
        let policy = self.policy.class(wanted);
        same < policy.window
            && cpu + policy.reservation.cpu_millis <= self.policy.budget.cpu_millis
            && memory + policy.reservation.memory_bytes <= self.policy.budget.memory_bytes
    }

    /// One consistent read of every permit. Active permits never exceed a
    /// class window or the budget; no job holds two permits; a permit is
    /// ACTIVE without a release time or RELEASED with one; a released grant
    /// never changes again and no permit disappears.
    #[track_caller]
    pub fn observe(&mut self, db: &rusqlite::Connection, at: &str) {
        type Row = (String, String, i64, Option<i64>, String, i64, i64);
        let rows: Vec<Row> = db
            .prepare(
                "SELECT p.job_id,p.state,p.generation,p.released_at,j.job_class,\
                 p.cpu_millis_est,p.memory_bytes_est FROM permits p JOIN jobs j ON j.id=p.job_id",
            )
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        self.reads += 1;
        let mut holders = BTreeSet::new();
        let mut windows: BTreeMap<String, u32> = BTreeMap::new();
        let (mut cpu, mut memory, mut live) = (0, 0, 0);
        for (job, state, generation, released_at, job_class, cpu_millis, memory_bytes) in rows {
            assert!(
                holders.insert(job.clone()),
                "{at}: job {job} holds two permits"
            );
            match (state.as_str(), released_at) {
                ("ACTIVE", None) => {
                    *windows.entry(job_class).or_default() += 1;
                    cpu += cpu_millis as u64;
                    memory += memory_bytes as u64;
                    live += 1;
                }
                ("RELEASED", Some(_)) => (),
                other => panic!("{at}: permit of {job} is {other:?}"),
            }
            let now = Grant {
                state,
                generation,
                released_at,
            };
            if let Some(prior) = self.grants.get(&job) {
                if prior.generation == now.generation {
                    assert!(
                        prior.state == "ACTIVE" || *prior == now,
                        "{at}: the released permit of {job} changed: {prior:?} to {now:?}"
                    );
                } else {
                    assert!(
                        now.generation > prior.generation,
                        "{at}: the permit of {job} went back: {prior:?} to {now:?}"
                    );
                }
            }
            self.grants.insert(job, now);
        }
        for job in self.grants.keys() {
            assert!(holders.contains(job), "{at}: the permit of {job} is gone");
        }
        for (job_class, count) in &windows {
            let window = self.policy.class(class(job_class)).window;
            assert!(
                *count <= window,
                "{at}: {count} ACTIVE {job_class} permits exceed the window of {window}"
            );
        }
        assert!(
            cpu <= self.policy.budget.cpu_millis && memory <= self.policy.budget.memory_bytes,
            "{at}: ACTIVE permits reserve {cpu} millicores and {memory} bytes, over the budget"
        );
        self.high_water = self.high_water.max(live);
    }

    /// After reconciliation every ACTIVE permit belongs to a job whose
    /// wrapper or whose recorded child is a live process.
    #[track_caller]
    pub fn owners_alive(&self, db: &rusqlite::Connection, at: &str) {
        type Row = (
            String,
            u32,
            String,
            String,
            Option<u32>,
            Option<String>,
            Option<String>,
        );
        let rows: Vec<Row> = db
            .prepare(
                "SELECT p.job_id,l.observer_pid,l.observer_start_token,l.observer_boot_id,\
                 l.child_pid,l.child_start_token,l.child_boot_id FROM permits p \
                 JOIN job_lifecycle l ON l.job_id=p.job_id WHERE p.state='ACTIVE'",
            )
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for (job, pid, start_token, boot_id, child_pid, child_start, child_boot) in rows {
            let observer = ProcessIdentity {
                pid,
                start_token,
                boot_id,
            };
            let child = match (child_pid, child_start, child_boot) {
                (Some(pid), Some(start_token), Some(boot_id)) => Some(ProcessIdentity {
                    pid,
                    start_token,
                    boot_id,
                }),
                _ => None,
            };
            let alive =
                |identity: &ProcessIdentity| probe_live(identity) == IdentityStatus::VerifiedMatch;
            assert!(
                alive(&observer) || child.as_ref().is_some_and(alive),
                "{at}: the ACTIVE permit of {job} has no live owner: wrapper {observer:?}, child {child:?}"
            );
        }
    }

    /// The end of a run: nothing is ACTIVE, nothing is still queued or
    /// running, and every permit ever read was released. Returns how many.
    #[track_caller]
    pub fn finished(&mut self, db: &rusqlite::Connection, at: &str) -> usize {
        self.observe(db, at);
        let (active, unfinished): (i64, i64) = db
            .query_row(
                "SELECT (SELECT count(*) FROM permits WHERE state='ACTIVE'),(SELECT count(*) \
                 FROM traffic_entries WHERE state IN ('QUEUED','RESERVED','RUNNING','RETAINED'))",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((active, unfinished), (0, 0), "{at}: the final active count");
        for (job, grant) in &self.grants {
            assert!(
                grant.state == "RELEASED" && grant.released_at.is_some(),
                "{at}: the permit of {job} was not released: {grant:?}"
            );
        }
        self.grants.len()
    }
}

/// (class, millicores, bytes) of each ACTIVE permit.
fn active(db: &rusqlite::Connection) -> Vec<(String, u64, u64)> {
    let rows: Vec<(String, i64, i64)> = db
        .prepare(
            "SELECT j.job_class,p.cpu_millis_est,p.memory_bytes_est FROM permits p \
             JOIN jobs j ON j.id=p.job_id WHERE p.state='ACTIVE'",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    rows.into_iter()
        .map(|(class, cpu, memory)| (class, cpu as u64, memory as u64))
        .collect()
}
