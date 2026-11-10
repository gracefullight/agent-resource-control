import { test, expect } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, dirname, join } from "node:path";
import { root } from "./acceptance";
import { hash } from "./source";
import { ACCEPTED_HEAD, HYGIENE, gitBytes, auditTrackedEvidence, assertHygiene, isNativeExport } from "./hygiene";

const trackedCoverageFiles = (dir: string) => gitBytes(dir, ["ls-files", "-z"]).toString().split("\0")
  .filter(path => /\.(profraw|profdata|lcov)$/i.test(path)).sort();
const assertNoTrackedCoverage = (dir: string) => {
  const paths = trackedCoverageFiles(dir);
  if (paths.length) throw Error(`tracked coverage files: ${paths.join(", ")}`);
};

test("final index has bounded evidence and no prohibited artifact anywhere under .arc-verify", () => {
  const audit = auditTrackedEvidence(root);
  assertHygiene(audit);
  expect(audit.rustCoverage.files).toBeGreaterThan(0);
  expect(audit.prohibited).toEqual([]);
  expect(() => assertNoTrackedCoverage(root)).not.toThrow();
});

test("selected contracts and historical evidence retain their bytes and provenance", () => {
  const retention = JSON.parse(readFileSync(resolve(root, HYGIENE, "retention.json"), "utf8"));
  expect(retention.acceptedHead).toBe(ACCEPTED_HEAD);
  expect([...new Set(retention.retained.map((e: any) => e.category))].sort()).toEqual(["accounting", "receipt", "review", "summary", "uncovered"]);
  const tracked = new Set(gitBytes(root, ["ls-files", "-z", "--", ".arc-verify/rust-coverage"]).toString().split("\0"));
  for (const selected of retention.retained) {
    expect(tracked.has(selected.destination)).toBe(true);
    const bytes = readFileSync(resolve(root, selected.destination));
    if (selected.action !== "derive-compact") expect(hash(bytes)).toBe(selected.source.sha256);
    else {
      const receipt = JSON.parse(bytes.toString());
      expect(receipt.classification).toBe("Historical");
      expect(receipt.provenance.originalSha256).toBe(selected.source.sha256);
      expect(receipt.provenance.storageCommit).toBe(ACCEPTED_HEAD);
      expect(receipt.commands.length).toBeGreaterThan(0);
      for (const command of receipt.commands) {
        expect(command.stdoutSha256).toMatch(/^[a-f0-9]{64}$/);
        expect(command.stderrSha256).toMatch(/^[a-f0-9]{64}$/);
        expect(command.stdout).toBeUndefined();
        expect(command.stderr).toBeUndefined();
      }
    }
  }
});

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
