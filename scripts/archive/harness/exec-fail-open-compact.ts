//! Seal after collection; neither sealing nor archive creation overwrites evidence.
import { existsSync, mkdirSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalBytes, reference, requireValue, requireStaged, hash, directory, artifact } from "./exec-fail-open-inputs";
import { retainedEntry, decodeContainer, selectedBytes, type Entry, type Container } from "./exec-fail-open-container";
import { checkPredecessorRead } from "./exec-fail-open-reads";
export type ArtifactRef = { path: string; sha256: string; selector?: { path: string; sha256: string } };
export const rejectedCollectionSha256 = "cf4b46bee490dcce468f62701751b0b2e214380e13a4e3eea0e65d0200402324";
export const rejectedIntermediateSha256 = "9949863a0a81dbf0db11927cbc3f37ad459207a0e207dc239becbe92170785e8";
export const rejectedAssemblySha256 = "2cf4dd7f4b009e99e6bc48c6a02c110fa1e23d0df4db2b1631d814335932b14e";
export const rejectedLatestSha256 = "14bb652542229e85860db6f79003f541a0195e6e64a9fe89b268b2e98318282d";
export const rejectedAttempt12Sha256 = "1567633dec40c085d0625abde7b909560ccc1f186fee8dc58378e8cc15d35261";
export const attempt13Sha256 = "d23a70c9bb337a54be4d9169730fce34669c6910f9981442d14f7000510c121d";
export const rejectedIntermediatePath = directory + "/implementation/rejected-attempt10-collection.json";
export const rejectedAssemblyPath = directory + "/implementation/commands/assemble.json";
export const rejectedLatestPath = directory + "/implementation/rejected-attempt11-collection.json";
export const rejectedAttempt12Path = directory + "/implementation/rejected-attempt12-collection.json";
export const attempt13Path = directory + "/implementation/attempt13-collection.json";
export const persistent4Sha256 = "4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366";
export const persistent4Path = directory + "/implementation/persistent4-collection.json";
export const task16PredecessorPath = directory + "/implementation/task16-predecessor.json";
export function seal(root: string, path: string, entries: Entry[], purpose: Container["purpose"] = "collection") {
  requireValue(path.startsWith(directory + "/") && path.endsWith(".json"), "unsafe seal path");
  const value = { schemaVersion: 1, kind: "retained-evidence-container", purpose, entries };
  const bytes = Buffer.from(JSON.stringify(value, null, 2)+"\n"); decodeContainer(bytes);
  // Validate the path before any mkdir/write, including symlinked parents.
  requireValue(!path.split("/").some(p => !p || p === "." || p === "..") && !/[\\\x00-\x1f\x7f]/.test(path), "unsafe seal path");
  canonicalBytes(root, `${directory}/contract.json`);
  const parent = path.slice(0, path.lastIndexOf("/"));
  mkdirSync(resolve(root, parent), { recursive: true });
  const { realpathSync } = require("node:fs") as typeof import("node:fs");
  requireValue(realpathSync(resolve(root, parent)) === resolve(realpathSync(root), parent), "nonregular seal parent");
  writeFileSync(resolve(root, path), bytes, { flag: "wx", mode: 0o644 });
  return { ref: reference(root, path), entries: Object.fromEntries(entries.map(e => [e.path, { ...reference(root, path), selector: { path: e.path, sha256: e.sha256 } }])) };
}
function pendingEntries(root: string, pending: string, excludeRejected = false) {
  requireValue(pending.startsWith("target/exec-fail-open/") && !pending.split("/").some(p => !p || p === "." || p === ".."), "unsafe pending directory");
  return readdirSync(resolve(root, pending)).filter(n => n.endsWith(".json") && (!excludeRejected || n !== "rejected-assembly.json")).sort().map(n => {
    const record = JSON.parse(canonicalBytes(root, `${pending}/${n}`).toString());
    const entry = retainedEntry(record.path, Buffer.from(record.bytes, "base64"));
    requireValue(entry.bytes === record.bytes && entry.sha256 === record.sha256, "pending receipt bytes differ"); return entry;
  });
}
export function sealPending(root: string, pending: string, path: string, extra: Entry[] = [], archived?: Entry[]) {
  const records = pendingEntries(root, pending);
  return archived ? seal(root, path, [
    ...[...records, ...extra].map(e => ({ ...e, scope: "current" as const })),
    ...archived.map(e => ({ ...e, scope: "archive" as const })),
  ], "combined") : seal(root, path, [...records, ...extra]);
}
// Rotation is limited to the specifically rejected, staged container. The new
// container is validated and written outside the evidence tree before rename.
export function rotateRejected(root: string, pending: string, path: string, extra: Entry[]) {
  requireValue(path === `${directory}/implementation/collection-next.json`, "invalid rotation path");
  const previous = canonicalBytes(root, path); requireStaged(root, path, previous);
  const previousHash = hash(previous), intermediate = previousHash === rejectedIntermediateSha256,
    attempt13 = previousHash === attempt13Sha256, attempt12 = previousHash === rejectedAttempt12Sha256 || attempt13,
    latest = previousHash === rejectedLatestSha256 || attempt12;
  requireValue(previousHash === rejectedCollectionSha256 || intermediate || latest, "rejected container identity differs");
  const previousContainer = decodeContainer(previous);
  if (latest) {
    requireValue(previousContainer.purpose === "combined" && previousContainer.entries.length === (attempt13 ? 52 : 62) &&
      (!attempt13 || previousContainer.entries.filter(e => e.scope === "current").length === 43 && previousContainer.entries.filter(e => e.scope === "archive").length === 9), "latest collection differs");
    canonicalArchive(root, reference(root, path));
  }
  const latestBytes = attempt12 ? selectedBytes(previous, { path: rejectedLatestPath, sha256: rejectedLatestSha256 }, true) : previous;
  const attempt12Bytes = attempt13 ? selectedBytes(previous, { path: rejectedAttempt12Path, sha256: rejectedAttempt12Sha256 }, true) : previous;
  const intermediateBytes = latest ? selectedBytes(previous, { path: rejectedIntermediatePath, sha256: rejectedIntermediateSha256 }, true) : previous;
  const original = intermediate || latest ? selectedBytes(previous, { path, sha256: rejectedCollectionSha256 }, true) : previous;
  const old = decodeContainer(original);
  requireValue(old.purpose === "combined" && old.entries.length === 53 && old.entries.some(e => e.path.endsWith("/native/terminal-2.json") && e.sha256 === "753a3ec4c14eaa831f2c298fbc7ae9b313ee5a6fa7c2c3c05de3c43d58f81564"), "rejected selections differ");
  const history = [retainedEntry(path, original)];
  if (intermediate || latest) {
    const intermediateContainer = decodeContainer(intermediateBytes);
    requireValue(intermediateContainer.purpose === "combined" && intermediateContainer.entries.length === 59, "intermediate collection differs");
    requireValue(hash(selectedBytes(intermediateBytes, { path, sha256: rejectedCollectionSha256 }, true)) === rejectedCollectionSha256, "nested rejected collection differs");
    const envelope = latest ? undefined : JSON.parse(canonicalBytes(root, `${pending}/rejected-assembly.json`).toString());
    const failedBytes = latest ? selectedBytes(previous, { path: rejectedAssemblyPath, sha256: rejectedAssemblySha256 }, true) : Buffer.from(envelope.bytes, "base64");
    const failed = retainedEntry(rejectedAssemblyPath, failedBytes);
    requireValue(failed.sha256 === rejectedAssemblySha256 && (!envelope || envelope.path === failed.path && failed.bytes === envelope.bytes && envelope.sha256 === failed.sha256) && JSON.parse(failedBytes.toString()).exit === 1, "failed assembly receipt differs");
    history.push(retainedEntry(rejectedIntermediatePath, intermediateBytes), failed);
    if (latest) history.push(retainedEntry(rejectedLatestPath, latestBytes));
    if (attempt12) history.push(retainedEntry(rejectedAttempt12Path, attempt12Bytes));
    if (attempt13) history.push(retainedEntry(attempt13Path, previous));
  }
  const entries = [...[...pendingEntries(root, pending, intermediate || latest), ...extra].map(e => ({ ...e, scope: "current" as const })),
    ...[...canonicalEntries(root), ...history].map(e => ({ ...e, scope: "archive" as const }))];
  const bytes = Buffer.from(JSON.stringify({ schemaVersion: 1, kind: "retained-evidence-container", purpose: "combined", entries }, null, 2) + "\n");
  decodeContainer(bytes);
  for (const entry of entries.filter(e => e.scope === "archive"))
    requireValue(selectedBytes(bytes, { path: entry.path, sha256: entry.sha256 }, true).equals(Buffer.from(entry.bytes, "base64")), "rotation archive round trip differs");
  const temporary = resolve(root, pending, "rotation-container.tmp");
  writeFileSync(temporary, bytes, { flag: "wx", mode: 0o644 });
  requireValue(hash(canonicalBytes(root, `${pending}/rotation-container.tmp`)) === hash(bytes), "rotation temporary differs");
  renameSync(temporary, resolve(root, path));
  requireValue(canonicalBytes(root, path).equals(bytes), "rotated container differs");
  const ref = reference(root, path);
  return { ref, entries: Object.fromEntries(entries.map(e => [e.path, { ...ref, selector: { path: e.path, sha256: e.sha256 } }])) };
}
export function canonicalEntries(root: string) {
  return ["gates", "timing", "historical", "preservation"].map(n => {
    const original = `${directory}/implementation/${n}.json`;
    return retainedEntry(original, canonicalBytes(root, original));
  });
}
function externalArchive(root: string, entry: Entry): Entry {
  requireValue(entry.encoding === "base64" && entry.scope === "archive", "only archive entries may be externalized");
  const bytes = Buffer.from(entry.bytes, "base64");
  requireValue(hash(bytes) === entry.sha256 && bytes.length === entry.size, "external archive input differs");
  const path = `target/exec-fail-open/task13-preserved/archive/${entry.sha256}.json`;
  mkdirSync(resolve(root, path, ".."), { recursive: true });
  const { realpathSync } = require("node:fs") as typeof import("node:fs");
  requireValue(realpathSync(resolve(root, path, "..")) === resolve(realpathSync(root), path, ".."), "unsafe external archive parent");
  if (!existsSync(resolve(root, path))) writeFileSync(resolve(root, path), bytes, { flag: "wx", mode: 0o644 });
  requireValue(canonicalBytes(root, path).equals(bytes), "external archive copy differs");
  return { ...entry, encoding: "external", bytes: path };
}
// Replace current observations only after retaining the exact accepted predecessor.
export function rotateTask13(root: string, pending: string, path: string, extra: Entry[]) {
  requireValue(path === `${directory}/implementation/collection-next.json`, "invalid correction rotation path");
  const prior = canonicalBytes(root, path); requireStaged(root, path, prior);
  const old = decodeContainer(prior), archived = canonicalArchive(root, reference(root, path));
  const original = canonicalBytes(root, "target/exec-fail-open/task13-preserved/collection-next-original.json");
  requireValue(hash(original) === persistent4Sha256 && original.length === 152867036, "accepted predecessor bytes differ");
  const decoded = decodeContainer(original);
  requireValue(old.purpose === "combined" && decoded.purpose === "combined" && decoded.entries.length === 53, "accepted predecessor inventory differs");
  const isOriginal = hash(prior) === persistent4Sha256;
  requireValue(isOriginal || archived[persistent4Path] && archiveBytes(root, archived[persistent4Path]).equals(original), "accepted predecessor lineage differs");
  // decodeContainer authenticates inline selections; canonicalArchive also
  // authenticates every external selection before the predecessor is retained.
  const canonical = canonicalEntries(root).map(e => externalArchive(root, { ...e, scope: "archive" }));
  const previous = externalArchive(root, { ...retainedEntry(isOriginal ? persistent4Path : task16PredecessorPath, prior), scope: "archive" });
  const history = old.entries.filter(e => e.scope === "archive" && e.path !== previous.path && !canonical.some(c => c.path === e.path))
    .map(e => e.encoding === "base64" ? externalArchive(root, e) : e);
  const entries = [...[...pendingEntries(root, pending), ...extra].map(e => ({ ...e, scope: "current" as const })), ...canonical, ...history, previous];
  const bytes = Buffer.from(JSON.stringify({ schemaVersion: 1, kind: "retained-evidence-container", purpose: "combined", entries }, null, 2) + "\n");
  requireValue(bytes.length <= 10000000, "compact correction exceeds storage limit"); decodeContainer(bytes);
  const temporary = resolve(root, pending, "task13-rotation.tmp");
  writeFileSync(temporary, bytes, { flag: "wx", mode: 0o644 });
  requireValue(canonicalBytes(root, `${pending}/task13-rotation.tmp`).equals(bytes), "correction temporary differs");
  renameSync(temporary, resolve(root, path));
  requireValue(canonicalBytes(root, path).equals(bytes), "corrected container differs");
  const ref = reference(root, path);
  return { ref, entries: Object.fromEntries(entries.map(e => [e.path, { ...ref, selector: { path: e.path, sha256: e.sha256 } }])) };
}
export function archiveCanonical(root: string, path = `${directory}/implementation/canonical-archive.json`) {
  return seal(root, path, canonicalEntries(root), "archive");
}
export function archiveBytes(root: string, ref: ArtifactRef) {
  requireValue(ref.selector, "archive selection required");
  requireValue(ref.path.startsWith(directory + "/"), "archive outside evidence directory");
  const bytes = canonicalBytes(root, ref.path); requireValue(hash(bytes) === ref.sha256, "archive container hash differs");
  requireStaged(root, ref.path, bytes);
  requireValue(["archive", "combined"].includes(decodeContainer(bytes).purpose), "not archive evidence");
  return selectedBytes(bytes, ref.selector, true, root);
}
// Resolve a reference from the retired container only as archival evidence.
// Current artifact selection never follows this route.
export function legacyArchiveBytes(root: string, archived: ArtifactRef, legacy: ArtifactRef, nestedArchive = false) {
  requireValue(legacy.path === archived.path && [rejectedCollectionSha256, rejectedIntermediateSha256, rejectedLatestSha256, rejectedAttempt12Sha256, attempt13Sha256].includes(legacy.sha256) && legacy.selector, "invalid legacy reference");
  checkPredecessorRead(root, legacy.path); checkPredecessorRead(root, legacy.selector.path);
  const previous = archiveBytes(root, archived);
  requireValue(hash(previous) === legacy.sha256, "legacy container identity differs");
  return selectedBytes(previous, legacy.selector, nestedArchive);
}
export function canonicalArchive(root: string, ref: ArtifactRef) {
  requireValue(!ref.selector, "whole archive reference required");
  requireValue(ref.path.startsWith(directory + "/"), "archive outside evidence directory");
  const bytes = artifact(root, ref); requireStaged(root, ref.path, bytes);
  const c = decodeContainer(bytes);
  requireValue(c.purpose === "archive" || c.purpose === "combined", "not archive evidence");
  const latestEntry = c.entries.find(e => e.path === rejectedLatestPath);
  requireValue(!latestEntry || latestEntry.scope === "archive", "canonical archive entries differ");
  const attempt12Entry = c.entries.find(e => e.path === rejectedAttempt12Path);
  requireValue(!attempt12Entry || attempt12Entry.scope === "archive", "canonical archive entries differ");
  const attempt13Entry = c.entries.find(e => e.path === attempt13Path);
  requireValue(!attempt13Entry || attempt13Entry.scope === "archive", "canonical archive entries differ");
  const persistent4Entry = c.entries.find(e => e.path === persistent4Path);
  requireValue(!persistent4Entry || persistent4Entry.scope === "archive", "canonical archive entries differ");
  const task16Entry = c.entries.find(e => e.path === task16PredecessorPath);
  requireValue(!task16Entry || task16Entry.scope === "archive", "canonical archive entries differ");
  const entries = c.entries.filter(e => c.purpose === "archive" || e.scope === "archive");
  for (const entry of entries.filter(e => e.encoding === "external"))
    selectedBytes(bytes, { path: entry.path, sha256: entry.sha256 }, true, root);
  const expected = ["gates", "timing", "historical", "preservation"].map(n => `${directory}/implementation/${n}.json`);
  const rejected = entries.filter(e => e.path === ref.path);
  const intermediate = entries.find(e => e.path === rejectedIntermediatePath), failure = entries.find(e => e.path === rejectedAssemblyPath), latest = entries.find(e => e.path === rejectedLatestPath);
  const base = rejected.length === 1 && rejected[0].sha256 === rejectedCollectionSha256;
  const oldPair = intermediate?.sha256 === rejectedIntermediateSha256 && failure?.sha256 === rejectedAssemblySha256;
  const validHistory = entries.length === expected.length || base && (entries.length === expected.length + 1 || oldPair &&
    (entries.length === expected.length + 3 || latest?.sha256 === rejectedLatestSha256 &&
      (entries.length === expected.length + 4 || attempt12Entry?.sha256 === rejectedAttempt12Sha256 &&
        (entries.length === expected.length + 5 || attempt13Entry?.sha256 === attempt13Sha256 &&
          (entries.length === expected.length + 6 || (entries.length === expected.length + 7 || entries.length === expected.length + 8 && !!task16Entry) && persistent4Entry?.sha256 === persistent4Sha256)))));
  requireValue(validHistory && expected.every(path => entries.some(e => e.path === path)), "canonical archive entries differ");
  if (rejected.length) {
    const previous = selectedBytes(bytes, { path: ref.path, sha256: rejectedCollectionSha256 }, true, root), old = decodeContainer(previous);
    requireValue(old.purpose === "combined" && old.entries.length === 53 && old.entries.some(e => e.path.endsWith("/native/terminal-2.json") && e.sha256 === "753a3ec4c14eaa831f2c298fbc7ae9b313ee5a6fa7c2c3c05de3c43d58f81564"), "rejected archive selections differ");
  }
  if (intermediate && failure) {
    const earlier = selectedBytes(bytes, { path: intermediate.path, sha256: intermediate.sha256 }, true, root), decoded = decodeContainer(earlier);
    requireValue(decoded.purpose === "combined" && decoded.entries.length === 59 && hash(selectedBytes(earlier, { path: ref.path, sha256: rejectedCollectionSha256 }, true)) === rejectedCollectionSha256, "intermediate archive differs");
    requireValue(JSON.parse(selectedBytes(bytes, { path: failure.path, sha256: failure.sha256 }, true, root).toString()).exit === 1, "failed assembly archive differs");
  }
  if (latest) {
    const recent = selectedBytes(bytes, { path: latest.path, sha256: latest.sha256 }, true, root), decoded = decodeContainer(recent);
    requireValue(decoded.purpose === "combined" && decoded.entries.length === 62 &&
      hash(selectedBytes(recent, { path: ref.path, sha256: rejectedCollectionSha256 }, true)) === rejectedCollectionSha256 &&
      hash(selectedBytes(recent, { path: rejectedIntermediatePath, sha256: rejectedIntermediateSha256 }, true)) === rejectedIntermediateSha256 &&
      hash(selectedBytes(recent, { path: rejectedAssemblyPath, sha256: rejectedAssemblySha256 }, true)) === rejectedAssemblySha256, "latest archive differs");
  }
  if (attempt12Entry) {
    const prior = selectedBytes(bytes, { path: rejectedAttempt12Path, sha256: rejectedAttempt12Sha256 }, true, root), decoded = decodeContainer(prior);
    requireValue(decoded.purpose === "combined" && decoded.entries.length === 62 &&
      hash(selectedBytes(prior, { path: rejectedLatestPath, sha256: rejectedLatestSha256 }, true)) === rejectedLatestSha256 &&
      hash(selectedBytes(prior, { path: ref.path, sha256: rejectedCollectionSha256 }, true)) === rejectedCollectionSha256 &&
      hash(selectedBytes(prior, { path: rejectedIntermediatePath, sha256: rejectedIntermediateSha256 }, true)) === rejectedIntermediateSha256 &&
      hash(selectedBytes(prior, { path: rejectedAssemblyPath, sha256: rejectedAssemblySha256 }, true)) === rejectedAssemblySha256, "attempt-12 archive differs");
  }
  if (attempt13Entry) {
    const prior = selectedBytes(bytes, { path: attempt13Path, sha256: attempt13Sha256 }, true, root), decoded = decodeContainer(prior);
    requireValue(decoded.purpose === "combined" && decoded.entries.length === 52 &&
      decoded.entries.filter(e => e.scope === "current").length === 43 && decoded.entries.filter(e => e.scope === "archive").length === 9 &&
      hash(selectedBytes(prior, { path: rejectedAttempt12Path, sha256: rejectedAttempt12Sha256 }, true)) === rejectedAttempt12Sha256 &&
      hash(selectedBytes(prior, { path: rejectedLatestPath, sha256: rejectedLatestSha256 }, true)) === rejectedLatestSha256 &&
      hash(selectedBytes(prior, { path: ref.path, sha256: rejectedCollectionSha256 }, true)) === rejectedCollectionSha256 &&
      hash(selectedBytes(prior, { path: rejectedIntermediatePath, sha256: rejectedIntermediateSha256 }, true)) === rejectedIntermediateSha256 &&
      hash(selectedBytes(prior, { path: rejectedAssemblyPath, sha256: rejectedAssemblySha256 }, true)) === rejectedAssemblySha256, "attempt-13 archive differs");
  }
  if (persistent4Entry) {
    const prior = selectedBytes(bytes, { path: persistent4Path, sha256: persistent4Sha256 }, true, root), decoded = decodeContainer(prior);
    requireValue(prior.length === 152867036 && decoded.purpose === "combined" && decoded.entries.length === 53 &&
      hash(selectedBytes(prior, { path: attempt13Path, sha256: attempt13Sha256 }, true)) === attempt13Sha256, "persistent predecessor differs");
  }
  if (task16Entry) {
    const seen = new Set([hash(bytes)]);
    let parent = bytes, predecessor: Entry | undefined = task16Entry;
    while (predecessor) {
      requireValue(predecessor.scope === "archive" && !seen.has(predecessor.sha256), "correction predecessor cycle or scope differs");
      seen.add(predecessor.sha256);
      const previous = selectedBytes(parent, { path: predecessor.path, sha256: predecessor.sha256 }, true, root), decoded = decodeContainer(previous);
      requireValue(decoded.purpose === "combined" && decoded.entries.some(e => e.path === persistent4Path && e.sha256 === persistent4Sha256 && e.scope === "archive"), "correction predecessor lineage differs");
      for (const entry of decoded.entries.filter(e => e.encoding === "external"))
        selectedBytes(previous, { path: entry.path, sha256: entry.sha256 }, true, root);
      parent = previous;
      predecessor = decoded.entries.find(e => e.path === task16PredecessorPath);
    }
  }
  return Object.fromEntries(entries.map(e => [e.path, { ...ref, selector: { path: e.path, sha256: e.sha256 } }]));
}
// Fixtures use the same authenticated predecessor before and after rotation.
export function retainedPredecessor(root: string, digest: string, archivedPath: string) {
  requireValue((digest === rejectedLatestSha256 && archivedPath === rejectedLatestPath) ||
    (digest === rejectedAttempt12Sha256 && archivedPath === rejectedAttempt12Path) ||
    (digest === attempt13Sha256 && archivedPath === attempt13Path), "unknown predecessor identity");
  const path = `${directory}/implementation/collection-next.json`, bytes = canonicalBytes(root, path);
  requireStaged(root, path, bytes);
  const archives = canonicalArchive(root, reference(root, path));
  if (hash(bytes) === digest) return bytes;
  const ref = archives[archivedPath];
  requireValue(ref?.selector?.sha256 === digest, "predecessor archive missing or altered");
  return archiveBytes(root, ref);
}
export function compactReferences(root: string, ref: ArtifactRef) {
  requireValue(!ref.selector, "container reference expected");
  const bytes = artifact(root, ref), c = decodeContainer(bytes);
  requireValue(c.purpose === "collection" || c.purpose === "combined", "only current collection entries can be selected");
  requireStaged(root, ref.path, bytes);
  return c.entries.filter(e => c.purpose === "collection" || e.scope === "current").map(e => ({ ...ref, selector: { path: e.path, sha256: e.sha256 } }));
}
