//! Task-2 owns fresh, bounded CI/remote-Git evidence capture.
use arc_detector::EndpointContext;
use std::ffi::OsString;

pub(super) fn extend(_: &[OsString], _: &mut EndpointContext) -> Option<()> {
    Some(())
}
