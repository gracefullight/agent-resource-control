//! Resolve launchers before closing descriptors so Python observes the exec boundary.
use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn executable() -> PathBuf {
    // PATH may name a Rust tool-manager shim, which reopens closed standard
    // descriptors at startup. Resolve with all three descriptors open, then
    // invoke the interpreter itself in both the direct and wrapped cases.
    let output = Command::new("python3")
        .args([
            "-c",
            "import os,sys; sys.stdout.buffer.write(os.fsencode(sys.executable))",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("resolve native Python interpreter");
    assert!(
        output.status.success(),
        "Python resolution failed: {output:?}"
    );
    let path = PathBuf::from(OsString::from_vec(output.stdout));
    assert!(
        path.is_absolute() && path.is_file(),
        "invalid interpreter: {path:?}"
    );
    path
}
