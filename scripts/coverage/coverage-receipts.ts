import { relative, resolve } from "node:path";
import { account } from "./accounting";
import { CRATES } from "./config";
import { enforce } from "./gate";
import { readEvidence, type EvidenceRef } from "./evidence";
import { hash, sourceInventory, requireInputIdentity } from "./source";
import { parseCargoExecution } from "../archive/harness/managed-exec-test-output";

type ProfileProof = {
  target:string; merge:string[]; mergeList:EvidenceRef;
  profiles:{path:string;sha256:string;size:number}[];
  childProof:{binary:string;pid:number;profiles:string[];exit:number}[];
};
export function requireProfiles(root:string, proof:ProfileProof) {
  const paths=proof.profiles.map(profile=>profile.path);
  if(!paths.length || new Set(paths).size!==paths.length) throw Error("CHILD_PROFILE: empty/duplicate profiles");
  for(const profile of proof.profiles) {
    if(!/^arc-\d+-.+\.profraw$/.test(profile.path) || profile.size<=0) throw Error("CHILD_PROFILE: invalid profile");
    readEvidence(root,{path:relative(root,resolve(proof.target,profile.path)),sha256:profile.sha256,bytes:profile.size});
  }
  const expected=paths.map(path=>resolve(proof.target,path)).sort();
  const merged=readEvidence(root,proof.mergeList)!.toString().trim().split("\n").sort();
  if(JSON.stringify(expected)!==JSON.stringify([...proof.merge].sort()) || JSON.stringify(expected)!==JSON.stringify(merged)) throw Error("CHILD_PROFILE: merge input mismatch");
  if(proof.childProof.length!==2) throw Error("CHILD_PROFILE: missing owned binary probes");
  for(const [binary,exit] of [["arcctl",0],["arc-daemon",1]] as const) {
    const child=proof.childProof.find(child=>child.binary===binary);
    if(!child || !Number.isSafeInteger(child.pid) || child.pid<=0 || child.exit!==exit || !child.profiles.length ||
      child.profiles.some(path=>!path.startsWith(`arc-${child.pid}-`) || !paths.includes(path))) throw Error(`CHILD_PROFILE: missing ${binary} PID profiles`);
  }
}

// Read and recompute a completed existing coverage run. This never collects
// coverage or falls back to a historical passing report.
export function coverageEvidence(root:string, evidence:{receipt:EvidenceRef;summary:EvidenceRef;profiles:EvidenceRef;rawReports:{lcov:EvidenceRef;llvmJson:EvidenceRef}}, sourceHash:string) {
  const json=(ref:EvidenceRef)=>JSON.parse(readEvidence(root,ref)!.toString());
  const receipt=json(evidence.receipt), summary=json(evidence.summary);
  if(receipt.pass!==true || receipt.exit!==0 || receipt.sourceHash!==sourceHash || summary.sourceHash!==sourceHash || receipt.runId!==summary.runId) throw Error("COVERAGE: failed/stale receipt");
  requireInputIdentity(root,summary.inputs);
  const lcov=readEvidence(root,evidence.rawReports.lcov)!.toString(), llvm=readEvidence(root,evidence.rawReports.llvmJson)!;
  if(hash(lcov)!==summary.rawSha256 || hash(llvm)!==summary.llvmSha256) throw Error("COVERAGE: raw report hash mismatch");
  const result=account(lcov,sourceInventory(root),root,undefined,JSON.parse(llvm.toString()));
  enforce(result);
  if(result.threshold!==80 || JSON.stringify(result.crates)!==JSON.stringify(summary.crates) || JSON.stringify(result.files)!==JSON.stringify(summary.files) ||
    JSON.stringify(result.crates.map(crate=>crate.crate))!==JSON.stringify(CRATES)) throw Error("COVERAGE: accounting differs from native summary");
  const native=receipt.receipts.filter((command:{argv:string[]})=>command.argv.includes("--no-report"));
  if(native.length!==1 || native[0].exit!==0 || !native[0].argv.includes("--workspace") || !native[0].argv.includes("--all-targets")) throw Error("COVERAGE: missing native workspace collection");
  const logs=native[0].output;
  const execution=parseCargoExecution(readEvidence(root,logs.stdout)!.toString());
  readEvidence(root,logs.stderr);
  if(!execution.tests.length || execution.ignored || execution.tests.some(test=>test.status!=="ok")) throw Error("COVERAGE: incomplete native tests");
  requireProfiles(root,json(evidence.profiles));
  return result;
}
