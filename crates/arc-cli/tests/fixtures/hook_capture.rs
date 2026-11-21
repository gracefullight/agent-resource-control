//! Native hook capture fixture with an explicit handshake.
use std::{
    io::{Read, Write},
    os::unix::ffi::OsStrExt,
};

pub(super) fn run_if_requested() {
    if !matches!(
        std::env::var("ARC_FIXTURE_MODE").as_deref(),
        Ok("hook-capture" | "hook-capture-pid")
    ) || !handshake()
    {
        return;
    }
    if let Some(marker) = std::env::var_os("ARC_HOOK_CAPTURE_MARKER") {
        use std::fs::OpenOptions;
        writeln!(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(marker)
                .unwrap(),
            "{}",
            if std::env::var("ARC_FIXTURE_MODE").as_deref() == Ok("hook-capture-pid") {
                std::process::id().to_string()
            } else {
                "start".to_owned()
            }
        )
        .unwrap();
    }
    let mut environment: Vec<_> = std::env::vars_os()
        .map(|(key, value)| (key.as_bytes().to_vec(), value.as_bytes().to_vec()))
        .collect();
    environment.sort();
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    let args: Vec<_> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.as_bytes().to_vec())
        .collect();
    let mut output = serde_json::json!({"environment":environment,"cwd":std::env::current_dir().unwrap().as_os_str().as_bytes(),"argv0":std::env::args_os().next().unwrap().as_bytes(),"args":args,"stdin":input});
    if std::env::var("ARC_FIXTURE_MODE").as_deref() == Ok("hook-capture-pid") {
        output["pid"] = serde_json::json!(std::process::id());
    }
    if let Some(record) = std::env::var_os("ARC_HOOK_CAPTURE_RECORD") {
        // Keep the detailed observation separate from a fixed, byte-exact
        // stdout payload, including a non-text byte, for stream comparisons.
        std::fs::write(record, serde_json::to_vec(&output).unwrap()).unwrap();
        std::io::stdout()
            .write_all(b"hook-stdout\0payload\n")
            .unwrap();
    } else {
        println!("{output}");
    }
    eprintln!("child-stderr");
    std::process::exit(17);
}

fn handshake() -> bool {
    const HELLO: &[u8] = b"arc-hook-fixture-capture-v1";
    std::env::var_os("ARC_HOOK_CAPTURE_KEY")
        .and_then(|path| std::fs::read(path).ok())
        .is_some_and(|bytes| bytes == HELLO)
}
