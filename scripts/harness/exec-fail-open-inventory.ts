//! Count the filesystem/index union before collection, including retained failures.
import { readdirSync, lstatSync } from "node:fs";
import { resolve } from "node:path";
import { directory, requireValue } from "./exec-fail-open-inputs";
import { indexEntries } from "../coverage/hygiene";
export const remainingAllocation = [
  "implementation/task9-compact-repair.json", "implementation/collection.json", "implementation/canonical-archive.json",
  "qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json",
].map(p => `${directory}/${p}`);
export function inventory(root: string, planned: string[] = []) {
  const fs: string[] = [];
  const visit = (path: string) => {
    for (const name of readdirSync(resolve(root, path))) {
      const p = `${path}/${name}`, stat = lstatSync(resolve(root, p));
      requireValue(!stat.isSymbolicLink(), "nonregular inventory path");
      if (stat.isDirectory()) visit(p); else { requireValue(stat.isFile(), "nonregular inventory path"); fs.push(p); }
    }
  };
  visit(directory);
  const indexed = indexEntries(root, directory).map(e => e.path);
  const retained = [...new Set([...fs, ...indexed])].sort();
  requireValue(planned.every(p => p.startsWith(directory + "/") && !p.split("/").some(s => !s || s === "." || s === "..") && !/[\\\x00-\x1f\x7f]/.test(p)), "unsafe planned inventory path");
  const union = [...new Set([...retained, ...planned])].sort();
  requireValue(union.length < 150, "planned inventory must remain below 150");
  return { filesystem: fs.length, indexed: indexed.length, retained: retained.length, added: union.length-retained.length, projected: union.length, paths: union };
}
