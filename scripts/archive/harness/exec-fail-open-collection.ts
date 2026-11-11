//! Select native observations by the actual workspace command's wall-clock window.
import { readdirSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalBytes, directory, equal, requireValue, load } from "./exec-fail-open-inputs";
import { retainedEntry } from "./exec-fail-open-container";
import { sealPending, canonicalEntries, rotateRejected, rotateTask13 } from "./exec-fail-open-compact";
import { scenarios } from "./exec-fail-open-native";
export function nativeEntries(root: string, workspace: any, required = scenarios) {
  requireValue(workspace && Date.parse(workspace.ended) >= Date.parse(workspace.started), "missing workspace timing window");
  const base = "target/exec-fail-open/native";
  const candidates = readdirSync(resolve(root, base)).filter(name => /^\d+$/.test(name)).filter(name => required.every(id => {
    const path = `${base}/${name}/${id}.json`; if (!existsSync(resolve(root, path))) return false;
    const row = load(root, path);
    return row.recordedUnixMs >= Date.parse(workspace.started) && row.recordedUnixMs <= Date.parse(workspace.ended);
  }));
  requireValue(candidates.length === 1, "no unique complete native observations from workspace command");
  return readdirSync(resolve(root, base, candidates[0])).filter(n => n.endsWith(".json")).sort().map(name => retainedEntry(
    `${directory}/implementation/native/${name}`, canonicalBytes(root, `${base}/${candidates[0]}/${name}`)));
}
export function sealCollection(root: string, pending: string, path = `${directory}/implementation/collection.json`, workspaceArgv = ["cargo", "test", "--workspace"], required = scenarios, combined = false, rotate = false, correction = false) {
  const records = readdirSync(resolve(root, pending)).filter(n => n.endsWith(".json")).map(n => {
    const record = load(root, `${pending}/${n}`); return JSON.parse(Buffer.from(record.bytes, "base64").toString());
  });
  const workspace = records.filter(r => equal(r.argv, workspaceArgv) && r.exit === 0);
  requireValue(workspace.length === 1, "one successful workspace command required for collection");
  const native = nativeEntries(root, workspace[0], required);
  return correction ? rotateTask13(root, pending, path, native) : rotate ? rotateRejected(root, pending, path, native) : sealPending(root, pending, path, native, combined ? canonicalEntries(root) : undefined);
}
if (import.meta.main) {
  const args = process.argv.slice(2), correction = args[0] === "--rotate-task13", rotate = args[0] === "--rotate-rejected", combined = args[0] === "--combined" || rotate || correction;
  const [pending, path] = combined ? args.slice(1) : args;
  const { root } = await import("./exec-fail-open-inputs");
  requireValue(!combined || path, "combined collection requires a new explicit path");
  console.log(JSON.stringify(sealCollection(root, pending, path, undefined, undefined, combined, rotate, correction)));
}
