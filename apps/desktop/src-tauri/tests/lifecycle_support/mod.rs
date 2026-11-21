//! Built-arcctl requests for the close-to-hide case. Each child gets the
//! private daemon fixture's cleared environment, temporary HOME, explicit
//! paths, forwarded coverage profile and the shared 60 s output watchdog.
use super::{daemon_bridge_support::RealDaemon, native};
use std::process::{Command, Output, Stdio};

pub fn arcctl(case: &RealDaemon, args: &[&str]) -> Output {
    let mut command = Command::new(case.fixture.binary("arcctl"));
    case.isolate(&mut command);
    command
        .args(args)
        .current_dir(&case.fixture.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    case.profile_command(&mut command);
    native::OwnedChild::new(command.spawn().unwrap())
        .wait_with_output()
        .unwrap_or_else(|error| panic!("arcctl {args:?} did not complete: {error}"))
}

/// The Meter snapshot is the one reply in which the answering daemon states
/// its own PID and start time. The sampler publishes it after startup, so the
/// read is repeated; the deadline is a diagnostic watchdog, never a result.
#[cfg(target_os = "macos")]
mod answering_daemon {
    use super::{arcctl, RealDaemon};
    use arc_core::process_identity::ProcessIdentity;
    use serde_json::Value;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    fn published(source: &str, mut read: impl FnMut() -> Option<Value>) -> ProcessIdentity {
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(snapshot) = read() {
                return arc_ipc::meter::decode_snapshot(&snapshot)
                    .unwrap()
                    .daemon_instance;
            }
            assert!(Instant::now() < end, "{source}: no Meter snapshot");
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn through_cli(case: &RealDaemon) -> ProcessIdentity {
        published("arcctl meter snapshot", || {
            let output = arcctl(case, &["meter", "snapshot", "--json"]);
            if output.status.success() {
                return Some(serde_json::from_slice(&output.stdout).unwrap());
            }
            assert!(
                String::from_utf8_lossy(&output.stderr).starts_with("METER_UNAVAILABLE"),
                "{output:?}"
            );
            None
        })
    }

    pub fn through_bridge(read: impl Fn() -> Result<Value, Value>) -> ProcessIdentity {
        published("registered meter_snapshot", || match read() {
            Ok(reply) => Some(reply["data"].clone()),
            Err(error) => {
                assert_eq!(error["retryable"], true, "{error}");
                None
            }
        })
    }
}
#[cfg(target_os = "macos")]
pub use answering_daemon::{through_bridge, through_cli};
