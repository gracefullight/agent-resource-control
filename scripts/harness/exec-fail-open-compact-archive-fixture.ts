import { copyFileSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalBytes, directory, hash, requireValue, root as productRoot } from "./exec-fail-open-inputs";
import { decodeContainer, selectedBytes } from "./exec-fail-open-container";
import { archiveBytes, canonicalArchive, persistent4Sha256, rotateTask13, task16PredecessorPath } from "./exec-fail-open-compact";

// Copy every authenticated external dependency on the predecessor chain.
// The chain may gain another predecessor when a later collection is sealed.
export function installArchiveDependencies(root: string, original: Buffer, requireNested = true) {
  const copied = new Map<string, string>(), firstLevel = new Set<string>(), nested = new Set<string>();
  const seen = new Set<string>();
  let bytes = original, depth = 0;
  for (;;) {
    const container = decodeContainer(bytes);
    for (const entry of container.entries.filter(e => e.encoding === "external")) {
      const old = copied.get(entry.bytes);
      requireValue(!old || old === entry.sha256, "archive fixture path has conflicting digests");
      if (depth === 0) firstLevel.add(entry.bytes);
      else nested.add(entry.bytes);
      if (!old) {
        const selected = selectedBytes(bytes, { path: entry.path, sha256: entry.sha256 }, true, productRoot);
        requireValue(hash(selected) === entry.sha256 && selected.length === entry.size, "archive fixture source differs");
        const destination = resolve(root, entry.bytes);
        mkdirSync(resolve(destination, ".."), { recursive: true });
        writeFileSync(destination, selected);
        requireValue(hash(readFileSync(destination)) === entry.sha256, "archive fixture copy differs");
        copied.set(entry.bytes, entry.sha256);
      }
    }
    const predecessor = container.entries.find(e => e.path === `${directory}/implementation/task16-predecessor.json`);
    if (!predecessor) break;
    requireValue(!seen.has(predecessor.sha256), "archive fixture predecessor cycle");
    seen.add(predecessor.sha256);
    bytes = selectedBytes(bytes, { path: predecessor.path, sha256: predecessor.sha256 }, true, productRoot);
    depth++;
  }
  const reported = JSON.parse(canonicalBytes(productRoot, "target/exec-fail-open/task16-attempt3/nested-fixture-dependencies.json").toString());
  for (const item of reported.nestedOnly) {
    requireValue(item.expected === item.actual && copied.get(item.raw) === item.expected && (!requireNested || nested.has(item.raw)),
      `reported nested fixture dependency differs: ${item.raw}`);
  }
  const nestedOnly = [...nested].filter(path => !firstLevel.has(path));
  requireValue(!requireNested || nestedOnly.length > 0, "archive fixture has no nested-only dependency");
  return nestedOnly;
}

export function checkRepeatedRotation(root: string, git: (...args: string[]) => void) {
  const path = `${directory}/implementation/collection-next.json`, original = canonicalBytes(productRoot, path);
  const prior = decodeContainer(original);
  requireValue(prior.entries.some(e => e.path === task16PredecessorPath && e.scope === "archive"), "prior rotation absent");
  const nested = installArchiveDependencies(root, original);
  const saved = "target/exec-fail-open/task13-preserved/collection-next-original.json";
  mkdirSync(resolve(root, saved, ".."), { recursive: true });
  copyFileSync(resolve(productRoot, saved), resolve(root, saved));
  requireValue(hash(canonicalBytes(root, saved)) === persistent4Sha256, "retained original differs");
  writeFileSync(resolve(root, path), original);
  const canonical = ["gates", "timing", "historical", "preservation"].map(n => `${directory}/implementation/${n}.json`);
  for (const p of canonical) writeFileSync(resolve(root, p), canonicalBytes(productRoot, p));
  const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
  const current = Buffer.from('{"kind":"command","passed":true}\n');
  mkdirSync(resolve(root, pending), { recursive: true });
  writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: hash(current), bytes: current.toString("base64") }));
  git("add", directory);
  const rotated = rotateTask13(root, pending, path, []); git("add", directory);
  const archive = canonicalArchive(root, rotated.ref), previous = archiveBytes(root, archive[task16PredecessorPath]);
  requireValue(previous.equals(original), "repeated rotation did not retain exact predecessor");
  for (const entry of prior.entries) {
    const selector = { path: entry.path, sha256: entry.sha256 }, archived = entry.scope === "archive";
    requireValue(selectedBytes(previous, selector, archived, root).equals(selectedBytes(original, selector, archived, productRoot)),
      `repeated rotation changed predecessor selection: ${entry.path}`);
  }
  for (const p of canonical) requireValue(archiveBytes(root, archive[p]).equals(canonicalBytes(productRoot, p)), `canonical archive differs: ${p}`);
  const file = resolve(root, nested[0]), backup = `${file}.backup`;
  renameSync(file, backup);
  try {
    let missingRejected = false, alteredRejected = false;
    try { canonicalArchive(root, rotated.ref); } catch { missingRejected = true; }
    writeFileSync(file, "altered nested archive");
    try { canonicalArchive(root, rotated.ref); } catch { alteredRejected = true; }
    requireValue(missingRejected && alteredRejected, "repeated rotation accepted missing or altered nested archive");
  } finally { rmSync(file, { force: true }); renameSync(backup, file); }
  canonicalArchive(root, rotated.ref);
}
