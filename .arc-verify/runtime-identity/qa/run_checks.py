"""Independent fixed-gate capture; writes only assigned QA artifacts."""
import ctypes
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import subprocess
import tempfile
import time

OUT = Path(__file__).resolve().parent
BASELINE = "073802555b639aa2fec183d93f748d9437cbbcad"
COMMANDS = {
    "fmt": ["cargo", "fmt", "--all", "--", "--check"],
    "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    "test": ["cargo", "test", "--workspace"],
}


def capture(argv):
    run = subprocess.run(argv, capture_output=True, text=True, timeout=20)
    return {"argv": argv, "exitCode": run.returncode, "stdout": run.stdout, "stderr": run.stderr}


def source():
    paths = subprocess.check_output([
        "git", "ls-files", "-z", "--", "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"
    ]).decode().split("\0")
    paths += ["crates/arc-core/tests/runtime.rs", "crates/arc-daemon/tests/support/mod.rs"]
    files = {
        path: {"sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest(),
               "mode": oct(Path(path).stat().st_mode & 0o777)}
        for path in sorted(set(paths)) if path
    }
    digest = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"sourceFingerprint": digest, "sourceFiles": files}


def save(name, data):
    (OUT / name).write_text(json.dumps(data, indent=2) + "\n")


def metadata(path):
    st = path.lstat()
    return {"path": str(path), "uid": st.st_uid, "mode": oct(st.st_mode & 0o7777),
            "inode": st.st_ino, "typeBits": oct(st.st_mode & 0o170000)}


if __name__ == "__main__":
    started = datetime.datetime.now(datetime.timezone.utc)
    environment = {
        "collectedAt": started.isoformat(), "baseline": BASELINE,
        "deadline": "2026-11-08T09:39:05Z", "maxParallel": 3,
        "thisActorConcurrency": 1, "commandTimeoutSeconds": 120,
        "attemptLimit": 3, "noInjectedOmaClaim": True,
        "resources": {name: resource.getrlimit(limit) for name, limit in [
            ("cpu", resource.RLIMIT_CPU), ("addressSpace", resource.RLIMIT_AS),
            ("processes", resource.RLIMIT_NPROC), ("files", resource.RLIMIT_NOFILE)]},
        "platform": platform.platform(), "system": platform.system(),
        "environment": {key: os.environ.get(key) for key in [
            "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_NET_OFFLINE", "RUSTUP_TOOLCHAIN"]},
        "commands": {},
    }
    for name, argv in {
        "head": ["git", "rev-parse", "HEAD"],
        "baselineTree": ["git", "rev-parse", BASELINE + "^{tree}"],
        "platform": ["uname", "-srm"],
        "rustc": ["rustc", "--version"], "cargo": ["cargo", "--version"],
        "clippy": ["cargo", "clippy", "--version"], "rustfmt": ["rustfmt", "--version"],
        "root": ["getconf", "DARWIN_USER_TEMP_DIR"],
        "status": ["git", "status", "--short", "--untracked-files=normal", "--", ".", ":!av.yaml"],
    }.items():
        environment["commands"][name] = capture(argv)
    root_record = environment["commands"]["root"]
    if platform.system() == "Darwin" and root_record["exitCode"] == 0:
        root = Path(root_record["stdout"].strip())
        # Independent native confstr query, using the macOS constant from unistd.h.
        libc = ctypes.CDLL(None)
        libc.confstr.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_size_t]
        libc.confstr.restype = ctypes.c_size_t
        length = libc.confstr(65537, None, 0)
        buffer = ctypes.create_string_buffer(length)
        returned = libc.confstr(65537, buffer, length)
        environment["confstr"] = {"selector": 65537, "length": length, "returned": returned,
                                   "root": os.fsdecode(buffer.value), "agreesWithGetconf": Path(os.fsdecode(buffer.value)) == root}
        environment["rootMetadataBefore"] = metadata(root)
        probe = Path(tempfile.mkdtemp(prefix="qa-", dir=root))
        (probe / "write").write_bytes(b"owned QA root-writability probe\n")
        environment["fixtureWriteProbe"] = {"path": str(probe), "writeSucceeded": True,
                                             "metadata": metadata(probe)}
        (probe / "write").unlink()
        probe.rmdir()
        environment["fixtureWriteProbe"]["ownedProbeRemoved"] = True
        environment["runtimeBefore"] = [metadata(p) for p in [root / "arc", root / f"arc-instance-{os.geteuid()}",
            root / f"arc-instance-{os.geteuid()}" / "daemon.lock", root / f"arc-fixtures-{os.geteuid()}.lock"] if p.exists()]
    migration = "crates/arc-storage/migrations/0001_phase0.sql"
    baseline_bytes = subprocess.check_output(["git", "show", BASELINE + ":" + migration])
    final_bytes = Path(migration).read_bytes()
    environment["migration"] = {"baselineSha256": hashlib.sha256(baseline_bytes).hexdigest(),
                                 "finalSha256": hashlib.sha256(final_bytes).hexdigest(),
                                 "byteEqual": baseline_bytes == final_bytes}
    save("environment.json", environment)
    before = source()
    save("source-before.json", before)
    results = {}
    child_env = os.environ.copy()
    child_env["CARGO_NET_OFFLINE"] = "true"
    # Keep generated compiler artifacts within this assignment's QA directory.
    child_env["CARGO_TARGET_DIR"] = str(OUT / "target")
    for name, argv in COMMANDS.items():
        print("Running " + " ".join(argv), flush=True)
        start = time.monotonic()
        timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
        with (OUT / (name + ".txt")).open("w") as log:
            try:
                run = subprocess.run(argv, env=child_env, stdout=log, stderr=subprocess.STDOUT, timeout=120)
                exit_code = run.returncode
            except subprocess.TimeoutExpired:
                exit_code = 124
        results[name] = {"argv": argv, "exitCode": exit_code, "startedAt": timestamp,
                         "elapsedSeconds": round(time.monotonic() - start, 3),
                         "output": str((OUT / (name + ".txt")).relative_to(Path.cwd())),
                         "environment": {"CARGO_NET_OFFLINE": "true", "CARGO_TARGET_DIR": child_env["CARGO_TARGET_DIR"]}}
        save("commands.json", results)
        print(f"{name}: exit {exit_code}", flush=True)
        if exit_code:
            print((OUT / (name + ".txt")).read_text()[-14000:], flush=True)
    after = source()
    after["unchangedDuringChecks"] = before == after
    save("source.json", after)
    if platform.system() == "Darwin" and root_record["exitCode"] == 0:
        save("runtime-after.json", [metadata(p) for p in [root, root / "arc", root / f"arc-instance-{os.geteuid()}",
            root / f"arc-instance-{os.geteuid()}" / "daemon.lock", root / f"arc-fixtures-{os.geteuid()}.lock"] if p.exists()])
