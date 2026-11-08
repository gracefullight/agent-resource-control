import { test, expect } from "bun:test";
import { writeFileSync, utimesSync, unlinkSync } from "node:fs";
import { join } from "node:path";
import { fixture } from "./fixtures";

async function check(change:(args:any,f:ReturnType<typeof fixture>)=>void,diagnostic?:string) {
  const f=fixture();
  const args:any={...f,started:Date.now()-1000,before:"current-source",after:"current-source",testExit:0};
  // A separate old passing report must never be consulted as fallback.
  writeFileSync(join(f.root,"old-passing-report.lcov"),f.lcov);
  change(args,f);
  const input=join(f.root,"input.json");writeFileSync(input,JSON.stringify(args));
  const child=Bun.spawn([process.execPath,join(import.meta.dir,"fixture-runner.ts"),input],{stdout:"pipe",stderr:"pipe"});
  const [stdout,stderr,exit]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
  if(diagnostic) {expect(exit).not.toBe(0);expect(stderr).toContain(diagnostic);expect(stdout).not.toContain("PASS");}
  else {expect(exit).toBe(0);expect(stdout).toContain("PASS");}
}
test("exact 80% succeeds",()=>check(()=>{}));
test("failed subprocess tests cannot reuse old passing artifacts",()=>check(a=>{a.mode="test"},"TEST_FAILURE: exited 23"));
test("test exit is checked before report validity",()=>check(a=>{a.testExit=7;a.path="missing"},"TEST_FAILURE: exited 7"));
test("missing tool returns nonzero",()=>check(a=>{a.mode="tool"},"MISSING_TOOL: exited 127"));
test("mismatched tool returns nonzero",()=>check(a=>{a.mode="version"},"TOOL_VERSION"));
test("missing report returns nonzero",()=>check((_,f)=>unlinkSync(f.path),"MISSING_REPORT"));
test("stale report returns nonzero",()=>check((a,f)=>{utimesSync(f.path,1,1);a.started=Date.now()},"STALE_REPORT"));
test("source changed during measurement returns nonzero",()=>check(a=>{a.after="changed"},"STALE_REPORT"));
test("malformed report returns nonzero",()=>check((_,f)=>writeFileSync(f.path,"not lcov"),"MALFORMED_REPORT"));
test("missing crate returns nonzero",()=>check((_,f)=>writeFileSync(f.path,f.inventory.slice(1).map(x=>f.record(x.path)).join("")),"MISSING_CRATE"));
test("missing production file returns nonzero",()=>check(a=>{a.inventory.push({...a.inventory[0],path:"crates/arc-core/src/missing.rs"})},"MISSING_FILE"));
test("one failing crate fails despite high workspace average",()=>check((_,f)=>{
  writeFileSync(f.path,f.inventory.map((x,i)=>f.record(x.path,i===0?[1,1,1,0,0]:[1,1,1,1,1])).join(""));
},"BELOW_THRESHOLD: arc-core 3/5"));
test("a displayed 80.00% below the exact boundary fails",()=>check((a,f)=>{
  a.inventory=a.inventory.map((x:any)=>({...x,lines:100002}));
  writeFileSync(f.path,f.inventory.map(x=>f.record(x.path,Array.from({length:100001},(_,i)=>i<80000?1:0))).join(""));
},"BELOW_THRESHOLD"));

test("inconsistent LF/LH cannot inflate coverage to a passing verdict",()=>check((_,f)=>{
  const bad=f.inventory.map(x=>f.record(x.path,[1]).replace("LF:1","LF:100")).join("");
  writeFileSync(f.path,bad);
},"MALFORMED_REPORT"));
