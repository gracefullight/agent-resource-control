"""Use Cargo's worktree-local target after shared-cache permission denial."""
import json
from pathlib import Path
import sys
import record as r

original = r.OUT
r.OUT = original / "local-cache"
r.OUT.mkdir(exist_ok=True)
r.ENV["CARGO_TARGET_DIR"] = str(r.ROOT / "target")
if sys.argv[1] == "prepare":
    context = json.loads((original / "execution-context.json").read_text())
    context["recordedAt"] = r.now()
    context["environment"]["CARGO_TARGET_DIR"] = r.ENV["CARGO_TARGET_DIR"]
    context["cacheChange"] = "Shared review target/debug/.cargo-lock denied. Use standard worktree-local target; offline cache only. Prior failures preserved."
    r.save("execution-context.json", context)
    print("Prepared local-cache context; no dependency installation or download.")
else:
    r.gate(sys.argv[1])
