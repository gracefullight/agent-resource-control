//! Preserve access metadata on the replacement file before atomic rename.
use std::{
    fs::File,
    io,
    os::unix::{fs::MetadataExt, io::AsRawFd},
};

#[cfg(target_os = "macos")]
fn copy_metadata(source: &File, destination: &File) -> io::Result<()> {
    // SAFETY: both descriptors remain open, and a null state is supported by
    // fcopyfile. METADATA includes stat, ACL and extended attributes, not data.
    let result = unsafe {
        libc::fcopyfile(
            source.as_raw_fd(),
            destination.as_raw_fd(),
            std::ptr::null_mut(),
            libc::COPYFILE_METADATA,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn copy_metadata(source: &File, destination: &File) -> io::Result<()> {
    use std::ffi::CStr;

    let metadata = source.metadata()?;
    let current = destination.metadata()?;
    if (current.uid(), current.gid()) != (metadata.uid(), metadata.gid()) {
        // SAFETY: the destination descriptor is open and owned by this call.
        if unsafe { libc::fchown(destination.as_raw_fd(), metadata.uid(), metadata.gid()) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    destination.set_permissions(metadata.permissions())?;

    // Linux represents POSIX ACLs as extended attributes. Copy all attributes
    // or fail without replacing the original configuration.
    let source_fd = source.as_raw_fd();
    let destination_fd = destination.as_raw_fd();
    // SAFETY: a null buffer with size zero queries the required list size.
    let size = unsafe { libc::flistxattr(source_fd, std::ptr::null_mut(), 0) };
    if size < 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ENOTSUP) {
            Ok(())
        } else {
            Err(error)
        };
    }
    let mut names = vec![0_u8; size as usize];
    // SAFETY: names has exactly the writable capacity provided to the syscall.
    let length = unsafe { libc::flistxattr(source_fd, names.as_mut_ptr().cast(), names.len()) };
    if length < 0 {
        return Err(io::Error::last_os_error());
    }
    names.truncate(length as usize);
    for name in names.split_inclusive(|byte| *byte == 0) {
        let name = CStr::from_bytes_with_nul(name).map_err(io::Error::other)?;
        // SAFETY: a null value with size zero queries the attribute size.
        let size = unsafe { libc::fgetxattr(source_fd, name.as_ptr(), std::ptr::null_mut(), 0) };
        if size < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut value = vec![0_u8; size as usize];
        // SAFETY: value has the writable capacity supplied to the syscall.
        let length = unsafe {
            libc::fgetxattr(
                source_fd,
                name.as_ptr(),
                value.as_mut_ptr().cast(),
                value.len(),
            )
        };
        if length < 0 {
            return Err(io::Error::last_os_error());
        }
        value.truncate(length as usize);
        // SAFETY: the attribute name and value remain live through the call.
        if unsafe {
            libc::fsetxattr(
                destination_fd,
                name.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn copy_metadata(source: &File, destination: &File) -> io::Result<()> {
    let metadata = source.metadata()?;
    let current = destination.metadata()?;
    if (current.uid(), current.gid()) != (metadata.uid(), metadata.gid()) {
        // SAFETY: the destination descriptor is open and owned by this call.
        if unsafe { libc::fchown(destination.as_raw_fd(), metadata.uid(), metadata.gid()) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    destination.set_permissions(metadata.permissions())
}

pub(super) fn preserve(source: &File, destination: &File) -> io::Result<()> {
    copy_metadata(source, destination)?;
    let source = source.metadata()?;
    let destination = destination.metadata()?;
    if (source.uid(), source.gid(), source.mode() & 0o7777)
        != (
            destination.uid(),
            destination.gid(),
            destination.mode() & 0o7777,
        )
    {
        return Err(io::Error::other(
            "owner, group or mode could not be preserved",
        ));
    }
    Ok(())
}
