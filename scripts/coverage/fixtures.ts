// Controlled LCOV/process fixtures. These are never production measurements.
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { CRATES } from "./config";
import type { SourceFile } from "./source";
export function fixture() {
  const root=mkdtempSync(join(tmpdir(),"arc-coverage-fixture-"));
  const inventory:SourceFile[]=CRATES.map(crate=>{
    const path=`crates/${crate}/src/lib.rs`;
    mkdirSync(join(root,`crates/${crate}/src`),{recursive:true});
    writeFileSync(join(root,path),"fixture\n".repeat(5));
    return {path,crate,sha256:"fixture",lines:6,omitted:[],testOnly:false,spans:[]};
  });
  const record=(path:string,hits=[1,1,1,1,0])=>`SF:${join(root,path)}\n${hits.map((n,i)=>`DA:${i+1},${n}`).join("\n")}\nLF:${hits.length}\nLH:${hits.filter(n=>n>0).length}\nend_of_record\n`;
  const lcov=inventory.map(f=>record(f.path)).join("");
  const path=join(root,"report.lcov");writeFileSync(path,lcov);
  return {root,inventory,record,lcov,path};
}
