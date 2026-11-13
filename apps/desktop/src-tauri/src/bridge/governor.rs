//! Governor mutations retain the shared bridge permit through the committed reply.
use super::{DesktopBridge, Endpoint, READ_DEADLINE};
use crate::{
    dto::Received,
    error::{DesktopError, ErrorCode},
};
use arc_ipc::{governor::*, Request};
use serde_json::{json, Value};
use std::time::Instant;

impl DesktopBridge {
    pub fn governor_get_switch(
        &self,
        query: GetGovernorSwitchRequest,
    ) -> Result<Received<GovernorSwitchReadResult>, DesktopError> {
        decode_get_switch(&json!(query))
            .map_err(|_| DesktopError::new(ErrorCode::InvalidRequest))?;
        self.sync_read(|bridge| bridge.read_governor(query))
    }
    pub async fn governor_get_switch_async(
        &self,
        query: GetGovernorSwitchRequest,
    ) -> Result<Received<GovernorSwitchReadResult>, DesktopError> {
        decode_get_switch(&json!(query))
            .map_err(|_| DesktopError::new(ErrorCode::InvalidRequest))?;
        self.blocking(move |bridge| bridge.read_governor(query))
            .await
    }
    pub fn governor_set_switch(
        &self,
        request: SetGovernorSwitchRequest,
    ) -> Result<Received<GovernorMutationResult>, DesktopError> {
        if !request.is_valid() {
            return Err(DesktopError::new(ErrorCode::InvalidRequest));
        }
        self.sync_read(|bridge| bridge.write_governor(request))
            .map_err(pre_dispatch)
    }
    pub async fn governor_set_switch_async(
        &self,
        request: SetGovernorSwitchRequest,
    ) -> Result<Received<GovernorMutationResult>, DesktopError> {
        if !request.is_valid() {
            return Err(DesktopError::new(ErrorCode::InvalidRequest));
        }
        self.blocking(move |bridge| bridge.write_governor(request))
            .await
            .map_err(pre_dispatch)
    }
    fn read_governor(
        &self,
        query: GetGovernorSwitchRequest,
    ) -> Result<Received<GovernorSwitchReadResult>, DesktopError> {
        let value = self.governor_exchange(
            OPERATION_GET_SWITCH,
            json!(query),
            Instant::now() + READ_DEADLINE,
            false,
        )?;
        let result = decode_switch_read_result(&value, &query)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        self.received(result)
    }
    fn write_governor(
        &self,
        request: SetGovernorSwitchRequest,
    ) -> Result<Received<GovernorMutationResult>, DesktopError> {
        let value = self.governor_exchange(
            OPERATION_SET_SWITCH,
            json!(request),
            Instant::now() + READ_DEADLINE,
            true,
        )?;
        let result = decode_mutation_result(&value, &request)
            .map_err(|_| DesktopError::new(ErrorCode::Uncertain))?;
        // Even a local clock/shutdown failure after COMMIT is ambiguous to JS.
        self.received(result)
            .map_err(|_| DesktopError::new(ErrorCode::Uncertain))
    }
    fn governor_exchange(
        &self,
        operation: &'static str,
        payload: Value,
        deadline: Instant,
        mutation: bool,
    ) -> Result<Value, DesktopError> {
        #[cfg(unix)]
        {
            use arc_ipc::client::Session;
            let socket = match &self.0.endpoint {
                Endpoint::System => {
                    arc_core::socket_path().map_err(|_| DesktopError::new(ErrorCode::Local))?
                }
                Endpoint::Socket(socket) => socket.clone(),
            };
            // A Session authenticates the owner and never replays a sent write.
            let mut session =
                Session::connect_until(&socket, deadline, &|| self.is_shutdown(), None).map_err(
                    |error| {
                        if mutation {
                            DesktopError::new(ErrorCode::Rejected)
                        } else {
                            error.into()
                        }
                    },
                )?;
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let mut request = Request::new(
                format!(
                    "desktop-governor-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ),
                operation,
            );
            request.payload = payload;
            session
                .exchange(&request, &|| self.is_shutdown())
                .map_err(|error| {
                    if mutation {
                        DesktopError::new(ErrorCode::Uncertain)
                    } else {
                        error.into()
                    }
                })
        }
        #[cfg(not(unix))]
        {
            let _ = (operation, payload, deadline, mutation);
            Err(DesktopError::new(ErrorCode::Unsupported))
        }
    }
}
// These errors originate before the blocking closure acquired its permit.
// A Busy/Cancelled socket result inside a mutation is already Uncertain.
fn pre_dispatch(error: DesktopError) -> DesktopError {
    if matches!(error.code, ErrorCode::Busy | ErrorCode::Cancelled) {
        DesktopError::new(ErrorCode::Rejected)
    } else {
        error
    }
}
