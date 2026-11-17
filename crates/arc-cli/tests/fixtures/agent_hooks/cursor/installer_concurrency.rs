use super::*;
use std::{
    io::{Read, Write},
    os::unix::{
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(90);

pub(super) fn controlled_configure(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
) -> Result<(), &'static str> {
    let socket = env::var_os("CURSOR_TEST_CONTROL_SOCKET").unwrap();
    let mut stream = UnixStream::connect(socket).expect("connect installer control socket");
    stream.set_read_timeout(Some(WATCHDOG)).unwrap();
    let mut blocked = stream.try_clone().unwrap();
    let mode = env::var("CURSOR_TEST_CONTROL_MODE").unwrap();
    cursor::configure_intercepted(
        action,
        scope,
        workspace,
        context,
        || {
            if mode == "contention" {
                blocked.write_all(b"C").unwrap();
            }
        },
        || {
            if mode == "hold" {
                stream.write_all(b"R").unwrap();
                let mut release = [0];
                stream
                    .read_exact(&mut release)
                    .expect("waiting for first installer release");
                assert_eq!(release, *b"X");
            }
        },
    )
}

fn accept(listener: &UnixListener, label: &str) -> UnixStream {
    let deadline = Instant::now() + WATCHDOG;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "waiting for {label} connection");
        let mut fd = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: fd points to a live pollfd and a valid Unix listener.
        let result = unsafe {
            libc::poll(
                &mut fd,
                1,
                remaining.as_millis().min(i32::MAX as u128) as i32,
            )
        };
        if result > 0 {
            let (stream, _) = listener
                .accept()
                .expect("accept installer control connection");
            stream.set_read_timeout(Some(WATCHDOG)).unwrap();
            return stream;
        }
        if result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        panic!(
            "waiting for {label} connection: {}",
            std::io::Error::last_os_error()
        );
    }
}

fn signal(stream: &mut UnixStream, expected: u8, label: &str) {
    let mut value = [0];
    stream
        .read_exact(&mut value)
        .unwrap_or_else(|error| panic!("waiting for {label}: {error}"));
    assert_eq!(value[0], expected, "unexpected {label} signal");
}

fn success(child: process::MonitoredChild, label: &str) {
    let output = child.finish();
    assert!(
        output.status.success(),
        "{label}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn config_path(fixture: &Fixture, scope: &str) -> PathBuf {
    let base = if scope == "user" {
        &fixture.home
    } else {
        &fixture.workspace
    };
    fs::create_dir(base.join(".cursor")).unwrap();
    base.join(".cursor/hooks.json")
}

#[test]
fn competing_arc_processes_are_serialized_in_both_scopes() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let fixture = Fixture::new();
        let path = config_path(&fixture, scope);
        fs::write(&path, source).unwrap();
        let first_socket = fixture.root.join("first.sock");
        let second_socket = fixture.root.join("second.sock");
        let first_listener = UnixListener::bind(&first_socket).unwrap();
        let second_listener = UnixListener::bind(&second_socket).unwrap();
        let first = fixture.spawn_worker(
            &format!("install-{scope}"),
            &[
                ("CURSOR_TEST_CONTROL_SOCKET", first_socket.to_str().unwrap()),
                ("CURSOR_TEST_CONTROL_MODE", "hold"),
            ],
        );
        let mut first_control = accept(&first_listener, "first installer");
        signal(&mut first_control, b'R', "first installer snapshot");
        let second = fixture.spawn_worker(
            &format!("uninstall-{scope}"),
            &[
                (
                    "CURSOR_TEST_CONTROL_SOCKET",
                    second_socket.to_str().unwrap(),
                ),
                ("CURSOR_TEST_CONTROL_MODE", "contention"),
            ],
        );
        let mut second_control = accept(&second_listener, "second installer");
        signal(
            &mut second_control,
            b'C',
            "second installer lock contention",
        );
        first_control.write_all(b"X").unwrap();
        success(first, "first installer");
        success(second, "second installer");
        let original: Value = serde_json::from_str(source).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
            original
        );
    }
}

#[test]
fn intervening_in_place_edits_are_detected_in_both_scopes() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        for action in ["install", "uninstall"] {
            let fixture = Fixture::new();
            let path = config_path(&fixture, scope);
            fs::write(&path, source).unwrap();
            if action == "uninstall" {
                fixture.worker(&format!("install-{scope}"), &[]);
            }
            let before = fs::read_to_string(&path).unwrap();
            let expected = before.replacen('{', "{\"intervening\":\"preserve me\",", 1);
            fixture.worker(
                &format!("{action}-{scope}"),
                &[
                    ("CURSOR_INTERVENING_EDIT", "1"),
                    (
                        "CURSOR_EXPECT_ERROR_MESSAGE",
                        "Cursor configuration changed during the update; retry the operation.",
                    ),
                ],
            );
            assert_eq!(fs::read_to_string(path).unwrap(), expected);
        }
    }
}

#[test]
fn settings_keep_large_integers_and_precise_decimals_in_both_scopes() {
    let source = r#"{"version":1,"unrelated":{"huge":123456789012345678901234567890123456789,"precise":0.123456789012345678901234567890123456789,"exponent":1.234567890123456789e+308,"text":"123456789012345678901234567890123456789"},"hooks":{"preToolUse":[{"command":"unrelated","timeout":0.123456789012345678901234567890123456789}]}}"#;
    for scope in ["user", "workspace"] {
        let fixture = Fixture::new();
        let path = config_path(&fixture, scope);
        fs::write(&path, source).unwrap();
        for action in ["install", "uninstall"] {
            fixture.worker(&format!("{action}-{scope}"), &[]);
            let output = fs::read_to_string(&path).unwrap();
            for fragment in [
                "\"huge\": 123456789012345678901234567890123456789",
                "\"precise\": 0.123456789012345678901234567890123456789",
                "\"timeout\": 0.123456789012345678901234567890123456789",
                "\"exponent\": 1.234567890123456789e+308",
                "\"text\": \"123456789012345678901234567890123456789\"",
            ] {
                assert!(
                    output.contains(fragment),
                    "missing {fragment} from {output}"
                );
            }
        }
    }
}
