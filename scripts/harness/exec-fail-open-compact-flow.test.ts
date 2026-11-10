import { test, expect } from "bun:test";
import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { compactFirstRun } from "./exec-fail-open-compact-fixture";
import { directory, boundJSON, context, root, reference, validateCommon } from "./exec-fail-open-inputs";
import { validateGates, validateGateData } from "./exec-fail-open-gates";
import { validateEvidence } from "./exec-fail-open-evidence";
import { validateCommand } from "./exec-fail-open-receipts";
import { assemble } from "./exec-fail-open-assemble";
test("compact complete flow retains 140 originals, eleven commands, thirteen outages, archive, QA, final and both terminals at 148 files",async()=>{
  await compactFirstRun(async(dir,s)=>{
    console.log(JSON.stringify({fixtureOnly:true,retained:140,commands:s.ready.commands.length,outageCases:boundJSON(dir,s.ready.gates.timings).cases.length,containerEntries:Object.keys(s.collection.entries).length,trackedFiles:s.count(),terminalExits:[s.terminalGates.receipt.exit,s.terminalEvidence.receipt.exit]}));
    expect(s.count()).toBe(148);expect(s.terminalGates.receipt.exit).toBe(0);expect(s.terminalEvidence.receipt.exit).toBe(0);
    expect(s.ready.commands.length).toBe(11);expect(boundJSON(dir,s.ready.gates.timings).cases.length).toBe(13);
    expect(boundJSON(dir,s.ready.gates.historical).negativeControl.selector.path).toContain("negative-control.json");
    expect(s.ready.gates.attempts.length).toBe(1);
    expect(()=>validateGates(dir)).toThrow();expect(()=>validateCommand(s.reproduction.receipt,context(root),dir,"reviewer")).toThrow("fixture evidence is inadmissible");
    expect(()=>assemble(s.names,s.recipe,{collection:s.collection.ref,archive:s.archive.ref})).toThrow("archive does not preserve");
    const candidate=s.ready.gates;
    expect(()=>validateGateData(dir,{...candidate,commands:candidate.commands.slice(1)},undefined,s.recipe)).toThrow("partition differs");
    const failed=s.collection.entries[`${directory}/implementation/commands/failed.json`];
    expect(()=>validateGateData(dir,{...candidate,commands:[failed,...candidate.commands.slice(1)]},undefined,s.recipe)).toThrow("failed fixture subprocess");
    const current=candidate.commands[0], original=readFileSync(resolve(dir,current.path));
    try{
      const container=JSON.parse(original.toString()), entry=container.entries.find((e:any)=>e.path===current.selector.path);
      const value=JSON.parse(Buffer.from(entry.bytes,"base64").toString());value.sourceHash="stale";
      const {retainedEntry}=await import("./exec-fail-open-container");Object.assign(entry,retainedEntry(entry.path,Buffer.from(JSON.stringify(value))));
      writeFileSync(resolve(dir,current.path),JSON.stringify(container));s.git("add",directory);
      const substituted={...reference(dir,current.path),selector:{path:entry.path,sha256:entry.sha256}};
      expect(()=>validateGateData(dir,{...candidate,commands:[substituted,...candidate.commands.slice(1)]},undefined,s.recipe)).toThrow("stale");
    }finally{writeFileSync(resolve(dir,current.path),original);s.git("add",directory);}
    expect(validateEvidence(dir,s.recipe).passed).toBe(true);
    const qaPath=resolve(dir,directory,"qa/review.json"),qaBytes=readFileSync(qaPath);
    try{const qa=JSON.parse(qaBytes.toString());qa.actorId="backend";writeFileSync(qaPath,JSON.stringify(qa));expect(()=>validateEvidence(dir,s.recipe)).toThrow();}
    finally{writeFileSync(qaPath,qaBytes);}
  });
},3600000);
test("combined complete flow preserves 143 originals and seals all downstream artifacts at 149", async () => {
  await compactFirstRun(async (dir, s) => {
    expect(s.prior).toEqual({ indexedBefore: 142, filesystemBefore: 143, unionBefore: 143, rejectedUnindexed: true, indexedAfter: 143, unchanged: true, selected: 44 });
    expect(s.previousRefs).toHaveLength(44);
    expect(s.count()).toBe(149);
    expect(s.terminalGates.receipt.exit).toBe(0);
    expect(s.terminalEvidence.receipt.exit).toBe(0);
    expect(boundJSON(dir, s.previousEntry)).toEqual(s.previousValue);
    expect(reference(dir, s.previousEntry.path).sha256).toBe(s.previousEntry.sha256);
    const { archiveBytes, compactReferences, legacyArchiveBytes, rejectedIntermediatePath, rejectedIntermediateSha256, rejectedAssemblyPath, rejectedLatestPath, rejectedLatestSha256 } = await import("./exec-fail-open-compact");
    const { decodeContainer } = await import("./exec-fail-open-container");
    const { artifact } = await import("./exec-fail-open-inputs");
    const previousBytes = readFileSync(resolve(root, `${directory}/implementation/collection.json`));
    expect(readFileSync(resolve(dir, s.previousRefs[0].path))).toEqual(previousBytes);
    const originalEntries = decodeContainer(previousBytes).entries;
    for (const ref of s.previousRefs) {
      const entry = originalEntries.find(e => e.path === ref.selector.path)!;
      expect(artifact(dir, ref)).toEqual(Buffer.from(entry.bytes, "base64"));
    }
    const rejectedPath = `${directory}/implementation/collection-next.json`, rejectedRef = s.collection.entries[rejectedPath];
    const latestRef = s.collection.entries[rejectedLatestPath], latestBytes = archiveBytes(dir, latestRef);
    expect(latestBytes).toEqual(s.latestPrevious);
    const latestEntries = decodeContainer(latestBytes).entries;
    expect(latestEntries).toHaveLength(62);
    expect(() => artifact(dir, latestRef)).toThrow("archived bytes");
    for (const entry of latestEntries) {
      const legacy = { path: rejectedPath, sha256: rejectedLatestSha256, selector: { path: entry.path, sha256: entry.sha256 } };
      expect(legacyArchiveBytes(dir, latestRef, legacy, entry.scope === "archive")).toEqual(Buffer.from(entry.bytes, "base64"));
    }
    expect(archiveBytes(dir, rejectedRef)).toEqual(s.originalRejected);
    expect(decodeContainer(archiveBytes(dir, rejectedRef)).entries).toHaveLength(53);
    expect(() => artifact(dir, rejectedRef)).toThrow("archived bytes");
    for (const [path, bytes] of [[rejectedIntermediatePath, s.rejectedPrevious], [rejectedAssemblyPath, s.failedAssembly]]) {
      expect(archiveBytes(dir, s.collection.entries[path])).toEqual(bytes);
      expect(() => artifact(dir, s.collection.entries[path])).toThrow("archived bytes");
    }
    const oldTerminal = decodeContainer(s.rejectedPrevious).entries.find((e: any) => e.path.endsWith("/native/terminal-2.json"));
    const oldRef = { path: rejectedPath, sha256: rejectedIntermediateSha256, selector: { path: oldTerminal.path, sha256: oldTerminal.sha256 } };
    expect(legacyArchiveBytes(dir, s.collection.entries[rejectedIntermediatePath], oldRef)).toEqual(Buffer.from(oldTerminal.bytes, "base64"));
    expect(() => artifact(dir, oldRef)).toThrow("artifact hash differs");
    for (const [path, bytes] of Object.entries(s.originals)) {
      expect(archiveBytes(dir, s.collection.entries[path])).toEqual(bytes);
      expect(() => artifact(dir, s.collection.entries[path])).toThrow("archived bytes");
      expect(compactReferences(dir, s.collection.ref).some(r => r.selector.path === path)).toBe(false);
    }
    const candidate = s.ready.gates, archived = s.collection.entries[`${directory}/implementation/gates.json`];
    const failedPath = `${directory}/implementation/commands/failed.json`, unrelatedPath = `${directory}/implementation/commands/unrelated.json`;
    expect(candidate.attempts.map((a: any) => a.receipt.selector.path)).toEqual([failedPath]);
    const attempt = candidate.attempts[0], failedReceipt = boundJSON(dir, s.collection.entries[failedPath]);
    expect(failedReceipt.exit).not.toBe(0);
    expect(attempt.resolvedBy).toEqual(candidate.commands[0]);
    const retry = boundJSON(dir, attempt.resolvedBy);
    expect(retry.argv).toEqual(failedReceipt.argv);
    expect(Date.parse(retry.started)).toBeGreaterThanOrEqual(Date.parse(failedReceipt.ended));
    expect(boundJSON(dir, s.collection.entries[unrelatedPath]).successor.path).toBe(`${directory}/final.json`);
    expect(() => validateGateData(dir, { ...candidate, attempts: [candidate.attempts[0], candidate.attempts[0]] }, undefined, s.recipe)).toThrow("duplicate or invalid attempt");
    expect(() => validateGateData(dir, { ...candidate, attempts: [{ ...candidate.attempts[0], receipt: candidate.commands[0] }] }, undefined, s.recipe)).toThrow("attempt is not a failed command");
    expect(() => validateGateData(dir, { ...candidate, attempts: [{ ...candidate.attempts[0], receipt: s.collection.entries[`${directory}/implementation/commands/unrelated-plain.json`] }] }, undefined, s.recipe)).toThrow("matching successful command");
    expect(() => validateGateData(dir, { ...candidate, attempts: [{ ...attempt, resolvedBy: candidate.commands[1] }] }, undefined, s.recipe)).toThrow("matching successful command");
    expect(() => assemble(s.names, s.recipe, { collection: s.collection.ref, archive: s.collection.ref }, ["unrelated-plain"])).toThrow("matching successful command");
    const alteredPath = `${directory}/implementation/commands/altered-attempt.json`;
    mkdirSync(resolve(dir, alteredPath, ".."), { recursive: true });
    try {
      for (const [patch, error] of [
        [{ sourceHash: "stale" }, "stale"],
        [{ sourceHashAfter: "stale" }, "attempt provenance"],
        [{ actorId: "reviewer", skill: "oma-qa" }, "actor mismatch"],
        [{ skill: "oma-qa" }, "attempt provenance"],
        [{ host: { fixtureOnly: false } }, "attempt provenance"],
        [{ cwd: "elsewhere" }, "attempt invocation"],
        [{ pid: 0 }, "attempt process/timing"],
        [{ executable: { ...failedReceipt.executable, sha256: "0".repeat(64) } }, "attempt executable"],
        [{ stderrSha256: "0".repeat(64) }, "command output digest"],
        [{ argv: [...failedReceipt.argv, "other"] }, "matching successful command"],
        [{ ended: "9999-01-01T00:00:00.000Z" }, "matching successful command"],
      ] as const) {
        writeFileSync(resolve(dir, alteredPath), JSON.stringify({ ...failedReceipt, ...patch }));
        expect(() => validateGateData(dir, { ...candidate, attempts: [{ ...attempt, receipt: reference(dir, alteredPath) }] }, undefined, s.recipe)).toThrow(error);
      }
    } finally { unlinkSync(resolve(dir, alteredPath)); }
    expect(() => validateGateData(dir, { ...candidate, attempts: [{ ...candidate.attempts[0], receipt: s.collection.entries[unrelatedPath] }] }, undefined, s.recipe)).toThrow("attempted successor read");
    expect(() => validateGateData(dir, { ...candidate, commands: [archived, ...candidate.commands.slice(1)] }, undefined, s.recipe)).toThrow("archived bytes");
    for (const name of ["connect_access_denied_before_register_runs_once", "connect_access_denied_after_register_ack_loss_stays_uncertain", "connect_access_denied_before_claim_stays_uncertain", "connect_access_denied_after_claim_ack_loss_stays_uncertain", "invalid_socket_parent_not_directory_does_not_launch", "invalid_socket_endpoint_not_socket_does_not_launch", "broken_stderr_unavailable_preserves_child_exit", "broken_stderr_uncertain_preserves_child_exit"])
      expect(() => validateGateData(dir, candidate, undefined, { ...s.recipe, requiredTests: [name] })).toThrow(`missing live execution: ${name}`);
    const containerPath = resolve(dir, s.collection.ref.path), original = readFileSync(containerPath);
    try {
      const edited = JSON.parse(original.toString());
      edited.entries.find((e: any) => e.path === archived.selector.path).scope = "current";
      writeFileSync(containerPath, JSON.stringify(edited)); s.git("add", directory);
      expect(() => validateGateData(dir, { ...candidate, priorObservations: reference(dir, s.collection.ref.path) }, undefined, s.recipe)).toThrow("canonical archive entries differ");
    } finally { writeFileSync(containerPath, original); s.git("add", directory); }
    try {
      const { retainedEntry } = await import("./exec-fail-open-container");
      for (const [patch, error] of [[{ sourceHash: "stale" }, "stale"], [{ actorId: "reviewer", skill: "oma-qa" }, "actor mismatch"]] as const) {
        const edited = JSON.parse(original.toString()), selected = candidate.commands[0];
        const entry = edited.entries.find((e: any) => e.path === selected.selector.path);
        const value = { ...JSON.parse(Buffer.from(entry.bytes, "base64").toString()), ...patch };
        Object.assign(entry, retainedEntry(entry.path, Buffer.from(JSON.stringify(value))));
        writeFileSync(containerPath, JSON.stringify(edited)); s.git("add", directory);
        const changed = reference(dir, s.collection.ref.path);
        const first = { ...changed, selector: { path: entry.path, sha256: entry.sha256 } };
        expect(() => validateGateData(dir, { ...candidate, priorObservations: changed, commands: [first, ...candidate.commands.slice(1)] }, undefined, s.recipe)).toThrow(error);
      }
    } finally { writeFileSync(containerPath, original); s.git("add", directory); }
    expect(validateGates(dir, s.recipe).passed).toBe(true);
    expect(validateEvidence(dir, s.recipe).passed).toBe(true);
  }, { combined: true, unrelatedSuccessor: true });
}, 3600000);
