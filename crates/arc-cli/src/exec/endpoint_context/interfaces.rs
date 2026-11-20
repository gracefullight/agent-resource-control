//! A fresh, complete local interface inventory; never resolve names over DNS.
use arc_detector::{EndpointContext, EndpointFact, EndpointKind};
use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ptr,
};

pub(super) fn capture(context: &mut EndpointContext) -> Option<()> {
    if context
        .facts
        .iter()
        .any(|f| f.kind == EndpointKind::SshDestination && f.selector == "@local-addresses")
    {
        return Some(());
    }
    let mut head: *mut libc::ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return None;
    }
    struct Guard(*mut libc::ifaddrs);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { libc::freeifaddrs(self.0) }
        }
    }
    let _guard = Guard(head);
    let mut current = head;
    let mut seen = BTreeSet::new();
    let mut count = 0;
    while !current.is_null() {
        count += 1;
        if count > 256 {
            return None;
        }
        let entry = unsafe { &*current };
        if !entry.ifa_addr.is_null() {
            let address = unsafe { &*entry.ifa_addr };
            let ip = match address.sa_family as i32 {
                libc::AF_INET => {
                    let addr = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in) };
                    Some(IpAddr::V4(Ipv4Addr::from(
                        addr.sin_addr.s_addr.to_ne_bytes(),
                    )))
                }
                libc::AF_INET6 => {
                    let addr = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in6) };
                    Some(IpAddr::V6(Ipv6Addr::from(addr.sin6_addr.s6_addr)))
                }
                _ => None,
            };
            if let Some(ip) = ip {
                if ip.is_unspecified() {
                    return None;
                }
                seen.insert(ip.to_string());
            }
        }
        current = entry.ifa_next;
    }
    if seen.is_empty() {
        return None;
    }
    context.facts.push(EndpointFact {
        kind: EndpointKind::SshDestination,
        selector: "@local-addresses".into(),
        endpoints: seen.into_iter().collect(),
    });
    context.is_valid_at(context.captured_at_ms).then_some(())
}
