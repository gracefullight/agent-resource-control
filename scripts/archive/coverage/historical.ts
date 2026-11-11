import { mkdtempSync, rmSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gitBytes, indexEntries } from "./hygiene";

// Original source contracts are evaluated against immutable Git objects, never
// the feature checkout or a newly selected baseline. Mutations belong to fixtures.
export function withHistoricalCheckout<T>(root: string, revision: string, run: (dir: string) => T): T {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "arc-historical-")));
  try {
    gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, dir]);
    gitBytes(dir, ["read-tree", revision]);
    const entries = [...indexEntries(dir, "crates"), ...indexEntries(dir, "scripts/harness/validate.sh")];
    gitBytes(dir, ["checkout-index", "--stdin"], entries.map(e => e.path).join("\n") + "\n");
    return run(dir);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}
