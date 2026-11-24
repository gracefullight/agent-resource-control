//! Native process attribution on the Meter's adaptive cadence (lane A:
//! MTR-007, AT-MTR-03, MET-027, NFR-012, R-005 item 2).
//!
//! `meter_cadence::bind` runs [`sampler`] after the job-tree sampler on each
//! Meter publication, on the Meter thread: slow when idle, faster while a
//! job is active (SRS 10.7), and never faster than that under memory
//! pressure (see [`due`]). While the writer's committed state has an active
//! job, a pass reads the Proven roots from the writer before and after its
//! native scan (`arc_platform::process_scan`) and trusts only lease evidence
//! present, unchanged, in both reads. It classifies the readings with
//! `arc_meter::attribution` under the rules of `arc_ipc::meter::attribution`.
//!
//! Every pass has a sequence number and records the wall-clock and
//! monotonic time just before and after its native scan. A delta between
//! two scans is stored over the envelope `[earlier scan's start, later
//! scan's end]`, rounded outward and widened by any measured backward
//! clock step, so it contains both reads; a larger backward step fails the
//! pass. A successful pass closes one interval and moves the scanner's
//! durable frontier in one transaction; the first pass after start or after
//! a failure is a baseline that resolves every open frontier into a failed
//! interval. A failed pass (a roots read, the scan, the clock or the commit)
//! discards the baseline, tells the store and this process's summary reads
//! that the stretch from its floor is unresolved, and counts nothing. The
//! floor is the last committed scan's start, lowered to what the boot clock
//! implies when the wall clock stepped back (`arc_storage::attribution::
//! floor_of`), so a window that the reversed wall clock reaches again stays
//! unresolved; a failure no write could record is carried into the next
//! baseline, and a restarted scanner derives the floor from the durable
//! frontier's boot-clock reading before its first pass.
//! Before any native scan, an independent, exclusive lifecycle witness is
//! committed unclean. It is certified clean only after the interval seal;
//! an abandoned witness makes earlier windows uncertain even if this
//! scanner never committed a baseline.
//!
//! The durable frontier is also a fence: until this scanner seals it, a
//! restarted daemon treats every earlier window as unresolved (the store's
//! rule), because a backward step this process read and could not record
//! is invisible once the clock is corrected. The seal is written only on a
//! normal process exit, after the Meter thread stopped, together with any
//! failure or step this scanner still holds; a crash, a timed-out Meter
//! thread or an unwritable store leaves the fence.
//! Process names live only for one scan; no PID, argv, path or cwd is
//! stored, logged or reported. The opt-in self-observer sends each process
//! only its own readings over an authenticated, ephemeral socket.
use crate::db_worker::{attribution, resource_samples, DbClient};
use arc_core::meter::MeterSnapshot;
use arc_ipc::{
    jobs::usage::Vendor, meter::attribution::vendor_for_executable as legacy_vendor_for_executable,
};
use arc_meter::attribution::{Attributor, ManagedRoot, ScanTotals};
use arc_storage::attribution::{
    self as store, floor_of, BootClock, Frontier, Interval, IntervalStore, LifecycleGuard,
    ManagedRootEvidence, ScanPoint, ScannerId,
};
use clock::{envelope, Clock, ScanClock, WallShift};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Once, Weak},
    time::{Duration, Instant},
};

#[path = "process_attribution/clock.rs"]
mod clock;
#[path = "process_attribution/probe.rs"]
mod probe;

/// Extend the legacy executable mapping locally while keeping its old wire
/// vocabulary unchanged for peers that predate Codex.
fn vendor_for_executable(name: &str) -> Option<Vendor> {
    if name == "codex" {
        Some(Vendor::Codex)
    } else {
        legacy_vendor_for_executable(name)
    }
}

/// One pass's budget across its writer requests.
const PASS_BUDGET: Duration = Duration::from_secs(2);

/// What one successful pass committed, for the self-observer.
#[derive(Debug, Clone, Copy)]
pub(super) struct Committed {
    pub seq: u64,
    pub scan_start_ms: u64,
    pub scan_end_ms: u64,
    /// The closed interval's envelope; `None` for a baseline.
    pub interval: Option<(u64, u64)>,
}

struct Baseline {
    seq: u64,
    clock: ScanClock,
}

