//! Deterministic exchange decisions, not authentication or native-process proof.
use super::*;
use std::io::Cursor;

struct ScriptedReply {
    input: Cursor<Vec<u8>>,
    sent: bool,
}
impl Read for ScriptedReply {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.input.read(bytes)
    }
}
impl Write for ScriptedReply {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.sent = true;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl TrackedStream for ScriptedReply {
    fn sent(&self) -> bool {
        self.sent
    }
}

#[test]
fn meter_failures_are_scoped_without_changing_other_operations() {
    for (operation, code, expected) in [
        (
            crate::meter::OPERATION_METER_SNAPSHOT,
            crate::ERROR_UNSUPPORTED_CAPABILITY,
            Failure::Unsupported,
        ),
        (
            crate::OPERATION_HEALTH,
            crate::ERROR_UNSUPPORTED_CAPABILITY,
            Failure::Rejected,
        ),
        (
            crate::meter::OPERATION_METER_SNAPSHOT,
            crate::meter::ERROR_METER_UNAVAILABLE,
            Failure::Unavailable,
        ),
        (
            crate::OPERATION_HEALTH,
            crate::meter::ERROR_METER_UNAVAILABLE,
            Failure::Rejected,
        ),
    ] {
        let request = Request::new("unsupported-1", operation);
        let response = crate::Response::error("unsupported-1", code);
        let mut input = Vec::new();
        crate::write_response(&mut input, &response).unwrap();
        let mut reply = Some(ScriptedReply {
            input: Cursor::new(input),
            sent: false,
        });
        let result = exchange_with(
            &request,
            Instant::now() + Duration::from_secs(1),
            &|| false,
            &mut false,
            ExchangeOptions {
                continuity: None,
                retry_busy: false,
            },
            |_| {
                Ok(reply
                    .take()
                    .expect("an explicit terminal Meter reply must not retry"))
            },
        );
        assert_eq!(result, Err(expected));
    }
}

#[test]
fn typed_meter_read_honors_cancellation_and_expired_absolute_deadline() {
    let socket = Path::new("/unused-meter-contract-test.sock");
    assert_eq!(
        crate::meter::snapshot_until(
            socket,
            "cancelled",
            Instant::now() + Duration::from_secs(1),
            &|| true
        ),
        Err(Failure::Cancelled)
    );
    assert_eq!(
        crate::meter::snapshot_until(socket, "expired", Instant::now(), &|| false),
        Err(Failure::Unavailable)
    );
}
