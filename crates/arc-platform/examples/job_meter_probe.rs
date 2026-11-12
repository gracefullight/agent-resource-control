//! Feasibility only: the independent C reference and production wait4 observe
//! the same workload execution. This is not persisted AT-MTR-01 acceptance.
#[cfg(unix)]
use arc_platform::child_wait::{wait_owned_child, WaitMode};
#[cfg(unix)]
use std::{io::Read, process::Command, time::Instant};

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let workload = args.next().ok_or("missing workload path")?;
    let duration = args.next().unwrap_or_else(|| "10".into());
    let started = Instant::now();
    let mut child = Command::new(workload)
        .arg(duration)
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let identity = arc_core::process_identity::ProcessIdentity::capture(child.id());
    let event = loop {
        if let Some(event) = wait_owned_child(child.id(), WaitMode::Blocking)? {
            if event.terminal_usage.is_some() {
                break event;
            }
        }
    };
    let elapsed = started.elapsed().as_secs_f64();
    let mut reference = String::new();
    child
        .stdout
        .take()
        .ok_or("missing stdout")?
        .read_to_string(&mut reference)?;
    let identity = identity?;
    if !event.status.success() {
        return Err("workload failed".into());
    }
    let usage = event
        .terminal_usage
        .unwrap()
        .map_err(|_| "invalid terminal usage")?;
    println!(
        "{{\"child\":{{\"pid\":{},\"start_token\":\"{}\",\"boot_id\":\"{}\"}},\"run_elapsed_seconds\":{},\"status_code\":0,\"terminal\":{{\"raw_user\":{{\"seconds\":{},\"microseconds\":{}}},\"raw_system\":{{\"seconds\":{},\"microseconds\":{}}},\"user_core_seconds\":{},\"system_core_seconds\":{},\"total_core_seconds\":{}}},\"reference\":{}}}",
        identity.pid, identity.start_token, identity.boot_id, elapsed,
        usage.raw_user.seconds, usage.raw_user.microseconds,
        usage.raw_system.seconds, usage.raw_system.microseconds,
        usage.user_core_seconds, usage.system_core_seconds, usage.total_core_seconds,
        reference.trim()
    );
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("native job-meter feasibility probe is unsupported on this platform");
    std::process::exit(2);
}
