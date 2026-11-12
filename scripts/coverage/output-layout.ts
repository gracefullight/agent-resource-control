import { resolve } from "node:path";
import { EVIDENCE } from "./config";

// Only scratch output moves. Historical baseline identities and standalone
// destinations remain unchanged. Every collection still owns a unique run.
export function coverageLayout(root:string, id:string, scratchRoot?:string) {
  if(!/^[A-Za-z0-9_-]+$/.test(id)) throw Error("COVERAGE_OUTPUT: invalid run identity");
  if(scratchRoot!==undefined && (!scratchRoot.startsWith("target/") || scratchRoot.includes("\\") ||
    scratchRoot.split("/").some(part=>!part || part==="." || part===".."))) throw Error("COVERAGE_OUTPUT: scratch root must be canonical below target/");
  const base=scratchRoot===undefined?`${EVIDENCE}/baseline`:`${scratchRoot}/coverage`;
  return {
    directory:resolve(root,base,"runs",id),
    target:resolve(root,scratchRoot===undefined?"target/arc-coverage":`${base}/profiles`,id),
    latest:resolve(root,base,"latest.json"),
    nativeDirectory:scratchRoot===undefined?undefined:resolve(root,base,"runs",id,"native"),
  };
}
