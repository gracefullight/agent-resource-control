import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { readEvidence, type EvidenceRef } from "./evidence";

const GATES=["format","clippy","workspaceTest","coverage","architecture","activeBun"] as const;

// Inspect references written in gates.json only. Receipts and command registries
// remain opaque; no external JSON traversal or running-operation exception.
function rawLogReferences(value:unknown, rawLog=false):EvidenceRef[] {
  if(!value || typeof value!=="object") return [];
  if(Array.isArray(value)) return value.flatMap(child=>rawLogReferences(child,rawLog));
  const record=value as Record<string,unknown>;
  if(typeof record.path==="string" && (rawLog || record.path.endsWith(".log"))) return [record as EvidenceRef];
  return Object.entries(record).flatMap(([key,child])=>
    rawLogReferences(child,["rawLogs","logs","stdout","stderr"].includes(key)));
}

export function desktopEvidence(root:string, options:{historicalAvailability?:boolean} = {}) {
  const report=JSON.parse(readFileSync(resolve(root,".arc-verify/desktop-foundation/gates.json"),"utf8"));
  const gateOutcomes=Object.fromEntries(GATES.map(name=>{
    const status:unknown=report?.gates?.[name]?.status;
    if(typeof status!=="string" || !status.trim()) throw Error(`EVIDENCE: missing gate outcome ${name}`);
    return [name,status];
  }));
  const verifiedLogs:string[]=[], unavailableLogs:string[]=[];
  for(const ref of rawLogReferences(report)) {
    const bytes=readEvidence(root,ref,{prefixes:["target/"],sizeRequired:false,optional:options.historicalAvailability===true});
    (bytes===undefined?unavailableLogs:verifiedLogs).push(ref.path);
  }
  return {gateOutcomes,verifiedLogs,unavailableLogs,currentAcceptance:false as const};
}
