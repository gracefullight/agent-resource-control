fn main() {
    // Metadata describes this build's actual checkout, including an uncommitted overlay.
    // Missing Git evidence remains empty so the bridge can report null.
    let git = |args: &[&str]| std::process::Command::new("git").args(args).output().ok();
    let commit = git(&["rev-parse", "--verify", "HEAD"])
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| {
            matches!(value.len(), 40 | 64)
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .unwrap_or_default();
    let dirty = git(&["status", "--porcelain", "--untracked-files=all"])
        .filter(|output| output.status.success())
        .map(|output| {
            if !output.stdout.is_empty() {
                "true"
            } else if output.stderr.is_empty() {
                "false"
            } else {
                ""
            }
        })
        .unwrap_or_default();
    println!("cargo:rustc-env=ARC_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=ARC_BUILD_DIRTY={dirty}");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"])
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
    {
        println!("cargo:rerun-if-changed={}", head.trim());
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|reference| git(&["rev-parse", "--git-path", reference.trim()]))
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
    {
        println!("cargo:rerun-if-changed={}", reference.trim());
    }
    tauri_build::build();
}
