//! Resident IPC server and clean shutdown.

use crate::platform;
use arc_ipc::{
    self, DoctorReport, FrameError, Request, Response, ERROR_FRAME_TOO_LARGE, ERROR_INVALID_REQUEST,
};
use rusqlite::Connection;
use std::{
    error::Error,
    ffi::CString,
    fs::{self, File, Permissions},
    io::{self, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::UnixStream,
        },
    },
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::{
    net::UnixListener,
    signal::unix::{signal, SignalKind},
};

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

/// Serve until SIGTERM or SIGINT, then close SQLite and remove the socket.
pub async fn serve(connection: Connection, socket: PathBuf) -> Result<(), Box<dyn Error>> {
    // Install the handlers before exposing readiness through the socket.
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    let listener = bind_socket(&socket)?;
    let database = Arc::new(Mutex::new(Some(connection)));
    println!(
        "arc-daemon ready. Socket: {}. Protocol: 1. Sweeper off.",
        socket.display()
    );
    io::stdout().flush()?;

    loop {
        tokio::select! {
            _ = terminate.recv() => break,
            _ = interrupt.recv() => break,
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        match stream.into_std() {
                            Ok(stream) => {
                                let _ = stream.set_nonblocking(false);
                                let database = Arc::clone(&database);
                                std::thread::spawn(move || {
                                    handle_connection(stream, &database);
                                });
                            }
                            Err(error) => {
                                eprintln!("IPC stream setup failed: {}.", sanitize(&error));
                            }
                        }
                    }
                    Err(error) => eprintln!("IPC accept failed: {}.", sanitize(&error)),
                }
            }
        }
    }

    drop(listener);
    let connection = database
        .lock()
        .map_err(|_| io::Error::other("database lock poisoned"))?
        .take();
    if let Some(connection) = connection {
        // Close without a WAL checkpoint, then leave the socket path clean.
        connection.close().map_err(|(_, error)| error)?;
    }
    match fs::remove_file(&socket) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn handle_connection(mut stream: UnixStream, database: &Mutex<Option<Connection>>) {
    if let Err(error) = platform::ensure_same_user(&stream) {
        eprintln!("Rejected IPC peer: {}.", sanitize(&error));
        return;
    }

    loop {
        let frame = match arc_ipc::read_frame(&mut stream) {
            Ok(frame) => frame,
            Err(FrameError::Closed) | Err(FrameError::UnexpectedEof) => return,
            Err(error) => {
                let response = Response::error("", error_code(&error));
                let _ = arc_ipc::write_response(&mut stream, &response);
                return;
            }
        };

        let request = match Request::decode(&frame) {
            Ok(request) => request,
            Err(_) => {
                let response = Response::error("", ERROR_INVALID_REQUEST);
                if arc_ipc::write_response(&mut stream, &response).is_err() {
                    return;
                }
                continue;
            }
        };

        let response = arc_ipc::dispatch(&request, || doctor(database));
        if arc_ipc::write_response(&mut stream, &response).is_err() {
            return;
        }
    }
}

fn doctor(database: &Mutex<Option<Connection>>) -> Result<DoctorReport, String> {
    let guard = database
        .lock()
        .map_err(|_| "database lock poisoned".to_owned())?;
    let connection = guard
        .as_ref()
        .ok_or_else(|| "database is closed".to_owned())?;
    let integrity_check: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let schema_migrations: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    Ok(DoctorReport {
        integrity_check,
        schema_migrations,
    })
}

fn error_code(error: &FrameError) -> &'static str {
    match error {
        FrameError::TooLarge { .. } => ERROR_FRAME_TOO_LARGE,
        _ => ERROR_INVALID_REQUEST,
    }
}

