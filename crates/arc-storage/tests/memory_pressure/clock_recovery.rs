use super::{admission::*, observation::*};
use arc_core::meter::MemoryPressure;
use arc_ipc::{jobs::Outcome, traffic::JobRequest};
use arc_policy::{DecisionKind, PolicyMode, PolicyReason};
use arc_storage::jobs::{self, traffic::*};
use arc_traffic::Policy;

#[test]
fn queued_admission_and_cancellation_tolerate_backward_or_missing_utc() {
    for utc in [999, 0] {
        for stale in [false, true] {
            let mut c = database();
            let p = Provider::new(MemoryPressure::Critical);
            add(&mut c, 1, &p);
            add(&mut c, 2, &p);
            if stale {
                p.0.lock().unwrap().time(4001);
                assert_eq!(p.policy().reason, PolicyReason::ObservationStale);
            } else {
                p.set(MemoryPressure::Normal);
                assert_eq!(p.policy().reason, PolicyReason::PressureNormal);
            }
            assert_eq!(p.policy().mode, PolicyMode::L0);
            let reply = poll_observed(
                &mut c,
                &JobRequest::Poll {
                    job_id: id(1),
                    observer: owner(),
                },
                &OwnerEvidence::gather(&owner()),
                &Policy::default(),
                &context(1),
                None,
                utc,
                &p,
            )
            .unwrap();
            let generation = generation(reply);
            let job = jobs::get(&c, &id(1)).unwrap().unwrap();
            assert!(job.is_valid());
            assert_eq!(job.updated_at_ms, 1000);
            assert_eq!(job.launch_claimed_at_ms, Some(1000));
            assert!(matches!(
                cancel_observed(
                    &mut c,
                    &JobRequest::Cancel {
                        job_id: id(2),
                        observer: owner(),
                        permit_generation: None
                    },
                    &OwnerEvidence::gather(&owner()),
                    utc,
                    &p,
                )
                .unwrap(),
                arc_ipc::traffic::JobResult::Cancelled {}
            ));
            let read = arc_storage::policy::job(&c, &id(2), p.policy(), 1000).unwrap();
            let decision = read.last_decision.unwrap();
            assert_eq!(decision.kind, DecisionKind::Cancel);
            assert_eq!(decision.decided_at_ms, 1000);
            assert_eq!(decision.observation, p.policy().observation);
            complete(
                &mut c,
                &JobRequest::Complete {
                    job_id: id(1),
                    observer: owner(),
                    permit_generation: generation,
                    outcome: Outcome::Exited { code: 17 },
                },
                utc,
            )
            .unwrap();
            assert_eq!(counts(&c), (1, 0));
            let job = jobs::get(&c, &id(1)).unwrap().unwrap();
            assert_eq!(job.outcome, Some(Outcome::Exited { code: 17 }));
            assert!(job.is_valid());
            assert_eq!(job.completed_at_ms, Some(1000));
        }
    }
}
