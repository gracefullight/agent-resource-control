//! Resident IPC server and clean shutdown.

use rusqlite::Connection;
use std::{
    error::Error,
    ffi::CString,
    fs::{self, File, Permissions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
        },
    },
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::net::UnixListener;

/// Create missing ARC directories privately; never repair existing entries.
pub fn prepare_socket_directory(socket: &Path) -> io::Result<()> {
    let parent = validate_directory_path(socket)?;
    let mut missing = Vec::new();
    let inspected = walk_directory(&parent, false, &mut missing)?;
    let directory = if missing.is_empty() {
        inspected.expect("existing path returns a directory")
    } else {
        walk_directory(&parent, true, &mut missing)?.expect("creation returns a directory")
    };
    let metadata = directory.metadata()?;
    validate_private_directory(&parent, &metadata)?;
    let current = open_private_directory(&parent)?.metadata()?;
    if (metadata.dev(), metadata.ino()) != (current.dev(), current.ino()) {
        return Err(directory_error(&parent, "changed during preparation"));
    }
    Ok(())
}

/// Read-only validation before admission, mkdir, SQLite or stale-socket removal.
pub fn validate_socket_path(socket: &Path) -> io::Result<()> {
    // SAFETY: sockaddr_un is a plain C structure whose zero value is valid.
    let address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let bytes = socket.as_os_str().as_bytes();
    if bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(directory_error(
            socket,
            format!(
                "has an invalid Unix socket path (maximum {} bytes, no NUL)",
                address.sun_path.len() - 1
            ),
        ));
    }
    validate_directory_path(socket)?;
    validate_socket_entry(socket)
}

fn validate_socket_entry(socket: &Path) -> io::Result<()> {
    match fs::symlink_metadata(socket) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o7777 != 0o600
            {
                return Err(directory_error(
                    socket,
                    "must be an owned 0600 Unix socket, not a symlink or other entry",
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn open_private_directory(path: &Path) -> io::Result<File> {
    let path = std::path::absolute(path)?;
    let directory = walk_directory(&path, false, &mut Vec::new())?
        .ok_or_else(|| directory_error(&path, "does not exist"))?;
    validate_private_directory(&path, &directory.metadata()?)?;
    Ok(directory)
}

fn validate_directory_path(socket: &Path) -> io::Result<PathBuf> {
    let parent = socket
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the socket path needs a parent directory",
            )
        })?;
    if is_shared_root(parent)? {
        return Err(directory_error(parent, "is a shared temporary directory"));
    }
    let parent = std::path::absolute(parent)?;
    // Check existing components before creating anything. Directory-relative
    // opens prevent ancestor replacement from redirecting mkdir into a link.
    let mut missing = Vec::new();
    let inspected = walk_directory(&parent, false, &mut missing)?;
    if let Some(directory) = &inspected {
        validate_private_directory(&parent, &directory.metadata()?)?;
    }
    Ok(parent)
}

fn directory_error(path: &Path, reason: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "{} {reason}; choose a separate owned 0700 directory through ARC_SOCKET_PATH",
            path.display()
        ),
    )
}

fn validate_private_directory(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    // SAFETY: geteuid has no preconditions and does not mutate process state.
    let uid = unsafe { libc::geteuid() };
    if let Some(reason) = private_directory_problem(metadata.uid(), metadata.mode(), uid) {
        return Err(directory_error(path, reason));
    }
    Ok(())
}

fn private_directory_problem(owner: u32, mode: u32, uid: u32) -> Option<String> {
    if owner != uid {
        Some(format!(
            "is owned by UID {owner}, expected effective UID {uid}"
        ))
    } else if mode & 0o7777 != 0o700 {
        Some(format!(
            "has permissions {:04o}, expected 0700",
            mode & 0o7777
        ))
    } else {
        None
    }
}

/// Existing ordinary ancestors may be shared; the socket parent and all
/// directories created for ARC must be owned and private.
fn walk_directory(
    path: &Path,
    create: bool,
    missing: &mut Vec<PathBuf>,
) -> io::Result<Option<File>> {
    let mut directory = File::open("/")?;
    let mut walked = PathBuf::from("/");
    let mut missing_depth = 0;
    for component in path.components() {
        let name = match component {
            Component::RootDir | Component::CurDir => continue,
            Component::Normal(name) => name,
            Component::ParentDir => std::ffi::OsStr::new(".."),
            Component::Prefix(_) => unreachable!("Unix paths have no prefixes"),
        };
        walked.push(name);
        // Inspect beyond a missing prefix when `..` returns to an existing
        // ancestor, so a later link is rejected before any mkdir occurs.
        if !create && missing_depth > 0 {
            if component == Component::ParentDir {
                missing_depth -= 1;
            } else {
                missing_depth += 1;
                missing.push(walked.clone());
            }
            continue;
        }
        let name = CString::new(name.as_bytes())
            .map_err(|_| directory_error(&walked, "contains a NUL byte"))?;
        match open_directory_at(&directory, &name, &walked) {
            Ok(opened) => {
                // A creator may have filled a formerly missing component
                // between the inspection and creation walks.
                if create && missing.contains(&walked) {
                    validate_private_directory(&walked, &opened.metadata()?)?;
                }
                directory = opened;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if !create {
                    missing.push(walked.clone());
                    missing_depth = 1;
                    continue;
                }
                directory = create_private_directory_at(&directory, &name, &walked)?;
            }
            Err(error) => {
                return Err(directory_error(
                    &walked,
                    format!(
                    "cannot be opened without symlink redirection (must be a directory): {error}"
                ),
                ))
            }
        }
    }
    Ok((missing_depth == 0).then_some(directory))
}

