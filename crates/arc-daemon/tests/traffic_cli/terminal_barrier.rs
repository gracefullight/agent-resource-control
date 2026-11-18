//! Keep the native target alive until its actual identity is attached.
use super::*;
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::Write,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
};

pub(super) struct Barrier {
    gate: Option<File>,
}
impl Barrier {
    pub(super) fn prepare(fixture: &Fixture, command: &mut Command) -> Self {
        let gate_path = fixture.root.join("terminal-gate");
        let gate_name = CString::new(gate_path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(gate_name.as_ptr(), 0o600) }, 0);
        // The parent's read/write endpoint prevents an open-time race; the
        // target still waits for a byte rather than for a duration.
        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&gate_path)
            .unwrap();
        let wrapper = fixture.root.join("cargo");
        fs::write(
            &wrapper,
            r#"#!/bin/sh
exec 3<"$ARC_TERMINAL_GATE"
printf '%s' "$$" > "$ARC_TERMINAL_READY.pending"
/bin/mv "$ARC_TERMINAL_READY.pending" "$ARC_TERMINAL_READY"
IFS= read -r release <&3
exec "$ARC_TERMINAL_EXEC" "$@"
"#,
        )
        .unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        command
            .env("ARC_TERMINAL_GATE", gate_path)
            .env("ARC_TERMINAL_READY", fixture.root.join("terminal-ready"))
            .env("ARC_TERMINAL_EXEC", binary("arc-exec-fixture"));
        Self { gate: Some(gate) }
    }

    pub(super) fn wait_for_attachment(&mut self, fixture: &Fixture, child: &mut OwnedChild) {
        let marker = fixture.root.join("terminal-ready");
        eventually(|| {
            assert!(
                child.child.try_wait().unwrap().is_none(),
                "terminal wrapper exited before target readiness"
            );
            marker.is_file()
        });
        let pid: u32 = fs::read_to_string(marker).unwrap().parse().unwrap();
        let identity = arc_core::process_identity::ProcessIdentity::capture(pid).unwrap();
        child.own_process(identity.clone());
        eventually(|| {
            assert!(
                child.child.try_wait().unwrap().is_none(),
                "terminal wrapper exited before attachment"
            );
            database(fixture).query_row(
                "SELECT count(*) FROM job_lifecycle WHERE child_pid=?1 AND child_start_token=?2 AND child_boot_id=?3",
                (&identity.pid, &identity.start_token, &identity.boot_id),
                |row| row.get::<_, i64>(0),
            ).unwrap() == 1
        });
    }

    pub(super) fn release(&mut self) {
        if let Some(mut gate) = self.gate.take() {
            gate.write_all(b"release\n").unwrap();
        }
    }
}
impl Drop for Barrier {
    fn drop(&mut self) {
        if let Some(mut gate) = self.gate.take() {
            let _ = gate.write_all(b"release\n");
        }
    }
}
