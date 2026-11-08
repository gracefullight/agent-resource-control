//! Shared configuration for the user-local ARC database.

pub mod process_identity;

use std::{env, io, path::PathBuf};

/// Optional path override for a database on a user-local filesystem.
pub const ARC_DB_PATH: &str = "ARC_DB_PATH";

/// Optional path override for the daemon's owner-only Unix domain socket.
pub const ARC_SOCKET_PATH: &str = "ARC_SOCKET_PATH";

/// Resolve the native per-user runtime root, independently of TMPDIR on macOS.
pub fn runtime_root() -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: confstr accepts a null sizing query, or a writable buffer of
        // the supplied length. The returned bytes are validated below.
        darwin_runtime_root(|buffer, length| unsafe {
            libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buffer, length)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(env::temp_dir())
    }
}

#[cfg(target_os = "macos")]
fn darwin_runtime_root(
    mut confstr: impl FnMut(*mut libc::c_char, usize) -> usize,
) -> io::Result<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let invalid = || io::Error::other("confstr(_CS_DARWIN_USER_TEMP_DIR) returned an invalid path");
    let mut length = confstr(std::ptr::null_mut(), 0);
    for _ in 0..4 {
        if length == 0 {
            return Err(io::Error::other(format!(
                "confstr(_CS_DARWIN_USER_TEMP_DIR) failed: {}",
                io::Error::last_os_error()
            )));
        }
        if length > 1024 * 1024 {
            return Err(invalid());
        }
        let mut bytes = vec![0xff_u8; length];
        let returned = confstr(bytes.as_mut_ptr().cast(), bytes.len());
        if returned == 0 {
            return Err(io::Error::other(format!(
                "confstr(_CS_DARWIN_USER_TEMP_DIR) failed: {}",
                io::Error::last_os_error()
            )));
        }
        if returned > bytes.len() {
            length = returned;
            continue;
        }
        bytes.truncate(returned);
        if bytes.pop() != Some(0) || bytes.is_empty() || bytes.contains(&0) {
            return Err(invalid());
        }
        let path = PathBuf::from(OsString::from_vec(bytes));
        return path.is_absolute().then_some(path).ok_or_else(invalid);
    }
    Err(io::Error::other("confstr runtime path kept changing size"))
}

/// Stable admission anchor, independent of socket and database overrides.
#[cfg(unix)]
pub fn instance_anchor() -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let root = runtime_root()?;
    #[cfg(not(target_os = "macos"))]
    let root = PathBuf::from("/tmp");
    // SAFETY: geteuid has no preconditions or process-wide side effects.
    let uid = unsafe { libc::geteuid() };
    Ok(root.join(format!("arc-instance-{uid}")).join("daemon.lock"))
}

/// Resolve the override or `<runtime_root>/arc/arc.sock`.
pub fn socket_path() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os(ARC_SOCKET_PATH).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    Ok(runtime_root()?.join("arc").join("arc.sock"))
}

/// Resolve the override or the platform's user-local application data directory.
pub fn database_path() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os(ARC_DB_PATH).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    #[cfg(target_os = "macos")]
    let base = required_directory("HOME")?.join("Library/Application Support");

    #[cfg(target_os = "windows")]
    let base = required_directory("LOCALAPPDATA")?;

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = match env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
        Some(path) if PathBuf::from(&path).is_absolute() => PathBuf::from(path),
        _ => required_directory("HOME")?.join(".local/share"),
    };

    Ok(base.join("arc").join("arc.sqlite3"))
}

fn required_directory(variable: &str) -> io::Result<PathBuf> {
    env::var_os(variable)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{variable} is not set")))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    fn reply(buffer: *mut libc::c_char, length: usize, value: &[u8]) -> usize {
        if !buffer.is_null() {
            // SAFETY: the resolver supplied this writable buffer and length.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    value.as_ptr(),
                    buffer.cast(),
                    length.min(value.len()),
                )
            };
        }
        value.len()
    }

    #[test]
    fn confstr_preserves_native_bytes_and_retries_a_growing_buffer() {
        let mut calls = 0;
        let value = b"/native/\xff/\0";
        let path = darwin_runtime_root(|buffer, length| {
            calls += 1;
            if calls == 1 {
                2
            } else {
                reply(buffer, length, value)
            }
        })
        .unwrap();
        assert_eq!(calls, 3);
        assert_eq!(path.as_os_str().as_bytes(), &value[..value.len() - 1]);
    }

    #[test]
    fn confstr_rejects_errors_malformed_paths_and_unstable_lengths() {
        assert!(darwin_runtime_root(|_, _| 0).is_err());
        let mut calls = 0;
        assert!(darwin_runtime_root(|_, _| {
            calls += 1;
            if calls == 1 {
                8
            } else {
                0
            }
        })
        .is_err());
        for value in [
            b"".as_slice(),
            b"\0",
            b"relative\0",
            b"/no-terminator",
            b"/interior\0nul\0",
        ] {
            assert!(darwin_runtime_root(|buffer, length| reply(buffer, length, value)).is_err());
        }
        assert!(darwin_runtime_root(|_, length| length + 1).is_err());
        assert!(darwin_runtime_root(|_, _| usize::MAX).is_err());
    }
}
