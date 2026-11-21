//! The compute location of one class and reason pair. `Classification::new`
//! is the only caller, so every recognizer and every wire check agree.
use crate::{JobClass, Locality, Reason};

pub(crate) fn of(classification: JobClass, reason: Reason) -> Locality {
    match classification {
        // A build whose engine or builder endpoint was proven to be on this
        // machine runs in a container here (SRS 8.5: local Docker engine,
        // local Buildx builder, `act`). It is local compute for admission.
        JobClass::Build if reason == Reason::LocalEndpointBuild => Locality::LocalContainer,
        JobClass::Build | JobClass::Test | JobClass::Lint | JobClass::Typecheck => {
            Locality::HostLocal
        }
        JobClass::RemoteCi | JobClass::RemoteExec => Locality::Remote,
        JobClass::Mixed => Locality::Mixed,
        JobClass::Unknown => Locality::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Classification, Uncertainty};

    #[test]
    fn only_a_local_endpoint_build_is_a_local_container() {
        let container = Classification::new(JobClass::Build, Reason::LocalEndpointBuild);
        assert_eq!(container.locality, Locality::LocalContainer);
        assert_eq!(container.uncertainty, Uncertainty::None);
        assert!(container.locality.is_local_compute());
        // Both wire forms of the record validate and name the same record.
        let projected = container.host_local_projection();
        assert_eq!(projected.locality, Locality::HostLocal);
        assert!(container.is_consistent() && projected.is_consistent());
        assert_eq!(projected.canonical(), container);
        assert_eq!(container.canonical(), container);
        // Protocol-1 has neither the reason nor the location.
        assert_eq!(container.legacy().locality, Locality::Unknown);

        for (class, reason, locality) in [
            (JobClass::Build, Reason::CargoBuild, Locality::HostLocal),
            (JobClass::Build, Reason::CompoundLocal, Locality::HostLocal),
            (JobClass::Test, Reason::CargoTest, Locality::HostLocal),
            (JobClass::Lint, Reason::CargoClippy, Locality::HostLocal),
            (
                JobClass::Typecheck,
                Reason::BunScriptTypecheck,
                Locality::HostLocal,
            ),
            (
                JobClass::RemoteExec,
                Reason::RemoteEndpointExec,
                Locality::Remote,
            ),
            (
                JobClass::RemoteCi,
                Reason::RemoteCiDispatch,
                Locality::Remote,
            ),
            (JobClass::Mixed, Reason::CompoundMixed, Locality::Mixed),
            (
                JobClass::Unknown,
                Reason::EndpointAmbiguous,
                Locality::Unknown,
            ),
        ] {
            assert_eq!(of(class, reason), locality, "{class:?} {reason:?}");
            let record = Classification::new(class, reason);
            assert_eq!(record.host_local_projection(), record);
            assert!(record.is_consistent(), "{record:?}");
        }
        // The reason alone never moves another class into a container.
        assert_eq!(
            of(JobClass::Unknown, Reason::LocalEndpointBuild),
            Locality::Unknown
        );
        // A container location on any other record is not a valid wire form.
        let mut forged = Classification::new(JobClass::Build, Reason::CargoBuild);
        forged.locality = Locality::LocalContainer;
        assert!(!forged.is_consistent());
    }
}
