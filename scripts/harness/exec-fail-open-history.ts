// The Rust compatibility tests retain this executable entry point. Historical
// evidence validation and fixture preparation live in the retired archive.
import { resolve } from "node:path";
import { historicalBinary } from "../archive/harness/exec-fail-open-history";

export { historicalBinary };

if (import.meta.main) {
  const { binary } = historicalBinary(resolve(import.meta.dir, "../.."));
  process.stdout.write(binary + "\n");
}
