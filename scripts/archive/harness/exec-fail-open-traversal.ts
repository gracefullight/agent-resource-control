//! Follow only selected records; a sibling in the same container is not a cycle.
import { artifact, directory, requireValue } from "./exec-fail-open-inputs";
import { decodeContainer, referenceIdentity } from "./exec-fail-open-container";
import { checkPredecessorRead } from "./exec-fail-open-reads";
import { resolve, relative, sep, isAbsolute } from "node:path";
export function reachableEvidence(root: string, value: any, ancestors: string[] = [], inspect?: (record: unknown) => void) {
  const required = new Set<string>(), complete = new Set<string>();
  const visit = (value: any, stack: string[], field?: string) => {
    if (Array.isArray(value)) { value.forEach(v => visit(v, stack, field)); return; }
    if (!value || typeof value !== "object") return;
    const path = typeof value.path === "string" ? value.path : "";
    const inside = resolve(root, path).startsWith(resolve(root, directory) + sep);
    // Canonical root-relative legacy references may be outside the evidence
    // directory (for example, target receipts). Executable metadata and
    // absolute tool paths are not legacy receipt references.
    if (typeof value.path === "string" && typeof value.sha256 === "string" && (inside || (field !== "executable" && !isAbsolute(path)))) {
      checkPredecessorRead(root, path);
      if (typeof value.selector?.path === "string") checkPredecessorRead(root, value.selector.path);
      requireValue(path === relative(root, resolve(root, path)), "unsafe artifact path");
      const identity = referenceIdentity(value);
      requireValue(value.path !== `${directory}/terminal.json` && value.selector?.path !== `${directory}/terminal.json` && !stack.includes(identity), "self or terminal successor dependency");
      const key = JSON.stringify([identity, value.sha256, value.selector?.sha256]);
      if (inside) required.add(value.path); if (complete.has(key)) return;
      const bytes = artifact(root, value);
      const logical = value.selector?.path ?? value.path;
      let record: any;
      try { record = JSON.parse(bytes.toString()); }
      catch (error) { if (logical.endsWith(".json")) throw error; }
      if (record !== undefined) {
        // Whole-container references authenticate retained/archive bytes only.
        // Entries are semantic prerequisites exclusively via explicit selectors.
        if (record?.kind === "retained-evidence-container" && value.selector === undefined) decodeContainer(bytes);
        else { inspect?.(record); visit(record, [...stack, identity]); }
      }
      complete.add(key);
    } else Object.entries(value).forEach(([key, v]) => visit(v, stack, key));
  };
  visit(value, ancestors); return [...required];
}
