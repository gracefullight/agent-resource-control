import { existsSync, readFileSync, statSync } from "node:fs";
import { account } from "./accounting";
import { hash, type SourceFile } from "./source";

export function requireExit(code:number,label:string) {
  if(code!==0) throw Error(`${label}: exited ${code}`);
}
export function requireVersion(actual:string,wanted:string,label:string) {
  if(actual.trim()!==wanted) throw Error(`TOOL_VERSION: ${label}: expected ${wanted}, got ${actual.trim()}`);
}
export function freshReport(path:string,started:number) {
  if(!existsSync(path)) throw Error(`MISSING_REPORT: ${path}`);
  if(statSync(path).mtimeMs<started) throw Error(`STALE_REPORT: ${path}`);
  return readFileSync(path,"utf8");
}
export function validateReport(args:{path:string;llvmPath?:string;started:number;before:string;after:string;inventory:SourceFile[];root:string;testExit:number}) {
  requireExit(args.testExit,"TEST_FAILURE");
  if(args.before!==args.after) throw Error("STALE_REPORT: source/configuration changed during measurement");
  const raw=freshReport(args.path,args.started);
  const llvm=args.llvmPath?freshReport(args.llvmPath,args.started):undefined;
  const result=account(raw,args.inventory,args.root,undefined,llvm===undefined?undefined:JSON.parse(llvm));
  return {...result,sourceHash:args.before,rawSha256:hash(raw),...(llvm===undefined?{}:{llvmSha256:hash(llvm)})};
}
export function enforce(result:{crates:{crate:string;covered:number;total:number;pass:boolean}[]}) {
  const failed=result.crates.filter(c=>!c.pass);
  if(failed.length) throw Error(`BELOW_THRESHOLD: ${failed.map(c=>`${c.crate} ${c.covered}/${c.total}`).join(", ")} (requires 80% per crate)`);
}
