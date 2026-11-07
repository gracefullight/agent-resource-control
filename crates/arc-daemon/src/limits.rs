//! Validated resident resource limits. Test profiles cannot enable capabilities.

use std::{io, num::NonZeroUsize, time::Duration};

#[derive(Clone, Debug)]
pub(crate) struct RuntimeLimits {
    pub connections: NonZeroUsize,
    pub requests: NonZeroUsize,
    pub db_queue: NonZeroUsize,
    pub header_timeout: Duration,
    pub frame_timeout: Duration,
    pub write_timeout: Duration,
    pub db_response_timeout: Duration,
    pub db_execution_timeout: Duration,
    pub db_busy_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            connections: NonZeroUsize::new(64).unwrap(),
            requests: NonZeroUsize::new(16).unwrap(),
            db_queue: NonZeroUsize::new(8).unwrap(),
            header_timeout: Duration::from_millis(1_000),
            frame_timeout: Duration::from_millis(2_000),
            write_timeout: Duration::from_millis(500),
            db_response_timeout: Duration::from_millis(1_500),
            db_execution_timeout: Duration::from_millis(1_000),
            db_busy_timeout: Duration::from_millis(100),
            shutdown_timeout: Duration::from_millis(3_500),
        }
    }
}

impl RuntimeLimits {
    pub(crate) fn test_profile() -> Self {
        Self {
            connections: NonZeroUsize::new(8).unwrap(),
            requests: NonZeroUsize::new(4).unwrap(),
            db_queue: NonZeroUsize::new(2).unwrap(),
            header_timeout: Duration::from_millis(150),
            frame_timeout: Duration::from_millis(300),
            write_timeout: Duration::from_millis(100),
            db_response_timeout: Duration::from_millis(500),
            db_execution_timeout: Duration::from_millis(300),
            db_busy_timeout: Duration::from_millis(50),
            ..Self::default()
        }
    }

    /// Parse explicitly; callers can test this without mutating process state.
    pub(crate) fn from_profile(profile: Option<&std::ffi::OsStr>) -> io::Result<Self> {
        let limits = match profile {
            None => Self::default(),
            Some(value) if value.is_empty() => Self::default(),
            Some(value) if value == "test" => Self::test_profile(),
            Some(_) => return Err(invalid()),
        };
        limits.validate()?;
        Ok(limits)
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        let max = tokio::sync::Semaphore::MAX_PERMITS;
        if [self.connections, self.requests, self.db_queue]
            .iter()
            .any(|cap| cap.get() > max)
            || self.requests > self.connections
            || self.db_queue.get().checked_add(1).is_none()
            || self
                .requests
                .get()
                .checked_mul(arc_ipc::MAX_FRAME_BYTES as usize)
                .is_none()
        {
            return Err(invalid());
        }
        let budgets = [
            self.header_timeout,
            self.frame_timeout,
            self.write_timeout,
            self.db_response_timeout,
            self.db_execution_timeout,
            self.db_busy_timeout,
            self.shutdown_timeout,
        ];
        if budgets.iter().any(Duration::is_zero)
            || self.header_timeout > self.frame_timeout
            || self.db_execution_timeout > self.db_response_timeout
            || self.db_busy_timeout > self.db_execution_timeout
            || self.shutdown_timeout > Duration::from_millis(3_500)
        {
            return Err(invalid());
        }
        match self
            .frame_timeout
            .checked_add(self.db_response_timeout)
            .and_then(|sum| sum.checked_add(self.write_timeout))
        {
            Some(sum) if sum <= Duration::from_secs(4) => Ok(()),
            _ => Err(invalid()),
        }
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid ARC IPC limits or profile",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_reject_invalid_config_and_profiles() {
        for profile in [
            None,
            Some(std::ffi::OsStr::new("")),
            Some(std::ffi::OsStr::new("test")),
        ] {
            RuntimeLimits::from_profile(profile)
                .unwrap()
                .validate()
                .unwrap();
        }
        assert!(RuntimeLimits::from_profile(Some(std::ffi::OsStr::new("unbounded"))).is_err());
        for limits in [
            RuntimeLimits {
                requests: NonZeroUsize::new(65).unwrap(),
                ..RuntimeLimits::default()
            },
            RuntimeLimits {
                header_timeout: Duration::ZERO,
                ..RuntimeLimits::default()
            },
            RuntimeLimits {
                frame_timeout: Duration::MAX,
                ..RuntimeLimits::default()
            },
            RuntimeLimits {
                db_busy_timeout: Duration::from_secs(2),
                ..RuntimeLimits::default()
            },
            RuntimeLimits {
                connections: NonZeroUsize::new(usize::MAX).unwrap(),
                ..RuntimeLimits::default()
            },
        ] {
            assert!(limits.validate().is_err());
        }
    }
}
