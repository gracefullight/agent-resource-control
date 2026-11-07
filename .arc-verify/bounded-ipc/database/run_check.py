"""Run exact check argv, recording native exit/output without changing gates."""

import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[3]
evidence = Path(__file__).resolve().parent
name, timeout, *argv = sys.argv[1:]
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
began = time.monotonic()
environment = os.environ.copy()
environment["CARGO_NET_OFFLINE"] = "true"
environment["CARGO_TARGET_DIR"] = str(root / "target/database-worker-evidence")
timed_out = False
with (evidence / f"{name}.txt").open("w") as output:
    process = subprocess.Popen(argv, cwd=root, env=environment, stdout=output,
                               stderr=subprocess.STDOUT, start_new_session=True)
    try:
        code = process.wait(timeout=float(timeout))
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        code = 124
record = {"argv": argv, "exitCode": code, "timedOut": timed_out,
          "startedAt": started, "elapsedSeconds": time.monotonic() - began,
          "output": str((evidence / f"{name}.txt").relative_to(root)),
          "environment": {key: environment[key] for key in ("CARGO_NET_OFFLINE", "CARGO_TARGET_DIR")}}
(evidence / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps(record))
sys.exit(code)
