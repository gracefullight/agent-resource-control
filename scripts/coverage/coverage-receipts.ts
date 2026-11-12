import { relative, resolve } from "node:path";
import { account } from "./accounting";
import { BASELINE, CRATES, METER_INPUTS, METER_SRS_SHA256, PINS } from "./config";
import { enforce, requireVersion, requireWorkspace } from "./gate";
import { readEvidence, referenceFile, type EvidenceRef } from "./evidence";
import { hash, sourceInventory, requireInputIdentity } from "./source";
import { parseCargoExecution } from "../archive/harness/managed-exec-test-output";
import type { Receipt } from "./commands";

export type ProfileProof = {
  target:string; merge:string[]; mergeList:EvidenceRef;
  profiles:{path:string;sha256:string;size:number}[];
  childProof:{binary:string;pid:number;profiles:string[];exit:number}[];
};
export function requireProfiles(root:string, proof:ProfileProof) {
  const paths=proof.profiles.map(profile=>profile.path);
  if(!paths.length || new Set(paths).size!==paths.length) throw Error("CHILD_PROFILE: empty/duplicate profiles");
  for(const profile of proof.profiles) {
    if(!/^arc-\d+-[A-Za-z0-9_-]+\.profraw$/.test(profile.path) || profile.size<=0) throw Error("CHILD_PROFILE: invalid profile");
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
export type CoverageEvidence = {receipt:EvidenceRef;summary:EvidenceRef;profiles:EvidenceRef;rawReports:{lcov:EvidenceRef;llvmJson:EvidenceRef}};
export function coverageEvidence(root:string, evidence:CoverageEvidence, sourceHash:string) {
  const json=(ref:EvidenceRef)=>JSON.parse(readEvidence(root,ref)!.toString());
  const receipt=json(evidence.receipt), summary=json(evidence.summary);
  if(receipt.pass!==true || receipt.exit!==0 || receipt.sourceHash!==sourceHash || summary.sourceHash!==sourceHash || receipt.runId!==summary.runId) throw Error("COVERAGE: failed/stale receipt");
  const current=requireInputIdentity(root,summary.inputs);
  // Receipt, summary and caller must describe the inputs that produced reports.
  if(sourceHash!==current.sha256) throw Error("COVERAGE: source hash/input digest mismatch");
  const lcov=readEvidence(root,evidence.rawReports.lcov)!.toString(), llvm=readEvidence(root,evidence.rawReports.llvmJson)!;
  if(hash(lcov)!==summary.rawSha256 || hash(llvm)!==summary.llvmSha256) throw Error("COVERAGE: raw report hash mismatch");
  const result=account(lcov,sourceInventory(root),root,undefined,JSON.parse(llvm.toString()));
  enforce(result);
  if(result.threshold!==80 || summary.schemaVersion!==1 || summary.threshold!==80 || summary.pass!==true || JSON.stringify(result.crates)!==JSON.stringify(summary.crates) || JSON.stringify(result.files)!==JSON.stringify(summary.files) ||
    JSON.stringify(result.crates.map(crate=>crate.crate))!==JSON.stringify(CRATES)) throw Error("COVERAGE: accounting differs from native summary");
  const native=receipt.receipts.filter((command:{argv:string[]})=>command.argv.includes("--no-report"));
  if(native.length!==1 || native[0].exit!==0 || !native[0].argv.includes("--workspace") || !native[0].argv.includes("--all-targets")) throw Error("COVERAGE: missing native workspace collection");
  const logs=native[0].output;
  const stdout=readEvidence(root,logs.stdout)!.toString();
  const execution=parseCargoExecution(stdout);
  readEvidence(root,logs.stderr);
  if(!execution.tests.length || execution.ignored || execution.tests.some(test=>test.status!=="ok") || /[1-9]\d* filtered out;/.test(stdout)) throw Error("COVERAGE: incomplete native tests");
  requireProfiles(root,json(evidence.profiles));
  return result;
}

export type CoverageReuse = {
  schemaVersion:1; runId:string; sourceHash:string; acceptedBaseline:string;
  evidence:CoverageEvidence; metadata:EvidenceRef; frozenInputs:Record<string,string>;
};

// A fresh receipt means the same complete inputs, not a timestamp or a previous
// passing assertion. This function performs no collection and has no fallback.
export function reuseCoverage(root:string, path:string) {
  requireVersion(Bun.version,PINS.bun,"Bun");
  const ref=referenceFile(root,path);
  const bundle=JSON.parse(readEvidence(root,ref)!.toString()) as CoverageReuse;
  if(bundle.schemaVersion!==1 || !bundle.runId || bundle.acceptedBaseline!==BASELINE || !/^[a-f0-9]{64}$/.test(bundle.sourceHash)) throw Error("COVERAGE_REUSE: invalid receipt identity");
  const json=(ref:EvidenceRef)=>JSON.parse(readEvidence(root,ref)!.toString());
  const receipt=json(bundle.evidence.receipt), summary=json(bundle.evidence.summary);
  if(receipt.schemaVersion!==1 || receipt.acceptedBaseline!==BASELINE || summary.acceptedBaseline!==BASELINE ||
    receipt.runId!==bundle.runId || summary.runId!==bundle.runId) throw Error("COVERAGE_REUSE: run/baseline mismatch");
  const current=requireInputIdentity(root,summary.inputs);
  for(const path of METER_INPUTS) {
    if(!current.files[path] || bundle.frozenInputs?.[path]!==current.files[path]) throw Error(`COVERAGE_REUSE: frozen input mismatch ${path}`);
  }
  if(current.files[METER_INPUTS[0]]!==METER_SRS_SHA256) throw Error("COVERAGE_REUSE: authoritative SRS mismatch");
  const versions=summary.versions;
  const llvm=versions?.rustc?.match(/LLVM version: (\S+)/)?.[1];
  if(versions?.bun!==PINS.bun || !versions?.rustc?.includes(`release: ${PINS.rust}\n`) ||
    versions?.cargoLlvmCov?.trim()!==`cargo-llvm-cov ${PINS.cargoLlvmCov}` ||
    !llvm || !versions?.["llvm-cov"]?.includes(llvm) || !versions?.["llvm-profdata"]?.includes(llvm) ||
    versions.host!==summary.host || !/(apple-darwin|unknown-linux)/.test(summary.host)) throw Error("COVERAGE_REUSE: missing/mismatched pinned collection tools");
  const metadata=json(bundle.metadata);
  requireWorkspace(metadata,root);
  const proof=json(bundle.evidence.profiles) as ProfileProof;
  // Recompute every crate's integer floor and validate the complete report,
  // native log, profile bytes and exact merge membership with shared helpers.
  const result=coverageEvidence(root,bundle.evidence,bundle.sourceHash);
  const commands=receipt.receipts as Receipt[];
  if(!Array.isArray(commands) || !commands.length) throw Error("COVERAGE_REUSE: missing collection commands");
  const rust=["rustup","run",PINS.rust,"cargo"];
  const exact=(argv:string[])=>{
    const matches=commands.filter(command=>JSON.stringify(command.argv)===JSON.stringify(argv));
    if(matches.length!==1 || matches[0].exit!==0) throw Error(`COVERAGE_REUSE: missing/failed command ${argv.join(" ")}`);
    return matches[0];
  };
  const native=exact([...rust,"llvm-cov","--workspace","--all-targets","--locked","--offline","--no-report","--no-cfg-coverage","--no-cfg-coverage-nightly"]);
  if(!Number.isFinite(Date.parse(native.started)) || !Number.isFinite(Date.parse(native.ended)) || Date.parse(native.ended)<Date.parse(native.started)) throw Error("COVERAGE_REUSE: invalid collection interval");
  if(JSON.stringify(JSON.parse(exact([...rust,"metadata","--format-version","1","--locked","--offline"]).stdout))!==JSON.stringify(metadata)) throw Error("COVERAGE_REUSE: metadata differs from collection");
  for(const [format,ref] of [["--lcov",bundle.evidence.rawReports.lcov],["--json",bundle.evidence.rawReports.llvmJson]] as const) {
    const reports=commands.filter(command=>command.argv.slice(0,6).join("\0")===[...rust,"llvm-cov","report"].join("\0") && command.argv.includes(format));
    const report=reports[0], argv=report?.argv ?? [];
    if(reports.length!==1 || report.exit!==0 || argv[argv.indexOf("--output-path")+1]!==resolve(root,ref.path) ||
      !argv.includes("--disable-default-ignore-filename-regex") || argv[argv.indexOf("--failure-mode")+1]!=="any" ||
      !Number.isFinite(Date.parse(report.started)) || Date.parse(report.started)<Date.parse(native.ended)) throw Error("COVERAGE_REUSE: missing/failed/stale native report command");
  }
  for(const child of proof.childProof) {
    const argv=[resolve(proof.target,"debug",child.binary),...(child.binary==="arcctl"?["--help"]:[])];
    const matches=commands.filter(command=>JSON.stringify(command.argv)===JSON.stringify(argv));
    if(matches.length!==1 || matches[0].pid!==child.pid || matches[0].exit!==child.exit) throw Error("COVERAGE_REUSE: child probe/receipt mismatch");
  }
  for(const command of commands) {
    const expected=proof.childProof.find(child=>child.binary==="arc-daemon" && command.pid===child.pid &&
      command.argv[0]===resolve(proof.target,"debug","arc-daemon"))?1:0;
    if(command.exit!==expected || resolve(command.cwd)!==resolve(root)) throw Error("COVERAGE_REUSE: failed collection command or wrong workspace");
  }
  return {receipt:ref,runId:bundle.runId,sourceHash:current.sha256,result,recollected:false as const};
}
