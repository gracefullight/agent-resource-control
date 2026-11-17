// Compile the accepted pre-Meter daemon solely as a compatibility peer. This
// does not rerun or recertify any historical mission's gates or evidence.
import { mkdirSync, writeFileSync, readFileSync, renameSync } from "node:fs";
import { resolve, relative, join } from "node:path";
import { hash } from "../coverage/source";
import { compactFixtureBuild } from "../coverage/hygiene";
import { LEGACY_HEAD, legacyBuildEnvironment, reusableLegacyReceipt } from "./legacy-reuse";
import { lockLegacyPreparation } from "./legacy-lock";

const root = resolve(import.meta.dir, "../..");
async function main() {
const unlock = lockLegacyPreparation(root);
try {
const acceptedHead = LEGACY_HEAD;
const previous = reusableLegacyReceipt(root);
if (previous) {
  console.log(`Legacy compatibility peer: ${previous.binary.path}`);
  return;
}
const directory = join(root, "target/host-meter/legacy", crypto.randomUUID());
const source = join(directory, "source");
mkdirSync(source, { recursive: true });
const startedAt = new Date().toISOString();
const archive = Bun.spawnSync(["git", "archive", "--format=tar", acceptedHead,
  "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crates", "apps/desktop"],
  { cwd: root, stdout: "pipe", stderr: "pipe" });
if (archive.exitCode !== 0) throw Error(`LEGACY: archive failed: ${archive.stderr}`);
const archivePath = join(directory, "accepted.tar");
writeFileSync(archivePath, archive.stdout, { flag: "wx" });
const unpack = Bun.spawnSync(["tar", "-xf", archivePath, "-C", source], { stdout: "pipe", stderr: "pipe" });
if (unpack.exitCode !== 0) throw Error(`LEGACY: extraction failed: ${unpack.stderr}`);
const argv = ["cargo", "build", "--locked", "--offline", "-p", "arc-daemon", "-p", "arc-cli", "--bins"];
// Keep the old peer uninstrumented and separate from current Cargo artifacts.
const env = legacyBuildEnvironment(process.env, join(directory, "target"));
delete env.ARC_LEGACY_COORDINATION_FD;
const child = Bun.spawn(argv, { cwd: source, env, stdout: "pipe", stderr: "pipe" });
const timer = setTimeout(() => child.kill("SIGTERM"), 7_200_000);
const [out, err, exitCode] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).arrayBuffer(), child.exited]);
clearTimeout(timer);
const ref = (path: string) => { const bytes = readFileSync(path); return { path: relative(root, path), sha256: hash(bytes), bytes: bytes.length }; };
writeFileSync(join(directory, "stdout.log"), Buffer.from(out));
writeFileSync(join(directory, "stderr.log"), Buffer.from(err));
process.stdout.write(Buffer.from(out)); process.stderr.write(Buffer.from(err));
const receipt = { acceptedHead, argv, cwd: source, startedAt, finishedAt: new Date().toISOString(), exitCode,
  purpose: "accepted pre-Meter compatibility peer only; no historical recertification",
  archive: ref(archivePath), stdout: ref(join(directory, "stdout.log")), stderr: ref(join(directory, "stderr.log")),
  ...(exitCode === 0 ? { binary: ref(join(directory, "target/debug/arc-daemon")), binaries: {
    arcctl: ref(join(directory, "target/debug/arcctl")), fixture: ref(join(directory, "target/debug/arc-exec-fixture")),
  } } : {}) };
writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
if (exitCode !== 0) throw Error(`LEGACY: compilation failed: ${exitCode}`);
// The compatibility peer remains at the receipt's validated path; neither
// the historical archive nor receipts depend on Cargo's intermediate files.
compactFixtureBuild(join(directory, "target"), ["arc-daemon", "arcctl", "arc-exec-fixture"]);
const pointer = join(root, "target/host-meter/legacy/latest.json");
const pending = `${pointer}.${crypto.randomUUID()}.tmp`;
writeFileSync(pending, JSON.stringify(receipt, null, 2) + "\n", { flag: "wx" });
renameSync(pending, pointer);
console.log(`Legacy compatibility peer: ${receipt.binary!.path}`);
} finally { unlock(); }
}
await main();
