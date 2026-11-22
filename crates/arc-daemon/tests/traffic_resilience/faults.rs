//! NFR-009 item 2: seeded interleavings of the faults the product handles.
//!
//! Every seed runs each of the six fault kinds at least once, in an order and
//! with victims, classes and extra steps drawn from a small generator kept
//! here (no crate is added). Jobs are built `arcctl exec` wrappers with
//! offline stub children against a real daemon. The driver never sleeps for
//! correctness: it waits for a state it can read (a queue entry, a child at
//! its barrier, a wrapper's exit) before the next step.
use crate::{
    fault::Fault,
    fixture::{Job, World},
    invariants::Monitor,
    journey::eventually,
    protocol, support,
};
use arc_traffic::TrafficClass;
use std::cell::RefCell;

#[path = "fault_steps.rs"]
mod steps;

/// The fixed seeds. A failure prints the seed and the steps taken so far.
/// Every seed applies all six fault kinds; the list was chosen so that it
/// also reaches, between its seeds, a queued wrapper failing open during the
/// outage and a permit released by the daemon's start alone.
const SEEDS: [u64; 4] = [1, 3, 5, 7];
/// Steps drawn per seed beyond the six fault kinds.
const EXTRA_STEPS: usize = 4;
/// Wrappers alive at once; a further start becomes a release.
const MAX_LIVE: usize = 4;

/// SplitMix64: enough for a reproducible interleaving.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
    fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[self.below(of.len())]
    }
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for last in (1..items.len()).rev() {
            items.swap(last, self.below(last + 1));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    RetriedRequest,
    LostReply,
    WrapperKilledQueued,
    WrapperKilledRunning,
    FailedSpawn,
    DaemonKillRestart,
}
const KINDS: [Kind; 6] = [
    Kind::RetriedRequest,
    Kind::LostReply,
    Kind::WrapperKilledQueued,
    Kind::WrapperKilledRunning,
    Kind::FailedSpawn,
    Kind::DaemonKillRestart,
];

#[derive(Debug, Clone, Copy)]
enum Step {
    Fault(Kind),
    Start,
    Release,
}

const CLASSES: [(&str, TrafficClass); 3] = [
    ("build", TrafficClass::Build),
    ("test", TrafficClass::Test),
    ("clippy", TrafficClass::Lint),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Enqueued, the wrapper is polling.
    Queued,
    /// The child is at its barrier and recorded: the permit is ACTIVE.
    Running,
    /// The wrapper was killed; its recorded child still holds the permit.
    Orphan,
    Done,
}

struct Tracked {
    job: Job,
    class: TrafficClass,
    spawnable: bool,
    phase: Phase,
    /// How often the stub must have started when the run ends.
    starts: usize,
}

/// What one seed's run really did. Each fault kind must show here: a step
/// that was scheduled but met no job to act on would not count.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    ran_with_permit: usize,
    failed_open: usize,
    failed_spawn: usize,
    killed_queued: usize,
    orphaned: usize,
    kept_across_daemon_kill: usize,
    released_at_daemon_start: usize,
    protocol_granted: usize,
    protocol_cancelled: usize,
    proxied: usize,
}

struct Run {
    w: World,
    rng: Rng,
    jobs: Vec<Tracked>,
    monitor: Monitor,
    log: Log,
    protocol_jobs: u128,
    tally: Tally,
}

/// The seed and the steps taken, printed when the run fails.
struct Log {
    seed: u64,
    steps: RefCell<Vec<String>>,
}
impl Drop for Log {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "NFR_009_FAILED seed {:#x} after steps {:?}",
                self.seed,
                self.steps.borrow()
            );
        }
    }
}

impl Run {
    fn live(&self, phase: Phase) -> Vec<usize> {
        (0..self.jobs.len())
            .filter(|index| self.jobs[*index].phase == phase)
            .collect()
    }

    /// Every invariant that holds at any instant, plus the stub start bound.
    fn observe(&mut self, at: &str) {
        let db = self.w.db();
        self.monitor.observe(&db, at);
        for (index, tracked) in self.jobs.iter().enumerate() {
            assert!(
                tracked.job.starts() <= 1,
                "{at}: the stub of job {index} started {} times",
                tracked.job.starts()
            );
        }
    }

    /// The invariants after a step: the daemon has reconciled what the step
    /// changed, so no ACTIVE permit may be left without a live owner.
    fn check(&mut self, at: &str) {
        self.observe(at);
        let db = self.w.db();
        self.monitor.owners_alive(&db, at);
    }

    fn start(&mut self, spawnable: bool) -> usize {
        let (subcommand, class) = self.rng.pick(&CLASSES);
        self.start_class(subcommand, class, spawnable)
    }

