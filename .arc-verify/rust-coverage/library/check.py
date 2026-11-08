"""Bounded command receipts for the library coverage task."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path.cwd()
OUT = ROOT / ".arc-verify/rust-coverage/library"


def identity():
    result = subprocess.run(
        ["bun", "-e", 'import {inputIdentity} from "./scripts/coverage/source"; '
         'console.log(JSON.stringify(inputIdentity(process.cwd())))'],
        capture_output=True, text=True, check=True, timeout=30,
    )
    return json.loads(result.stdout)


label, *command = sys.argv[1:]
before = identity()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
clock = time.monotonic()
try:
    result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=180)
    stdout, stderr, code = result.stdout, result.stderr, result.returncode
except subprocess.TimeoutExpired as error:
    stdout, stderr, code = error.stdout or b"", error.stderr or b"", 124
after = identity()
receipt = {
    "task": "task-3", "actor": "library-coverage", "label": label,
    "argv": command, "cwd": str(ROOT), "startedAt": started,
    "elapsedSeconds": time.monotonic() - clock, "exit": code,
    "localHead": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "before": before, "after": after,
    "sourceStable": before == after,
    "stdoutSha256": hashlib.sha256(stdout).hexdigest(),
    "stderrSha256": hashlib.sha256(stderr).hexdigest(),
}
(OUT / f"{label}.stdout.txt").write_bytes(stdout)
(OUT / f"{label}.stderr.txt").write_bytes(stderr)
(OUT / f"{label}.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps({k: receipt[k] for k in ["label", "argv", "exit", "elapsedSeconds", "sourceStable"]}))
print(stdout.decode(errors="replace")[-5000:])
print(stderr.decode(errors="replace")[-5000:])
sys.exit(code if code >= 0 else 128 - code)
