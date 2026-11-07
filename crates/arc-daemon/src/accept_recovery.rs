//! One accept future; test injection never enters the production environment.

use std::{io, time::Duration};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::Instant;

/// No retry threshold terminates service. Only invalid listener state is fatal.
#[derive(Default)]
pub(crate) struct Recovery {
    pub errors: u64,
    pub transient: u64,
    pub resource_pressure: u64,
    pub other: u64,
    pub fatal: u64,
    pub retries: u64,
    pub consecutive: u64,
    pub consecutive_high_water: u64,
    pub backoff_high_water_ms: u64,
    pub retry_at: Option<Instant>,
}

impl Recovery {
    pub(crate) fn failed(&mut self, error: &io::Error) -> bool {
        self.errors = self.errors.saturating_add(1);
        let counter = match error.raw_os_error() {
            Some(libc::EBADF | libc::EINVAL | libc::ENOTSOCK | libc::EOPNOTSUPP) => {
                self.fatal = self.fatal.saturating_add(1);
                return false;
            }
            Some(libc::EMFILE | libc::ENFILE | libc::ENOBUFS | libc::ENOMEM) => {
                &mut self.resource_pressure
            }
            Some(
                libc::EINTR | libc::EAGAIN | libc::ECONNABORTED | libc::EPROTO | libc::ECONNRESET,
            ) => &mut self.transient,
            _ => &mut self.other,
        };
        *counter = counter.saturating_add(1);
        self.consecutive = self.consecutive.saturating_add(1);
        self.consecutive_high_water = self.consecutive_high_water.max(self.consecutive);
        self.retries = self.retries.saturating_add(1);
        let delay_ms = (5u64 << self.consecutive.saturating_sub(1).min(5)).min(100);
        self.backoff_high_water_ms = self.backoff_high_water_ms.max(delay_ms);
        self.retry_at = Some(Instant::now() + Duration::from_millis(delay_ms));
        true
    }

    pub(crate) fn accepted(&mut self) {
        self.consecutive = 0;
        self.retry_at = None;
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "accept_errors": self.errors, "transient_errors": self.transient,
            "resource_pressure_errors": self.resource_pressure, "other_errors": self.other,
            "fatal_errors": self.fatal, "accept_retries": self.retries,
            "consecutive_errors": self.consecutive,
            "consecutive_errors_high_water": self.consecutive_high_water,
            "backoff_high_water_ms": self.backoff_high_water_ms,
            "shutdown_during_backoff": self.retry_at.is_some_and(|at| Instant::now() < at),
            // The supervisor is still executing this final export. Process exit
            // establishes its completion; it is separate from handler gauges.
            "retry_tasks": 0, "supervisors": {"current": 1, "high_water": 1},
        })
    }
}

#[derive(Default)]
pub(crate) struct Acceptor {
    #[cfg(test)]
    injection: Option<Injection>,
    #[cfg(test)]
    observer: Option<tokio::sync::watch::Sender<u64>>,
}

impl Acceptor {
    pub(crate) async fn accept(&mut self, listener: &UnixListener) -> io::Result<UnixStream> {
        #[cfg(test)]
        if let Some(injection) = &mut self.injection {
            if injection.remaining != 0 && injection.after_accepts == 0 {
                injection.remaining -= 1;
                return Err(io::Error::from_raw_os_error(injection.errno));
            }
        }
        let stream = listener.accept().await.map(|(stream, _)| stream)?;
        #[cfg(test)]
        if let Some(injection) = &mut self.injection {
            injection.after_accepts = injection.after_accepts.saturating_sub(1);
        }
        Ok(stream)
    }

    #[cfg(test)]
    pub(crate) fn injected(errno: i32, remaining: usize) -> Self {
        Self {
            injection: Some(Injection {
                errno,
                remaining,
                after_accepts: 0,
            }),
            observer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn after_accepts(mut self, count: usize) -> Self {
        self.injection.as_mut().unwrap().after_accepts = count;
        self
    }

    #[cfg(test)]
    pub(crate) fn observe(&mut self) -> tokio::sync::watch::Receiver<u64> {
        let (sender, receiver) = tokio::sync::watch::channel(0);
        self.observer = Some(sender);
        receiver
    }

    pub(crate) fn retrying(&self, _recovery: &Recovery) {
        #[cfg(test)]
        if let Some(observer) = &self.observer {
            observer.send_replace(_recovery.retries);
        }
    }
}

#[cfg(test)]
struct Injection {
    errno: i32,
    remaining: usize,
    after_accepts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn retry_policy_caps_backoff_and_saturates_without_a_fatal_threshold() {
        let mut recovery = Recovery::default();
        for i in 0..100_000 {
            let before = Instant::now();
            assert!(recovery.failed(&io::Error::from_raw_os_error(libc::EMFILE)));
            let delay = recovery.retry_at.unwrap() - before;
            assert_eq!(delay.as_millis(), (5u128 << i.min(5)).min(100));
            tokio::time::advance(delay).await;
        }
        assert_eq!(recovery.resource_pressure, 100_000);
        assert_eq!(recovery.retries, 100_000);
        assert_eq!(recovery.backoff_high_water_ms, 100);
        recovery.accepted();
        assert_eq!(recovery.consecutive, 0);
        assert!(recovery.retry_at.is_none());
        recovery.errors = u64::MAX;
        recovery.consecutive = u64::MAX;
        assert!(recovery.failed(&io::Error::from_raw_os_error(libc::ECONNABORTED)));
        assert_eq!(recovery.errors, u64::MAX);
        assert_eq!(recovery.consecutive, u64::MAX);
        for code in [libc::EBADF, libc::EINVAL, libc::ENOTSOCK, libc::EOPNOTSUPP] {
            assert!(!recovery.failed(&io::Error::from_raw_os_error(code)));
        }
        for code in [
            libc::EINTR,
            libc::EAGAIN,
            libc::EPROTO,
            libc::ENFILE,
            libc::ENOBUFS,
            libc::ENOMEM,
            libc::EACCES,
        ] {
            assert!(recovery.failed(&io::Error::from_raw_os_error(code)));
        }
    }
}