    fn start_class(&mut self, subcommand: &str, class: TrafficClass, spawnable: bool) -> usize {
        let job = if spawnable {
            self.w.start(subcommand)
        } else {
            self.w.start_unspawnable(subcommand)
        };
        self.jobs.push(Tracked {
            job,
            class,
            spawnable,
            phase: Phase::Queued,
            starts: 0,
        });
        let index = self.jobs.len() - 1;
        let db = self.w.db();
        let tracked = &mut self.jobs[index];
        eventually("the new job is enqueued", || {
            tracked.job.state(&db).is_some()
        });
        self.settle();
        index
    }

    /// Wait until no queued job can be admitted. The queue is first in,
    /// first out: only its head can be granted, and only into a free class
    /// window within the budget. While that holds for the head, wait for
    /// what the grant must lead to: the child at its barrier, or the failed
    /// spawn of a wrapper without its tool.
    fn settle(&mut self) {
        loop {
            self.observe("settling");
            let db = self.w.db();
            let mut head: Option<(i64, usize, String)> = None;
            for index in self.live(Phase::Queued) {
                let id = self.jobs[index].job.id(&db).unwrap();
                let (sequence, state): (i64, String) = db
                    .query_row(
                        "SELECT sequence,state FROM traffic_entries WHERE job_id=?1",
                        [&id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap();
                if head.as_ref().is_none_or(|(first, _, _)| sequence < *first) {
                    head = Some((sequence, index, state));
                }
            }
            let Some((_, index, state)) = head else {
                return;
            };
            if state == "QUEUED" && !self.monitor.admits(&db, self.jobs[index].class) {
                return;
            }
            let tracked = &mut self.jobs[index];
            if tracked.spawnable {
                tracked.job.running(&db);
                tracked.phase = Phase::Running;
                tracked.starts = 1;
                self.tally.ran_with_permit += 1;
            } else {
                let status = tracked
                    .job
                    .wrapper
                    .wait("wrapper without its tool")
                    .unwrap();
                let stderr = tracked.job.diagnostics();
                assert!(!status.success(), "{status:?}: {stderr}");
                assert!(stderr.contains("SPAWN_FAILED\n"), "{stderr}");
                tracked.phase = Phase::Done;
                let id = tracked.job.id(&db).unwrap();
                let (entry, permit): (String, String) = db
                    .query_row(
                        "SELECT t.state,p.state FROM traffic_entries t JOIN permits p \
                         ON p.job_id=t.job_id WHERE t.job_id=?1",
                        [&id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(
                    (entry.as_str(), permit.as_str()),
                    ("COMPLETED", "RELEASED"),
                    "a failed spawn keeps no permit"
                );
                self.tally.failed_spawn += 1;
            }
        }
    }

    /// The entry and permit state of one job, after `arcctl job status` made
    /// the daemon reconcile it.
    fn reconciled(&mut self, index: usize) -> (String, Option<String>) {
        let db = self.w.db();
        let id = self.jobs[index].job.id(&db).unwrap();
        self.w.status(&id);
        db.query_row(
            "SELECT t.state,(SELECT state FROM permits WHERE job_id=t.job_id) \
             FROM traffic_entries t WHERE t.job_id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    /// A running job's child exits 0 and its wrapper reports that.
    fn release(&mut self, index: usize) {
        let tracked = &mut self.jobs[index];
        tracked.job.release();
        let status = tracked.job.wrapper.wait("released wrapper").unwrap();
        assert!(
            status.success(),
            "{status:?}: {}",
            tracked.job.diagnostics()
        );
        tracked.phase = Phase::Done;
        assert_eq!(
            self.reconciled(index),
            ("COMPLETED".to_owned(), Some("RELEASED".to_owned()))
        );
    }

    /// An orphaned child exits. Nobody reports its exit: the daemon finds it
    /// gone and releases the permit.
    fn release_orphan(&mut self, index: usize) {
        let tracked = &mut self.jobs[index];
        tracked.job.release();
        tracked.job.child_gone();
        tracked.phase = Phase::Done;
        assert_eq!(
            self.reconciled(index),
            ("ABANDONED".to_owned(), Some("RELEASED".to_owned()))
        );
    }

    /// End every job: each remaining child runs once and exits, in whatever
    /// order the queue allows.
    fn drain(&mut self) {
        loop {
            self.settle();
            if let Some(index) = self.live(Phase::Running).first() {
                self.release(*index);
            } else if let Some(index) = self.live(Phase::Orphan).first() {
                self.release_orphan(*index);
            } else {
                assert!(
                    self.live(Phase::Queued).is_empty(),
                    "a queue nobody can leave"
                );
                return;
            }
            self.check("draining");
        }
    }
}

fn run_seed(seed: u64) -> Tally {
    let mut rng = Rng(seed);
    let mut steps: Vec<Step> = KINDS.iter().map(|kind| Step::Fault(*kind)).collect();
    for _ in 0..EXTRA_STEPS {
        steps.push(match rng.below(4) {
            0 | 1 => Step::Start,
            2 => Step::Release,
            _ => Step::Fault(rng.pick(&KINDS)),
        });
    }
    rng.shuffle(&mut steps);
    let w = World::new();
    w.s.stub("tool");
    let mut run = Run {
        w,
        rng,
        jobs: Vec::new(),
        monitor: Monitor::new(),
        log: Log {
            seed,
            steps: RefCell::new(Vec::new()),
        },
        protocol_jobs: 0,
        tally: Tally::default(),
    };
    for (number, step) in steps.iter().enumerate() {
        run.log.steps.borrow_mut().push(format!("{step:?}"));
        let began = std::time::Instant::now();
        run.apply(*step);
        run.check(&format!("seed {seed:#x} step {number} {step:?}"));
        // Wall time is printed for the record, never asserted.
        if let Some(entry) = run.log.steps.borrow_mut().last_mut() {
            entry.push_str(&format!(" {}ms", began.elapsed().as_millis()));
        }
    }
    run.drain();

    // The end: every entry is reconciled by a status read, every permit was
    // released once, nothing is ACTIVE and every stub ran as often as its
    // job was started: once, or never for a job that could not start.
    for index in 0..run.jobs.len() {
        run.reconciled(index);
    }
    run.check(&format!("seed {seed:#x} end"));
    let db = run.w.db();
    let permits = run.monitor.finished(&db, &format!("seed {seed:#x} end"));
    for (index, tracked) in run.jobs.iter().enumerate() {
        assert_eq!(
            tracked.job.starts(),
            tracked.starts,
            "seed {seed:#x}: stub starts of job {index}"
        );
    }
    println!(
        "NFR_009_SEED {seed:#x} steps {:?} {:?} wrapper_jobs {} permits_released {permits} \
         active_high_water {} sqlite_reads {}",
        run.log.steps.borrow(),
        run.tally,
        run.jobs.len(),
        run.monitor.high_water,
        run.monitor.reads
    );
    // Every permit belongs to a job that was granted one, and to no other.
    assert_eq!(
        permits,
        run.tally.ran_with_permit + run.tally.failed_spawn + run.tally.protocol_granted,
        "seed {seed:#x}: permits against grants"
    );
    drop(db);
    let Run { w, jobs, tally, .. } = run;
    drop(jobs);
    w.finish();
    tally
}

#[test]
fn nfr_009_seeded_fault_interleavings_never_duplicate_or_lose_a_permit() {
    let _serial = support::serial();
    let mut all = Tally::default();
    for seed in SEEDS {
        println!("NFR_009 seed {seed:#x}");
        let tally = run_seed(seed);
        // Each of the six fault kinds acted on a job in this seed: two jobs
        // through the proxy and two over the socket for the retried request
        // and the lost reply, a wrapper killed in each state, a spawn that
        // failed, and a daemon kill while a job held a permit.
        let acted = [
            (
                "retried request and lost reply, proxied",
                tally.proxied >= 2,
            ),
            (
                "retried request and lost reply, scheduled",
                tally.protocol_granted + tally.protocol_cancelled >= 2,
            ),
            ("wrapper killed while queued", tally.killed_queued >= 1),
            ("wrapper killed while running", tally.orphaned >= 1),
            ("failed spawn", tally.failed_spawn >= 1),
            (
                "daemon kill and restart",
                tally.kept_across_daemon_kill + tally.released_at_daemon_start >= 1,
            ),
        ];
        for (kind, happened) in acted {
            assert!(happened, "seed {seed:#x}: no {kind}: {tally:?}");
        }
        all.failed_open += tally.failed_open;
        all.kept_across_daemon_kill += tally.kept_across_daemon_kill;
        all.released_at_daemon_start += tally.released_at_daemon_start;
        all.protocol_granted += tally.protocol_granted;
        all.protocol_cancelled += tally.protocol_cancelled;
    }
    // The seed list as a whole also reaches the paths one seed may miss.
    assert!(
        all.failed_open >= 1
            && all.kept_across_daemon_kill >= 1
            && all.released_at_daemon_start >= 1
            && all.protocol_granted >= 1
            && all.protocol_cancelled >= 1,
        "the seed list misses a path: {all:?}"
    );
}
