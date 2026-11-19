import { resolve } from "node:path";
import { chmodSync, lstatSync, readdirSync, rmSync } from "node:fs";
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

// Profiles and the profraw merge list live at the target root. Only the
// instrumented Cargo build subtree is disposable after reports are validated.
export function removeInstrumentedBuild(target:string) {
  rmSync(resolve(target,"debug"),{recursive:true,force:true});
}

// Test children can inherit a restrictive umask. After they exit, restore
// owner-read access only to profiles in this run's private target directory.
export function prepareProfiles(target:string) {
  const owner=process.getuid?.(), directory=lstatSync(target);
  if(owner===undefined || !directory.isDirectory() || directory.uid!==owner) throw Error("PROFILE: target is not an owned directory");
  const profiles=readdirSync(target).filter(p=>p.endsWith(".profraw")).sort();
  for(const profile of profiles) {
    const path=resolve(target,profile), file=lstatSync(path);
    if(!file.isFile() || file.uid!==owner || file.nlink!==1) throw Error(`PROFILE: not an owned regular file ${profile}`);
    if((file.mode&0o400)===0) chmodSync(path,(file.mode&0o7777)|0o400);
  }
  return profiles;
}