struct State {
    store: Option<IntervalStore>,
    scanner: Option<ScannerId>,
    attributor: Attributor,
    probes: probe::Probes,
    last_pass: Option<Instant>,
    seq: u64,
    baseline: Option<Baseline>,
    created_ms: u64,
    shift: WallShift,
    boot_id: Option<String>,
    /// The start of the last committed scan: the durable frontier.
    committed: Option<Clock>,
    /// The floor of a failure no write recorded yet.
    unrecorded_from: Option<u64>,
    /// A separate durable witness was armed before this scanner could run.
    lifecycle: Option<LifecycleGuard>,
    /// Sealed at exit: no pass runs again.
    stopped: bool,
}

/// Every scanner of this process, sealed by [`seal_at_exit`].
static SCANNERS: Mutex<Vec<Weak<Mutex<State>>>> = Mutex::new(Vec::new());
static AT_EXIT: Once = Once::new();

/// Runs on a normal exit (after `main` returns or `exit`), when the daemon
/// has joined its Meter thread; a scanner still busy is left unsealed.
extern "C" fn seal_at_exit() {
    let _ = std::panic::catch_unwind(|| {
        let scanners = SCANNERS.lock().map(|all| all.clone()).unwrap_or_default();
        for state in scanners.iter().filter_map(Weak::upgrade) {
            if let Ok(mut state) = state.try_lock() {
                state.seal();
            }
        }
    });
}

fn register(state: &Arc<Mutex<State>>) {
    if let Ok(mut all) = SCANNERS.lock() {
        all.retain(|weak| weak.strong_count() > 0);
        all.push(Arc::downgrade(state));
    }
    AT_EXIT.call_once(|| {
        // SAFETY: registers a plain `extern "C"` function with no arguments.
        unsafe { libc::atexit(seal_at_exit) };
    });
}

/// The scan's own cadence: the Meter's interval, but never faster than the
/// policy's idle or active cadence. Memory pressure speeds the host Meter up,
/// never this scan, so an idle daemon under pressure still scans slowly.
fn due(state: &State, snapshot: &MeterSnapshot, active: bool) -> bool {
    let floor = crate::meter::policy().cadence(active, false).interval_ms;
    let interval = Duration::from_millis(snapshot.cadence.interval_ms.max(floor));
    state
        .last_pass
        .is_none_or(|last| last.elapsed() >= interval)
}

pub(super) fn sampler(client: &DbClient) -> arc_meter::SampleHook {
    let client = client.clone();
    let store = arc_core::database_path()
        .ok()
        .and_then(|path| IntervalStore::beside(&path));
    let scanner = ScannerId::random();
    if let Some(scanner) = &scanner {
        store::register_scanner(scanner);
    }
    let shift = WallShift::from_env();
    let mut state = State {
        store,
        scanner,
        attributor: Attributor::default(),
        probes: probe::Probes::new(),
        last_pass: None,
        seq: 0,
        baseline: None,
        created_ms: Clock::now(&shift).wall_ms(),
        shift,
        boot_id: arc_core::process_identity::boot_id().ok(),
        committed: None,
        unrecorded_from: None,
        lifecycle: None,
        stopped: false,
    };
    state.arm_lifecycle();
    state.recover_floor();
    let state = Arc::new(Mutex::new(state));
    register(&state);
    Box::new(move |snapshot| {
        let Ok(mut state) = state.lock() else { return };
        if state.stopped {
            return;
        }
        let state = &mut *state;
        if state.baseline.is_some() && state.probes.scan_held() {
            return;
        }
        let active = resource_samples::jobs_active(&client);
        if !due(state, snapshot, active) {
            return;
        }
        state.last_pass = Some(Instant::now());
        // A runtime per pass is created and dropped on this thread, never
        // inside the daemon's async context.
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        else {
            state.fail();
            return;
        };
        runtime.block_on(pass(&client, state, active));
    })
}

fn seconds(ns: u64) -> f64 {
    ns as f64 / 1e9
}

