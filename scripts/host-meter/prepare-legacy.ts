// Compile the accepted pre-Meter daemon solely as a compatibility peer. This
// does not rerun or recertify any historical mission's gates or evidence.
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { resolve, relative, join } from "node:path";
import { hash } from "../coverage/source";

const root = resolve(import.meta.dir, "../..");
const acceptedHead = "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de";
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
const env: NodeJS.ProcessEnv = { ...process.env, CARGO_TARGET_DIR: join(directory, "target") };
for (const key of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "LLVM_PROFILE_FILE", "CARGO_LLVM_COV_TARGET_DIR"]) delete env[key];
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
  ...(exitCode === 0 ? { binary: ref(join(directory, "target/debug/arc-daemon")) } : {}) };
writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
if (exitCode !== 0) throw Error(`LEGACY: compilation failed: ${exitCode}`);
writeFileSync(join(root, "target/host-meter/legacy/latest.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(`Legacy compatibility peer: ${receipt.binary!.path}`);
