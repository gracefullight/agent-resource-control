import { mkdirSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { root } from "../coverage/acceptance";
import { account } from "../coverage/accounting";
import { BASELINE, CRATES } from "../coverage/config";
import { gitBytes } from "../coverage/hygiene";
import { hash, inputIdentity, sourceInventory } from "../coverage/source";
import { FEATURE_BASELINE } from "../coverage/current-preservation";
import { directory, managedGit, managedInputs, nativeHost, requiredCommands } from "./managed-exec-evidence";
import { parseCargoExecution } from "./managed-exec-test-output";

// Isolated validator fixtures, never execution or acceptance receipts.
export function coverageFixture() {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), "arc-coverage-identity-")));
  gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, dir]);
  const put = (path: string, value: string | Buffer) => {
    mkdirSync(dirname(resolve(dir, path)), { recursive: true }); writeFileSync(resolve(dir, path), value);
  };
  const save = (path: string, value: unknown) => put(path, JSON.stringify(value, null, 2) + "\n");
  const load = (path: string) => JSON.parse(readFileSync(resolve(dir, path), "utf8"));
  const ref = (path: string) => ({ path, sha256: hash(readFileSync(resolve(dir, path))) });
  put("Cargo.toml", `[workspace]\nmembers = [${CRATES.map(c => `"crates/${c}"`).join(",")}]\n`);
  for (const path of ["Cargo.lock", "rust-toolchain.toml", "mise.toml", "package.json", ".gitignore",
    ".arc-verify/rust-coverage/contract/accounting.json", `${directory}/contract.json`, `${directory}/decision.md`,
    "docs/rust-coverage.md", "scripts/coverage/unit.txt", "scripts/harness/unit.txt"]) put(path, "unit fixture\n");
  for (const crate of CRATES) {
    put(`crates/${crate}/Cargo.toml`, `[package]\nname = "${crate}"\n`);
    put(`crates/${crate}/src/lib.rs`, Array.from({ length: 10 }, (_, i) => `pub const N${i}: u8 = ${i};`).join("\n") + "\n");
  }
  const identity = inputIdentity(dir), inventory = sourceInventory(dir);
  function collection(id: string, covered: number) {
    const base = `.arc-verify/rust-coverage/baseline/runs/${id}`, target = resolve(dir, "target/arc-coverage", id);
    const files = inventory.map(f => ({ filename: resolve(dir, f.path),
      segments: Array.from({ length: 10 }, (_, i) => [[i + 1, 1, i < covered ? 1 : 0, true, true, false],
        [i + 1, 2, 0, false, false, false]]).flat(),
      summary: { lines: { count: 10, covered } } }));
    const raw = files.map(f => `SF:${f.filename}\n` + Array.from({ length: 10 }, (_, i) => `DA:${i + 1},${i < covered ? 1 : 0}`).join("\n") +
      `\nLF:10\nLH:${covered}\nend_of_record\n`).join("");
    const llvm = { type: "llvm.coverage.json.export", version: "3.1.0", data: [{ files }] };
    put(`${base}/raw.lcov`, raw); save(`${base}/raw.json`, llvm);
    const accounting = account(raw, inventory, dir, undefined, llvm);
    const childProof = ["arcctl", "arc-daemon"].map((binary, i) => ({ binary, pid: 100 + i,
      profiles: [`arc-${100 + i}-fixture.profraw`], exit: i }));
    const profiles = childProof.flatMap(c => c.profiles).map(path => {
      const bytes = Buffer.from(`profile fixture ${path}`); put(`target/arc-coverage/${id}/${path}`, bytes);
      return { path, size: bytes.length, sha256: hash(bytes) };
    });
    save(`${base}/profiles.json`, { target, childProof, profiles, merge: profiles.map(p => resolve(target, p.path)) });
    save(`${base}/inputs.json`, identity); save(`${base}/source-inventory.json`, inventory);
    save(`${base}/summary.json`, { ...accounting, runId: id, sourceHash: identity.sha256, inputs: identity,
      acceptedBaseline: BASELINE, host: "aarch64-apple-darwin", threshold: 80,
      rawSha256: hash(raw), llvmSha256: ref(`${base}/raw.json`).sha256, childProof });
    save(`${base}/receipt.json`, { schemaVersion: 1, runId: id, sourceHash: identity.sha256,
      acceptedBaseline: BASELINE, pass: true, exit: 0, receipts: childProof.map(c => ({
        pid: c.pid, exit: c.exit, argv: [resolve(target, "debug", c.binary)] })) });
    put(`${base}/uncovered.md`, "fixture uncovered lines\n");
    return { runId: id, directory: base, sourceHash: identity.sha256, pass: true };
  }
  const reviewed = collection("reviewed-unit", 8), later = collection("later-unit", 9);
  const latestPath = ".arc-verify/rust-coverage/baseline/latest.json";
  save(latestPath, reviewed);
  const source = managedInputs(dir), host = nativeHost();
  const output = "running 1 test\ntest fixture ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
  const command = (argv: string[], actorId = "backend", second = 0) => ({
    schemaVersion: 1, actorId, sourceHash: source.sha256, host, argv, cwd: ".", started: `2026-11-08T09:00:0${second}Z`,
    ended: `2026-11-08T09:00:0${second}Z`, status: "passed", exit: 0, timeoutAllowanceMs: 3600000, skipFlags: false,
    stdoutSha256: hash(output), stderrSha256: hash(""), output: { stdout: output, stderr: "" }, execution: parseCargoExecution(output) });
  const receipts = requiredCommands.map((argv, i) => {
    const path = `${directory}/command-${i}.json`;
    save(path, command(argv, "backend", argv.at(-1) === "scripts/coverage/evidence.test.ts" ? 1 : 0)); return ref(path);
  });
  put(`${directory}/qa.md`, "macOS protocol coverage reconciliation ".repeat(40));
  put(`${directory}/final.md`, "lifecycle desktop native Windows Phase 1 scheduling ".repeat(25));
  put(`${directory}/preservation.md`, "unit fixture\n");
  save(`${directory}/qa.json`, { schemaVersion: 1, actorId: "reviewer", skill: "oma-qa", status: "passed", sourceHash: source.sha256,
    host, reviewedReceipts: receipts, report: ref(`${directory}/qa.md`), commands: [command(["cargo", "test", "--workspace"], "reviewer")] });
  const report = load(`${reviewed.directory}/summary.json`), profiles = load(`${reviewed.directory}/profiles.json`);
  save(`${directory}/final-summary.json`, { schemaVersion: 1, baseline: FEATURE_BASELINE, nativeMacos: true, sourceBound: true,
    independentReviewPassed: true, host, inputs: source, git: managedGit(dir), receipts, review: ref(`${directory}/qa.json`),
    qaReport: ref(`${directory}/qa.md`), finalReport: ref(`${directory}/final.md`), coverage: {
      latest: ref(latestPath), summary: ref(`${reviewed.directory}/summary.json`), runId: reviewed.runId, sourceHash: identity.sha256,
      crates: report.crates, childProof: profiles.childProof, profilesChecked: profiles.profiles.length, independentlyRecomputed: true } });
  const final = load(`${directory}/final-summary.json`), qa = load(`${directory}/qa.json`);
  qa.coverage = final.coverage; save(`${directory}/qa.json`, qa);
  final.review = ref(`${directory}/qa.json`); save(`${directory}/final-summary.json`, final);
  gitBytes(dir, ["add", "--", directory]);
  return { dir, save, put, load, ref, reviewed, later, latestPath, collection,
    advance: () => save(latestPath, later), stage: () => gitBytes(dir, ["add", "--", directory]) };
}
