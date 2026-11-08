import { test, expect } from "bun:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, chmodSync, unlinkSync, symlinkSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { root } from "./acceptance";
import { ACCEPTED_HEAD, gitBytes, checkExactPreservation, indexEntries } from "./hygiene";

test("all protected local/index paths, bytes and modes equal accepted HEAD", () => {
  expect(checkExactPreservation(root)).toEqual({ acceptedHead: ACCEPTED_HEAD, files: 84, pass: true });
});

test("exact preservation rejects additions, removals, byte changes, modes and symlinks", () => {
  const dir = mkdtempSync(join(tmpdir(), "arc-preservation-"));
  try {
    gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, dir]);
    gitBytes(dir, ["read-tree", ACCEPTED_HEAD]);
    const protectedFiles = indexEntries(dir, "crates").filter(e => e.path.endsWith(".rs") || e.path.endsWith("/0001_phase0.sql"))
      .concat(indexEntries(dir, "scripts/harness/validate.sh"));
    gitBytes(dir, ["checkout-index", "--stdin"], protectedFiles.map(e => e.path).join("\n") + "\n");
    expect(checkExactPreservation(dir).pass).toBe(true);
    const entry = protectedFiles[0], path = resolve(dir, entry.path), bytes = readFileSync(path);
    writeFileSync(path, Buffer.concat([bytes, Buffer.from("\n")]));
    expect(() => checkExactPreservation(dir)).toThrow("local bytes/mode changed");
    writeFileSync(path, bytes);
    chmodSync(path, 0o755);
    expect(() => checkExactPreservation(dir)).toThrow("local bytes/mode changed");
    chmodSync(path, 0o644);
    unlinkSync(path);
    expect(() => checkExactPreservation(dir)).toThrow("local protected path set changed");
    symlinkSync("missing-target", path);
    expect(() => checkExactPreservation(dir)).toThrow("regular file required");
    unlinkSync(path); writeFileSync(path, bytes);
    const added = "crates/arc-core/src/added-untracked.rs";
    mkdirSync(dirname(resolve(dir, added)), { recursive: true });
    writeFileSync(resolve(dir, added), "// additional file\n");
    expect(() => checkExactPreservation(dir)).toThrow("local protected path set changed");
    gitBytes(dir, ["add", "--", added]);
    expect(() => checkExactPreservation(dir)).toThrow("index protected path set changed");
    gitBytes(dir, ["update-index", "--force-remove", "--", added]);
    unlinkSync(resolve(dir, added));
    gitBytes(dir, ["update-index", "--cacheinfo", `100755,${entry.oid},${entry.path}`]);
    expect(() => checkExactPreservation(dir)).toThrow("index bytes/mode changed");
    gitBytes(dir, ["update-index", "--cacheinfo", `${entry.mode},${entry.oid},${entry.path}`]);
    gitBytes(dir, ["update-index", "--force-remove", "--", entry.path]);
    expect(() => checkExactPreservation(dir)).toThrow("index protected path set changed");
    gitBytes(dir, ["update-index", "--add", "--cacheinfo", `${entry.mode},${entry.oid},${entry.path}`]);
    expect(checkExactPreservation(dir).pass).toBe(true);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 30000);
