//! Holds a real protocol-1 reservation as an independently authenticated peer.
use arc_core::process_identity::{random_job_id, ProcessIdentity};
use arc_ipc::{
    jobs::JobId,
    traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_JOB},
};
use std::{fs, os::unix::net::UnixStream, thread, time::Duration};

fn send(request: &JobRequest) -> JobResult {
    let mut frame = arc_ipc::Request::new("independent-holder", OPERATION_TRAFFIC_JOB);
    frame.payload = serde_json::to_value(request).unwrap();
    let mut stream = UnixStream::connect(arc_core::socket_path().unwrap()).unwrap();
    let response = arc_ipc::exchange(&mut stream, &frame).unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK, "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

pub fn run() {
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let job_id = JobId::try_from(random_job_id().unwrap()).unwrap();
    let (class, reason) = match std::env::var("HOLDER_CLASS").unwrap().as_str() {
        "TEST" => ("TEST", "CARGO_TEST"),
        "BUILD" => ("BUILD", "CARGO_BUILD"),
        _ => panic!("unsupported fixture class"),
    };
    let classification = serde_json::from_value(serde_json::json!({"classification":class,"locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"})).unwrap();
    let grant = send(&JobRequest::Enqueue {
        job_id: job_id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: None,
    });
    let JobResult::Granted {
        permit_generation, ..
    } = grant
    else {
        panic!("expected independent holder grant")
    };
    fs::write(std::env::var_os("HOLDER_READY").unwrap(), job_id.as_str()).unwrap();
    let release = std::env::var_os("HOLDER_RELEASE").unwrap();
    while !std::path::Path::new(&release).exists() {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        send(&JobRequest::Cancel {
            job_id,
            observer,
            permit_generation: Some(permit_generation)
        }),
        JobResult::Cancelled {}
    );
}
