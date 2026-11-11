import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { gitBytes } from "./hygiene";
import { evidenceRefs, readEvidence, OUTPUT_ROOTS, type EvidenceRef } from "./evidence";
import { hash } from "./source";

export const TERMINAL_BASE = "6b5218cf63b174b7976e52d81d56c7e3bf81112d";
// This is the commit that actually contains the terminal-outcome delivery and
// its repair overlay. It is not the source base named inside those receipts.
export const TERMINAL_DELIVERY = "a9e7a31566e833dceeb82d6d75cd7c6a399583ab";
export const TERMINAL_DIRECTORY = ".arc-verify/terminal-outcomes";

export function gitEvidence(root: string, revision: string, ref: EvidenceRef) {
  const bytes=gitBytes(root,["show",`${revision}:${ref.path}`]);
  if(hash(bytes)!==ref.sha256 || (ref.bytes!==undefined && bytes.length!==ref.bytes)) throw Error(`HISTORICAL: Git object mismatch ${revision}:${ref.path}`);
  return bytes;
}

export function terminalHistory(root: string) {
  const paths=gitBytes(root,["ls-tree","-r","--name-only",TERMINAL_DELIVERY,"--",TERMINAL_DIRECTORY]).toString().trim().split("\n");
  if(!paths.length || !paths.includes(`${TERMINAL_DIRECTORY}/gates.json`)) throw Error("HISTORICAL: missing committed delivery");
  for(const path of paths) if(!readFileSync(resolve(root,path)).equals(gitBytes(root,["show",`${TERMINAL_DELIVERY}:${path}`]))) throw Error(`HISTORICAL: receipt changed ${path}`);
  const report=JSON.parse(readFileSync(resolve(root,TERMINAL_DIRECTORY,"gates.json"),"utf8"));
  if(report.acceptedHead!==TERMINAL_BASE || report.sourceIdentity.acceptedHead!==TERMINAL_BASE) throw Error("HISTORICAL: source base differs");
  const installed=report.installation;
  for(const ref of [...installed.acceptedSourceHashes,...installed.repairOverlay,...installed.acceptedReports,installed.documentation]) gitEvidence(root,TERMINAL_DELIVERY,ref);
  for(const ref of installed.repairOverlay) {
    if(ref.baselineSha256!==null) gitEvidence(root,TERMINAL_BASE,{path:ref.path,sha256:ref.baselineSha256});
    else {
      const absent=Bun.spawnSync(["git","cat-file","-e",`${TERMINAL_BASE}:${ref.path}`],{cwd:root,stdout:"pipe",stderr:"pipe"});
      if(absent.exitCode!==128) throw Error(`HISTORICAL: expected absent baseline input ${ref.path}`);
    }
  }
  // Dependency additions today do not rewrite the original Cargo inputs.
  for(const path of ["Cargo.toml","Cargo.lock"]) {
    if(!gitBytes(root,["show",`${TERMINAL_BASE}:${path}`]).equals(gitBytes(root,["show",`${TERMINAL_DELIVERY}:${path}`]))) throw Error(`HISTORICAL: unexpected original manifest overlay ${path}`);
  }
  const refs=new Map<string,EvidenceRef>();
  for(const ref of evidenceRefs(report).filter(r=>OUTPUT_ROOTS.some(prefix=>r.path.startsWith(prefix)))) {
    if(refs.has(ref.path) && refs.get(ref.path)!.sha256!==ref.sha256) throw Error(`HISTORICAL: conflicting hash ${ref.path}`);
    refs.set(ref.path,ref);
  }
  const available:string[]=[], unavailable:string[]=[];
  for(const ref of refs.values()) {
    const bytes=readEvidence(root,ref,{optional:true,sizeRequired:false});
    (bytes===undefined?unavailable:available).push(ref.path);
  }
  const gaps=[];
  if(unavailable.includes(report.sourceIdentity.manifest.path)) gaps.push("Original worktree aggregate cannot be authenticated without its input manifest; the committed delivered source/repair hashes are verified separately.");
  if(unavailable.length) gaps.push("Missing historical raw logs/reports cannot independently establish their recorded runtime passes or coverage here.");
  return {report,sourceBase:TERMINAL_BASE,deliveryCommit:TERMINAL_DELIVERY,sourceOverlayVerified:installed.acceptedSourceHashes.length+installed.repairOverlay.length,
    available,unavailable,gaps,currentAcceptance:false as const};
}
