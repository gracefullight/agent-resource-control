//! Opt-in, per-invocation observation of the cache's result branch.
//!
//! An inherited connected Unix datagram socket carries only the branch and
//! the four classification fields. No path, script, cache key or other input
//! is sent. With no explicitly supplied descriptor this code does nothing.
use arc_detector::Classification;
use std::mem::{size_of, zeroed};

const ENV_FD: &str = "ARC_TEST_ANALYSIS_OBSERVER_FD";
const MAX_MESSAGE: usize = 512;

pub(super) fn report(branch: &'static str, classification: &Classification) {
    let Some(fd) = std::env::var(ENV_FD)
        .ok()
        .and_then(|value| value.parse::<libc::c_int>().ok())
        .filter(|fd| *fd > 2)
    else {
        return;
    };
    let mut kind: libc::c_int = 0;
    let mut kind_len = size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: both output buffers are initialized and their lengths match.
    let is_datagram = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut libc::c_int).cast(),
            &mut kind_len,
        ) == 0
            && kind == libc::SOCK_DGRAM
    };
    // SAFETY: zero is a valid initial byte representation for this C buffer.
    let mut address: libc::sockaddr_storage = unsafe { zeroed() };
    let mut address_len = size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    // SAFETY: address is writable for address_len bytes.
    let is_unix = unsafe {
        libc::getsockname(
            fd,
            (&mut address as *mut libc::sockaddr_storage).cast(),
            &mut address_len,
        ) == 0
            && i32::from(address.ss_family) == libc::AF_UNIX
    };
    if !is_datagram || !is_unix {
        return;
    }
    let Ok(bytes) = serde_json::to_vec(&serde_json::json!({
        "branch": branch,
        "result": classification,
    })) else {
        return;
    };
    if bytes.len() <= MAX_MESSAGE {
        // SAFETY: bytes remains valid for its full length; MSG_DONTWAIT
        // prevents a full or unresponsive peer from delaying classification.
        let _ = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_DONTWAIT) };
    }
}
