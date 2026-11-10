import { selectedBytes, safeEntryPath } from "./exec-fail-open-container";
import { checkPredecessorRead } from "./exec-fail-open-reads";
import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { resolve, relative } from "node:path";
import { arch, platform, release, hostname } from "node:os";
import { spawnSync } from "node:child_process";
import { hash, walk } from "../coverage/source";
import { gitBytes, indexEntries, prohibitedReasons } from "../coverage/hygiene";
import { baseline, baselineTree } from "./exec-fail-open-history";
export { baseline, baselineTree, hash };
export const directory = ".arc-verify/exec-fail-open";
export const root = resolve(import.meta.dir, "../..");
export const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
export function requireValue(ok: unknown, message: string): asserts ok { if (!ok) throw Error(`EXEC_FAIL_OPEN: ${message}`); }
export const digest = (s: unknown) => typeof s === "string" && /^[a-f0-9]{64}$/.test(s);
export function canonicalBytes(root: string, path: string) {
  requireValue(typeof path === "string" && !/[\x00-\x1f\x7f]/.test(path) && !path.includes("\\") && !path.split("/").some(p => !p || p === ".." || p === ".") && !path.startsWith("/"), "unsafe artifact path");
  checkPredecessorRead(root, path);
  const file = resolve(root, path), stat = lstatSync(file);
  requireValue(stat.isFile() && !stat.isSymbolicLink() && realpathSync(file) === resolve(realpathSync(root), path), `nonregular artifact: ${path}`);
  return readFileSync(file);
}
export function evidenceBytes(root: string, path: string) {
  requireValue(typeof path === "string" && path.startsWith(directory + "/"), "artifact outside evidence directory");
  const bytes = canonicalBytes(root, path);
  requireValue(prohibitedReasons(path, bytes).length === 0, `prohibited evidence: ${path}`); return bytes;
}
export function requireStaged(root: string, path: string, bytes = canonicalBytes(root, path)) {
  requireValue(indexEntries(root, path).some(e => e.path === path && e.mode === "100644") && bytes.equals(gitBytes(root, ["show", `:${path}`])), "unstaged container or index bytes/type differ");
}
export function artifact(root: string, ref: any) {
  requireValue(ref && digest(ref.sha256), "missing artifact reference");
  if (ref.selector !== undefined) { safeEntryPath(ref.selector?.path); checkPredecessorRead(root, ref.selector.path); }
  const bytes = canonicalBytes(root, ref.path);
  requireValue(hash(bytes) === ref.sha256, `artifact hash differs: ${ref.path}`);
  if (ref.selector !== undefined) {
    safeEntryPath(ref.selector?.path); checkPredecessorRead(root, ref.selector.path);
    requireValue(ref.path.startsWith(directory + "/"), "container outside evidence directory");
    requireStaged(root, ref.path, bytes);
    return selectedBytes(bytes, ref.selector);
  }
  return bytes;
}
export const boundJSON = (root: string, ref: any) => JSON.parse(artifact(root, ref).toString());
export const reference = (root: string, path: string) => ({ path, sha256: hash(canonicalBytes(root, path)) });
export const load = (root: string, path: string) => JSON.parse(canonicalBytes(root, path).toString());
function optionalInputPresent(root: string, path: string) {
  try { lstatSync(resolve(root, path)); return true; }
  catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") return false;
    throw error;
  }
}
export function currentInputs(root: string) {
  const contract = load(root, `${directory}/contract.json`), f = contract.inputFingerprint;
  const paths = [...f.requiredFiles, ...f.optionalIfPresent.filter((p: string) => optionalInputPresent(root, p)),
    ...f.recursiveScopes.flatMap((p: string) => walk(resolve(root, p)).map(file => relative(root, file)))];
  const files = Object.fromEntries([...new Set<string>(paths)].sort().map(path => [path, {
    sha256: hash(canonicalBytes(root, path)), mode: lstatSync(resolve(root, path)).mode & 0o777,
  }]));
  return { sha256: hash(JSON.stringify(files)), files };
}
const gitScopes = ["crates", "scripts/coverage", "scripts/harness", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", "package.json", ".gitignore", ".cargo", "build.rs", "bun.lock", "bun.lockb", "README.md", "docs/managed-exec.md", "docs/rust-coverage.md"];
function gitDiff(root: string, ...revisions: string[]) {
  return hash(gitBytes(root, ["-c", "core.quotePath=true", "-c", "diff.algorithm=myers", "diff", "--no-ext-diff", "--no-textconv", "--no-renames", "--no-color", "--no-relative", "--no-indent-heuristic", "--src-prefix=a/", "--dst-prefix=b/", "--unified=3", "--inter-hunk-context=0", "--full-index", "--binary", ...revisions, "--", ...gitScopes]));
}
// Bind the contract inventory to a committed tree without consulting the index.
// These are source comparison digests, not raw `git diff` digests.
function inputComparison(root: string, revision: string, files: ReturnType<typeof currentInputs>["files"]) {
  const f = load(root, `${directory}/contract.json`).inputFingerprint;
  const exact = new Set<string>([...f.requiredFiles, ...f.optionalIfPresent]);
  const scopes: string[] = f.recursiveScopes;
  const tree = gitBytes(root, ["rev-parse", `${revision}^{tree}`]).toString().trim();
  const listed = gitBytes(root, ["ls-tree", "-r", "-z", "--full-tree", revision, "--", ...[...exact, ...scopes].map(p => `:(literal)${p}`)]).toString();
  const committed: Record<string, { mode: string; type: string; object: string }> = {};
  for (const entry of listed.split("\0").filter(Boolean)) {
    const match = entry.match(/^(\d+) (\w+) ([a-f0-9]+)\t(.+)$/s);
    requireValue(match && (exact.has(match[4]) || scopes.some(p => match[4].startsWith(`${p}/`))), "invalid committed input entry");
    committed[match[4]] = { mode: match[1], type: match[2], object: match[3] };
  }
  const paths = [...new Set([...Object.keys(committed), ...Object.keys(files)])].sort();
  const comparisons = Object.fromEntries(paths.map(path => [path, {
    committed: committed[path] ?? null, input: files[path] ? { type: "file", ...files[path] } : null,
  }]));
  return hash(JSON.stringify({ version: 1, revision, tree, comparisons }));
}
export function observedGit(root: string, input = currentInputs(root)) {
  const head = gitBytes(root, ["rev-parse", "HEAD"]).toString().trim();
  return { head, tree: gitBytes(root, ["rev-parse", `${head}^{tree}`]).toString().trim(), baselineTree,
    headDiffSha256: gitDiff(root, baseline, head), baselineInputsSha256: inputComparison(root, baseline, input.files),
    originInputsSha256: inputComparison(root, head, input.files) };
}
function validateGitBinding(root: string, origin: any, current: ReturnType<typeof observedGit>) {
  requireValue(origin && /^[a-f0-9]{40}$/.test(origin.head) && /^[a-f0-9]{40}$/.test(origin.tree) && origin.baselineTree === baselineTree &&
    digest(origin.headDiffSha256) && digest(origin.baselineInputsSha256) && digest(origin.originInputsSha256), "missing origin revision/source provenance");
  requireValue(gitBytes(root, ["rev-parse", `${baseline}^{tree}`]).toString().trim() === baselineTree, "accepted baseline tree differs");
  const originTree = spawnSync("git", ["rev-parse", "--verify", `${origin.head}^{tree}`], { cwd: root, encoding: "utf8" });
  requireValue(originTree.status === 0 && originTree.stdout.trim() === origin.tree, "origin revision unavailable or tree differs");
  requireValue(spawnSync("git", ["merge-base", "--is-ancestor", baseline, origin.head], { cwd: root }).status === 0, "origin revision not descended from accepted baseline");
  const inputs = currentInputs(root);
  requireValue(origin.baselineInputsSha256 === current.baselineInputsSha256 && origin.headDiffSha256 === gitDiff(root, baseline, origin.head) &&
    origin.originInputsSha256 === inputComparison(root, origin.head, inputs.files), "stale receipt inputs/git or origin delivery differs");
}
export function tools() {
  return Object.fromEntries([["cargo", ["cargo", "--version"]], ["rustc", ["rustc", "--version"]], ["bun", ["bun", "--version"]], ["cargoLlvmCov", ["cargo", "llvm-cov", "--version"]]].map(([name, args]) => {
    const argv = args as string[], r = spawnSync(argv[0], argv.slice(1), { encoding: "utf8", timeout: 30000 });
    requireValue(r.status === 0 && r.stdout.trim(), `version probe failed: ${name}`);
    return [name, { path: realpathSync(Bun.which(argv[0])!), version: r.stdout.trim(), exit: r.status }];
  }));
}
export function nativeHost() {
  const r = spawnSync("rustc", ["-vV"], { encoding: "utf8" });
  requireValue(r.status === 0, "rust host probe failed");
  return { platform: platform(), release: release(), arch: arch(), hostname: hostname(), rustHost: r.stdout.match(/^host: (.+)$/m)?.[1] };
}
export function context(root: string) {
  const input = currentInputs(root);
  const value = { schemaVersion: 1, baselineHead: baseline, baselineTree, sourceHash: input.sha256,
    currentInputs: input.files, observedGit: observedGit(root, input), host: nativeHost(), tools: tools() };
  return Object.defineProperty(value, "validationRoot", { value: root, enumerable: false }) as typeof value & { validationRoot: string };
}
export function validateCommon(value: any, ctx: ReturnType<typeof context>, kind: string, actor?: string) {
  requireValue(!value?.fixtureOnly, "fixture evidence is inadmissible for production acceptance");
  requireValue(value?.schemaVersion === 1 && value.kind === kind && value.baselineHead === baseline && value.baselineTree === baselineTree, "wrong receipt contract/baseline");
  requireValue(value.sourceHash === ctx.sourceHash && equal(value.currentInputs, ctx.currentInputs) && hash(JSON.stringify(value.currentInputs)) === value.sourceHash, "stale receipt inputs/git");
  validateGitBinding(ctx.validationRoot, value.observedGit, ctx.observedGit);
  requireValue(ctx.host.platform === "darwin" && equal(value.host, ctx.host) && equal(value.tools, ctx.tools), "native host/tools mismatch");
  requireValue(!actor || value.actorId === actor, "receipt actor mismatch");
  if (actor) requireValue(value.skill === (actor === "reviewer" ? "oma-qa" : "oma-backend"), "receipt skill mismatch");
}
export function validateIndex(root: string, required: string[], sealedCount?: number) {
  const entries = indexEntries(root, directory);
  const sealed = entries.filter(e => e.path !== `${directory}/terminal.json`);
  requireValue(entries.length < 150 && sealed.length > 0 && sealed.length <= 148, "evidence count must reserve terminal slot and remain below 150");
  requireValue(sealedCount === undefined || sealedCount === sealed.length, "sealed inventory count changed");
  requireValue(required.every(path => entries.some(e => e.path === path)), "required evidence is unstaged");
  for (const entry of entries) requireValue(entry.mode === "100644" && evidenceBytes(root, entry.path).equals(gitBytes(root, ["show", `:${entry.path}`])), `index bytes/type differ: ${entry.path}`);
  return entries.length;
}
