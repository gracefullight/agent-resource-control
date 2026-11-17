// The Rust compatibility tests retain this executable entry point. Historical
// evidence validation and fixture preparation live in the retired archive.
import { dirname, resolve } from "node:path";
import { historicalBinary as validatedHistoricalBinary } from "../archive/harness/exec-fail-open-history";
import { compactFixtureBuild } from "../coverage/hygiene";

// The archive validates its source and receipt before returning. Keep that
// frozen implementation unchanged and compact only the build it owns.
export function compactHistoricalPreparation<T extends { binary: string }>(prepared: T): T {
  compactFixtureBuild(dirname(dirname(prepared.binary)), ["arcctl"]);
  return prepared;
}

export function historicalBinary(root: string) {
  return compactHistoricalPreparation(validatedHistoricalBinary(root));
}

if (import.meta.main) {
  const { binary } = historicalBinary(resolve(import.meta.dir, "../.."));
  process.stdout.write(binary + "\n");
}
