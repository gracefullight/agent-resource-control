//! Copy and verify access metadata before replacing a settings inode.
use std::{fs::File, io, os::unix::fs::MetadataExt};

pub(super) type Fingerprint = (Option<Vec<u8>>, Vec<(Vec<u8>, Vec<u8>)>);

#[cfg(target_os = "macos")]
pub(super) fn fingerprint(file: &File) -> io::Result<Fingerprint> {
    Ok((acl(file)?, xattrs(file)?))
}

#[cfg(target_os = "linux")]
pub(super) fn fingerprint(file: &File) -> io::Result<Fingerprint> {
    Ok((None, xattrs(file)?))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn fingerprint(_file: &File) -> io::Result<Fingerprint> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(target_os = "macos")]
pub(super) fn preserve(source: &File, destination: &File) -> io::Result<()> {
    use std::{ffi::c_void, os::fd::AsRawFd, ptr};
    unsafe extern "C" {
        fn fcopyfile(from: i32, to: i32, state: *mut c_void, flags: u32) -> i32;
    }
    // COPYFILE_METADATA = COPYFILE_ACL | COPYFILE_STAT | COPYFILE_XATTR.
    // SAFETY: both descriptors refer to open files, and a null state is allowed.
    if unsafe {
        fcopyfile(
            source.as_raw_fd(),
            destination.as_raw_fd(),
            ptr::null_mut(),
            7,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    verify_posix(source, destination)?;
    if acl(source)? != acl(destination)? {
        return Err(io::Error::other("Claude settings ACL was not preserved"));
    }
    if xattrs(source)? != xattrs(destination)? {
        return Err(io::Error::other(
            "Claude settings extended attributes were not preserved",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn xattrs(file: &File) -> io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    // SAFETY: null buffer with zero length asks for the list size.
    let length = unsafe { libc::flistxattr(fd, std::ptr::null_mut(), 0, 0) };
    if length < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut names = vec![0u8; length as usize];
    // SAFETY: the buffer is writable for its allocated length.
    let actual = unsafe { libc::flistxattr(fd, names.as_mut_ptr().cast(), names.len(), 0) };
    if actual < 0 || actual as usize != names.len() {
        return Err(io::Error::other("Claude settings attributes changed"));
    }
    let mut result = Vec::new();
    for name in names.split_inclusive(|byte| *byte == 0) {
        if name.last() != Some(&0) {
            return Err(io::Error::other("Invalid extended attribute name"));
        }
        // SAFETY: name is terminated and a null buffer requests the size.
        let length =
            unsafe { libc::fgetxattr(fd, name.as_ptr().cast(), std::ptr::null_mut(), 0, 0, 0) };
        if length < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut value = vec![0u8; length as usize];
        // SAFETY: the name and output buffer remain valid for this call.
        let actual = unsafe {
            libc::fgetxattr(
                fd,
                name.as_ptr().cast(),
                value.as_mut_ptr().cast(),
                value.len(),
                0,
                0,
            )
        };
        if actual < 0 || actual as usize != value.len() {
            return Err(io::Error::other("Claude settings attributes changed"));
        }
        result.push((name.to_vec(), value));
    }
    result.sort();
    Ok(result)
}

#[cfg(target_os = "macos")]
fn acl(file: &File) -> io::Result<Option<Vec<u8>>> {
    use std::{ffi::c_void, os::fd::AsRawFd, slice};
    unsafe extern "C" {
        fn acl_get_fd_np(fd: i32, kind: i32) -> *mut c_void;
        fn acl_to_text(acl: *mut c_void, len: *mut isize) -> *mut libc::c_char;
        fn acl_free(object: *mut c_void) -> i32;
    }
    // SAFETY: the descriptor is open; 0x100 is macOS ACL_TYPE_EXTENDED.
    let acl = unsafe { acl_get_fd_np(file.as_raw_fd(), 0x100) };
    if acl.is_null() {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ENOENT) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    let mut length = 0isize;
    // SAFETY: acl is a live ACL returned above; length is writable.
    let text = unsafe { acl_to_text(acl, &mut length) };
    let result = if text.is_null() || length < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: acl_to_text returned length readable bytes.
        Ok(Some(
            unsafe { slice::from_raw_parts(text.cast(), length as usize) }.to_vec(),
        ))
    };
    // SAFETY: both objects were returned by the ACL API and are freed once.
    unsafe {
        if !text.is_null() {
            acl_free(text.cast());
        }
        acl_free(acl);
    }
    result
}

#[cfg(target_os = "linux")]
pub(super) fn preserve(source: &File, destination: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let metadata = source.metadata()?;
    // SAFETY: the destination descriptor is open and uid/gid come from fstat.
    if unsafe { libc::fchown(destination.as_raw_fd(), metadata.uid(), metadata.gid()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    destination.set_permissions(metadata.permissions())?;
    for (name, value) in xattrs(source)? {
        // SAFETY: name is NUL terminated and value is valid for its length.
        if unsafe {
            libc::fsetxattr(
                destination.as_raw_fd(),
                name.as_ptr().cast(),
                value.as_ptr().cast(),
                value.len(),
                0,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    verify_posix(source, destination)?;
    if xattrs(source)? != xattrs(destination)? {
        return Err(io::Error::other(
            "Claude settings extended attributes were not preserved",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn xattrs(file: &File) -> io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    // SAFETY: null pointer with zero length requests the size only.
    let length = unsafe { libc::flistxattr(fd, std::ptr::null_mut(), 0) };
    if length < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut names = vec![0u8; length as usize];
    // SAFETY: names is writable for its allocated length.
    let actual = unsafe { libc::flistxattr(fd, names.as_mut_ptr().cast(), names.len()) };
    if actual < 0 || actual as usize != names.len() {
        return Err(io::Error::last_os_error());
    }
    let mut result = Vec::new();
    for name in names.split_inclusive(|byte| *byte == 0) {
        if name.last() != Some(&0) {
            return Err(io::Error::other("Invalid extended attribute name"));
        }
        // SAFETY: name is NUL terminated; null buffer requests the size.
        let length = unsafe { libc::fgetxattr(fd, name.as_ptr().cast(), std::ptr::null_mut(), 0) };
        if length < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut value = vec![0; length as usize];
        // SAFETY: name is NUL terminated and value is a writable buffer.
        let actual = unsafe {
            libc::fgetxattr(
                fd,
                name.as_ptr().cast(),
                value.as_mut_ptr().cast(),
                value.len(),
            )
        };
        if actual < 0 || actual as usize != value.len() {
            return Err(io::Error::last_os_error());
        }
        result.push((name.to_vec(), value));
    }
    result.sort();
    Ok(result)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn preserve(_source: &File, _destination: &File) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Cannot preserve Claude settings access metadata",
    ))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn verify_posix(source: &File, destination: &File) -> io::Result<()> {
    let original = source.metadata()?;
    let copied = destination.metadata()?;
    if original.uid() != copied.uid()
        || original.gid() != copied.gid()
        || original.mode() != copied.mode()
    {
        return Err(io::Error::other(
            "Claude settings ownership or permissions were not preserved",
        ));
    }
    Ok(())
}
