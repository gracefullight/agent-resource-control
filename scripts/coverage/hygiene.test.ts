import { test, expect } from "bun:test";
import { lstatSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync, truncateSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, dirname, join } from "node:path";
import { gitBytes, auditTrackedEvidence, assertHygiene, isNativeExport, targetBytesOutsideDebug, assertTargetLimit, compactFixtureBuild, TARGET_LIMIT_BYTES } from "./hygiene";

const root = resolve(import.meta.dir, "../..");

test("post-workspace target outside debug stays within 500 MB", () => {
  assertTargetLimit(targetBytesOutsideDebug(root));
}, 120000);

test("target size excludes the exact debug and runner directories only", () => {
  const dir = mkdtempSync(join(tmpdir(), "arc-target-size-"));
  try {
    mkdirSync(join(dir, "target/debug"), { recursive: true });
    writeFileSync(join(dir, "target/debug/large"), "");
    truncateSync(join(dir, "target/debug/large"), TARGET_LIMIT_BYTES + 1);
    mkdirSync(join(dir, "target/av-runner"));
    writeFileSync(join(dir, "target/av-runner/receipt"), "");
    truncateSync(join(dir, "target/av-runner/receipt"), TARGET_LIMIT_BYTES + 1);
    mkdirSync(join(dir, "target/debug-other"));
    writeFileSync(join(dir, "target/debug-other/file"), "1234");
    mkdirSync(join(dir, "target/av-runner-other"));
    writeFileSync(join(dir, "target/av-runner-other/file"), "5678");
    symlinkSync("debug", join(dir, "target/link"));
    const counted = ["target", "target/debug-other", "target/debug-other/file", "target/av-runner-other", "target/av-runner-other/file", "target/link"];
    const allocated = () => counted.reduce((sum, path) => sum + lstatSync(join(dir, path)).blocks * 512, 0);
    expect(targetBytesOutsideDebug(dir)).toBe(allocated());
    expect(() => assertTargetLimit(TARGET_LIMIT_BYTES)).not.toThrow();
    expect(() => assertTargetLimit(TARGET_LIMIT_BYTES + 1)).toThrow("maximum");
    expect(() => assertTargetLimit(-1)).toThrow("maximum");
    expect(() => assertTargetLimit(Number.NaN)).toThrow("maximum");
    // Truncation creates a sparse file: its apparent size must not be charged
    // as allocated disk usage, even in a non-excluded directory.
    truncateSync(join(dir, "target/debug-other/file"), TARGET_LIMIT_BYTES + 1);
    expect(lstatSync(join(dir, "target/debug-other/file")).size).toBe(TARGET_LIMIT_BYTES + 1);
    expect(lstatSync(join(dir, "target/debug-other/file")).blocks * 512).toBeLessThan(TARGET_LIMIT_BYTES);
    expect(targetBytesOutsideDebug(dir)).toBe(allocated());
    expect(() => assertTargetLimit(targetBytesOutsideDebug(dir))).not.toThrow();
    truncateSync(join(dir, "target/debug-other/file"), 4);
    truncateSync(join(dir, "target/av-runner-other/file"), TARGET_LIMIT_BYTES + 1);
    expect(targetBytesOutsideDebug(dir)).toBe(allocated());
    expect(() => assertTargetLimit(targetBytesOutsideDebug(dir))).not.toThrow();
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test("fixture build compaction preserves validated binaries and adjacent receipts", () => {
  const dir = mkdtempSync(join(tmpdir(), "arc-compact-fixture-"));
  try {
    const target = join(dir, "owned/target"), debug = join(target, "debug");
    mkdirSync(join(debug, "deps"), { recursive: true });
    writeFileSync(join(debug, "arcctl"), "binary");
    writeFileSync(join(debug, "deps/object"), "intermediate");
    writeFileSync(join(target, ".rustc_info.json"), "intermediate");
    writeFileSync(join(dir, "owned/receipt.json"), "receipt");
    expect(() => compactFixtureBuild(target, ["missing"])).toThrow();
    expect(readFileSync(join(debug, "deps/object"), "utf8")).toBe("intermediate");
    compactFixtureBuild(target, ["arcctl"]);
    expect(readFileSync(join(debug, "arcctl"), "utf8")).toBe("binary");
    expect(readFileSync(join(dir, "owned/receipt.json"), "utf8")).toBe("receipt");
    expect(() => readFileSync(join(debug, "deps/object"))).toThrow();
    expect(() => readFileSync(join(target, ".rustc_info.json"))).toThrow();
    const other = join(dir, "other"), second = join(dir, "second/target");
    writeFileSync(other, "untouched");
    mkdirSync(join(second, "debug"), { recursive: true });
    symlinkSync(other, join(second, "debug/arcctl"));
    writeFileSync(join(second, "debug/intermediate"), "still present");
    expect(() => compactFixtureBuild(second, ["arcctl"])).toThrow();
    expect(readFileSync(other, "utf8")).toBe("untouched");
    expect(readFileSync(join(second, "debug/intermediate"), "utf8")).toBe("still present");
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test("a symlinked target root still counts its non-debug build tree", () => {
  const dir=mkdtempSync(join(tmpdir(),"arc-target-link-"));
  try {
    const build=join(dir,"build");
    mkdirSync(join(build,"debug"),{recursive:true});
    writeFileSync(join(build,"debug/ignored"),"");
    truncateSync(join(build,"debug/ignored"),TARGET_LIMIT_BYTES+1);
    writeFileSync(join(build,"retained"),"");
    symlinkSync(build,join(dir,"target"));
    const allocated = () => ["target", "target/retained"].reduce((sum, path) => sum + lstatSync(join(dir, path)).blocks * 512, 0);
    expect(targetBytesOutsideDebug(dir)).toBe(allocated());
    truncateSync(join(build,"retained"),TARGET_LIMIT_BYTES+1);
    expect(targetBytesOutsideDebug(dir)).toBe(allocated());
    expect(()=>assertTargetLimit(targetBytesOutsideDebug(dir))).not.toThrow();
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

const trackedCoverageFiles = (dir: string) => gitBytes(dir, ["ls-files", "-z"]).toString().split("\0")
  .filter(path => /\.(profraw|profdata|lcov)$/i.test(path)).sort();
const assertNoTrackedCoverage = (dir: string) => {
  const paths = trackedCoverageFiles(dir);
  if (paths.length) throw Error(`tracked coverage files: ${paths.join(", ")}`);
};

test("ignore probes are deterministic without global excludes and keep durable JSON visible", () => {
  const dir = mkdtempSync(join(tmpdir(), "arc-ignore-"));
  try {
    gitBytes(dir, ["init", "--quiet"]);
    writeFileSync(join(dir, ".gitignore"), readFileSync(resolve(root, ".gitignore")));
    const cases: [string, string][] = [
      ...["profraw", "profdata", "lcov"].map(ext => [`root-probe.${ext}`, `/*.${ext}`] as [string, string]),
      [".arc-verify/probe/runs/id/summary.json", ".arc-verify/**/runs/"],
      [".arc-verify/runs/id/receipt.json", ".arc-verify/**/runs/"],
      [".arc-verify/probe/nested/profiles/payload", ".arc-verify/**/profiles/"],
      [".arc-verify/profiles/payload", ".arc-verify/**/profiles/"],
      ...["profraw", "profdata", "lcov"].map(ext => [`.arc-verify/probe/nested/file.${ext}`, `.arc-verify/**/*.${ext}`] as [string, string]),
      [".arc-verify/probe/nested/raw.json", ".arc-verify/**/raw.json"],
      [".arc-verify/probe/nested/llvm-export.json", ".arc-verify/**/llvm-export.json"],
      [".arc-verify/rust-coverage/baseline/latest.json", ".arc-verify/rust-coverage/baseline/latest.json"],
      [".arc-verify/rust-coverage/tooling/acceptance/runs/probe/binding.json", ".arc-verify/**/runs/"],
      [".arc-verify/rust-coverage/library/clippy.stdout.txt", ".arc-verify/rust-coverage/**"],
      [".arc-verify/rust-coverage/integration/repair-4/source-inventory.json", ".arc-verify/rust-coverage/**"],
    ];
    const negative = [".arc-verify/rust-coverage/contract/accounting.json", ".arc-verify/rust-coverage/contract/source-inventory.json",
      ".arc-verify/rust-coverage/historical/accepted-head-summary.json", ".arc-verify/evidence-hygiene/commands.json",
      ".arc-verify/rust-coverage/historical/accepted-head-commands.json", "docs/example.profraw", "fixtures/example.profdata", "fixtures/example.lcov", "scripts/coverage/hygiene.test.ts"];
    // A fresh repository has no info/exclude entries. Explicit core.excludesFile
    // bypasses the user's global excludes without changing their configuration.
    for (const [path, pattern] of cases) {
      const output = gitBytes(dir, ["-c", "core.excludesFile=/dev/null", "check-ignore", "--no-index", "--verbose", "--stdin"], path + "\n").toString();
      expect(output.trim().split("\t")[0].split(":").slice(2).join(":")).toBe(pattern);
      expect(output.trim().split("\t")[1]).toBe(path);
    }
    const result = Bun.spawnSync(["git", "-c", "core.excludesFile=/dev/null", "check-ignore", "--no-index", "--verbose", "--non-matching", "--stdin"],
      { cwd: dir, stdin: Buffer.from(negative.join("\n") + "\n"), stdout: "pipe", stderr: "pipe" });
    expect([0, 1]).toContain(result.exitCode); expect(result.stderr.toString()).toBe("");
    const rows = result.stdout.toString().trim().split("\n");
    expect(rows.length).toBe(negative.length);
    for (const [index, row] of rows.entries()) {
      const [rule, path] = row.split("\t"), pattern = rule.split(":").slice(2).join(":");
      expect(path).toBe(negative[index]);
      // --verbose reports matching retention negations. --quiet checks their
      // actual semantics: every negative probe must remain unignored.
      expect(pattern === "" || pattern.startsWith("!")).toBe(true);
      const probe = Bun.spawnSync(["git", "-c", "core.excludesFile=/dev/null", "check-ignore", "--no-index", "--quiet", "--", path],
        { cwd: dir, stdout: "pipe", stderr: "pipe" });
      expect(probe.exitCode).toBe(1); expect(probe.stderr.toString()).toBe("");
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test("index audit catches renamed native exports, directory and extension violations and count limits", () => {
  const dir = mkdtempSync(join(tmpdir(), "arc-audit-"));
  try {
    gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, dir]);
    gitBytes(dir, ["read-tree", "--empty"]);
    expect(() => assertNoTrackedCoverage(dir)).not.toThrow();
    const files = {
      ".arc-verify/another-area/renamed.txt": { type: "llvm.coverage.json.export", data: [] },
      ".arc-verify/unexpected/opaque": { data: [{ files: [{ filename: "source.rs", segments: [] }] }] },
      ".arc-verify/unexpected/branches.bin": { data: [{ files: [{ filename: "source.rs", branches: [] }] }] },
      ".arc-verify/unexpected/runs/id/summary.json": { pass: true },
      ".arc-verify/unexpected/profiles/data": "profile",
      ".arc-verify/unexpected/file.profraw": "profile",
      ".arc-verify/unexpected/file.profdata": "profile",
      ".arc-verify/unexpected/file.lcov": "coverage",
      ".arc-verify/unexpected/raw.json": {},
      ".arc-verify/unexpected/llvm-export.json": {},
      ".arc-verify/rust-coverage/baseline/latest.json": {},
      ".arc-verify/evidence-hygiene/summary.json": { pass: true, files: [], crates: [], rawSha256: "provenance" },
    };
    for (const [path, value] of Object.entries(files)) {
      mkdirSync(dirname(resolve(dir, path)), { recursive: true });
      writeFileSync(resolve(dir, path), JSON.stringify(value));
    }
    const outside = ["root.profraw", "docs/example.profdata", "fixtures/example.lcov"];
    for (const path of outside) {
      mkdirSync(dirname(resolve(dir, path)), { recursive: true });
      writeFileSync(resolve(dir, path), "coverage");
    }
    gitBytes(dir, ["add", "--", ".arc-verify", ...outside]);
    expect(trackedCoverageFiles(dir)).toEqual([...Object.keys(files).filter(path => /\.(profraw|profdata|lcov)$/i.test(path)), ...outside].sort());
    expect(() => assertNoTrackedCoverage(dir)).toThrow("tracked coverage files");
    const audit = auditTrackedEvidence(dir);
    expect(audit.prohibited.map(e => e.path).sort()).toEqual(Object.keys(files).filter(p => !p.endsWith("evidence-hygiene/summary.json")).sort());
    expect(() => assertHygiene(audit)).toThrow("prohibited tracked artifacts");
    const valid = { ...audit, prohibited: [], rustCoverage: { files: 149, bytes: 0 }, newEvidenceHygieneFiles: 149 };
    expect(() => assertHygiene(valid)).not.toThrow();
    expect(() => assertHygiene({ ...valid, rustCoverage: { files: 150, bytes: 0 } })).toThrow("count 150");
    expect(() => assertHygiene({ ...valid, newEvidenceHygieneFiles: 150 })).toThrow("count 150");
    expect(isNativeExport(Buffer.from("\uFEFF" + JSON.stringify({ type: "llvm.coverage.json.export" })))).toBe(true);
    expect(isNativeExport(Buffer.from("not JSON"))).toBe(false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 30000);
