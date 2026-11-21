//! Pass the accepted upstream socket so the daemon can authenticate a relay.
use arc_ipc::{Request, Response};
use std::{
    io::{self, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
};

pub fn exchange(target: &mut UnixStream, upstream: &UnixStream, request: &Request) -> Response {
    send(target, &[upstream], request);
    arc_ipc::read_message(target).unwrap()
}

pub fn send(target: &mut UnixStream, upstreams: &[&UnixStream], request: &Request) {
    assert!((1..=2).contains(&upstreams.len()));
    let body = serde_json::to_vec(request).unwrap();
    let mut frame = (body.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&body);
    let mut iov = libc::iovec {
        iov_base: frame.as_mut_ptr().cast(),
        iov_len: frame.len(),
    };
    let mut control = [0usize; 8];
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    let bytes = upstreams.len() * std::mem::size_of::<libc::c_int>();
    message.msg_controllen = unsafe { libc::CMSG_SPACE(bytes as _) } as _;
    let header = unsafe { libc::CMSG_FIRSTHDR(&message) };
    assert!(!header.is_null());
    unsafe {
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(bytes as _) as _;
        for (index, upstream) in upstreams.iter().enumerate() {
            *libc::CMSG_DATA(header).cast::<libc::c_int>().add(index) = upstream.as_raw_fd();
        }
    }
    let written = unsafe { libc::sendmsg(target.as_raw_fd(), &message, 0) };
    assert!(written > 0, "relay sendmsg: {}", io::Error::last_os_error());
    target.write_all(&frame[written as usize..]).unwrap();
}
