// Supplemental four-library diagnostic. This never substitutes for the root gate.
import { mkdirSync, readdirSync, readFileSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { command } from "../../../scripts/coverage/commands";
import { BASELINE, PINS } from "../../../scripts/coverage/config";
import { freshReport, requireVersion } from "../../../scripts/coverage/gate";
import { hash, inputIdentity, sourceInventory } from "../../../scripts/coverage/source";

const root = process.cwd();
const head = Bun.spawnSync(["git", "rev-parse", "HEAD"], { cwd: root });
if (head.exitCode) throw Error("cannot inspect local HEAD");
const localHead = head.stdout.toString().trim();
const owned = ["arc-core", "arc-detector", "arc-storage", "arc-ipc"];
const id = `libraries-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const dir = resolve(root, ".arc-verify/rust-coverage/library", id);
mkdirSync(dir, { recursive: true });
const save = (name: string, value: unknown) => writeFileSync(join(dir, name), JSON.stringify(value, null, 2) + "\n");
const before = inputIdentity(root), inventory = sourceInventory(root).filter(f => owned.includes(f.crate));
save("inputs.json", before); save("source-inventory.json", inventory);
const target = resolve(root, "target", id);
const env = { ...process.env, RUSTUP_TOOLCHAIN: PINS.rust, CARGO_NET_OFFLINE: "true", CARGO_TERM_COLOR: "never", CARGO_LLVM_COV_TARGET_DIR: target, LLVM_PROFILE_FILE_NAME: "arc-%p-%m.profraw" };
const rust = ["rustup", "run", PINS.rust];
async function run(argv: string[]) {
  const result = await command(argv, dir, root, env, 120000);
  if (result.exit) throw Error(`command exited ${result.exit}: ${argv.join(" ")}`);
  return result;
}
requireVersion(Bun.version, PINS.bun, "Bun");
const rustc = (await run([...rust, "rustc", "-Vv"])).stdout;
if (!rustc.includes(`release: ${PINS.rust}\n`)) throw Error("Rust version mismatch");
requireVersion((await run([...rust, "cargo", "llvm-cov", "--version"])).stdout, `cargo-llvm-cov ${PINS.cargoLlvmCov}`, "cargo-llvm-cov");
const sysroot = (await run([...rust, "rustc", "--print", "sysroot"])).stdout.trim();
const host = rustc.match(/^host: (.+)$/m)![1];
const llvm = join(sysroot, "lib/rustlib", host, "bin");
Object.assign(env, { LLVM_COV: join(llvm, "llvm-cov"), LLVM_PROFDATA: join(llvm, "llvm-profdata") });
const versions: Record<string, string> = { bun: Bun.version, rustc, host, cargoLlvmCov: PINS.cargoLlvmCov };
for (const tool of ["llvm-cov", "llvm-profdata"]) {
  versions[tool] = (await run([join(llvm, tool), "--version"])).stdout;
  if (!versions[tool].includes(rustc.match(/LLVM version: (\S+)/)![1])) throw Error("LLVM version mismatch");
}
save("versions.json", versions);
const metadata = JSON.parse((await run([...rust, "cargo", "metadata", "--format-version", "1", "--locked", "--offline"])).stdout);
const roots = [sysroot, `/rustc/${rustc.match(/^commit-hash: (.+)$/m)![1]}`, ...metadata.packages.filter((p: any) => !metadata.workspace_members.includes(p.id)).map((p: any) => dirname(p.manifest_path))];
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const ignore = roots.map(p => `^${escape(p)}(/|$)`).join("|");
save("non-workspace-roots.json", roots);
await run([...rust, "cargo", "llvm-cov", ...owned.flatMap(c => ["-p", c]), "--all-targets", "--locked", "--offline", "--no-report", "--no-cfg-coverage", "--no-cfg-coverage-nightly"]);
const started = Date.now();
for (const format of ["lcov", "json"]) await run([...rust, "cargo", "llvm-cov", "report", `--${format}`, "--output-path", join(dir, `raw.${format}`), "--failure-mode", "any", "--disable-default-ignore-filename-regex", "--ignore-filename-regex", ignore]);
const profiles = readdirSync(target).filter(p => p.endsWith(".profraw")).sort();
const lists = readdirSync(target).filter(p => p.endsWith("-profraw-list"));
if (!profiles.length || lists.length !== 1) throw Error("missing profiles/merge list");
const merged = readFileSync(join(target, lists[0]), "utf8").trim().split("\n").sort();
if (JSON.stringify(merged) !== JSON.stringify(profiles.map(p => join(target, p)))) throw Error("profile merge mismatch");
save("profiles.json", { target, merged, profiles: profiles.map(p => ({ path: p, size: statSync(join(target, p)).size, sha256: hash(readFileSync(join(target, p))) })) });
const raw = freshReport(join(dir, "raw.lcov"), started), nativeRaw = freshReport(join(dir, "raw.json"), started);
const native = JSON.parse(nativeRaw);
if (native.type !== "llvm.coverage.json.export" || native.version !== "3.1.0") throw Error("LLVM schema mismatch");
const natives = new Map<string, any>(native.data[0].files.map((f: any) => [f.filename, f]));
const known = new Map(inventory.map(f => [f.path, f])), maps = new Map<string, Map<number, number>>(), seen = new Set<string>();
// Same canonical DA union/test-only scope as the root accounting contract.
// Corroborate every DA hit and LF/LH against the untouched native JSON export.
for (const record of raw.trim().split("end_of_record").filter(s => s.trim())) {
  const filename = record.match(/^SF:(.+)$/m)![1], path = relative(root, realpathSync(filename));
  const file = known.get(path), original = natives.get(filename);
  if (!original || seen.has(filename)) throw Error(`native file mismatch: ${filename}`);
  seen.add(filename);
  if (!file && !/^crates\/[^/]+\/tests\//.test(path)) throw Error(`unknown file: ${path}`);
  const entries = new Map<number, number>();
  for (const match of record.matchAll(/^DA:(\d+),(\d+)$/gm)) {
    const line = Number(match[1]), hit = Number(match[2]);
    if (entries.has(line) || !Number.isSafeInteger(hit) || hit < 0 || line < 1 || (file && line > file.lines)) throw Error("invalid DA");
    entries.set(line, hit);
  }
  if (Number(record.match(/^LF:(\d+)$/m)![1]) !== original.summary.lines.count || Number(record.match(/^LH:(\d+)$/m)![1]) !== original.summary.lines.covered) throw Error("native summary mismatch");
  const expected = new Map<number, number>(); let next = 0, wrapped: any;
  for (let line = 1; next < original.segments.length; line++) {
    const onLine: any[] = [];
    while (next < original.segments.length && original.segments[next][0] === line) onLine.push(original.segments[next++]);
    const starts = onLine.filter(s => !s[5] && s[3] && s[4]);
    const skipped = onLine.length && !onLine[0][3] && onLine[0][4];
    if ((!skipped && (wrapped?.[3] || starts.length)) || onLine.some(s => s[3] && s[4])) expected.set(line, Math.max(wrapped?.[2] ?? 0, ...starts.map(s => s[2])));
    if (onLine.length) wrapped = onLine.at(-1);
  }
  if (entries.size !== expected.size || [...expected].some(([line, hit]) => entries.get(line) !== hit)) throw Error("native line mismatch");
  if (!file) continue;
  const map = maps.get(path) ?? new Map<number, number>();
  for (const [line, hit] of entries) map.set(line, Math.max(map.get(line) ?? 0, hit));
  maps.set(path, map);
}
if (seen.size !== natives.size) throw Error("native file set mismatch");
const files = inventory.filter(f => !f.testOnly).map(file => {
  const map = maps.get(file.path); if (!map) throw Error(`missing production file: ${file.path}`);
  const mapped = [...map.keys()].filter(n => !file.omitted.includes(n)).sort((a, b) => a - b);
  const uncovered = mapped.filter(n => map.get(n) === 0);
  return { path: file.path, crate: file.crate, total: mapped.length, covered: mapped.length - uncovered.length, mapped, uncovered, omitted: file.omitted };
});
const crates = owned.map(crate => {
  const own = files.filter(f => f.crate === crate), total = own.reduce((n, f) => n + f.total, 0), covered = own.reduce((n, f) => n + f.covered, 0);
  if (!total) throw Error(`missing denominator: ${crate}`);
  return { crate, total, covered, percent: 100 * covered / total, pass: BigInt(covered) * 100n >= BigInt(total) * 80n };
});
const after = inputIdentity(root); if (before.sha256 !== after.sha256) throw Error("source changed");
const result = { scope: "supplemental four-library diagnostic; not a workspace gate result", acceptedBaseline: BASELINE, localHead, sourceHash: before.sha256, measurementScriptSha256: hash(readFileSync(import.meta.path)), rawSha256: hash(raw), llvmSha256: hash(nativeRaw), host, files, crates, sourceStable: true, workspaceGatePassed: false };
save("summary.json", result);
writeFileSync(join(dir, "uncovered.md"), files.map(f => `${f.path}: ${f.covered}/${f.total}; uncovered: ${f.uncovered.join(", ") || "none"}`).join("\n") + "\n");
save("../diagnostic-latest.json", { directory: relative(root, dir), sourceHash: before.sha256, crates });
console.log(JSON.stringify({ directory: relative(root, dir), crates }, null, 2));
if (crates.some(c => !c.pass)) process.exitCode = 1;
