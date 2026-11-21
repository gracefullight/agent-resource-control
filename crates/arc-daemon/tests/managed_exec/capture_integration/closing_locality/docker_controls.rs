use super::*;

pub(super) fn forwarded_docker_socket_is_not_contacted_or_treated_as_local() {
    use std::{
        io::{Read, Write},
        net::{Ipv4Addr, TcpListener},
        os::unix::{fs::FileTypeExt, net::UnixStream},
        time::{Duration, Instant},
    };

    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let source = f.root.join("forwarder.c");
    // The operator's listener rule leaves SSH forwarding unresolved.
    let binary = f.root.join("ssh");
    let socket = f.root.join("forward.sock");
    let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    target.set_nonblocking(true).unwrap();
    let port = target.local_addr().unwrap().port().to_string();
    write(
        &source,
        r#"
#include <arpa/inet.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
int main(int argc, char **argv) {
    if (argc != 3 || strlen(argv[1]) >= sizeof(((struct sockaddr_un *)0)->sun_path)) return 1;
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un local; memset(&local, 0, sizeof(local));
    local.sun_family = AF_UNIX; strcpy(local.sun_path, argv[1]);
    if (fd < 0 || bind(fd, (struct sockaddr *)&local, sizeof(local)) || listen(fd, 16)) return 2;
    for (;;) {
        int client = accept(fd, 0, 0);
        if (client < 0) continue;
        int remote = socket(AF_INET, SOCK_STREAM, 0);
        struct sockaddr_in target; memset(&target, 0, sizeof(target));
        target.sin_family = AF_INET; target.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        target.sin_port = htons((unsigned short)atoi(argv[2]));
        if (remote >= 0 && !connect(remote, (struct sockaddr *)&target, sizeof(target))) {
            char byte;
            if (read(client, &byte, 1) == 1) (void)write(remote, &byte, 1);
        }
        if (remote >= 0) close(remote);
        close(client);
    }
}
"#,
    );
    let mut compiler = Command::new("/usr/bin/cc");
    compiler
        .env_clear()
        .env("HOME", &s.home)
        .env("PATH", "/usr/bin:/bin")
        .arg("-o")
        .arg(&binary)
        .arg(&source);
    instrumentation::forward(&mut compiler);
    assert!(
        child_watchdog::output(&mut compiler, "docker forwarder compiler")
            .unwrap()
            .status
            .success()
    );
    let mut server = Command::new(&binary);
    server
        .env_clear()
        .env("HOME", &s.home)
        .arg(&socket)
        .arg(port);
    instrumentation::forward(&mut server);
    let _server = child_watchdog::Owned::spawn(&mut server).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !fs::symlink_metadata(&socket).is_ok_and(|m| m.file_type().is_socket()) {
        assert!(Instant::now() < deadline, "forwarder did not bind");
        std::thread::sleep(Duration::from_millis(10));
    }

    // Every sentinel is a barrier through the sequential forwarder. A probe
    // connection made before it would reach the target first (and deliver EOF
    // instead of the sentinel), even if the server was scheduled late.
    let barrier = || {
        let deadline = Instant::now() + Duration::from_secs(60);
        // The socket file exists from bind(), before listen() accepts. A
        // refused connect reaches nothing, so retrying keeps the barrier.
        let mut probe = loop {
            match UnixStream::connect(&socket) {
                Ok(stream) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                    assert!(Instant::now() < deadline, "forwarder did not listen");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("forwarder connect: {error}"),
            }
        };
        probe.write_all(b"x").unwrap();
        let mut target_connection = loop {
            match target.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "forwarder did not reach target");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("target accept: {error}"),
            }
        };
        // Darwin lets an accepted socket inherit the listener's nonblocking
        // mode; the forwarder writes the sentinel only after it connects.
        target_connection.set_nonblocking(false).unwrap();
        target_connection
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let mut byte = [0u8; 1];
        target_connection
            .read_exact(&mut byte)
            .expect("unexpected contact before sentinel");
        assert_eq!(byte, *b"x");
    };
    barrier();

    prod_context(&s, &format!("unix://{}", socket.display()));
    write(
        &s.home.join(".docker/config.json"),
        r#"{"currentContext":"prod"}"#,
    );
    write(&s.home.join(".docker/buildx/instances/local-builder"),
        json!({"Name":"local-builder","Driver":"docker","Dynamic":false,"Nodes":[{"Endpoint":"prod"}]}).to_string());
    let cases: [(&str, &[&str]); 3] = [
        ("engine", &["docker", "build", "."]),
        (
            "builder",
            &[
                "docker",
                "buildx",
                "build",
                "--builder",
                "local-builder",
                ".",
            ],
        ),
        (
            "prod context",
            &["docker", "--context", "prod", "build", "."],
        ),
    ];
    let mut daemon = f.spawn_isolated("test");
    for (name, argv) in cases {
        let before = admission_counts(&f);
        let inspection = s.classify(&f, argv, &[]);
        barrier();
        let (job, _) = s.exec(&f, argv, &[]);
        barrier();
        assert!(!assert_routes(name, &inspection, &job, "UNKNOWN"));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(admission_counts(&f), before);
    }
    assert_eq!(s.calls(), 3);
    eprintln!("forwarded Docker controls: denominator 3, UNKNOWN 3, false-local 0 per route");
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[cfg(target_os = "macos")]
pub(super) fn docker_local_listener_controls_use_both_production_routes_and_admission() {
    use std::os::unix::net::UnixListener;

    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("act")).unwrap();
    let source = s.home.join("socket-inventory.c");
    let library = s.home.join("socket-inventory.dylib");
    write(&source, include_str!("docker_controls/socket_inventory.c"));
    let mut compiler = Command::new("/usr/bin/cc");
    compiler
        .env_clear()
        .env("HOME", &s.home)
        .env("PATH", "/usr/bin:/bin")
        .args(["-dynamiclib", "-o"])
        .arg(&library)
        .arg(&source);
    instrumentation::forward(&mut compiler);
    let result =
        child_watchdog::output(&mut compiler, "socket inventory fixture compiler").unwrap();
    assert!(result.status.success(), "{result:?}");

    let socket = f.root.join("local-docker.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let socket_path = socket.to_str().unwrap();
    let host = format!("unix://{socket_path}");
    let injected = format!("{}:{}", s.interfaces.display(), library.display());
    prod_context(&s, &host);
    write(
        &s.home.join(".docker/config.json"),
        r#"{"currentContext":"prod"}"#,
    );
    write(&s.home.join(".docker/buildx/instances/local-builder"),
        json!({"Name":"local-builder","Driver":"docker","Dynamic":false,"Nodes":[{"Endpoint":"prod"}]}).to_string());

    let mut daemon = f.spawn_isolated("test");
    let cases: [(&str, &[&str], Option<&str>); 5] = [
        ("DOCKER_HOST engine", &["docker", "build", "."], Some(&host)),
        ("act on local engine", &["act", "-j", "build"], Some(&host)),
        ("selected engine", &["docker", "build", "."], None),
        (
            "builder",
            &[
                "docker",
                "buildx",
                "build",
                "--builder",
                "local-builder",
                ".",
            ],
            None,
        ),
        (
            "explicit prod context",
            &["docker", "--context", "prod", "build", "."],
            None,
        ),
    ];
    for (name, argv, docker_host) in cases {
        let env = [
            (preload_key(), Some(injected.as_str())),
            ("ARC_TEST_DOCKER_SOCKET", Some(socket_path)),
            ("ARC_TEST_DOCKER_LISTENER", Some("dockerd")),
            ("DOCKER_HOST", docker_host),
        ];
        let before = admission_counts(&f);
        let inspection = s.classify(&f, argv, &env);
        assert!(
            matches!(listener.accept(), Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "classification contacted socket"
        );
        let (job, id) = s.exec(&f, argv, &env);
        assert!(
            matches!(listener.accept(), Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "execution capture contacted socket"
        );
        // `arcctl classify` and the negotiated status reader name the Local
        // Container; the locality-v1 reader `exec` used keeps HOST_LOCAL.
        let negotiated =
            local_container::recorded(&f, arc_ipc::compute_location::OPERATION_JOB_STATUS, &id);
        for (result, locality) in [
            (&inspection, "LOCAL_CONTAINER"),
            (&negotiated, "LOCAL_CONTAINER"),
            (&job["classification"], "HOST_LOCAL"),
        ] {
            assert_eq!(result["classification"], "BUILD", "{name}: {result}");
            assert_eq!(result["locality"], locality, "{name}: {result}");
            assert_eq!(result["reason"], "LOCAL_ENDPOINT_BUILD", "{name}");
            assert_eq!(result["uncertainty"], "NONE", "{name}: {result}");
        }
        assert_recorded_fixture_child(&s, &job);
        let after = admission_counts(&f);
        assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1), "{name}");
    }
    for mode in [
        "ssh",
        "multiple",
        "unreadable",
        "not-listening",
        "pid1-shared",
        "pid1-unreadable",
    ] {
        let env = [
            (preload_key(), Some(injected.as_str())),
            ("ARC_TEST_DOCKER_SOCKET", Some(socket_path)),
            ("ARC_TEST_DOCKER_LISTENER", Some(mode)),
        ];
        let before = admission_counts(&f);
        let argv = ["docker", "--context", "prod", "build", "."];
        let inspection = s.classify(&f, &argv, &env);
        let (job, _) = s.exec(&f, &argv, &env);
        assert!(!assert_routes(mode, &inspection, &job, "UNKNOWN"));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(admission_counts(&f), before);
        assert!(
            matches!(listener.accept(), Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "{mode} contacted socket"
        );
    }
    assert_eq!(s.calls(), 11);
    eprintln!("local engine controls: denominator 5, LOCAL_CONTAINER 5 per route; unresolved listener controls: denominator 6, UNKNOWN 6 per route");
    daemon.stop(libc::SIGTERM);
    f.report();
}
