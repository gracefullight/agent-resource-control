import { test, expect } from "bun:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, chmodSync, unlinkSync, symlinkSync, rmSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { checkPreservation } from "./preservation";
import { root } from "./acceptance";
import { ACCEPTED_HEAD, gitBytes, checkExactPreservation, indexEntries } from "./hygiene";

test("protected bytes/modes equal accepted HEAD; harness path/type/mode survive", () => {
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


test("amendment 4 permits harness byte changes but retains harness structure and migration bytes", () => {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "arc-harness-preservation-")));
  try {
    gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, dir]);
    gitBytes(dir, ["read-tree", ACCEPTED_HEAD]);
    const entries = [...indexEntries(dir, "crates"), ...indexEntries(dir, "scripts/harness/validate.sh")];
    gitBytes(dir, ["checkout-index", "--stdin"], entries.map(e => e.path).join("\n") + "\n");
    const harness = resolve(dir, "scripts/harness/validate.sh"), bytes = readFileSync(harness);
    writeFileSync(harness, Buffer.concat([bytes, Buffer.from("\n# authorized byte change\n")]));
    expect(checkExactPreservation(dir).pass).toBe(true);
    expect(checkPreservation(dir).pass).toBe(true);
    gitBytes(dir, ["add", "--", "scripts/harness/validate.sh"]);
    expect(checkPreservation(dir).pass).toBe(true);
    chmodSync(harness, 0o644);
    expect(() => checkPreservation(dir)).toThrow("local bytes/mode changed");
    chmodSync(harness, 0o755);
    const entry = indexEntries(dir, "scripts/harness/validate.sh")[0];
    gitBytes(dir, ["update-index", "--cacheinfo", `100644,${entry.oid},${entry.path}`]);
    expect(() => checkPreservation(dir)).toThrow("index bytes/mode changed");
    gitBytes(dir, ["update-index", "--cacheinfo", `100755,${entry.oid},${entry.path}`]);
    unlinkSync(harness); symlinkSync("missing", harness);
    expect(() => checkPreservation(dir)).toThrow("regular file required");
    unlinkSync(harness); writeFileSync(harness, bytes); chmodSync(harness, 0o755);
    const migration = resolve(dir, "crates/arc-storage/migrations/0001_phase0.sql");
    writeFileSync(migration, Buffer.concat([readFileSync(migration), Buffer.from("\n")]));
    expect(() => checkPreservation(dir)).toThrow("protected bytes changed");
    expect(() => checkExactPreservation(dir)).toThrow("local bytes/mode changed");
    gitBytes(dir, ["add", "--", "crates/arc-storage/migrations/0001_phase0.sql"]);
    expect(() => checkExactPreservation(dir)).toThrow("index bytes/mode changed");
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 1800000);
