// Fault-inject the public, current arc-core library in an isolated test crate.
// This is a QA artifact, not a workspace member or production coverage input.
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
const root = resolve(import.meta.dir, "../../.."), dir = resolve(import.meta.dir, "runs", `native-faults-${crypto.randomUUID()}`);
mkdirSync(resolve(dir, "src"), { recursive: true });
writeFileSync(resolve(dir, "Cargo.toml"), `[package]\nname="independent-native-faults"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\narc-core={path=${JSON.stringify(resolve(root, "crates/arc-core"))}}\nlibc="0.2"\n`);
writeFileSync(resolve(dir, "src/lib.rs"), readFileSync(resolve(import.meta.dir, "native-faults.rs")));
const child = Bun.spawn(["bun", resolve(import.meta.dir, "run.ts"), "native-faults", "cargo", "test", "--manifest-path", resolve(dir, "Cargo.toml"), "--offline", "--", "--test-threads=1"],
  { cwd: root, stdout: "inherit", stderr: "inherit" });
process.exit(await child.exited);
