"""Fresh task-5 receipts; reuse the retained collector without rewriting history."""
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
spec = importlib.util.spec_from_file_location("previous_record", OUT.parent / "record.py")
record = importlib.util.module_from_spec(spec)
spec.loader.exec_module(record)
record.ROOT = ROOT
record.OUT = OUT
record.ENV["CARGO_TARGET_DIR"] = str(ROOT / "target")

if __name__ == "__main__":
    sys.exit(record.gate(sys.argv[1], sys.argv[2:]))
