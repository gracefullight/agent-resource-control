//! SwiftPM records absolute paths: prepare fixed trees and lease them in place.
use super::{cleanup, projects, tools::Tools, Form};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

const PER_FORM: usize = 2;
const WAIT: Duration = Duration::from_secs(300);
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static RETAIN_ROOT: AtomicBool = AtomicBool::new(false);
static POOL: OnceLock<Arc<Pool>> = OnceLock::new();

/// Populate private, path-bound trees before contending for the daemon.
pub(super) fn prewarm(tools: &Tools) {
    let _ = POOL.get_or_init(|| Pool::new(tools));
}

extern "C" fn remove_root() {
    if RETAIN_ROOT.load(Ordering::SeqCst) {
        eprintln!("retaining SwiftPM pool because diagnostic preservation failed");
        return;
    }
    if let Some(path) = ROOT.get() {
        if let Err(error) = fs::remove_dir_all(path) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("cannot remove SwiftPM pool {}: {error}", path.display());
                unsafe { libc::_exit(1) };
            }
        }
    }
}

struct Slot {
    form: Form,
    project: PathBuf,
    borrower: Option<PathBuf>,
    quarantined: bool,
}

struct Pool {
    slots: Mutex<Vec<Slot>>,
    available: Condvar,
}

impl Pool {
    fn new(tools: &Tools) -> Arc<Self> {
        let root = ROOT.get_or_init(|| {
            let root = super::super::root().join(format!(
                "target/traffic-native-tools/swift/pool-{}-{}",
                std::process::id(),
                arc_core::process_identity::random_job_id().unwrap()
            ));
            fs::create_dir_all(&root).unwrap();
            assert_eq!(unsafe { libc::atexit(remove_root) }, 0);
            root.canonicalize().unwrap()
        });
        let slots: Vec<_> = [Form::SwiftBuild, Form::SwiftTest]
            .into_iter()
            .flat_map(|form| {
                (0..PER_FORM).map(move |index| Slot {
                    form,
                    project: root.join(format!("{}-{index}", form.id())),
                    borrower: None,
                    quarantined: false,
                })
            })
            .collect();
        // Each tree is prepared at its final absolute path. Independent trees
        // compile concurrently, and no scenario may acquire a partial tree.
        std::thread::scope(|scope| {
            let jobs: Vec<_> = slots
                .iter()
                .map(|slot| {
                    scope.spawn(move || projects::build_tree(&slot.project, tools, slot.form))
                })
                .collect();
            for job in jobs {
                job.join().expect("SwiftPM pool preparation failed");
            }
        });
        Arc::new(Self {
            slots: Mutex::new(slots),
            available: Condvar::new(),
        })
    }

    fn lease(self: &Arc<Self>, form: Form, destination: &Path) -> Lease {
        self.lease_matching(form, destination, None)
    }

    fn lease_matching(
        self: &Arc<Self>,
        form: Form,
        destination: &Path,
        project: Option<&Path>,
    ) -> Lease {
        let end = Instant::now() + WAIT;
        let mut slots = self.slots.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some((index, slot)) = slots.iter_mut().enumerate().find(|(_, slot)| {
                slot.form == form
                    && slot.borrower.is_none()
                    && !slot.quarantined
                    && project.is_none_or(|path| slot.project == path)
            }) {
                slot.borrower = Some(destination.to_owned());
                return Lease {
                    pool: Arc::clone(self),
                    index,
                    project: slot.project.clone(),
                    destination: destination.to_owned(),
                };
            }
            let now = Instant::now();
            assert!(
                now < end,
                "SwiftPM lease watchdog: {form:?}, requested {project:?}; borrowers: {:?}",
                slots
                    .iter()
                    .map(|slot| (&slot.project, &slot.borrower, slot.quarantined))
                    .collect::<Vec<_>>()
            );
            slots = self
                .available
                .wait_timeout(slots, end.saturating_duration_since(now))
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
    }
}

struct Lease {
    pool: Arc<Pool>,
    index: usize,
    project: PathBuf,
    destination: PathBuf,
}

impl Lease {
    fn preserve(&self) -> io::Result<()> {
        cleanup::capture_failure(&self.project, &self.destination)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let failed = std::thread::panicking();
        let result = if failed { self.preserve() } else { Ok(()) }
            .and_then(|()| projects::reset_tree(&self.project));
        let mut slots = self
            .pool
            .slots
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let slot = &mut slots[self.index];
        if let Err(error) = result {
            // Do not hand out a dirty tree or discard its only diagnostics.
            RETAIN_ROOT.store(true, Ordering::SeqCst);
            slot.quarantined = true;
            eprintln!(
                "cannot preserve/reset SwiftPM lease {}: {error}",
                self.project.display()
            );
            if !failed {
                panic!("SwiftPM lease cleanup failed");
            }
        } else {
            slot.borrower = None;
        }
        self.pool.available.notify_all();
    }
}

#[derive(Default)]
pub(super) struct Leases {
    held: Mutex<Vec<Lease>>,
}

impl Leases {
    pub(super) fn acquire(&self, tools: &Tools, form: Form, destination: &Path) -> PathBuf {
        let lease = POOL
            .get_or_init(|| Pool::new(tools))
            .lease(form, destination);
        self.record(lease)
    }

    #[cfg(test)]
    pub(super) fn reacquire(
        &self,
        tools: &Tools,
        form: Form,
        destination: &Path,
        project: &Path,
    ) -> PathBuf {
        let lease =
            POOL.get_or_init(|| Pool::new(tools))
                .lease_matching(form, destination, Some(project));
        self.record(lease)
    }

    fn record(&self, lease: Lease) -> PathBuf {
        let project = lease.project.clone();
        self.held
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(lease);
        project
    }

    pub(super) fn release_baseline(&self, path: &Path) {
        self.release(path, true);
    }

    pub(super) fn release_success(&self, path: &Path) {
        self.release(path, false);
    }

    fn release(&self, path: &Path, preserve: bool) {
        let mut held = self.held.lock().unwrap_or_else(|error| error.into_inner());
        let index = held
            .iter()
            .position(|lease| lease.project == path)
            .expect("owned Swift baseline");
        // The enclosing managed comparison can still fail. Save this completed
        // baseline before returning its tree, and keep it until case.done().
        if preserve {
            held[index]
                .preserve()
                .expect("preserve Swift baseline diagnostics");
        }
        let lease = held.swap_remove(index);
        drop(held);
        drop(lease);
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