fn sanitize(error: &(impl std::fmt::Display + ?Sized)) -> String {
    error.to_string().replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_ipc::{
        ERROR_INVALID_REQUEST, ERROR_PROTOCOL_INCOMPATIBLE, OPERATION_DOCTOR, OPERATION_HELLO,
        STATUS_ERROR, STATUS_OK,
    };
    use std::io::Write;
    use std::os::unix::fs::{symlink, DirBuilderExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn migrated_database() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let directory = arc_core::runtime_root().unwrap().join(format!(
            "s{:x}-{:x}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        directory.join("arc.sqlite3")
    }

    fn request(operation: &str, version: u32) -> arc_ipc::Request {
        let mut request = arc_ipc::Request::new("server-test", operation);
        request.protocol_version = version;
        request
    }

    #[test]
    fn a_socketpair_serves_hello_and_survives_an_incompatible_version() {
        let path = migrated_database();
        let connection = arc_storage::open_and_migrate(&path).unwrap();
        let database = Arc::new(Mutex::new(Some(connection)));
        let (server, mut client) = UnixStream::pair().unwrap();
        let handler = std::thread::spawn(move || handle_connection(server, &database));

        let hello = arc_ipc::exchange(&mut client, &request(OPERATION_HELLO, 1)).unwrap();
        assert_eq!(hello.status, STATUS_OK);

        let incompatible = arc_ipc::exchange(&mut client, &request(OPERATION_HELLO, 99)).unwrap();
        assert_eq!(
            incompatible.error.map(|error| error.code),
            Some(ERROR_PROTOCOL_INCOMPATIBLE.to_owned())
        );

        let hello = arc_ipc::exchange(&mut client, &request(OPERATION_HELLO, 1)).unwrap();
        assert_eq!(hello.status, STATUS_OK);

        let doctor = arc_ipc::exchange(&mut client, &request(OPERATION_DOCTOR, 1)).unwrap();
        assert_eq!(doctor.status, STATUS_OK);
        let result = doctor.result.unwrap();
        assert_eq!(result["integrity_check"], "ok");
        assert_eq!(result["schema_migrations"], 1);
        assert_eq!(result["ipc_owner_only"], true);
        assert_eq!(result["tray_connected"], false);
        assert_eq!(result["vendors_detected"], false);
        assert_eq!(result["sweeper"], "off");

        drop(client);
        handler.join().unwrap();
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn permissive_existing_directories_are_rejected_without_mutation() {
        use std::os::unix::fs::MetadataExt;
        for mode in [0o755, 0o770] {
            let path = migrated_database();
            let directory = path.parent().unwrap();
            fs::set_permissions(directory, Permissions::from_mode(mode)).unwrap();
            fs::write(directory.join("sentinel"), b"leave this alone").unwrap();
            let before = fs::symlink_metadata(directory).unwrap();
            let result = prepare_socket_directory(&directory.join("arc.sock"));
            let after = fs::symlink_metadata(directory).unwrap();
            let snapshot = |m: &fs::Metadata| (m.dev(), m.ino(), m.mode(), m.uid(), m.gid());
            assert_eq!(snapshot(&before), snapshot(&after), "directory changed");
            assert!(result.is_err(), "mode {mode:o} was accepted");
            assert_eq!(
                fs::read(directory.join("sentinel")).unwrap(),
                b"leave this alone"
            );
            assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn new_nested_and_existing_private_directories_are_usable_without_repair() {
        let database = migrated_database();
        let root = database.parent().unwrap();
        let first = root.join("new");
        let parent = first.join("nested");
        let socket = parent.join("arc.sock");
        prepare_socket_directory(&socket).unwrap();
        for directory in [&first, &parent] {
            let metadata = fs::symlink_metadata(directory).unwrap();
            assert_eq!(metadata.mode() & 0o7777, 0o700);
            assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        }
        fs::write(parent.join("sentinel"), b"private content").unwrap();
        let before = fs::symlink_metadata(&parent).unwrap();
        prepare_socket_directory(&socket).unwrap();
        let after = fs::symlink_metadata(&parent).unwrap();
        assert_eq!(
            (
                before.dev(),
                before.ino(),
                before.mode(),
                before.uid(),
                before.gid()
            ),
            (
                after.dev(),
                after.ino(),
                after.mode(),
                after.uid(),
                after.gid()
            )
        );
        assert_eq!(
            fs::read(parent.join("sentinel")).unwrap(),
            b"private content"
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn final_intermediate_and_dangling_symlinks_are_not_followed() {
        for mode in [0o700, 0o755, 0o770] {
            let database = migrated_database();
            let root = database.parent().unwrap();
            let target = root.join("target");
            fs::create_dir(&target).unwrap();
            fs::set_permissions(&target, Permissions::from_mode(mode)).unwrap();
            fs::write(target.join("sentinel"), b"target content").unwrap();
            let before = fs::symlink_metadata(&target).unwrap();
            let link = root.join("link");
            symlink(&target, &link).unwrap();
            for socket in [
                link.join("arc.sock"),
                link.join("missing/arc.sock"),
                root.join("new/../link/arc.sock"),
            ] {
                assert!(prepare_socket_directory(&socket).is_err());
                let after = fs::symlink_metadata(&target).unwrap();
                assert_eq!(
                    (
                        before.dev(),
                        before.ino(),
                        before.mode(),
                        before.uid(),
                        before.gid()
                    ),
                    (
                        after.dev(),
                        after.ino(),
                        after.mode(),
                        after.uid(),
                        after.gid()
                    )
                );
                assert_eq!(
                    fs::read(target.join("sentinel")).unwrap(),
                    b"target content"
                );
                assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
                assert_eq!(fs::read_link(&link).unwrap(), target);
                assert!(
                    !root.join("new").exists(),
                    "created a prefix before rejecting a link"
                );
            }
            let dangling = root.join("dangling");
            let missing = root.join("absent");
            symlink(&missing, &dangling).unwrap();
            assert!(prepare_socket_directory(&dangling.join("arc.sock")).is_err());
            assert!(!missing.exists());
            assert_eq!(fs::read_link(&dangling).unwrap(), missing);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn a_directory_that_appears_between_walks_is_validated_before_nested_creation() {
        let database = migrated_database();
        let root = database.parent().unwrap();
        let raced = root.join("raced");
        let parent = raced.join("nested");
        let mut missing = Vec::new();
        assert!(walk_directory(&parent, false, &mut missing)
            .unwrap()
            .is_none());
        fs::create_dir(&raced).unwrap();
        fs::set_permissions(&raced, Permissions::from_mode(0o755)).unwrap();
        fs::write(raced.join("sentinel"), b"racing creator").unwrap();
        let before = fs::metadata(&raced).unwrap();
        assert!(walk_directory(&parent, true, &mut missing).is_err());
        let after = fs::metadata(&raced).unwrap();
        assert_eq!(
            (
                before.dev(),
                before.ino(),
                before.mode(),
                before.uid(),
                before.gid()
            ),
            (
                after.dev(),
                after.ino(),
                after.mode(),
                after.uid(),
                after.gid()
            )
        );
        assert_eq!(fs::read(raced.join("sentinel")).unwrap(), b"racing creator");
        assert_eq!(fs::read_dir(&raced).unwrap().count(), 1);
        assert!(!parent.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy() {
        assert!(private_directory_problem(42, 0o40700, 43)
            .unwrap()
            .contains("owned by UID 42"));
        assert!(private_directory_problem(42, 0o40700, 42).is_none());
        for mode in [0o755, 0o770, 0o1700, 0o2700, 0o4700, 0o600] {
            assert!(private_directory_problem(42, mode, 42).is_some());
        }
    }

    #[test]
    fn already_exists_is_validated_without_repair_or_following_links() {
        let path = migrated_database();
        let root = path.parent().unwrap();
        let directory = File::open(root).unwrap();
        let name = CString::new("existing").unwrap();
        let existing = root.join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("sentinel"), b"unchanged").unwrap();
        for mode in [0o755, 0o770, 0o700] {
            fs::set_permissions(&existing, Permissions::from_mode(mode)).unwrap();
            let before = fs::symlink_metadata(&existing).unwrap();
            let result = create_private_directory_at(&directory, &name, &existing);
            assert_eq!(result.is_ok(), mode == 0o700);
            let after = fs::symlink_metadata(&existing).unwrap();
            assert_eq!(
                (
                    before.dev(),
                    before.ino(),
                    before.mode(),
                    before.uid(),
                    before.gid()
                ),
                (
                    after.dev(),
                    after.ino(),
                    after.mode(),
                    after.uid(),
                    after.gid()
                )
            );
            assert_eq!(fs::read(existing.join("sentinel")).unwrap(), b"unchanged");
            assert_eq!(fs::read_dir(&existing).unwrap().count(), 1);
        }
        let link = root.join("link");
        symlink(&existing, &link).unwrap();
        assert!(
            create_private_directory_at(&directory, &CString::new("link").unwrap(), &link).is_err()
        );
        assert_eq!(fs::read_link(&link).unwrap(), existing);
        assert_eq!(fs::metadata(&existing).unwrap().mode() & 0o7777, 0o700);
        assert_eq!(fs::read_dir(&existing).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_shared_temporary_root_is_refused_instead_of_tightened() {
        let error = prepare_socket_directory(&arc_core::runtime_root().unwrap().join("arc.sock"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("shared temporary directory"),
            "{error}"
        );
    }

    #[test]
    fn socket_length_validation_counts_native_bytes_and_preserves_entries() {
        use std::os::unix::ffi::OsStringExt;
        let database = migrated_database();
        let root = database.parent().unwrap();
        let address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let max = address.sun_path.len() - 1;
        let prefix = root.as_os_str().as_bytes().len() + 1;
        let mut name = vec![b'x'; max - prefix];
        // Non-UTF-8 filename bytes must be measured and preserved unchanged.
        name[0] = 0xff;
        let boundary = root.join(std::ffi::OsString::from_vec(name.clone()));
        assert!(validate_socket_path(&boundary).is_ok());
        name.push(b'x');
        let native_overlong = root.join(std::ffi::OsString::from_vec(name.clone()));
        assert!(validate_socket_path(&native_overlong).is_err());
        // The sandbox denies creating non-UTF-8 names. Keep native-byte
        // boundary coverage above and use a writable ASCII entry for the
        // independent inode/content preservation assertion.
        name[0] = b'x';
        let overlong = root.join(std::ffi::OsString::from_vec(name));
        fs::write(&overlong, b"preserved overlong entry").unwrap();
        let before = fs::symlink_metadata(&overlong).unwrap();
        assert!(validate_socket_path(&overlong).is_err());
        let after = fs::symlink_metadata(&overlong).unwrap();
        assert_eq!(
            (before.dev(), before.ino(), before.mode()),
            (after.dev(), after.ino(), after.mode())
        );
        assert_eq!(fs::read(&overlong).unwrap(), b"preserved overlong entry");
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_positional_array_is_rejected_and_the_connection_stays_usable() {
        let path = migrated_database();
        let connection = arc_storage::open_and_migrate(&path).unwrap();
        let database = Arc::new(Mutex::new(Some(connection)));
        let (server, mut client) = UnixStream::pair().unwrap();
        let handler = std::thread::spawn(move || handle_connection(server, &database));

        let mut frame = Vec::new();
        arc_ipc::write_frame(&mut frame, br#"[1,"req-1","daemon.hello",null]"#).unwrap();
        client.write_all(&frame).unwrap();
        let response = arc_ipc::read_response(&mut client).unwrap();
        assert_eq!(response.status, STATUS_ERROR);
        assert_eq!(
            response.error.map(|error| error.code),
            Some(ERROR_INVALID_REQUEST.to_owned())
        );

        let hello = arc_ipc::exchange(&mut client, &request(OPERATION_HELLO, 1)).unwrap();
        assert_eq!(hello.status, STATUS_OK);

        drop(client);
        handler.join().unwrap();
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
