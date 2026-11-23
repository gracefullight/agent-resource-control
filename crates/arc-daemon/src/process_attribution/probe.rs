//! Optional self-observation for isolated native fixtures.
//!
//! Only the process which opened a stream can receive its own classification,
//! delta and cumulative CPU reading, with the scanner's own pass sequence,
//! scan bounds and committed interval envelope, and whether a pass failed.
//! The kernel supplies its peer PID; no target or PID is accepted
//! from the stream, no identity is written to disk, and no other process's
//! readings are sent. Connections and identity tuples exist only in memory.
use super::Committed;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_meter::attribution::{Attributed, Class};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::MetadataExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
};

const MAX_CLIENTS: usize = 16;
const ENV_PATH: &str = "ARC_ATTRIBUTION_SELF_OBSERVE";

struct Client {
    identity: ProcessIdentity,
    stream: UnixStream,
}

pub(super) struct Probes {
    listener: Option<UnixListener>,
    socket: Option<(PathBuf, u64, u64)>,
    clients: Vec<Client>,
}

impl Probes {
    pub(super) fn new() -> Self {
        let mut result = Self {
            listener: None,
            socket: None,
            clients: Vec::new(),
        };
        let Some(path) = std::env::var_os(ENV_PATH).map(PathBuf::from) else {
            return result;
        };
        let Some(parent) = path.parent() else {
            return result;
        };
        let Ok(metadata) = fs::symlink_metadata(parent) else {
            return result;
        };
        if !path.is_absolute()
            || !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o7777 != 0o700
        {
            return result;
        }
        let Ok(listener) = UnixListener::bind(&path) else {
            return result;
        };
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            return result;
        };
        if listener.set_nonblocking(true).is_err() {
            let _ = fs::remove_file(&path);
            return result;
        }
        result.socket = Some((path, metadata.dev(), metadata.ino()));
        result.listener = Some(listener);
        result
    }

    /// Accept before either a successful or failed first baseline is reported.
    fn accept(&mut self) {
        if let Some(listener) = &self.listener {
            while self.clients.len() < MAX_CLIENTS {
                let Ok((stream, _)) = listener.accept() else {
                    break;
                };
                if crate::platform::ensure_same_user(&stream).is_err() {
                    continue;
                }
                let Some(identity) = crate::platform::peer_pid(&stream)
                    .ok()
                    .and_then(|pid| ProcessIdentity::capture(pid).ok())
                else {
                    continue;
                };
                if stream.set_nonblocking(true).is_ok() {
                    self.clients.push(Client { identity, stream });
                }
            }
        }
    }

    /// Lines: `counted <class> <vendor> <delta_ns> <cpu_ns> <seq> <scan_start_ms>
    /// <scan_end_ms> <interval_start_ms> <interval_end_ms>` and `first <class>
    /// <vendor> <cpu_ns> <seq> <scan_start_ms> <scan_end_ms>`.
    pub(super) fn observe(&mut self, processes: &[Attributed<String>], pass: Committed) {
        self.accept();
        self.clients.retain_mut(|client| {
            if probe_live(&client.identity) != IdentityStatus::VerifiedMatch {
                return false;
            }
            let Some(row) = processes.iter().find(|p| p.identity == client.identity) else {
                return true;
            };
            let (class, vendor) = match &row.class {
                Class::Proven(vendor) => ("proven", vendor.as_str()),
                Class::Estimated(vendor) => ("estimated", vendor.as_str()),
                Class::Unattributed => ("unattributed", "-"),
            };
            let scan = format!("{} {} {}", pass.seq, pass.scan_start_ms, pass.scan_end_ms);
            let line = match (row.delta_ns, pass.interval) {
                (Some(ns), Some((start, end))) => format!(
                    "counted {class} {vendor} {ns} {} {scan} {start} {end}\n",
                    row.cpu_time_ns
                ),
                _ => format!("first {class} {vendor} {} {scan}\n", row.cpu_time_ns),
            };
            send(&mut client.stream, &line)
        });
    }

    /// A completed native scan of the peer itself followed by a failed
    /// baseline write. Both events carry the same pass and scan bounds; no
    /// process identity or CPU fact is disclosed about any other process.
    pub(super) fn failed_baseline(
        &mut self,
        processes: &[Attributed<String>],
        pass: Committed,
        error: arc_storage::jobs::JobError,
    ) {
        self.accept();
        let scan = format!("{} {} {}", pass.seq, pass.scan_start_ms, pass.scan_end_ms);
        let cause = match error {
            arc_storage::jobs::JobError::StorageFailure => "storage",
            arc_storage::jobs::JobError::Busy => "busy",
            _ => "rejected",
        };
        let lines = format!("baseline_scanned {scan}\nbaseline_write_failed {scan} {cause}\n");
        self.clients.retain_mut(|client| {
            if probe_live(&client.identity) != IdentityStatus::VerifiedMatch {
                return false;
            }
            if processes.iter().any(|row| row.identity == client.identity) {
                send(&mut client.stream, &lines)
            } else {
                true
            }
        });
    }

    /// Tell every connected process that pass `seq` failed at `at_ms` and
    /// counted nothing: `failed <seq> <at_ms>`.
    pub(super) fn failed(&mut self, seq: u64, at_ms: u64) {
        let line = format!("failed {seq} {at_ms}\n");
        self.clients
            .retain_mut(|client| send(&mut client.stream, &line));
    }
}

fn send(stream: &mut UnixStream, line: &str) -> bool {
    matches!(stream.write(line.as_bytes()), Ok(n) if n == line.len())
}

impl Drop for Probes {
    fn drop(&mut self) {
        if let Some((path, dev, ino)) = &self.socket {
            if fs::symlink_metadata(path).is_ok_and(|m| (m.dev(), m.ino()) == (*dev, *ino)) {
                let _ = fs::remove_file(path);
            }
        }
    }
}
