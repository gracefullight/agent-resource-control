//! Independently executed, bounded protocol-1 old-client fixture, not an old
//! arcctl binary. It models the accepted Cargo enqueue, inherited or fresh
//! claim, poll, witness, attach and completion path for this test only.
//! Accepted commit 16772fe2aa350413225f3fb2bb9227d14ae5a6c4:
//! exec/traffic.rs sha256 905bbfcaedc4a51d2b1acaa823e9c3600544e30f80fbdad397e64d604225ac2b
//! exec/witness.rs sha256 720f1ba33a6a53c1e95e401f8b37c6e05eefb3bad5063f0737306c9f1a090b9a
//! arc-ipc/src/traffic.rs sha256 ceb3f22ee284b65a834de2a2df4d9e67f3e93cdbf8ddb1deb8772ba00cfa3530
use arc_core::process_identity::{random_job_id, ProcessIdentity};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileExt, OpenOptionsExt},
            net::UnixStream,
            process::CommandExt,
        },
    },
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn exchange(socket: &Path, operation: &str, payload: Value) -> Value {
    let mut request = arc_ipc::Request::new("frozen-v1", operation);
    request.payload = payload;
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let response = arc_ipc::exchange(&mut stream, &request).unwrap();
    assert_eq!(response.protocol_version, 1);
    assert_eq!(response.request_id, "frozen-v1");
    assert_eq!(response.status, arc_ipc::STATUS_OK, "{response:?}");
    response.result.unwrap()
}

fn expect_keys(value: &Value, expected: &[&str]) {
    let mut actual: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "unexpected protocol-1 reply shape");
}

pub fn run() {
    let socket = arc_core::socket_path().unwrap();
    let probe = exchange(&socket, "traffic.capabilities", Value::Null);
    expect_keys(&probe, &["capability"]);
    assert_eq!(probe["capability"], "traffic.v1");
    let owner = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = random_job_id().unwrap();
    let inherited = std::env::var("ARC_TRAFFIC_LINEAGE").ok().filter(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    });
    let claim = inherited
        .clone()
        .unwrap_or_else(|| format!("{}{}", random_job_id().unwrap(), random_job_id().unwrap()));

    let path = socket
        .parent()
        .unwrap()
        .join(format!(".traffic-witness-{id}"));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let mut initial = [0u8; 136];
    initial[..8].copy_from_slice(b"ARCWIT03");
    initial[8..16].copy_from_slice(&(owner.pid as u64).to_be_bytes());
    initial[16..24].copy_from_slice(&owner.start_token.parse::<u64>().unwrap().to_be_bytes());
    initial[40..104].copy_from_slice(claim.as_bytes());
    initial[104..].copy_from_slice(id.as_bytes());
    file.write_all_at(&initial, 0).unwrap();
    file.sync_all().unwrap();
    File::open(socket.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();

    let enqueue = json!({"action":"ENQUEUE","job_id":id,"observer":owner,
        "classification":{"classification":"TEST","locality":"HOST_LOCAL","reason":"CARGO_TEST","uncertainty":"NONE"},
        "lineage_context":claim});
    if std::env::var_os("FROZEN_DROP_ENQUEUE_REPLY").is_some() {
        let mut request = arc_ipc::Request::new("frozen-lost", "traffic.job");
        request.payload = enqueue.clone();
        let mut stream = UnixStream::connect(&socket).unwrap();
        arc_ipc::write_message(&mut stream, &request).unwrap();
        drop(stream);
    }
    let mut result = exchange(&socket, "traffic.job", enqueue.clone());
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        match result["state"].as_str().unwrap() {
            "REUSED" => {
                expect_keys(&result, &["state", "parent_job_id", "permit_generation"]);
                break;
            }
            "QUEUED" => {
                assert!(
                    Instant::now() < deadline,
                    "frozen old client remained queued"
                );
                thread::sleep(Duration::from_millis(40));
                result = exchange(
                    &socket,
                    "traffic.job",
                    if inherited.is_some() {
                        enqueue.clone()
                    } else {
                        json!({"action":"POLL","job_id":id,"observer":owner})
                    },
                );
            }
            other => panic!("unexpected frozen-client grant: {other}"),
        }
    }
    let generation = result["permit_generation"].as_u64().unwrap();
    let fd = file.as_raw_fd();
    let mut command = Command::new(std::env::var_os("FROZEN_CARGO").unwrap());
    command
        .args(["test", "--offline"])
        .current_dir(std::env::var_os("FROZEN_CARGO_DIR").unwrap())
        .env("ARC_TRAFFIC_LINEAGE", &claim)
        .env_remove("ARC_FIXTURE_FROZEN_TRAFFIC_V1");
    // The accepted client writes the native identity before exec while holding
    // the witness lock. This keeps lost-ack recovery bound to the old claim.
    unsafe {
        command.pre_exec(move || {
            let mut info: libc::proc_bsdinfo = std::mem::zeroed();
            let size = std::mem::size_of_val(&info) as libc::c_int;
            if libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            ) != size
            {
                return Err(io::Error::last_os_error());
            }
            let start = info
                .pbi_start_tvsec
                .checked_mul(1_000_000)
                .and_then(|v| v.checked_add(info.pbi_start_tvusec))
                .ok_or_else(|| io::Error::other("invalid start time"))?;
            let mut bytes = initial;
            bytes[24..32].copy_from_slice(&(libc::getpid() as u64).to_be_bytes());
            bytes[32..40].copy_from_slice(&start.to_be_bytes());
            if libc::pwrite(fd, bytes.as_ptr().cast(), bytes.len(), 0) != bytes.len() as isize
                || libc::fsync(fd) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let identity = ProcessIdentity::capture(child.id()).unwrap();
    let attach = exchange(
        &socket,
        "traffic.job",
        json!({"action":"ATTACH","job_id":id,"observer":owner,"child":identity,"permit_generation":generation}),
    );
    expect_keys(&attach, &["state"]);
    assert_eq!(attach["state"], "RUNNING");
    let status = child.wait().unwrap();
    assert!(status.success());
    let complete = exchange(
        &socket,
        "traffic.job",
        json!({"action":"COMPLETE","job_id":id,"observer":owner,"permit_generation":generation,"outcome":{"kind":"EXITED","code":0}}),
    );
    expect_keys(&complete, &["state", "queue_wait_ms", "outcome"]);
    assert_eq!(complete["state"], "COMPLETED");
    fs::remove_file(path).unwrap();
}
