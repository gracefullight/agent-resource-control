import { test, expect } from "bun:test";
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { root } from "./acceptance";
import { inputIdentity, hash, sourceInventory } from "./source";
import { EVIDENCE, CRATES, BASELINE } from "./config";
import { account } from "./accounting";

test("current successful source-bound coverage contains all crates, files and child profiles",()=>{
  const load=(path:string)=>JSON.parse(readFileSync(resolve(root,path),"utf8"));
  const latest=load(`${EVIDENCE}/baseline/latest.json`);
  expect(latest.pass).toBe(true);expect(latest.sourceHash).toBe(inputIdentity(root).sha256);
  const report=load(`${latest.directory}/summary.json`), receipt=load(`${latest.directory}/receipt.json`);
  expect(report.sourceHash).toBe(latest.sourceHash);expect(receipt.sourceHash).toBe(latest.sourceHash);
  expect(report.runId).toBe(latest.runId);expect(receipt.runId).toBe(latest.runId);
  expect(report.acceptedBaseline).toBe(BASELINE);expect(receipt.pass).toBe(true);expect(receipt.exit).toBe(0);
  const raw=readFileSync(resolve(root,latest.directory,"raw.lcov"),"utf8");
  expect(hash(raw)).toBe(report.rawSha256);
  const llvm=readFileSync(resolve(root,latest.directory,"raw.json"),"utf8");
  expect(hash(llvm)).toBe(report.llvmSha256);
  const recomputed=account(raw,sourceInventory(root),root,undefined,JSON.parse(llvm));
  expect(recomputed.crates).toEqual(report.crates);expect(recomputed.files).toEqual(report.files);
  expect(report.crates.map((x:any)=>x.crate)).toEqual([...CRATES]);expect(recomputed.pass).toBe(true);
  const profiles=load(`${latest.directory}/profiles.json`);
  expect(profiles.profiles.length).toBeGreaterThan(0);
  expect([...profiles.merge].sort()).toEqual(profiles.profiles.map((p:any)=>resolve(profiles.target,p.path)).sort());
  for(const profile of profiles.profiles) {
    const bytes=readFileSync(resolve(profiles.target,profile.path));
    expect(bytes.length).toBe(profile.size);expect(bytes.length).toBeGreaterThan(0);
    expect(hash(bytes)).toBe(profile.sha256);
  }
  for(const child of profiles.childProof) {
    expect(child.profiles.length).toBeGreaterThan(0);
    expect(receipt.receipts.some((r:any)=>r.pid===child.pid && r.exit===child.exit && r.argv[0]===resolve(profiles.target,"debug",child.binary))).toBe(true);
    for(const profile of child.profiles) {
      expect(profile.startsWith(`arc-${child.pid}-`)).toBe(true);
      expect(profiles.merge).toContain(resolve(profiles.target,profile));
    }
  }
  expect(profiles.childProof.map((c:any)=>c.binary)).toEqual(["arcctl","arc-daemon"]);
  expect(existsSync(resolve(root,latest.directory,"uncovered.md"))).toBe(true);
});
