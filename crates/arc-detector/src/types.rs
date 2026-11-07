use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobClass {
    #[serde(rename = "BUILD")]
    Build,
    #[serde(rename = "TEST")]
    Test,
    #[serde(rename = "LINT")]
    Lint,
    #[serde(rename = "REMOTE_CI")]
    RemoteCi,
    #[serde(rename = "REMOTE_EXEC")]
    RemoteExec,
    #[serde(rename = "MIXED")]
    Mixed,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locality {
    #[serde(rename = "HOST_LOCAL")]
    HostLocal,
    #[serde(rename = "REMOTE")]
    Remote,
    #[serde(rename = "MIXED")]
    Mixed,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    #[serde(rename = "CARGO_BUILD")]
    CargoBuild,
    #[serde(rename = "CARGO_TEST")]
    CargoTest,
    #[serde(rename = "CARGO_CLIPPY")]
    CargoClippy,
    #[serde(rename = "CARGO_FMT")]
    CargoFmt,
    #[serde(rename = "GH_WORKFLOW_RUN")]
    GhWorkflowRun,
    #[serde(rename = "GH_RUN_WATCH")]
    GhRunWatch,
    #[serde(rename = "GH_RUN_VIEW")]
    GhRunView,
    #[serde(rename = "SSH_REMOTE_COMMAND")]
    SshRemoteCommand,
    #[serde(rename = "COMPOUND_LOCAL")]
    CompoundLocal,
    #[serde(rename = "COMPOUND_REMOTE")]
    CompoundRemote,
    #[serde(rename = "COMPOUND_MIXED")]
    CompoundMixed,
    #[serde(rename = "EMPTY_COMMAND")]
    EmptyCommand,
    #[serde(rename = "UNSUPPORTED_SHELL")]
    UnsupportedShell,
    #[serde(rename = "UNSUPPORTED_SYNTAX")]
    UnsupportedSyntax,
    #[serde(rename = "UNRESOLVED_COMMAND")]
    UnresolvedCommand,
    #[serde(rename = "UNRESOLVED_SCRIPT")]
    UnresolvedScript,
    #[serde(rename = "UNRESOLVED_EXPANSION")]
    UnresolvedExpansion,
    #[serde(rename = "UNRESOLVED_COMPOUND")]
    UnresolvedCompound,
    #[serde(rename = "SSH_FORWARDING")]
    SshForwarding,
    #[serde(rename = "SSH_AMBIGUOUS")]
    SshAmbiguous,
    #[serde(rename = "DOCKER_ENDPOINT_UNKNOWN")]
    DockerEndpointUnknown,
    #[serde(rename = "INPUT_LIMIT")]
    InputLimit,
    #[serde(rename = "TOKEN_LIMIT")]
    TokenLimit,
    #[serde(rename = "INVOCATION_LIMIT")]
    InvocationLimit,
    #[serde(rename = "NESTING_LIMIT")]
    NestingLimit,
    #[serde(rename = "WORK_LIMIT")]
    WorkLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Uncertainty {
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "OBSERVE")]
    Observe,
}

/// Input is data only. Unknown fields and explicit null shell kinds are invalid.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassifyRequest {
    pub command: String,
    #[serde(default = "default_shell")]
    pub shell_kind: String,
}
fn default_shell() -> String {
    "sh".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    pub classification: JobClass,
    pub locality: Locality,
    pub reason: Reason,
    pub uncertainty: Uncertainty,
}
impl Classification {
    pub(crate) fn new(classification: JobClass, reason: Reason) -> Self {
        let (locality, uncertainty) = match classification {
            JobClass::Build | JobClass::Test | JobClass::Lint => {
                (Locality::HostLocal, Uncertainty::None)
            }
            JobClass::RemoteCi | JobClass::RemoteExec => (Locality::Remote, Uncertainty::None),
            JobClass::Mixed => (Locality::Mixed, Uncertainty::None),
            JobClass::Unknown => (Locality::Unknown, Uncertainty::Observe),
        };
        Self {
            classification,
            locality,
            reason,
            uncertainty,
        }
    }
    pub(crate) fn unknown(reason: Reason) -> Self {
        Self::new(JobClass::Unknown, reason)
    }
    /// Validate the complete wire combination before displaying untrusted IPC data.
    pub fn is_consistent(&self) -> bool {
        use {JobClass as C, Reason as R};
        let reason_valid = match self.reason {
            R::CargoBuild => self.classification == C::Build,
            R::CargoTest => self.classification == C::Test,
            R::CargoClippy | R::CargoFmt => self.classification == C::Lint,
            R::GhWorkflowRun | R::GhRunWatch | R::GhRunView => self.classification == C::RemoteCi,
            R::SshRemoteCommand => self.classification == C::RemoteExec,
            R::CompoundLocal => matches!(self.classification, C::Build | C::Test | C::Lint),
            R::CompoundRemote => matches!(self.classification, C::RemoteCi | C::RemoteExec),
            R::CompoundMixed => self.classification == C::Mixed,
            _ => self.classification == C::Unknown,
        };
        reason_valid && *self == Self::new(self.classification, self.reason)
    }
}