fn interval(start_ms: u64, end_ms: u64, totals: &ScanTotals<String>) -> Interval {
    let groups = |map: &BTreeMap<String, u64>| {
        map.iter()
            .map(|(vendor, ns)| (vendor.clone(), seconds(*ns)))
            .collect()
    };
    Interval {
        start_ms,
        end_ms,
        proven: groups(&totals.proven_ns),
        estimated: groups(&totals.estimated_ns),
        unattributed: seconds(totals.unattributed_ns),
        skipped: totals.skipped,
    }
}

/// Evidence present, unchanged, in both reads.
fn held_throughout(
    before: Vec<ManagedRootEvidence>,
    after: &[ManagedRootEvidence],
) -> Vec<ManagedRoot<String>> {
    before
        .into_iter()
        .filter(|evidence| after.contains(evidence))
        .map(|evidence| ManagedRoot {
            identity: evidence.identity,
            vendor: evidence.vendor,
        })
        .collect()
}

impl State {
    /// Never admit a scan before its independent lifetime witness is
    /// durable. If admission fails, this daemon's reads remain uncertain;
    /// retries keep that uncertainty until a baseline records it.
    fn arm_lifecycle(&mut self) -> bool {
        if self.lifecycle.is_some() {
            return true;
        }
        let armed = self
            .store
            .as_ref()
            .zip(self.scanner.as_ref())
            .and_then(|(store, scanner)| store.arm_lifecycle(scanner).ok());
        if armed.as_ref().is_none_or(|(_, uncertain)| *uncertain) {
            self.unrecorded_from = Some(0);
            if let Some(scanner) = &self.scanner {
                store::note_unrecorded_failure(scanner, 0);
            }
        }
        self.lifecycle = armed.map(|(guard, _)| guard);
        self.lifecycle.is_some()
    }

    fn clock(&self) -> Clock {
        Clock::now(&self.shift)
    }

    fn point(&self, clock: &Clock) -> ScanPoint {
        ScanPoint {
            wall_ms: clock.wall_ms(),
            boot: self
                .boot_id
                .clone()
                .zip(clock.boot_ns)
                .map(|(boot_id, ns)| BootClock { boot_id, ns }),
        }
    }

    /// The floor that `now` implies below the last committed scan or any
    /// durable frontier: the wall clock stepped back.
    fn reversal_floor(&self, now: &ScanPoint) -> Option<u64> {
        let own = self.committed.map(|start| {
            let point = self.point(&start);
            Frontier {
                start_ms: point.wall_ms,
                low_ms: point.wall_ms,
                boot: point.boot,
            }
        });
        let durable = self
            .store
            .as_ref()
            .and_then(|store| store.reversal_floor(now).ok().flatten());
        floor_of(own.as_slice(), now)
            .into_iter()
            .chain(durable)
            .min()
    }

    /// The unresolved stretch from `from` (kept at its earliest): this
    /// process's summary reads see it at once, the store when it can be
    /// written, and the next baseline records it either way.
    fn unresolved(&mut self, from: u64, now_ms: u64) {
        let from = self.unrecorded_from.map_or(from, |f| f.min(from));
        self.unrecorded_from = Some(from);
        if let Some(scanner) = &self.scanner {
            store::note_unrecorded_failure(scanner, from);
            if let Some(store) = &self.store {
                let _ = store.mark_failed_from(scanner, Some((self.seq.max(1), from)), now_ms);
            }
        }
    }

    /// Before the first pass: a wall clock behind what the durable frontier's
    /// boot-clock reading implies stepped back while no scanner ran.
    fn recover_floor(&mut self) {
        let now = self.clock();
        if let Some(floor) = self.reversal_floor(&self.point(&now)) {
            if let Some(scanner) = &self.scanner {
                store::note_unrecorded_failure(scanner, floor);
            }
            self.unrecorded_from = Some(self.unrecorded_from.map_or(floor, |old| old.min(floor)));
        }
    }

    /// Seal this scanner's frontier on a clean stop, with the floor of any
    /// failure it holds and of a backward step read now. Nothing to seal
    /// when it never committed or failed.
    fn seal(&mut self) {
        if std::mem::replace(&mut self.stopped, true) {
            return;
        }
        let (Some(store), Some(scanner)) = (&self.store, &self.scanner) else {
            return;
        };
        let Some(guard) = &self.lifecycle else {
            return;
        };
        if self.committed.is_none() && self.unrecorded_from.is_none() {
            let _ = store.seal_lifecycle(guard, scanner);
            return;
        }
        let now = self.clock();
        let point = self.point(&now);
        let floor = self
            .unrecorded_from
            .into_iter()
            .chain(self.reversal_floor(&point))
            .min();
        let failure = floor.map(|floor| (self.seq.max(1), floor));
        if store.seal(scanner, failure, point.wall_ms).is_ok() {
            let _ = store.seal_lifecycle(guard, scanner);
        }
    }

