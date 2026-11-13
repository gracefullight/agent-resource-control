use super::*;

#[test]
fn reserved_release_ack_is_generation_bound_and_cannot_ack_a_hold() {
    let request = GovernorLaunchRequest::Reserved {
        request: launch::LaunchRequest {
            job_id: id(),
            observer: observer(),
            permit_generation: 7,
        },
    };
    let result = GovernorLaunchResult::Reserved {
        result: Box::new(launch::LaunchResult::Reused {
            permit_generation: 7,
        }),
    };
    assert!(decode_launch_result(&json!(result)).is_ok());
    let ack = GovernorLaunchAck::Reserved {
        ack: launch::LaunchAck::Started {
            job_id: id(),
            observer: observer(),
            permit_generation: 7,
            child: ProcessIdentity {
                pid: 43,
                ..observer()
            },
        },
    };
    assert!(decode_launch_ack(&json!(ack), &request, &result).is_ok());
    let wrong = GovernorLaunchResult::Reserved {
        result: Box::new(launch::LaunchResult::Reused {
            permit_generation: 8,
        }),
    };
    assert!(!ack.matches(&request, &wrong));
    let held = GovernorLaunchResult::Reserved {
        result: Box::new(launch::LaunchResult::Cancelled {}),
    };
    assert!(!ack.matches(&request, &held));
    let invalid = GovernorLaunchRequest::Unreserved {
        job_id: id(),
        observer: ProcessIdentity {
            pid: 0,
            ..observer()
        },
    };
    assert!(decode_launch(&json!(invalid)).is_err());
    assert!(!ack.matches(&invalid, &result));
    for request in [
        GovernorJobRequest::Attach {
            job_id: id(),
            observer: observer(),
            release_id: "bad".into(),
            child: observer(),
        },
        GovernorJobRequest::Complete {
            job_id: id(),
            observer: ProcessIdentity {
                pid: 0,
                ..observer()
            },
            release_id: "a".repeat(32),
            outcome: arc_ipc::jobs::Outcome::Exited { code: 0 },
        },
    ] {
        assert!(decode_job(&json!(request)).is_err());
    }
}
