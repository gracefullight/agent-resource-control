import { expect, test } from "bun:test";
import { lstatSync, readdirSync } from "node:fs";
import { join } from "node:path";

test("mission evidence stays within the inventory budget", () => {
  let files = 0;
  let bytes = 0;
  const walk = (path: string) => {
    const stat = lstatSync(path);
    if (stat.isSymbolicLink()) throw Error(`symlinked evidence path: ${path}`);
    if (!stat.isDirectory()) {
      if (!stat.isFile()) throw Error(`non-regular evidence path: ${path}`);
      files++;
      bytes += stat.size;
      return;
    }
    for (const entry of readdirSync(path)) walk(join(path, entry));
  };
  walk(import.meta.dir);
  console.log(`Mission evidence: ${files} files, ${bytes} bytes`);
  expect(files).toBeLessThan(30);
  expect(bytes).toBeLessThan(5_000_000);
});