    /// Discard the baseline. The stretch from the last committed scan (or,
    /// without one, from this scanner's start), lowered to the floor a
    /// backward wall-clock step implies, is unresolved.
    fn fail(&mut self) {
        let now = self.clock();
        let point = self.point(&now);
        let last = self
            .baseline
            .take()
            .map(|b| b.clock.start)
            .or(self.committed);
        let from = last
            .map_or(self.created_ms, |start| start.wall_ms())
            .min(self.reversal_floor(&point).unwrap_or(u64::MAX));
        self.unresolved(from, point.wall_ms);
        self.attributor = Attributor::default();
        self.probes.failed(self.seq, point.wall_ms);
    }
}

async fn pass(client: &DbClient, state: &mut State, active: bool) {
    state.seq += 1;
    let seq = state.seq;
    if !state.arm_lifecycle() {
        // No native scan is admitted without a durable witness.
        return;
    }
    let end = Instant::now() + PASS_BUDGET;
    let (Some(store), Some(scanner)) = (state.store.clone(), state.scanner.clone()) else {
        return state.fail();
    };
    let before = if active {
        match attribution::managed(client, end).await {
            Some(roots) => roots,
            // Without the roots no Proven class can be assigned.
            None => return state.fail(),
        }
    } else {
        Vec::new()
    };
    let started = state.clock();
    let scan = arc_platform::process_scan::scan();
    let clock = ScanClock {
        start: started,
        end: state.clock(),
    };
    if scan.processes.is_empty() {
        return state.fail();
    }
    let after = if active {
        match attribution::managed(client, end).await {
            Some(roots) => roots,
            None => return state.fail(),
        }
    } else {
        Vec::new()
    };
    let roots = held_throughout(before, &after);
    let mut next = state.attributor.clone();
    let (totals, processes) = next.observe(
        &scan,
        &roots,
        |name| vendor_for_executable(name).map(|vendor| vendor.as_str().to_owned()),
        |member, root| {
            if member == root {
                arc_core::process_identity::probe_live(root)
                    == arc_core::process_identity::IdentityStatus::VerifiedMatch
            } else {
                crate::connection::job_ownership::managed_child(member, root)
            }
        },
    );
    let now_ms = state.clock().wall_ms();
    let start = state.point(&clock.start);
    let committed = match &state.baseline {
        Some(prev) => envelope(&prev.clock, &clock).and_then(|(from, to)| {
            store
                .close_at(
                    &scanner,
                    prev.seq,
                    seq,
                    &interval(from, to, &totals),
                    &start,
                    now_ms,
                )
                .ok()
                .map(|()| Some((from, to)))
        }),
        None => match store.begin_at(
            &scanner,
            seq,
            &start,
            clock.end_ms(),
            state.unrecorded_from,
            now_ms,
        ) {
            Ok(()) => Some(None),
            Err(error) => {
                state.probes.failed_baseline(
                    &processes,
                    Committed {
                        seq,
                        scan_start_ms: clock.start_ms(),
                        scan_end_ms: clock.end_ms(),
                        interval: None,
                    },
                    error,
                );
                None
            }
        },
    };
    let Some(interval) = committed else {
        return state.fail();
    };
    if interval.is_none() {
        // The baseline recorded every unresolved stretch as failed.
        state.unrecorded_from = None;
        store::clear_unrecorded_failure(&scanner);
    }
    state.committed = Some(clock.start);
    state.baseline = Some(Baseline { seq, clock });
    state.attributor = next;
    state.probes.observe(
        &processes,
        Committed {
            seq,
            scan_start_ms: clock.start_ms(),
            scan_end_ms: clock.end_ms(),
            interval,
        },
    );
}

#[cfg(test)]
#[path = "process_attribution/codex_mapping_tests.rs"]
mod codex_mapping_tests;
