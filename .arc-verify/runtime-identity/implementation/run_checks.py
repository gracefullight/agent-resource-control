"""Record the assigned Cargo commands without changing their arguments."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import time

OUT = Path(__file__).resolve().parent
COMMANDS = {
    "fmt": ["cargo", "fmt", "--all", "--", "--check"],
    "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    "test": ["cargo", "test", "--workspace"],
}


def source():
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--", "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
    ).decode().split("\0")
    paths += ["crates/arc-core/tests/runtime.rs", "crates/arc-daemon/tests/support/mod.rs"]
    files = {
        path: {"sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest(),
               "mode": oct(Path(path).stat().st_mode & 0o777)}
        for path in sorted(set(paths)) if path
    }
    digest = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"sourceFingerprint": digest, "sourceFiles": files}


if __name__ == "__main__":
    before = source()
    results = {}
    for name, command in COMMANDS.items():
        print("Running " + " ".join(command), flush=True)
        start = time.monotonic()
        timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
        with (OUT / (name + ".txt")).open("w") as log:
            try:
                result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=120)
                exit_code = result.returncode
            except subprocess.TimeoutExpired:
                exit_code = 124
        results[name] = {"argv": command, "exitCode": exit_code, "startedAt": timestamp,
                         "elapsedSeconds": round(time.monotonic() - start, 3),
                         "output": str((OUT / (name + ".txt")).relative_to(Path.cwd()))}
        (OUT / "commands.json").write_text(json.dumps(results, indent=2) + "\n")
        print(f"{name}: exit {exit_code}", flush=True)
        if exit_code:
            print((OUT / (name + ".txt")).read_text()[-12000:], flush=True)
            break
    after = source()
    after["unchangedDuringChecks"] = before == after
    (OUT / "source.json").write_text(json.dumps(after, indent=2) + "\n")