fn open_directory_at(parent: &File, name: &CString, path: &Path) -> io::Result<File> {
    let mut flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    // macOS ships these system aliases. Allow precisely those mappings, not
    // arbitrary aliases in TMPDIR or ARC_SOCKET_PATH.
    if is_system_directory_alias(path) {
        flags &= !libc::O_NOFOLLOW;
    }
    // SAFETY: parent is live and name is NUL-terminated. O_DIRECTORY enforces
    // the entry type; a descriptor is adopted only on success.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: openat returned a new owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn create_private_directory_at(parent: &File, name: &CString, path: &Path) -> io::Result<File> {
    // SAFETY: parent is live and name is NUL-terminated. The requested mode
    // never grants group/other access, even under a permissive child umask.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(directory_error(
                path,
                format!("could not be created: {error}"),
            ));
        }
        // Another creator won. Validate its entry without chmod or cleanup.
    }
    let directory = open_directory_at(parent, name, path).map_err(|error| {
        directory_error(path, format!("is not a directory or is a symlink: {error}"))
    })?;
    validate_private_directory(path, &directory.metadata()?)?;
    Ok(directory)
}

fn is_system_directory_alias(path: &Path) -> bool {
    if cfg!(target_os = "macos") {
        let target = match path.to_str() {
            Some("/tmp") => "/private/tmp",
            Some("/var") => "/private/var",
            _ => return false,
        };
        return fs::read_link(path)
            .is_ok_and(|actual| Path::new("/").join(actual) == Path::new(target));
    }
    false
}

fn is_shared_root(path: &Path) -> io::Result<bool> {
    let matches = |root: &Path| {
        path == root
            || fs::canonicalize(path)
                .ok()
                .zip(fs::canonicalize(root).ok())
                .is_some_and(|(path, root)| path == root)
    };
    Ok(matches(Path::new("/tmp"))
        || matches(Path::new("/var/tmp"))
        || matches(&arc_core::runtime_root()?))
}

/// Bind the owner-only socket, replacing a stale socket file after we hold the
/// single-instance lock.
pub fn bind_socket(socket: &Path) -> io::Result<UnixListener> {
    validate_socket_path(socket)?;
    if fs::symlink_metadata(socket).is_ok() {
        fs::remove_file(socket)?;
    }
    let listener = UnixListener::bind(socket).map_err(|error| {
        io::Error::new(error.kind(), format!("bind {}: {error}", socket.display()))
    })?;
    fs::set_permissions(socket, Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Serve with validated finite limits and one owned SQLite worker.
#[allow(dead_code)]
pub async fn serve(connection: Connection, socket: PathBuf) -> Result<(), Box<dyn Error>> {
    serve_with_source(connection, socket, None).await
}

pub(crate) async fn serve_with_source(
    mut connection: Connection,
    socket: PathBuf,
    meter: Option<arc_meter::MeterWorker>,
) -> Result<(), Box<dyn Error>> {
    let profile = std::env::var_os("ARC_IPC_PROFILE");
    let limits = crate::limits::RuntimeLimits::from_profile(profile.as_deref())?;
    let metrics = Arc::new(crate::runtime_metrics::RuntimeMetrics::new(&limits));
    // Recovery is performed before worker accounting and never launches work.
    // A busy or unavailable scan is retried on admission.
    let _ = crate::db_worker::traffic::recover_at_start(&mut connection);
    let worker = crate::db_worker::DbWorker::start(
        connection,
        crate::db_worker::DbConfig {
            queue_capacity: limits.db_queue,
            busy_timeout: limits.db_busy_timeout,
            execution_timeout: limits.db_execution_timeout,
        },
        Arc::clone(&metrics),
    )?;
    match meter {
        None => crate::lifecycle::supervise(socket, limits, metrics, worker).await,
        Some(meter) => {
            crate::lifecycle::supervise_with_source(
                socket,
                limits,
                metrics,
                worker,
                crate::accept_recovery::Acceptor::default(),
                crate::runtime_report::RuntimeReport::from_env(),
                Some(meter),
            )
            .await
        }
    }
}

/// Controlled worker fixtures use the same supervisor as the production entry.
#[cfg(test)]
pub(crate) async fn serve_with_worker(
    socket: PathBuf,
    limits: crate::limits::RuntimeLimits,
    metrics: Arc<crate::runtime_metrics::RuntimeMetrics>,
    worker: crate::db_worker::DbWorker,
) -> Result<(), Box<dyn Error>> {
    crate::lifecycle::supervise(socket, limits, metrics, worker).await
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
