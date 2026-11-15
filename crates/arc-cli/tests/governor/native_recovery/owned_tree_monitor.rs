//! Retain verified descendants before a wrapper can exit and orphan them.
use super::{children, confirmed_missing, info};
use arc_core::process_identity::{probe, IdentityStatus, ProcessIdentity};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Default)]
pub(super) struct Observations {
    anchor: Option<ProcessIdentity>,
    pub identities: Vec<ProcessIdentity>,
    // A live PID which could not be identified must never be silently ignored.
    pub unresolved: BTreeMap<u32, String>,
    pub enumeration_errors: BTreeMap<u32, String>,
}

impl Observations {
    // A helper captures the waiting fixture process's identity and atomically
    // publishes it. Never derive ownership from a PID-only request: that PID
    // could already belong to a different process by the time we read it.
    fn register(&mut self, directory: &Path) {
        let Some(anchor) = self.anchor.as_ref() else {
            self.enumeration_errors
                .insert(0, "missing session anchor".into());
            return;
        };
        if probe(anchor) != IdentityStatus::VerifiedMatch {
            self.enumeration_errors
                .insert(0, "session anchor unavailable".into());
            return;
        }
        let entries = match fs::read_dir(directory) {
            Ok(entries) => {
                self.enumeration_errors.remove(&0);
                entries
            }
            Err(error) => {
                self.enumeration_errors.insert(0, error.to_string());
                return;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    self.enumeration_errors.insert(0, error.to_string());
                    continue;
                }
            };
            let name = entry.file_name();
            let Some(request) = name.to_str().and_then(|name| name.strip_suffix(".request")) else {
                continue;
            };
            let path = entry.path();
            let Some((pid, token)) = request
                .split_once('.')
                .and_then(|(pid, token)| pid.parse::<u32>().ok().map(|pid| (pid, token)))
            else {
                discard(&path, &mut self.enumeration_errors);
                continue;
            };
            let identity = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ProcessIdentity>(&bytes).ok());
            let Some(identity) =
                identity.filter(|identity| identity.pid == pid && identity.start_token == token)
            else {
                discard(&path, &mut self.enumeration_errors);
                self.unresolved.remove(&pid);
                continue;
            };
            match probe(&identity) {
                IdentityStatus::VerifiedMatch
                    if unsafe { libc::getsid(pid as i32) } == anchor.pid as i32
                        && probe(anchor) == IdentityStatus::VerifiedMatch
                        && probe(&identity) == IdentityStatus::VerifiedMatch =>
                {
                    if !self.identities.contains(&identity) {
                        self.identities.push(identity.clone());
                    }
                    let ack = directory.join(format!("{pid}.{}.ack", identity.start_token));
                    match fs::write(ack, b"owned") {
                        Ok(()) => {
                            self.unresolved.remove(&pid);
                            discard(&path, &mut self.enumeration_errors);
                        }
                        Err(error) => {
                            self.unresolved
                                .insert(pid, format!("registration ack: {error}"));
                        }
                    }
                }
                IdentityStatus::Missing
                | IdentityStatus::Mismatch
                | IdentityStatus::VerifiedMatch => {
                    discard(&path, &mut self.enumeration_errors);
                    self.unresolved.remove(&pid);
                }
                IdentityStatus::Indeterminate if confirmed_missing(pid) => {
                    discard(&path, &mut self.enumeration_errors);
                    self.unresolved.remove(&pid);
                }
                IdentityStatus::Indeterminate => {
                    self.unresolved
                        .insert(pid, "registration identity unavailable".into());
                }
            }
        }
    }

    fn scan(&mut self) {
        let mut index = 0;
        while index < self.identities.len() {
            let parent = self.identities[index].clone();
            index += 1;
            match probe(&parent) {
                IdentityStatus::Missing | IdentityStatus::Mismatch => continue,
                IdentityStatus::Indeterminate => {
                    if !confirmed_missing(parent.pid) {
                        self.unresolved
                            .insert(parent.pid, "parent identity unavailable".into());
                    }
                    continue;
                }
                IdentityStatus::VerifiedMatch => {
                    self.unresolved.remove(&parent.pid);
                }
            }
            let pids = match children(parent.pid) {
                Ok(pids) => {
                    self.enumeration_errors.remove(&parent.pid);
                    pids
                }
                Err(error) => {
                    if !confirmed_missing(parent.pid) {
                        self.enumeration_errors
                            .insert(parent.pid, error.to_string());
                    }
                    continue;
                }
            };
            for pid in pids {
                let Some(child_info) = info(pid) else {
                    if !confirmed_missing(pid) {
                        self.unresolved
                            .insert(pid, "child information unavailable".into());
                    }
                    continue;
                };
                if child_info.pbi_ppid != parent.pid {
                    continue;
                }
                match ProcessIdentity::capture(pid) {
                    Ok(child)
                        if probe(&parent) == IdentityStatus::VerifiedMatch
                            && child_info
                                .pbi_start_tvsec
                                .checked_mul(1_000_000)
                                .and_then(|v| v.checked_add(child_info.pbi_start_tvusec))
                                .is_some_and(|start| child.start_token == start.to_string()) =>
                    {
                        self.unresolved.remove(&pid);
                        if !self.identities.contains(&child) {
                            self.identities.push(child);
                        }
                    }
                    _ if confirmed_missing(pid) => {
                        self.unresolved.remove(&pid);
                    }
                    _ => {
                        self.unresolved
                            .insert(pid, "child identity unavailable".into());
                    }
                }
            }
        }
        self.unresolved.retain(|pid, _| !confirmed_missing(*pid));
        // Dead identities cannot have live descendants still parented to them;
        // retain every live identity even after the original root exits.
        self.identities.retain(|identity| {
            !matches!(
                probe(identity),
                IdentityStatus::Missing | IdentityStatus::Mismatch
            )
        });
    }
}

