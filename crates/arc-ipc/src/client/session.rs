use super::*;

/// A single authenticated connection with no reconnect or request replay.
/// Used only around the CLI's one native spawn and its acknowledgement.
pub struct Session {
    stream: UnixStream,
    deadline: Instant,
}
impl Session {
    pub fn connect_until(
        socket: &Path,
        deadline: Instant,
        cancelled: &dyn Fn() -> bool,
        continuity: Option<&Continuity>,
    ) -> Result<Self, Failure> {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let stream = DeadlineStream::connect(socket, deadline, cancelled).map_err(|e| match e {
            ConnectFailure::Local => Failure::Local,
            ConnectFailure::InvalidEndpoint => Failure::InvalidEndpoint,
            ConnectFailure::Endpoint => Failure::Unavailable,
        })?;
        if let Some(continuity) = continuity {
            continuity.record(stream.peer_identity.clone());
        }
        Ok(Self {
            stream: stream.stream,
            deadline,
        })
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn shorten_deadline(&mut self, deadline: Instant) {
        self.deadline = self.deadline.min(deadline);
    }
    pub fn raw_fd(&self) -> std::os::fd::RawFd {
        self.stream.as_raw_fd()
    }
    pub fn exchange(
        &mut self,
        request: &Request,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Value, Failure> {
        let stream = self.stream.try_clone().map_err(|_| Failure::Local)?;
        let mut stream = DeadlineStream {
            stream,
            deadline: self.deadline,
            cancelled,
            sent: false,
            peer_identity: None,
        };
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::Uncertain);
        }
        let response = crate::exchange(&mut stream, request).map_err(|e| match e {
            crate::IpcError::Json(_) | crate::IpcError::NotAnObject | crate::IpcError::Frame(_) => {
                Failure::InvalidReply
            }
            _ => {
                if cancelled() {
                    Failure::Cancelled
                } else {
                    Failure::Uncertain
                }
            }
        })?;
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::Uncertain);
        }
        if response.protocol_version != PROTOCOL_VERSION {
            return Err(Failure::Protocol);
        }
        if response.request_id != request.request_id {
            return Err(Failure::InvalidReply);
        }
        match (response.status.as_str(), response.result, response.error) {
            (STATUS_OK, Some(value), None) => Ok(value),
            (crate::STATUS_ERROR, None, Some(error)) => Err(match error.code.as_str() {
                crate::ERROR_UNSUPPORTED_CAPABILITY => Failure::Unsupported,
                crate::ERROR_PROTOCOL_INCOMPATIBLE => Failure::Protocol,
                crate::ERROR_DB_BUSY => Failure::Busy,
                crate::policy::ERROR_POLICY_UNAVAILABLE => Failure::Unavailable,
                "" => Failure::InvalidReply,
                _ => Failure::Rejected,
            }),
            _ => Err(Failure::InvalidReply),
        }
    }
}
