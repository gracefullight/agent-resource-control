import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { command } from "./commands";
import { inputIdentity } from "./source";
import { EVIDENCE } from "./config";
export const root=resolve(import.meta.dir,"../..");
export async function realAcceptance(name:string,argv:string[]) {
  const dir=resolve(root,EVIDENCE,"tooling/acceptance/runs",`${name}-${crypto.randomUUID()}`);
  mkdirSync(dir,{recursive:true});
  const before=inputIdentity(root);
  const env={...process.env};delete env.SKIP_COVERAGE;
  const result=await command(argv,dir,root,env,3600000);
  const after=inputIdentity(root);
  writeFileSync(resolve(dir,"binding.json"),JSON.stringify({name,sourceHash:before.sha256,afterHash:after.sha256,unchanged:before.sha256===after.sha256,exit:result.exit,argv},null,2)+"\n");
  if(before.sha256!==after.sha256) throw Error(`${name}: source changed during acceptance`);
  if(result.exit!==0) throw Error(`${name}: command exited ${result.exit}\n${result.stdout}\n${result.stderr}\nReceipt: ${dir}`);
  return result;
}
