import { mkdirSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { hash } from "./source";

// These bytes come directly from Bun's child pipes, before UTF-8 decoding.
// The normal receipt keeps its text fields for existing readers.
export function nativeOutput(argv: string[], cwd: string, stdout: Buffer, stderr: Buffer) {
  if (!(argv[0] === "rustup" && argv[3] === "cargo" && argv[4] === "llvm-cov" &&
    argv.includes("--workspace") && argv.includes("--all-targets") && argv.includes("--no-report"))) return undefined;
  const directory = resolve(cwd, "target/traffic/native-coverage", crypto.randomUUID());
  mkdirSync(directory, { recursive: true });
  const save = (name: string, bytes: Buffer) => {
    const file = resolve(directory, name);
    writeFileSync(file, bytes, { flag: "wx" });
    return { path: relative(cwd, file), sha256: hash(bytes), bytes: bytes.length };
  };
  return { stdout: save("stdout.log", stdout), stderr: save("stderr.log", stderr) };
}
