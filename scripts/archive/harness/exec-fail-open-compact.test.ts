import { test, expect } from "bun:test";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, symlinkSync, copyFileSync, renameSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { directory, artifact, reference, boundJSON, hash, canonicalBytes, validateCommon, context, root as productRoot } from "./exec-fail-open-inputs";
import { retainedEntry, decodeContainer, selectedBytes } from "./exec-fail-open-container";
import { seal, archiveCanonical, archiveBytes, canonicalArchive, compactReferences, rotateRejected, rotateTask13, persistent4Path, persistent4Sha256, rejectedCollectionSha256, rejectedLatestSha256, rejectedLatestPath, rejectedAttempt12Path, rejectedAttempt12Sha256, retainedPredecessor, rejectedIntermediatePath, rejectedIntermediateSha256, rejectedAssemblyPath, rejectedAssemblySha256, legacyArchiveBytes } from "./exec-fail-open-compact";
import { reachableEvidence } from "./exec-fail-open-traversal";
import { predecessorReads } from "./exec-fail-open-reads";
import { fixtureFirstRun } from "./exec-fail-open-fixture";
import { validateCorrectionReview, validateQaTransition } from "./exec-fail-open-evidence";
import { validateGateData, validatePriorArchive } from "./exec-fail-open-gates";
import { installHistoricalQaFixture } from "./exec-fail-open-compact-historical-fixture";
import { checkRepeatedRotation, installArchiveDependencies } from "./exec-fail-open-compact-archive-fixture";
export function isolated(run: (root: string, git: (...args: string[]) => void) => void) {
  const root = mkdtempSync(resolve(tmpdir(), "arc-compact-"));
  const git = (...args: string[]) => { const r = spawnSync("git", args, { cwd: root, encoding: "utf8" }); expect(r.status).toBe(0); };
  try {
    git("init", "--quiet"); mkdirSync(resolve(root, directory, "implementation"), { recursive: true });
    writeFileSync(resolve(root, directory, "contract.json"), "{}\n"); run(root, git);
  } finally { rmSync(root, { recursive: true, force: true }); }
}
test("compact legacy, exact archive bytes, missing/duplicate/encoding/digest/path tampering and staged regular bytes", () => {
  isolated((root, git) => {
    const logical = `${directory}/implementation/commands/a.json`, path = `${directory}/implementation/collection.json`;
    const bytes = Buffer.from('{"kind":"command","status":"failed","secretFree":true}\r\n');
    const legacy = `${directory}/legacy.json`; writeFileSync(resolve(root, legacy), bytes);
    expect(artifact(root, reference(root, legacy))).toEqual(bytes);
    const e = retainedEntry(logical, bytes), sealed = seal(root, path, [e]);
    const ref = sealed.entries[logical]; expect(ref.sha256).toBe(hash(canonicalBytes(root, path))); expect(ref.selector.sha256).toBe(hash(bytes));
    expect(() => artifact(root, ref)).toThrow("unstaged container"); git("add", directory);
    expect(artifact(root, ref)).toEqual(bytes);
    expect(() => seal(root, path, [e])).toThrow("EEXIST");
    const original = readFileSync(resolve(root, path));
    try {
      writeFileSync(resolve(root, path), Buffer.concat([original, Buffer.from(" ")]));
      expect(() => artifact(root, ref)).toThrow("hash differs");
      expect(() => artifact(root, { ...ref, sha256: hash(readFileSync(resolve(root, path))) })).toThrow("index bytes");
    } finally { writeFileSync(resolve(root, path), original); }
    const c = JSON.parse(original.toString());
    for (const patch of [ { entries: [retainedEntry(logical, Buffer.from(JSON.stringify({ type: "llvm.coverage.json.export" })))] }, { entries: [] }, { entries: [e,e] }, { entries: [{...e,bytes:e.bytes+"!"}] }, { entries: [{...e,encoding:"utf8"}] }, { entries: [{...e,size:0}] }, { entries: [{...e,sha256:"0".repeat(64)}] }, { entries: [{...e,path:"../bad"}] }, { entries: [{...e,identity:"substituted"}] }, { entries: [{...e,verdict:"passed"}] } ])
      expect(() => decodeContainer(Buffer.from(JSON.stringify({...c,...patch})))).toThrow();
    for (const selector of [{path:logical,sha256:"0".repeat(64)}, {path:logical+"missing",sha256:e.sha256}, {path:"../bad",sha256:e.sha256}, {path:logical,sha256:e.sha256,extra:true}])
      expect(() => selectedBytes(original,selector)).toThrow();
    symlinkSync(resolve(root, path), resolve(root, directory, "link.json"));
    expect(() => artifact(root, {...ref,path:`${directory}/link.json`})).toThrow("nonregular");
    for (const name of ["gates","timing","historical","preservation"]) writeFileSync(resolve(root,directory,`implementation/${name}.json`),bytes);
    const archived = archiveCanonical(root); git("add", directory);
    for (const a of Object.values(archived.entries)) { expect(archiveBytes(root,a)).toEqual(bytes); expect(() => artifact(root,a)).toThrow("archived bytes"); }
  });
});
test("selected logical successors are rejected before any file access and sibling records are not cycles", () => {
  isolated((root, git) => {
    const base = `${directory}/implementation`, a = `${base}/native/a.json`, b = `${base}/native/b.json`;
    const path = `${base}/collection.json`, child = retainedEntry(b,Buffer.from('{"fixtureOnly":true}'));
    // Intra-container refs cannot carry the final container digest without a
    // hash fixed point. Legacy sibling -> selected leaf exercises shared path.
    const sealed=seal(root,path,[child,retainedEntry(a,Buffer.from('{}'))]); git("add",directory);
    expect(reachableEvidence(root,[sealed.entries[a],sealed.entries[b]])).toEqual([path]);
    const successor={...sealed.ref,selector:{path:`${directory}/final.json`,sha256:child.sha256}};
    predecessorReads(root,"gates",()=>expect(()=>artifact(root,{...successor,path:`${directory}/does-not-exist.json`})).toThrow("attempted successor read"));
    const outer=`${base}/outer.json`; writeFileSync(resolve(root,outer),JSON.stringify({child:sealed.entries[b]}));
    expect(reachableEvidence(root,reference(root,outer))).toEqual([outer,path]);
    const hidden=seal(root,`${base}/hidden.json`,[retainedEntry(a,Buffer.from(JSON.stringify({child:successor})))]);git("add",directory);
    predecessorReads(root,"gates",()=>expect(()=>reachableEvidence(root,hidden.entries[a])).toThrow("attempted successor read"));
    const identity=JSON.stringify([path,a]);expect(()=>reachableEvidence(root,sealed.entries[b],[identity])).not.toThrow();expect(()=>reachableEvidence(root,sealed.entries[a],[identity])).toThrow("self or terminal");
    // Unselected records are integrity-checked, never semantic prerequisites.
    const unrelated=seal(root,`${base}/unrelated.json`,[child,retainedEntry(a,Buffer.from(JSON.stringify({child:successor})))]);git("add",directory);
    predecessorReads(root,"gates",()=>expect(()=>reachableEvidence(root,unrelated.entries[b])).not.toThrow());
  });
});
test("legacy per-observation storage cannot complete with 140 retained files", () => {
  expect(() => fixtureFirstRun(()=>{},140)).toThrow("below 150");
});
test("combined storage isolates authenticated archival entries from current selections", () => {
  isolated((root, git) => {
    const base = `${directory}/implementation`, path = `${base}/next.json`;
    const current = retainedEntry(`${base}/commands/current.json`, Buffer.from('{"kind":"command","passed":true}\n'));
    const historical = ["gates", "timing", "historical", "preservation"].map(name =>
      retainedEntry(`${base}/${name}.json`, Buffer.from(JSON.stringify({ kind: `old-${name}`, passed: false,
        successor: { path: `${directory}/final.json`, sha256: "0".repeat(64) } }) + "\r\n")));
    const entries = [{ ...current, scope: "current" }, ...historical.map(e => ({ ...e, scope: "archive" }))];
    const combined = seal(root, path, entries, "combined"); git("add", directory);
    expect(compactReferences(root, combined.ref)).toEqual([combined.entries[current.path]]);
    expect(artifact(root, combined.entries[current.path])).toEqual(Buffer.from(current.bytes, "base64"));
    predecessorReads(root, "gates", () => expect(reachableEvidence(root, combined.entries[current.path])).toEqual([path]));
    expect(() => archiveBytes(root, combined.entries[current.path])).toThrow();
    for (const entry of historical) {
      const ref = combined.entries[entry.path];
      expect(archiveBytes(root, ref)).toEqual(Buffer.from(entry.bytes, "base64"));
      expect(() => artifact(root, ref)).toThrow("archived bytes");
    }
    const bytes = canonicalBytes(root, path), container = JSON.parse(bytes.toString());
    // Changing the envelope type must not turn tagged archived bytes into current evidence.
    expect(() => decodeContainer(Buffer.from(JSON.stringify({ ...container, purpose: "collection" })))).toThrow();
    for (const update of [
      { entries: [...container.entries, container.entries[1]] },
      { entries: container.entries.map((e: any, i: number) => i === 1 ? { ...e, bytes: e.bytes + "!" } : e) },
      { entries: container.entries.map((e: any, i: number) => i === 1 ? { ...e, scope: "unknown" } : e) },
      { entries: container.entries.map((e: any, i: number) => i === 1 ? { ...e, path: "../unsafe" } : e) },
    ]) expect(() => decodeContainer(Buffer.from(JSON.stringify({ ...container, ...update })))).toThrow();
    writeFileSync(resolve(root, path), Buffer.concat([bytes, Buffer.from(" ")]));
    expect(() => archiveBytes(root, combined.entries[historical[0].path])).toThrow("hash differs");
    const changed = { ...reference(root, path), selector: combined.entries[historical[0].path].selector };
    expect(() => archiveBytes(root, changed)).toThrow("index bytes");
  });
});
test("external archives authenticate persistent bytes and cannot become current evidence", () => {
  isolated((root, git) => {
    const base = `${directory}/implementation`, path = `${base}/external.json`;
    const current = { ...retainedEntry(`${base}/commands/current.json`, Buffer.from('{"kind":"command"}')), scope: "current" as const };
    const raw = Buffer.from('{"kind":"old-gates","passed":true}\r\n');
    const entry = retainedEntry(`${base}/gates.json`, raw);
    const external = `target/exec-fail-open/task13-preserved/archive/${entry.sha256}.json`;
    mkdirSync(resolve(root, external, ".."), { recursive: true });
    writeFileSync(resolve(root, external), raw);
    const archived = { ...entry, encoding: "external" as const, bytes: external, scope: "archive" as const };
    const others = ["timing", "historical", "preservation"].map(name => ({ ...retainedEntry(`${base}/${name}.json`, raw), scope: "archive" as const }));
    const sealed = seal(root, path, [current, archived, ...others], "combined"); git("add", directory);
    expect(archiveBytes(root, sealed.entries[entry.path])).toEqual(raw);
    expect(canonicalArchive(root, sealed.ref)[entry.path]).toEqual(sealed.entries[entry.path]);
    expect(() => artifact(root, sealed.entries[entry.path])).toThrow("archived bytes");
    const envelope = canonicalBytes(root, path), object = JSON.parse(envelope.toString());
    for (const patch of [
      { scope: "current" }, { bytes: "../unsafe" }, { bytes: external.replace(entry.sha256, "0".repeat(64)) },
      { encoding: "other" }, { sha256: "0".repeat(64) },
    ]) {
      const changed = { ...object, entries: object.entries.map((item: any) => item.path === entry.path ? { ...item, ...patch } : item) };
      expect(() => decodeContainer(Buffer.from(JSON.stringify(changed)))).toThrow();
    }
    writeFileSync(resolve(root, external), Buffer.from("altered"));
    expect(() => canonicalArchive(root, sealed.ref)).toThrow("external archive bytes/digest differ");
    rmSync(resolve(root, external));
    expect(() => archiveBytes(root, sealed.entries[entry.path])).toThrow();
    const other = resolve(root, "other.json"); writeFileSync(other, raw); symlinkSync(other, resolve(root, external));
    expect(() => archiveBytes(root, sealed.entries[entry.path])).toThrow("nonregular external archive");
    rmSync(resolve(root, external)); writeFileSync(resolve(root, external), raw);
    writeFileSync(resolve(root, path), Buffer.concat([envelope, Buffer.from(" ")]));
    expect(() => archiveBytes(root, { ...reference(root, path), selector: sealed.entries[entry.path].selector })).toThrow("index bytes");
  });
});
test("task-13 rotation externalizes archival entries from the original inline container", () => {
  isolated((root, git) => {
    const path = `${directory}/implementation/collection-next.json`;
    const saved = "target/exec-fail-open/task13-preserved/collection-next-original.json";
    const original = canonicalBytes(productRoot, saved), prior = decodeContainer(original);
    expect(original.length).toBe(152867036);
    expect(hash(original)).toBe(persistent4Sha256);
    expect(prior.entries.filter(e => e.scope === "archive" && e.encoding === "base64")).toHaveLength(10);
    mkdirSync(resolve(root, saved, ".."), { recursive: true });
    writeFileSync(resolve(root, saved), original);
    writeFileSync(resolve(root, path), original);
    const canonical = Object.fromEntries(["gates", "timing", "historical", "preservation"].map(name => {
      const p = `${directory}/implementation/${name}.json`;
      const bytes = canonicalBytes(productRoot, `target/exec-fail-open/task13-preserved/prior-canonical/${name}.json`);
      writeFileSync(resolve(root, p), bytes); return [p, bytes];
    }));
    const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
    mkdirSync(resolve(root, pending), { recursive: true });
    const current = retainedEntry(logical, Buffer.from('{"kind":"command","passed":true}\n'));
    writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: current.sha256, bytes: current.bytes }));
    git("add", directory);
    const rotated = rotateTask13(root, pending, path, []);
    const bytes = canonicalBytes(root, path), result = decodeContainer(bytes);
    expect(bytes.length).toBeLessThanOrEqual(10000000);
    expect(result.entries.filter(e => e.scope === "archive").every(e => e.encoding === "external")).toBe(true);
    expect(() => canonicalArchive(root, rotated.ref)).toThrow("unstaged container");
    git("add", directory);
    const archive = canonicalArchive(root, rotated.ref);
    expect(archiveBytes(root, archive[persistent4Path])).toEqual(original);
    expect(() => artifact(root, archive[persistent4Path])).toThrow("archived bytes");
    for (const [p, expected] of Object.entries(canonical)) expect(archiveBytes(root, archive[p])).toEqual(expected);
    for (const entry of prior.entries.filter(e => e.scope === "archive" && !(e.path in canonical))) {
      expect(archiveBytes(root, archive[entry.path])).toEqual(Buffer.from(entry.bytes, "base64"));
      expect(() => artifact(root, archive[entry.path])).toThrow("archived bytes");
    }
    expect(compactReferences(root, rotated.ref)).toEqual([rotated.entries[logical]]);
    expect(artifact(root, rotated.entries[logical])).toEqual(Buffer.from(current.bytes, "base64"));
    const external = result.entries.find(e => e.path === rejectedAssemblyPath)!;
    writeFileSync(resolve(root, external.bytes), "substituted");
    expect(() => archiveBytes(root, archive[external.path])).toThrow("external archive bytes/digest differ");
  });
}, 7200000);
test("correction rotation retains the staged predecessor with a different inventory", () => {
  isolated((root, git) => {
    const path = `${directory}/implementation/collection-next.json`;
    const product = canonicalBytes(productRoot, path), productEntries = decodeContainer(product).entries;
    const retained = productEntries.find(e => e.path === `${directory}/implementation/task16-predecessor.json` && e.scope === "archive");
    const prior = retained ? selectedBytes(product, { path: retained.path, sha256: retained.sha256 }, true, productRoot) : product;
    const old = decodeContainer(prior);
    expect(old.entries.length).not.toBe(53);
    writeFileSync(resolve(root, path), prior);
    const saved = "target/exec-fail-open/task13-preserved/collection-next-original.json";
    mkdirSync(resolve(root, saved, ".."), { recursive: true });
    copyFileSync(resolve(productRoot, saved), resolve(root, saved));
    installArchiveDependencies(root, prior, false);
    const canonical = ["gates", "timing", "historical", "preservation"].map(name => `${directory}/implementation/${name}.json`);
    for (const p of canonical) writeFileSync(resolve(root, p), canonicalBytes(productRoot, p));
    const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
    mkdirSync(resolve(root, pending), { recursive: true });
    const current = retainedEntry(logical, Buffer.from('{"kind":"command","passed":true}\n'));
    writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: current.sha256, bytes: current.bytes }));
    git("add", directory);
    const rotated = rotateTask13(root, pending, path, []);
    expect(canonicalBytes(root, path).length).toBeLessThanOrEqual(10000000);
    git("add", directory);
    const archive = canonicalArchive(root, rotated.ref);
    const predecessor = archiveBytes(root, archive[`${directory}/implementation/task16-predecessor.json`]);
    expect(predecessor).toEqual(prior);
    for (const e of old.entries) {
      const selector = { path: e.path, sha256: e.sha256 };
      expect(selectedBytes(predecessor, selector, e.scope === "archive", root)).toEqual(selectedBytes(prior, selector, e.scope === "archive", root));
    }
    for (const p of canonical) expect(archiveBytes(root, archive[p])).toEqual(canonicalBytes(root, p));
    expect(artifact(root, rotated.entries[logical])).toEqual(Buffer.from(current.bytes, "base64"));
  });
}, 7200000);
test("guarded rotation restores the rejected container and all four prior canonical files", () => {
  isolated((root, git) => {
    const path = `${directory}/implementation/collection-next.json`, product = canonicalBytes(productRoot, path);
    const previous = hash(product) === rejectedCollectionSha256 ? product : selectedBytes(product, { path, sha256: rejectedCollectionSha256 }, true, productRoot);
    expect(hash(previous)).toBe(rejectedCollectionSha256);
    writeFileSync(resolve(root, path), previous);
    const originals = Object.fromEntries(["gates", "timing", "historical", "preservation"].map(n => {
      const p = `${directory}/implementation/${n}.json`, bytes = Buffer.from(`{"kind":"old-${n}","passed":false}\r\n`);
      writeFileSync(resolve(root, p), bytes); return [p, bytes];
    }));
    const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
    mkdirSync(resolve(root, pending), { recursive: true });
    const current = retainedEntry(logical, Buffer.from('{"kind":"command","passed":true}\n'));
    writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: current.sha256, bytes: current.bytes }));
    git("add", directory);
    expect(() => seal(root, path, [current])).toThrow("EEXIST");
    const rotated = rotateRejected(root, pending, path, []);
    expect(() => compactReferences(root, rotated.ref)).toThrow("unstaged container");
    git("add", directory);
    const archive = canonicalArchive(root, rotated.ref);
    expect(archiveBytes(root, archive[path])).toEqual(previous);
    const old = decodeContainer(archiveBytes(root, archive[path])); expect(old.entries).toHaveLength(53);
    for (const entry of old.entries) {
      const legacy = { path, sha256: rejectedCollectionSha256, selector: { path: entry.path, sha256: entry.sha256 } };
      expect(legacyArchiveBytes(root, archive[path], legacy, entry.scope === "archive")).toEqual(Buffer.from(entry.bytes, "base64"));
      expect(() => artifact(root, legacy)).toThrow("artifact hash differs");
    }
    predecessorReads(root, "gates", () => expect(() => legacyArchiveBytes(root, archive[path], { path, sha256: rejectedCollectionSha256, selector: { path: `${directory}/final.json`, sha256: "0".repeat(64) } })).toThrow("attempted successor read"));
    expect(() => artifact(root, archive[path])).toThrow("archived bytes");
    for (const [p, bytes] of Object.entries(originals)) {
      expect(archiveBytes(root, archive[p])).toEqual(bytes);
      expect(() => artifact(root, archive[p])).toThrow("archived bytes");
    }
    expect(compactReferences(root, rotated.ref)).toEqual([rotated.entries[logical]]);
    expect(artifact(root, rotated.entries[logical])).toEqual(Buffer.from(current.bytes, "base64"));
    predecessorReads(root, "gates", () => expect(reachableEvidence(root, rotated.entries[logical])).toEqual([path]));
    expect(() => rotateRejected(root, pending, path, [])).toThrow("rejected container identity differs");
  });
});
test("latest rotation restores all 62 selections, nested archives and canonical predecessors", () => {
  isolated((root, git) => {
    const path = `${directory}/implementation/collection-next.json`, latest = retainedPredecessor(productRoot, rejectedLatestSha256, rejectedLatestPath);
    expect(hash(latest)).toBe(rejectedLatestSha256);
    const previous = decodeContainer(latest); expect(previous.entries).toHaveLength(62);
    writeFileSync(resolve(root, path), latest);
    const originals = Object.fromEntries(["gates", "timing", "historical", "preservation"].map(n => {
      const p = `${directory}/implementation/${n}.json`, bytes = Buffer.from(`{"kind":"predecessor-${n}","passed":false}\r\n`);
      writeFileSync(resolve(root, p), bytes); return [p, bytes];
    }));
    const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
    mkdirSync(resolve(root, pending), { recursive: true });
    const current = retainedEntry(logical, Buffer.from('{"kind":"command","passed":true}\n'));
    writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: current.sha256, bytes: current.bytes }));
    git("add", directory);
    expect(() => seal(root, path, [current])).toThrow("EEXIST");
    const rotated = rotateRejected(root, pending, path, []);
    expect(() => canonicalArchive(root, rotated.ref)).toThrow("unstaged container");
    git("add", directory);
    const archive = canonicalArchive(root, rotated.ref), latestRef = archive[rejectedLatestPath];
    expect(archiveBytes(root, latestRef)).toEqual(latest);
    expect(() => artifact(root, latestRef)).toThrow("archived bytes");
    for (const entry of previous.entries) {
      const ref = { path, sha256: rejectedLatestSha256, selector: { path: entry.path, sha256: entry.sha256 } };
      expect(legacyArchiveBytes(root, latestRef, ref, entry.scope === "archive")).toEqual(Buffer.from(entry.bytes, "base64"));
    }
    for (const entry of [previous.entries.find(e => e.scope === "current")!, previous.entries.find(e => e.scope === "archive")!]) {
      const ref = { path, sha256: rejectedLatestSha256, selector: { path: entry.path, sha256: entry.sha256 } };
      expect(() => legacyArchiveBytes(root, latestRef, ref, entry.scope !== "archive")).toThrow();
    }
    for (const [p, sha256] of [[path, rejectedCollectionSha256], [rejectedIntermediatePath, rejectedIntermediateSha256], [rejectedAssemblyPath, rejectedAssemblySha256]])
      expect(hash(archiveBytes(root, archive[p]))).toBe(sha256);
    for (const [p, bytes] of Object.entries(originals)) expect(archiveBytes(root, archive[p])).toEqual(bytes);
    expect(compactReferences(root, rotated.ref)).toEqual([rotated.entries[logical]]);
    const bytes = canonicalBytes(root, path), altered = JSON.parse(bytes.toString());
    altered.entries.find((entry: any) => entry.path === rejectedLatestPath).scope = "current";
    writeFileSync(resolve(root, path), JSON.stringify(altered)); git("add", directory);
    expect(() => canonicalArchive(root, reference(root, path))).toThrow("canonical archive entries differ");
  });
}, 3600000);
test("authenticated attempt-12 rotation retains both 62-entry predecessors in direct and archived states", () => {
  isolated((root, git) => {
    const path = `${directory}/implementation/collection-next.json`;
    const attempt12 = retainedPredecessor(productRoot, rejectedAttempt12Sha256, rejectedAttempt12Path);
    const latest = retainedPredecessor(productRoot, rejectedLatestSha256, rejectedLatestPath);
    writeFileSync(resolve(root, path), latest); git("add", directory);
    expect(retainedPredecessor(root, rejectedLatestSha256, rejectedLatestPath)).toEqual(latest);
    writeFileSync(resolve(root, path), attempt12);
    expect(() => retainedPredecessor(root, rejectedLatestSha256, rejectedLatestPath)).toThrow("unstaged container");
    git("add", directory);
    expect(retainedPredecessor(root, rejectedLatestSha256, rejectedLatestPath)).toEqual(latest);
    expect(retainedPredecessor(root, rejectedAttempt12Sha256, rejectedAttempt12Path)).toEqual(attempt12);
    const originals = Object.fromEntries(["gates", "timing", "historical", "preservation"].map(name => {
      const p = `${directory}/implementation/${name}.json`, bytes = Buffer.from(`{"kind":"attempt12-${name}","passed":false}\r\n`);
      writeFileSync(resolve(root, p), bytes); return [p, bytes];
    }));
    const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
    mkdirSync(resolve(root, pending), { recursive: true });
    const current = retainedEntry(logical, Buffer.from('{"kind":"command","passed":true}\n'));
    writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: current.sha256, bytes: current.bytes }));
    git("add", directory);
    const rotated = rotateRejected(root, pending, path, []);
    expect(() => retainedPredecessor(root, rejectedLatestSha256, rejectedLatestPath)).toThrow("unstaged container");
    git("add", directory);
    const archive = canonicalArchive(root, rotated.ref), priorRef = archive[rejectedAttempt12Path], latestRef = archive[rejectedLatestPath];
    expect(archiveBytes(root, priorRef)).toEqual(attempt12);
    expect(archiveBytes(root, latestRef)).toEqual(latest);
    expect(retainedPredecessor(root, rejectedLatestSha256, rejectedLatestPath)).toEqual(latest);
    expect(retainedPredecessor(root, rejectedAttempt12Sha256, rejectedAttempt12Path)).toEqual(attempt12);
    for (const [bytes, digest, ref] of [[attempt12, rejectedAttempt12Sha256, priorRef], [latest, rejectedLatestSha256, latestRef]] as const) {
      const entries = decodeContainer(bytes).entries; expect(entries).toHaveLength(62);
      for (const entry of entries) {
        const legacy = { path, sha256: digest, selector: { path: entry.path, sha256: entry.sha256 } };
        expect(legacyArchiveBytes(root, ref, legacy, entry.scope === "archive")).toEqual(Buffer.from(entry.bytes, "base64"));
      }
      expect(() => artifact(root, ref)).toThrow("archived bytes");
    }
    for (const [p, bytes] of Object.entries(originals)) expect(archiveBytes(root, archive[p])).toEqual(bytes);
    for (const [p, digest] of [[path, rejectedCollectionSha256], [rejectedIntermediatePath, rejectedIntermediateSha256], [rejectedAssemblyPath, rejectedAssemblySha256]])
      expect(hash(archiveBytes(root, archive[p]))).toBe(digest);
    expect(() => seal(root, path, [current])).toThrow("EEXIST");
    expect(() => rotateRejected(root, pending, path, [])).toThrow("rejected container identity differs");
    const bytes = canonicalBytes(root, path), value = JSON.parse(bytes.toString());
    for (const change of [
      (v: any) => { v.entries = v.entries.filter((e: any) => e.path !== rejectedLatestPath); },
      (v: any) => { v.entries.find((e: any) => e.path === rejectedLatestPath).scope = "current"; },
      (v: any) => { v.entries.find((e: any) => e.path === rejectedLatestPath).bytes += "!"; },
      (v: any) => { v.entries.push(v.entries.find((e: any) => e.path === rejectedLatestPath)); },
    ]) {
      const altered = structuredClone(value); change(altered); writeFileSync(resolve(root, path), JSON.stringify(altered)); git("add", directory);
      expect(() => retainedPredecessor(root, rejectedLatestSha256, rejectedLatestPath)).toThrow();
    }
  });
}, 3600000);
test("authenticated attempt-13 rotation preserves its 52 selections and prior canonical bytes", () => {
  isolated((root, git) => {
    const path = `${directory}/implementation/collection-next.json`;
    const attempt13Path = `${directory}/implementation/attempt13-collection.json`;
    const attempt13Hash = "d23a70c9bb337a54be4d9169730fce34669c6910f9981442d14f7000510c121d";
    const previous = retainedPredecessor(productRoot, attempt13Hash, attempt13Path);
    const previousEntries = decodeContainer(previous).entries;
    expect(previousEntries).toHaveLength(52);
    expect(previousEntries.filter(e => e.scope === "current")).toHaveLength(43);
    expect(previousEntries.filter(e => e.scope === "archive")).toHaveLength(9);
    writeFileSync(resolve(root, path), previous);
    const originals = Object.fromEntries(["gates", "timing", "historical", "preservation"].map(name => {
      const p = `${directory}/implementation/${name}.json`, bytes = canonicalBytes(productRoot, p);
      writeFileSync(resolve(root, p), bytes); return [p, bytes];
    }));
    const pending = "target/exec-fail-open/pending", logical = `${directory}/implementation/commands/current.json`;
    mkdirSync(resolve(root, pending), { recursive: true });
    const current = retainedEntry(logical, Buffer.from('{"kind":"command","passed":true}\n'));
    writeFileSync(resolve(root, pending, "current.json"), JSON.stringify({ path: logical, sha256: current.sha256, bytes: current.bytes }));
    git("add", directory);
    expect(retainedPredecessor(root, attempt13Hash, attempt13Path)).toEqual(previous);
    expect(() => retainedPredecessor(root, attempt13Hash, `${directory}/implementation/wrong.json`)).toThrow("unknown predecessor identity");
    for (const entry of previousEntries) {
      const selector = { path: entry.path, sha256: entry.sha256 };
      expect(selectedBytes(previous, selector, entry.scope === "archive")).toEqual(Buffer.from(entry.bytes, "base64"));
    }
    const rotated = rotateRejected(root, pending, path, []);
    expect(() => canonicalArchive(root, rotated.ref)).toThrow("unstaged container");
    git("add", directory);
    const archive = canonicalArchive(root, rotated.ref), priorRef = archive[attempt13Path];
    expect(archiveBytes(root, priorRef)).toEqual(previous);
    expect(retainedPredecessor(root, attempt13Hash, attempt13Path)).toEqual(previous);
    expect(() => artifact(root, priorRef)).toThrow("archived bytes");
    for (const entry of previousEntries) {
      const legacy = { path, sha256: attempt13Hash, selector: { path: entry.path, sha256: entry.sha256 } };
      expect(legacyArchiveBytes(root, priorRef, legacy, entry.scope === "archive")).toEqual(Buffer.from(entry.bytes, "base64"));
    }
    for (const entry of [previousEntries.find(e => e.scope === "current")!, previousEntries.find(e => e.scope === "archive")!]) {
      const legacy = { path, sha256: attempt13Hash, selector: { path: entry.path, sha256: entry.sha256 } };
      expect(() => legacyArchiveBytes(root, priorRef, legacy, entry.scope !== "archive")).toThrow();
    }
    for (const [p, bytes] of Object.entries(originals)) expect(archiveBytes(root, archive[p])).toEqual(bytes);
    for (const [p, sha256] of [[path, rejectedCollectionSha256], [rejectedIntermediatePath, rejectedIntermediateSha256], [rejectedAssemblyPath, rejectedAssemblySha256], [rejectedLatestPath, rejectedLatestSha256], [rejectedAttempt12Path, rejectedAttempt12Sha256]])
      expect(hash(archiveBytes(root, archive[p]))).toBe(sha256);
    expect(compactReferences(root, rotated.ref)).toEqual([rotated.entries[logical]]);
    expect(() => seal(root, path, [current])).toThrow("EEXIST");
    expect(() => rotateRejected(root, pending, path, [])).toThrow("rejected container identity differs");
    const changedScope = JSON.parse(canonicalBytes(root, path).toString());
    changedScope.entries.find((e: any) => e.path === attempt13Path).scope = "current";
    writeFileSync(resolve(root, path), JSON.stringify(changedScope)); git("add", directory);
    expect(() => canonicalArchive(root, reference(root, path))).toThrow("canonical archive entries differ");
    writeFileSync(resolve(root, path), Buffer.concat([previous, Buffer.from(" ")])); git("add", directory);
    expect(() => rotateRejected(root, pending, path, [])).toThrow("rejected container identity differs");
  });
}, 7200000);
test("production common bindings reject source, host and Actor substitutions",()=>{
  const ctx=context(productRoot),v={...ctx,kind:"command",actorId:"backend",skill:"oma-backend"};
  for(const patch of [{sourceHash:"stale"},{host:{...ctx.host,hostname:"substituted"}},{actorId:"reviewer"},{skill:"oma-qa"},{fixtureOnly:true}])
    expect(()=>validateCommon({...v,...patch},ctx,"command","backend")).toThrow();
});
test("correction rotation preserves the previously rotated predecessor", () => isolated(checkRepeatedRotation), 7200000);
test("accepted QA transition binds prior bytes and rejects unrelated execution changes", () => {
  isolated((root, git) => {
    const { qa, ctx } = installHistoricalQaFixture(root); git("add", directory);
    const transition = validateQaTransition(root, qa, ctx);
    expect(transition.previousSourceHash).toBe("6fd5018b3e56e6e1fbc9190fbe74ff3c990b89bbd8b73e9778d354f97d96d00e");
    expect(transition.currentSourceHash).toBe("33a540623527224ced9ad5569446aeda301c9c9bfc04dad8137332ba33da4a34");
    expect(transition.changedInputs.some(item => item.path === "crates/arc-cli/src/main.rs")).toBe(true);
    expect(() => validateQaTransition(root, { ...qa, sourceHash: "0".repeat(64) }, ctx)).toThrow("accepted QA bytes differ");
    const inputs = structuredClone(ctx.currentInputs);
    inputs["crates/arc-cli/src/exec.rs"].sha256 = "0".repeat(64);
    expect(() => validateQaTransition(root, qa, { ...ctx, currentInputs: inputs, sourceHash: hash(JSON.stringify(inputs)) })).toThrow("unapproved QA transition input");
    expect(() => validateQaTransition(root, qa, context(productRoot))).toThrow("QA transition inventory differs");
  });
}, 7200000);
test("transition requires a separately bound correction review and all three independent checks", () => {
  const ctx = {}, transition = { fixtureOnly: true }, gate = { path: "fixture", sha256: "a".repeat(64) };
  const argv = ["exec-fail-open-docs.test.ts", "exec-fail-open-compact.test.ts", "exec-fail-open-compact-flow.test.ts"].map(name => ["bun", "test", `scripts/harness/${name}`]);
  const commands = argv.map(args => ({ argv: args, timeoutAllowanceMs: 7200000, ended: "2026-11-09T12:00:00.000Z" }));
  const review = { status: "passed", openBlockers: 0, findings: [], frameworkReviewId: "12345678-1234-1234-1234-123456789abc",
    gates: gate, transitionSha256: hash(JSON.stringify(transition)), commands, reviewedAt: "2026-11-09T12:01:00.000Z" };
  const recipe = { common: () => {}, command: (value: any) => value } as any;
  const ready = { reference: gate, commands: [{ ended: "2026-11-09T11:00:00.000Z" }] };
  expect(() => validateCorrectionReview(review, ctx, ready, transition, recipe)).not.toThrow();
  for (const patch of [{ frameworkReviewId: "" }, { gates: { ...gate, sha256: "b".repeat(64) } }, { transitionSha256: "0".repeat(64) },
    { commands: commands.slice(1) }, { commands: [commands[0], commands[0], commands[2]] }, { openBlockers: 1 }, { reviewedAt: "2026-11-09T10:00:00.000Z" }])
    expect(() => validateCorrectionReview({ ...review, ...patch }, ctx, ready, transition, recipe)).toThrow();
  expect(() => validateCorrectionReview({ ...review, commands: [{ ...commands[0], timeoutAllowanceMs: 1 }, ...commands.slice(1)] }, ctx, ready, transition, recipe)).toThrow();
});
test("production gates require the current combined archive and authenticated predecessors", () => {
  const path = `${directory}/implementation/collection-next.json`;
  const gate = { ...JSON.parse(canonicalBytes(productRoot, `${directory}/implementation/gates.json`).toString()), ...context(productRoot) };
  const omitted = { ...gate }; delete omitted.priorObservations;
  expect(() => validateGateData(productRoot, omitted)).toThrow("required combined archive reference differs");
  expect(() => validateGateData(productRoot, { ...gate, priorObservations: reference(productRoot, `${directory}/implementation/canonical-archive.json`) }))
    .toThrow("required combined archive reference differs");
  isolated((root, git) => {
    const original = canonicalBytes(productRoot, path), container = decodeContainer(original);
    const nested = installArchiveDependencies(root, original);
    writeFileSync(resolve(root, path), original); git("add", directory);
    const current = container.entries.find(e => e.scope === "current")!;
    const candidate = () => { const ref = reference(root, path); return { priorObservations: ref,
      commands: [{ ...ref, selector: { path: current.path, sha256: current.sha256 } }] }; };
    expect(() => validatePriorArchive(root, candidate())).not.toThrow();
    const nestedFile = resolve(root, nested[0]), nestedBackup = `${nestedFile}.backup`;
    renameSync(nestedFile, nestedBackup);
    try {
      expect(() => validatePriorArchive(root, candidate())).toThrow();
      writeFileSync(nestedFile, "altered nested archive");
      expect(() => validatePriorArchive(root, candidate())).toThrow();
    } finally { rmSync(nestedFile, { force: true }); renameSync(nestedBackup, nestedFile); }
    const badCommand = candidate(); badCommand.commands[0].sha256 = "0".repeat(64);
    expect(() => validatePriorArchive(root, badCommand)).toThrow("archive does not bind current command container");
    for (const missing of [persistent4Path, `${directory}/implementation/attempt13-collection.json`]) {
      const edited = JSON.parse(original.toString()); edited.entries = edited.entries.filter((e: any) => e.path !== missing);
      writeFileSync(resolve(root, path), JSON.stringify(edited)); git("add", path);
      expect(() => validatePriorArchive(root, candidate())).toThrow();
    }
    writeFileSync(resolve(root, path), original); git("add", path);
    for (const logical of [persistent4Path, `${directory}/implementation/attempt13-collection.json`]) {
      const entry = container.entries.find(e => e.path === logical)!;
      const file = resolve(root, entry.bytes), backup = `${file}.backup`;
      renameSync(file, backup);
      try {
        expect(() => validatePriorArchive(root, candidate())).toThrow();
        writeFileSync(file, "altered external archive");
        expect(() => validatePriorArchive(root, candidate())).toThrow();
      } finally { rmSync(file, { force: true }); renameSync(backup, file); }
    }
    expect(() => validatePriorArchive(root, candidate())).not.toThrow();
  });
}, 7200000);
