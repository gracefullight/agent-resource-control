import { readFileSync, lstatSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

export const ACCEPTED_HEAD = "aacb3b4359f13c31c0705c0cffc5bf1ac00f387e";
export const HYGIENE = ".arc-verify/evidence-hygiene";

export function gitBytes(root: string, args: string[], stdin?: string): Buffer {
  const result = Bun.spawnSync(["git", ...args], { cwd: root, stdin: stdin === undefined ? undefined : Buffer.from(stdin), stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) throw Error(`HYGIENE: git ${args.join(" ")}: ${result.stderr}`);
  return result.stdout;
}

export type IndexEntry = { path: string; mode: string; oid: string };
export function indexEntries(root: string, scope: string): IndexEntry[] {
  return gitBytes(root, ["ls-files", "--stage", "-z", "--", scope]).toString().split("\0").filter(Boolean).map(row => {
    const [metadata, path] = row.split("\t"), [mode, oid, stage] = metadata.split(" ");
    if (stage !== "0") throw Error(`HYGIENE: unresolved index stage: ${path}`);
    return { path, mode, oid };
  });
}

// Check every blob regardless of extension. Summary/accounting JSON has no
// native export type or data[].files[] segments and remains trackable.
export function isNativeExport(bytes: Buffer): boolean {
  let value: any;
  try { value = JSON.parse(bytes.toString("utf8").replace(/^\uFEFF/, "")); } catch { return false; }
  if (!value || typeof value !== "object") return false;
  return value.type === "llvm.coverage.json.export" ||
    (Array.isArray(value.data) && value.data.some((unit: any) => Array.isArray(unit?.files) && unit.files.some((file: any) =>
      typeof file?.filename === "string" && (Array.isArray(file.segments) || Array.isArray(file.branches)))));
}

export function prohibitedReasons(path: string, bytes: Buffer): string[] {
  const reasons: string[] = [];
  if (/(?:^|\/)(runs|profiles)\//.test(path)) reasons.push("runtime directory");
  if (/\.(profraw|profdata|lcov)$/i.test(path)) reasons.push("coverage extension");
  if (/(?:^|\/)(raw|llvm-export)\.json$/i.test(path)) reasons.push("raw export filename");
  if (path === ".arc-verify/rust-coverage/baseline/latest.json") reasons.push("runtime pointer");
  if (isNativeExport(bytes)) reasons.push("native LLVM export content");
  return reasons;
}

export function auditTrackedEvidence(root: string) {
  const entries = indexEntries(root, ".arc-verify");
  const prohibited: { path: string; reasons: string[] }[] = [];
  let offset = 0;
  const blobs = entries.length ? gitBytes(root, ["cat-file", "--batch"], entries.map(e => e.oid).join("\n") + "\n") : Buffer.alloc(0);
  const rows = entries.map(entry => {
    const end = blobs.indexOf(10, offset);
    const [oid, type, sizeText] = blobs.subarray(offset, end).toString().split(" ");
    const size = Number(sizeText);
    if (end < offset || oid !== entry.oid || type !== "blob" || !Number.isSafeInteger(size)) throw Error(`HYGIENE: invalid blob response: ${entry.path}`);
    const bytes = blobs.subarray(end + 1, end + 1 + size);
    offset = end + size + 2;
    const reasons = prohibitedReasons(entry.path, bytes);
    if (reasons.length) prohibited.push({ path: entry.path, reasons });
    return { ...entry, bytes: size };
  });
  const totals = (prefix: string) => {
    const selected = rows.filter(e => e.path.startsWith(prefix));
    return { files: selected.length, bytes: selected.reduce((sum, e) => sum + e.bytes, 0) };
  };
  const existing = new Set(gitBytes(root, ["ls-tree", "-r", "--name-only", ACCEPTED_HEAD, "--", HYGIENE]).toString().trim().split("\n"));
  return { rustCoverage: totals(".arc-verify/rust-coverage/"), allArcVerify: totals(".arc-verify/"),
    newEvidenceHygieneFiles: rows.filter(e => e.path.startsWith(HYGIENE + "/") && !existing.has(e.path)).length, prohibited };
}

export function assertHygiene(audit: ReturnType<typeof auditTrackedEvidence>) {
  if (audit.rustCoverage.files >= 150) throw Error(`HYGIENE: rust-coverage count ${audit.rustCoverage.files} must be <150`);
  if (audit.newEvidenceHygieneFiles >= 150) throw Error(`HYGIENE: new evidence-hygiene count ${audit.newEvidenceHygieneFiles} must be <150`);
  if (audit.prohibited.length) throw Error(`HYGIENE: prohibited tracked artifacts: ${JSON.stringify(audit.prohibited)}`);
}

const protectedPath = (path: string) => (path.startsWith("crates/") && path.endsWith(".rs")) ||
  path === "crates/arc-storage/migrations/0001_phase0.sql" || path === "scripts/harness/validate.sh";

export function checkExactPreservation(root: string) {
  const accepted = gitBytes(root, ["ls-tree", "-r", "-z", ACCEPTED_HEAD, "--", "crates", "scripts/harness/validate.sh"])
    .toString().split("\0").filter(Boolean).map(row => {
      const [meta, path] = row.split("\t"), [mode, type, oid] = meta.split(" ");
      if (type !== "blob") throw Error(`PRESERVATION: unexpected accepted object: ${path}`);
      return { path, mode, oid };
    }).filter(e => protectedPath(e.path));
  const indexed = [...indexEntries(root, "crates"), ...indexEntries(root, "scripts/harness/validate.sh")].filter(e => protectedPath(e.path));
  const paths = (entries: { path: string }[]) => entries.map(e => e.path).sort();
  if (JSON.stringify(paths(accepted)) !== JSON.stringify(paths(indexed))) throw Error("PRESERVATION: index protected path set changed");
  const local: string[] = [];
  const visit = (dir: string) => {
    for (const name of readdirSync(resolve(root, dir))) {
      const path = join(dir, name), stat = lstatSync(resolve(root, path));
      if (stat.isDirectory()) visit(path);
      else if (protectedPath(path)) local.push(path);
    }
  };
  visit("crates");
  // Include a missing harness in the byte/state check below.
  local.push("scripts/harness/validate.sh");
  if (JSON.stringify(paths(accepted)) !== JSON.stringify(local.sort())) throw Error("PRESERVATION: local protected path set changed");
  for (const entry of accepted) {
    const current = indexed.find(e => e.path === entry.path)!;
    if (current.mode !== entry.mode || current.oid !== entry.oid) throw Error(`PRESERVATION: index bytes/mode changed: ${entry.path}`);
    const path = resolve(root, entry.path), stat = lstatSync(path);
    if (!stat.isFile() || stat.isSymbolicLink()) throw Error(`PRESERVATION: regular file required: ${entry.path}`);
    const mode = stat.mode & 0o111 ? "100755" : "100644";
    if (mode !== entry.mode || !readFileSync(path).equals(gitBytes(root, ["show", `${ACCEPTED_HEAD}:${entry.path}`])))
      throw Error(`PRESERVATION: local bytes/mode changed: ${entry.path}`);
  }
  return { acceptedHead: ACCEPTED_HEAD, files: accepted.length, pass: true };
}