fn discard(path: &Path, errors: &mut BTreeMap<u32, String>) {
    // Older PID-only requests were empty directories. Never traverse them.
    let result = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => fs::remove_dir(path),
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        if error.kind() != io::ErrorKind::NotFound {
            errors.insert(0, format!("remove registration request: {error}"));
        }
    }
}

pub(super) struct Monitor {
    observations: Arc<Mutex<Observations>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    directory: PathBuf,
    observe_ancestry: bool,
}

impl Monitor {
    pub fn start(
        root: ProcessIdentity,
        directory: PathBuf,
        observe_ancestry: bool,
    ) -> io::Result<Self> {
        let observations = Arc::new(Mutex::new(Observations {
            anchor: Some(root.clone()),
            identities: vec![root],
            ..Observations::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::clone(&observations);
        let signal = Arc::clone(&stop);
        let registrations = directory.clone();
        let thread = thread::Builder::new()
            .name("native-fixture-ownership".into())
            .spawn(move || {
                while !signal.load(Ordering::Acquire) {
                    let mut observations = state.lock().unwrap_or_else(|e| e.into_inner());
                    observations.register(&registrations);
                    if observe_ancestry {
                        observations.scan();
                    }
                    drop(observations);
                    thread::sleep(Duration::from_millis(5));
                }
            })?;
        Ok(Self {
            observations,
            stop,
            thread: Some(thread),
            directory,
            observe_ancestry,
        })
    }

    pub fn snapshot(&self) -> Vec<ProcessIdentity> {
        let mut observations = self.observations.lock().unwrap_or_else(|e| e.into_inner());
        observations.register(&self.directory);
        if self.observe_ancestry {
            observations.scan();
        }
        observations.identities.clone()
    }

    pub fn finish(mut self) -> Observations {
        self.stop.store(true, Ordering::Release);
        if self.thread.take().unwrap().join().is_err() {
            self.observations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .enumeration_errors
                .insert(0, "ownership observer panicked".into());
        }
        let mut observations = self.observations.lock().unwrap_or_else(|e| e.into_inner());
        observations.register(&self.directory);
        if self.observe_ancestry {
            observations.scan();
        }
        std::mem::take(&mut *observations)
    }

    pub fn recorded(&self) -> Vec<ProcessIdentity> {
        self.observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .identities
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::process::CommandExt, process::Command};

    #[test]
    fn reused_pid_request_is_discarded_and_unrelated_identity_is_not_owned() {
        let fixture = crate::native::Fixture::with_binary(Default::default());
        let mut command = Command::new("/bin/sleep");
        command.arg("300");
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = crate::native::OwnedChild::new(command.spawn().unwrap());
        let anchor = ProcessIdentity::capture(child.child.id()).unwrap();
        let mut observations = Observations {
            anchor: Some(anchor.clone()),
            identities: vec![anchor.clone()],
            ..Observations::default()
        };
        let replacement = ProcessIdentity::capture(std::process::id()).unwrap();
        let legacy_request = fixture.root.join(format!("{}.request", replacement.pid));
        fs::create_dir(&legacy_request).unwrap();
        observations.register(&fixture.root);
        assert!(
            !legacy_request.exists(),
            "PID-only requests cannot prove ownership"
        );
        assert!(!observations.identities.contains(&replacement));
        let mut original = replacement.clone();
        original.start_token = (replacement.start_token.parse::<u64>().unwrap() - 1).to_string();
        let request = fixture
            .root
            .join(format!("{}.{}.request", original.pid, original.start_token));
        fs::write(&request, serde_json::to_vec(&original).unwrap()).unwrap();
        observations.register(&fixture.root);
        assert!(!request.exists(), "stale request must be discarded");
        assert!(!observations.identities.contains(&replacement));
        assert!(!fixture
            .root
            .join(format!("{}.{}.ack", original.pid, original.start_token))
            .exists());

        // Even a current, real identity is rejected if it is outside the
        // fixture's anchored session.
        let request = fixture.root.join(format!(
            "{}.{}.request",
            replacement.pid, replacement.start_token
        ));
        fs::write(&request, serde_json::to_vec(&replacement).unwrap()).unwrap();
        observations.register(&fixture.root);
        assert!(!request.exists());
        assert!(!observations.identities.contains(&replacement));
        assert_eq!(probe(&replacement), IdentityStatus::VerifiedMatch);

        let owned_request = fixture
            .root
            .join(format!("{}.{}.request", anchor.pid, anchor.start_token));
        let mut stale_anchor = anchor.clone();
        stale_anchor.start_token = (anchor.start_token.parse::<u64>().unwrap() - 1).to_string();
        let stale_request = fixture.root.join(format!(
            "{}.{}.request",
            stale_anchor.pid, stale_anchor.start_token
        ));
        fs::write(&stale_request, serde_json::to_vec(&stale_anchor).unwrap()).unwrap();
        fs::write(&owned_request, serde_json::to_vec(&anchor).unwrap()).unwrap();
        observations.register(&fixture.root);
        assert!(!stale_request.exists());
        assert!(!owned_request.exists());
        assert!(fixture
            .root
            .join(format!("{}.{}.ack", anchor.pid, anchor.start_token))
            .exists());
    }
}
