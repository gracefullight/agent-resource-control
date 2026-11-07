"""Read-only receiving-checkout check of every indexed task-1 artifact."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[4]
manifest = json.loads(Path(__file__).with_name("delivery.json").read_text())
failures = []
for name, expected in manifest["files"].items():
    path = root / name
    try:
        data = path.read_bytes()
    except OSError as error:
        failures.append(dict(path=name, error=str(error)))
        continue
    actual = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    if actual != expected:
        failures.append(dict(path=name, expected=expected, actual=actual))
print(json.dumps(dict(verified=len(manifest["files"]), failures=failures)))
raise SystemExit(bool(failures))
