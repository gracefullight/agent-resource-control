use std::{env, path::PathBuf, process::Command};

fn run(command: &mut Command) {
    let status = command
        .status()
        .expect("execute installed native toolchain");
    assert!(
        status.success(),
        "native parent-identity helper compilation failed"
    );
}
fn main() {
    println!("cargo:rerun-if-changed=native/process_parent.c");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let object = out.join("process_parent.o");
    let archive = out.join("libarc_process_parent.a");
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        _ => panic!("unknown Darwin architecture"),
    };
    run(Command::new("/usr/bin/clang")
        .args([
            "-arch",
            arch,
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-c",
            "native/process_parent.c",
            "-o",
        ])
        .arg(&object));
    run(Command::new("/usr/bin/ar")
        .arg("crs")
        .arg(&archive)
        .arg(&object));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=arc_process_parent");
}
