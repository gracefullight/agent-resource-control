use super::*;
use crate::{
    admission::Requests,
    connection,
    db_worker::{DbConfig, DbWorker},
    lifecycle,
    limits::RuntimeLimits,
    runtime_metrics::RuntimeMetrics,
    transport_test_support as wire,
};
use arc_ipc::{
    ERROR_INVALID_REQUEST, ERROR_PROTOCOL_INCOMPATIBLE, OPERATION_DOCTOR, OPERATION_HELLO,
    STATUS_ERROR, STATUS_OK,
};
use std::os::unix::fs::{symlink, DirBuilderExt};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UnixStream;

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

#[tokio::test]
async fn a_socketpair_serves_hello_and_survives_an_incompatible_version() {
    let path = migrated_database();
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let limits = RuntimeLimits::default();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: limits.db_queue,
            busy_timeout: limits.db_busy_timeout,
            execution_timeout: limits.db_execution_timeout,
        },
        Arc::clone(&metrics),
    )
    .unwrap();
    let db = worker.client();
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let (server, client) = UnixStream::pair().unwrap();
    let handler = tokio::spawn(connection::handle_connection(
        server,
        limits,
        requests,
        metrics,
        move |deadline| lifecycle::doctor(db.clone(), deadline),
    ));

    let hello = wire::request(&client, &request(OPERATION_HELLO, 1)).await;
    assert_eq!(hello.status, STATUS_OK);

    let incompatible = wire::request(&client, &request(OPERATION_HELLO, 99)).await;
    assert_eq!(
        incompatible.error.map(|error| error.code),
        Some(ERROR_PROTOCOL_INCOMPATIBLE.to_owned())
    );

    let hello = wire::request(&client, &request(OPERATION_HELLO, 1)).await;
    assert_eq!(hello.status, STATUS_OK);

    let doctor = wire::request(&client, &request(OPERATION_DOCTOR, 1)).await;
    assert_eq!(doctor.status, STATUS_OK);
    let result = doctor.result.unwrap();
    assert_eq!(result["integrity_check"], "ok");
    assert_eq!(result["schema_migrations"], 2);
    assert_eq!(result["ipc_owner_only"], true);
    assert_eq!(result["tray_connected"], false);
    assert_eq!(result["vendors_detected"], false);
    assert_eq!(result["sweeper"], "off");

    drop(client);
    handler.await.unwrap();
    worker
        .shutdown(tokio::time::Instant::now() + std::time::Duration::from_secs(1))
        .await
        .unwrap();
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
    let error =
        prepare_socket_directory(&arc_core::runtime_root().unwrap().join("arc.sock")).unwrap_err();
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

#[tokio::test]
async fn a_positional_array_is_rejected_and_the_connection_stays_usable() {
    let path = migrated_database();
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let limits = RuntimeLimits::default();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: limits.db_queue,
            busy_timeout: limits.db_busy_timeout,
            execution_timeout: limits.db_execution_timeout,
        },
        Arc::clone(&metrics),
    )
    .unwrap();
    let db = worker.client();
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let (server, client) = UnixStream::pair().unwrap();
    let handler = tokio::spawn(connection::handle_connection(
        server,
        limits,
        requests,
        metrics,
        move |deadline| lifecycle::doctor(db.clone(), deadline),
    ));

    let mut frame = Vec::new();
    arc_ipc::write_frame(&mut frame, br#"[1,"req-1","daemon.hello",null]"#).unwrap();
    wire::send(&client, &frame).await;
    let response = wire::response(&client).await;
    assert_eq!(response.status, STATUS_ERROR);
    assert_eq!(
        response.error.map(|error| error.code),
        Some(ERROR_INVALID_REQUEST.to_owned())
    );

    let hello = wire::request(&client, &request(OPERATION_HELLO, 1)).await;
    assert_eq!(hello.status, STATUS_OK);

    drop(client);
    handler.await.unwrap();
    worker
        .shutdown(tokio::time::Instant::now() + std::time::Duration::from_secs(1))
        .await
        .unwrap();
    let _ = fs::remove_dir_all(path.parent().unwrap());
}
