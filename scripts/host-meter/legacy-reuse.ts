import { existsSync, lstatSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { hash } from "../coverage/source";

export const LEGACY_HEAD = "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de";

export function legacyBuildEnvironment(parent: NodeJS.ProcessEnv, target: string): NodeJS.ProcessEnv {
  const env = { ...parent, CARGO_TARGET_DIR: target };
  for (const key of Object.keys(env)) {
    if (["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
      "CARGO_BUILD_TARGET", "LLVM_COV_FLAGS", "LLVM_PROFDATA_FLAGS"].includes(key) ||
      /^(?:CARGO_LLVM_COV|LLVM_PROFILE|CARGO_PROFILE_|CARGO_TARGET_.*RUSTFLAGS)/.test(key)) delete env[key];
  }
  return env;
}

// Reuse only a complete, unaltered preparation. Failure leaves all files in
// place for diagnosis and does not silently replace a prior receipt.
export function reusableLegacyReceipt(root: string): any | undefined {
  const base = resolve(root, "target/host-meter/legacy"), latest = join(base, "latest.json");
  if (!existsSync(latest)) return undefined;
  const bytes = readFileSync(latest), receipt = JSON.parse(bytes.toString());
  if (receipt.acceptedHead !== LEGACY_HEAD || receipt.exitCode !== 0) throw Error("LEGACY: invalid preparation receipt");
  const binaryPath = receipt.binary?.path;
  const match = typeof binaryPath === "string" && binaryPath.match(/^target\/host-meter\/legacy\/([0-9a-f-]{36})\/target\/debug\/arc-daemon$/);
  if (!match) throw Error("LEGACY: invalid preparation path");
  const directory = join(base, match[1]), metadata = lstatSync(directory);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw Error("LEGACY: invalid preparation directory");
  if (!readFileSync(join(directory, "receipt.json")).equals(bytes)) throw Error("LEGACY: mismatched preparation receipt");
  for (const ref of [receipt.archive, receipt.stdout, receipt.stderr, receipt.binary, ...Object.values(receipt.binaries ?? {})] as any[]) {
    if (!ref || typeof ref.path !== "string") throw Error("LEGACY: missing preparation artifact");
    const path = resolve(root, ref.path);
    if (!path.startsWith(directory + "/")) throw Error("LEGACY: preparation artifact escaped its directory");
    const stat = lstatSync(path);
    if (!stat.isFile() || stat.isSymbolicLink()) throw Error("LEGACY: invalid preparation artifact");
    const content = readFileSync(path);
    if (content.length !== ref.bytes || hash(content) !== ref.sha256) throw Error(`LEGACY: preparation artifact changed: ${ref.path}`);
  }
  for (const name of ["arc-daemon", "arcctl", "arc-exec-fixture"]) {
    const stat = lstatSync(join(directory, "target/debug", name));
    if (!stat.isFile() || stat.isSymbolicLink()) throw Error(`LEGACY: invalid executable ${name}`);
  }
  return receipt;
}
