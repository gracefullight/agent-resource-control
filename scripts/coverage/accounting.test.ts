import { test, expect } from "bun:test";
import { account } from "./accounting";
import { analyzeSource, tokens, sourceInventory, historicalSourceInventory, inputIdentity, requireInputIdentity, hash } from "./source";
import { fixture } from "./fixtures";
import { CRATES, CRATE_ROOTS, HISTORICAL_CRATES } from "./config";
import { requireWorkspace } from "./gate";
import { cpSync, mkdirSync, readFileSync, writeFileSync, unlinkSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";

test("repeated compilation unions source lines instead of averaging binaries",()=>{
  const f=fixture();
  const result=account(f.lcov+f.record(f.inventory[0].path,[0,0,0,0,1]),f.inventory,f.root);
  expect(result.crates[0]).toMatchObject({covered:5,total:5,pass:true});
  expect(result.crates[1]).toMatchObject({covered:4,total:5});
});
test("zero-hit lines survive; per-file uncovered numbers are stable",()=>{
  const f=fixture(), result=account(f.lcov,f.inventory,f.root);
  expect(result.files[0].uncovered).toEqual([5]);expect(result.files[0].total).toBe(5);
});
test("test-only lines/files never dilute production denominator",()=>{
  const f=fixture();f.inventory[0].omitted=[1,2];
  f.inventory.push({...f.inventory[0],path:"crates/arc-core/src/test_fixture.rs",testOnly:true});
  const result=account(f.lcov,f.inventory,f.root);
  expect(result.crates[0]).toMatchObject({covered:2,total:3,pass:false});
  expect(result.files).toHaveLength(10);
});
test("canonical aliases merge and never duplicate denominators",()=>{
  const f=fixture();
  const alias=f.record("crates/arc-core/src/../src/lib.rs",[0,0,0,0,1]);
  expect(account(f.lcov+alias,f.inventory,f.root).crates[0].total).toBe(5);
});
test("unknown paths, zero denominators and malformed hits fail closed",()=>{
  const f=fixture();
  expect(()=>account(f.lcov.replace("DA:1,1","DA:1,-1"),f.inventory,f.root)).toThrow("MALFORMED_REPORT");
  expect(()=>account(f.lcov+f.record("Cargo.toml"),f.inventory,f.root)).toThrow("UNMAPPED_FILE");
  expect(()=>account(f.lcov,f.inventory.filter(x=>x.crate!=="arc-core"),f.root)).toThrow();
  expect(()=>account(f.inventory.map(x=>f.record(x.path,[])).join(""),f.inventory,f.root)).toThrow("MISSING_CRATE");
});
test("cfg omission handles nested Rust comments, literals and test-only statements",()=>{
  const source=`fn production() { let s = r#"{ // not code }"#; }\n#[cfg(all(test, target_os = "macos"))]\nmod tests { /* { /* nested */ } */ fn example() { let c = '}'; } }\nfn retained() {}\n#[cfg(test)]\nfn helper() { let s = "}"; }\n`;
  const a=analyzeSource(source);
  expect([...a.omitted]).toEqual([2,3,5,6]);
  expect(tokens(source).find(x=>x.text==='r#"{ // not code }"#')).toBeDefined();
});
test("test combined with any platform condition is not blanket-excluded",()=>{
  const a=analyzeSource('#[cfg(any(test, target_os = "linux"))]\nfn still_production() {}\n#[cfg(not(test))]\nfn production_only() {}');
  expect(a.omitted.size).toBe(0);
});
test("external cfg test module keeps its declared path; arbitrary filenames are not exclusions",()=>{
  const a=analyzeSource('#[cfg(test)]\n#[path = "other.rs"]\nmod tests;\nmod tests_in_production;');
  expect(a.modules).toEqual([{name:"tests",path:"other.rs",testOnly:true},{name:"tests_in_production",path:undefined,testOnly:false}]);
});
test("test-only grouped use consumes its semicolon and retains adjacent production",()=>{
  const source='#[cfg(test)]\nuse test_calls::{call_until, call_until_no_busy_retry, call_until_tracked, call_with};\nfn production() {}\n';
  expect([...analyzeSource(source).omitted]).toEqual([1,2]);
});
test("nested and multiline grouped imports stop at their semicolon, block items at their brace",()=>{
  const source=`#[cfg(all(test, unix))]
pub(crate) use super::{
    calls::{one, nested::{two, three}},
    other,
};
fn production() { retained(); }
#[cfg(test)]
fn helper() { nested(); }
fn also_production() {}
`;
  expect([...analyzeSource(source).omitted]).toEqual([1,2,3,4,5,7,8]);
  expect([...analyzeSource('#[cfg(any(test, unix))]\nuse super::{a, nested::{b,c}};\nfn production() {}').omitted]).toEqual([]);
});
test("genuinely shared test-only import or block lines remain ambiguous",()=>{
  for(const source of [
    '#[cfg(test)]\nuse test_calls::{one, two}; fn production() {}',
    '#[cfg(test)]\nuse super::{\n one, nested::{two, three}\n}; fn production() {}',
    'fn production() {} #[cfg(test)]\nuse super::{one};',
    '#[cfg(test)]\nfn helper() {} fn production() {}',
  ]) expect(()=>analyzeSource(source)).toThrow("line accounting is ambiguous");
});
test("live inventory classifies all ten crate roots and test-only module trees",()=>{
  const inventory=sourceInventory(resolve(import.meta.dir,"../.."));
  expect(new Set(inventory.filter(x=>!x.testOnly).map(x=>x.crate)).size).toBe(10);
  for(const crate of ["arc-platform","arc-meter"]) expect(inventory.some(x=>!x.testOnly && x.crate===crate)).toBe(true);
  expect(inventory.find(x=>x.path==="apps/desktop/src-tauri/src/lib.rs")?.crate).toBe("arc-desktop");
  expect(inventory.find(x=>x.path==="crates/arc-daemon/src/transport_tests.rs")?.testOnly).toBe(true);
  expect(inventory.find(x=>x.path==="crates/arc-daemon/src/transport.rs")?.testOnly).toBe(false);
});

test("record summaries must match DA lines before repeated instances merge",()=>{
  const f=fixture();
  const raw=f.lcov.replaceAll(/^(?:LF|LH):[^\n]*\n/gm,"");
  for(const summary of ["LF:100\nLH:1", "LF:5\nLH:5", "LF:-1\nLH:4",
    "LF:NaN\nLH:4", "LF:5\nLH:Infinity", "LF:9007199254740992\nLH:4",
    "LF:5\nLH:-1", "LF:5\nLH:4\nLF:5", "LF:5\nLH:4\nLH:4", "LF:5", "LH:4", ""]) {
    const bad=raw.replaceAll("end_of_record",`${summary}\nend_of_record`);
    expect(()=>account(bad,f.inventory,f.root)).toThrow("MALFORMED_REPORT");
  }
  // Later instances cannot repair an invalid earlier record's denominator.
  const bad=f.lcov.replace("LF:5","LF:100");
  expect(()=>account(bad+f.record(f.inventory[0].path,[0,0,0,0,1]),f.inventory,f.root)).toThrow("MALFORMED_REPORT");
});
test("duplicate DA entries and summaries outside a record are malformed",()=>{
  const f=fixture();
  expect(()=>account(f.lcov.replace("DA:1,1","DA:1,1\nDA:1,0"),f.inventory,f.root)).toThrow("MALFORMED_REPORT");
  expect(()=>account("LF:5\n"+f.lcov,f.inventory,f.root)).toThrow("MALFORMED_REPORT");
  expect(()=>account(f.lcov+"LH:4\n",f.inventory,f.root)).toThrow("MALFORMED_REPORT");
});

test("LLVM function summaries require corroboration by the complete segment-derived line map",()=>{
  const f=fixture();
  // LLVM 22 exports function-group LF/LH, while DA is a file-wide line union.
  // Keep the strict standalone LCOV contract; accept that distinction only
  // when a second native export proves every DA line and the original summary.
  const llvm:any={type:"llvm.coverage.json.export",version:"3.1.0",data:[{files:f.inventory.map(file=>({
    filename:resolve(f.root,file.path),summary:{lines:{count:7,covered:6}},
    segments:[1,1,1,1,0].flatMap((hit,i)=>[[i+1,1,hit,true,true,false],[i+1,2,0,false,false,false]]),
  }))}]};
  const raw=f.lcov.replaceAll("LF:5","LF:7").replaceAll("LH:4","LH:6");
  expect(()=>account(raw,f.inventory,f.root)).toThrow("MALFORMED_REPORT");
  const result=account(raw,f.inventory,f.root,undefined,llvm);
  expect(result.crates[0]).toMatchObject({covered:4,total:5,pass:true});
  expect(result.files[0].uncovered).toEqual([5]);
  for(const bad of [raw.replace("LF:7","LF:100"),raw.replace("DA:5,0\n",""),raw.replace("DA:5,0","DA:5,1")]) {
    expect(()=>account(bad,f.inventory,f.root,undefined,llvm)).toThrow("MALFORMED_REPORT");
  }
  const clone=()=>structuredClone(llvm);
  const missing=clone();missing.data[0].files.pop();
  expect(()=>account(raw,f.inventory,f.root,undefined,missing)).toThrow("MALFORMED_REPORT");
  const extra=clone();extra.data[0].files.push({...extra.data[0].files[0],filename:"extra.rs"});
  expect(()=>account(raw,f.inventory,f.root,undefined,extra)).toThrow("MALFORMED_REPORT");
  const invalid=clone();invalid.data[0].files[0].segments[0][2]=-1;
  expect(()=>account(raw,f.inventory,f.root,undefined,invalid)).toThrow("MALFORMED_REPORT");
});

test("desktop production roots and test-only helper binaries use the explicit registry",()=>{
  const f=fixture(), desktop=f.inventory.find(file=>file.crate==="arc-desktop")!;
  try {
    const probe="apps/desktop/src-tauri/tests/bin/desktop_probe.rs";
    mkdirSync(resolve(f.root,dirname(probe)),{recursive:true});
    writeFileSync(resolve(f.root,probe),"fn main() {}\n".repeat(5));
    expect(account(f.lcov+f.record(probe),f.inventory,f.root).crates.find(c=>c.crate==="arc-desktop")).toMatchObject({crate:"arc-desktop",covered:4,total:5,pass:true});
    const bad="crates/unknown/tests/pretend.rs";
    mkdirSync(resolve(f.root,dirname(bad)),{recursive:true});writeFileSync(resolve(f.root,bad),"test\n");
    expect(()=>account(f.lcov+f.record(bad,[1]),f.inventory,f.root)).toThrow("UNMAPPED_FILE");
    expect(()=>account(f.inventory.filter(file=>file!==desktop).map(file=>f.record(file.path)).join(""),f.inventory,f.root)).toThrow("MISSING_CRATE: arc-desktop");
    expect(()=>account(f.lcov,[...f.inventory,{...desktop,path:"apps/desktop/src-tauri/src/missing.rs"}],f.root)).toThrow("MISSING_FILE");
    const below=f.inventory.map(file=>f.record(file.path,file===desktop?[1,1,1,0,0]:[1,1,1,1,1])).join("");
    expect(account(below,f.inventory,f.root).crates.find(c=>c.crate==="arc-desktop")).toMatchObject({covered:3,total:5,pass:false});
    expect(()=>account(f.lcov,f.inventory.map(file=>file===desktop?{...file,crate:"arc-core"}:file),f.root)).toThrow("UNMAPPED_FILE");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("workspace metadata requires exactly ten names at the registered manifest roots",()=>{
  const f=fixture();
  try {
    const packages=CRATES.map(name=>{
      const manifest_path=resolve(f.root,CRATE_ROOTS[name].manifest);writeFileSync(manifest_path,`[package]\nname = "${name}"\n`);
      return {id:name,name,manifest_path};
    });
    const metadata={workspace_members:[...CRATES],packages};
    expect(()=>requireWorkspace(metadata,f.root)).not.toThrow();
    expect(()=>requireWorkspace({...metadata,workspace_members:CRATES.slice(0,-1)},f.root)).toThrow("MISSING_CRATE");
    expect(()=>requireWorkspace({...metadata,workspace_members:[...CRATES,"extra"],packages:[...packages,{...packages[0],name:"extra",id:"extra"}]},f.root)).toThrow("MISSING_CRATE");
    expect(()=>requireWorkspace({...metadata,packages:packages.map(p=>p.name==="arc-desktop"?{...p,manifest_path:packages[0].manifest_path}:p)},f.root)).toThrow("unexpected manifest root");
    expect(()=>requireWorkspace({...metadata,workspace_members:[...CRATES,"absent"]},f.root)).toThrow("MISSING_CRATE");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("freshness binds desktop configuration, test helpers, new source and verification scripts, excluding generated receipts",()=>{
  const f=fixture(), root=resolve(import.meta.dir,"../..");
  try {
    // Copy only declared inputs, never target artifacts or installed packages.
    for(const path of Object.keys(inputIdentity(root).files)) {
      const to=resolve(f.root,path);mkdirSync(dirname(to),{recursive:true});cpSync(resolve(root,path),to);
    }
    const original=inputIdentity(f.root);
    expect(()=>requireInputIdentity(f.root,original)).not.toThrow();
    expect(historicalSourceInventory(f.root).map(f=>f.crate).every(crate=>HISTORICAL_CRATES.includes(crate as typeof HISTORICAL_CRATES[number]))).toBe(true);
    for(const path of ["apps/desktop/src-tauri/tauri.conf.json","apps/desktop/tests/bridge-client.test.ts","bun.lock","scripts/coverage/config.ts",
      "docs/requirements/ARC_SRS_2026-11-06.md",".arc-verify/host-meter/contract.md"]) {
      const file=resolve(f.root,path), bytes=readFileSync(file);writeFileSync(file,Buffer.concat([bytes,Buffer.from("\n")]));
      expect(()=>requireInputIdentity(f.root,original)).toThrow("stale");writeFileSync(file,bytes);
    }
    const add="apps/desktop/src-tauri/tests/bin/client_probe.rs";
    mkdirSync(dirname(resolve(f.root,add)),{recursive:true});writeFileSync(resolve(f.root,add),"fn main() {}\n");
    expect(inputIdentity(f.root).files[add]).toBeDefined();
    expect(()=>requireInputIdentity(f.root,original)).toThrow("stale");unlinkSync(resolve(f.root,add));
    for(const path of ["target/desktop-foundation/run/log.json","apps/desktop/dist/output.js","apps/desktop/src-tauri/gen/schemas/schema.json",".arc-verify/desktop-foundation/gates.json",".arc-verify/desktop-foundation/qa.md"]) {
      mkdirSync(dirname(resolve(f.root,path)),{recursive:true});writeFileSync(resolve(f.root,path),"generated\n");
    }
    expect(inputIdentity(f.root)).toEqual(original);
    const missing="apps/desktop/src-tauri/src/lib.rs";unlinkSync(resolve(f.root,missing));
    expect(()=>requireInputIdentity(f.root,original)).toThrow("stale");
    unlinkSync(resolve(f.root,"apps/desktop/src-tauri/src/main.rs"));
    expect(()=>sourceInventory(f.root)).toThrow();
    unlinkSync(resolve(f.root,"apps/desktop/src-tauri/tauri.conf.json"));
    expect(()=>inputIdentity(f.root)).toThrow();
    expect(()=>requireInputIdentity(root,{...original,sha256:"0".repeat(64)})).toThrow("corrupt aggregate");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

function declarationFixture(source:string) {
  const f=fixture(), path="crates/arc-platform/src/boundary.rs";
  writeFileSync(resolve(f.root,path),source);
  const file={...f.inventory.find(x=>x.crate==="arc-platform")!,path,sha256:hash(source),lines:source.split("\n").length};
  f.inventory.push(file);
  const llvm={type:"llvm.coverage.json.export",version:"3.1.0",data:[{files:f.inventory.filter(x=>x!==file).map(x=>({
    filename:resolve(f.root,x.path),summary:{lines:{count:5,covered:4}},
    segments:[1,1,1,1,0].flatMap((hit,i)=>[[i+1,1,hit,true,true,false],[i+1,2,0,false,false,false]]),
  }))}]};
  return {...f,file,llvm};
}
test("native exports may omit hash-verified declarations without inventing executable lines",()=>{
  const source=`use crate::value::Reading;\nmod native;\npub use native::Source;\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct Sample { pub count: u64 }\npub enum State { Ready, Unknown(u64) }\npub trait Source: Send + 'static { fn sample(&mut self) -> Sample; }\n`;
  const f=declarationFixture(source);
  try {
    const result=account(f.lcov,f.inventory,f.root,undefined,f.llvm);
    expect(result.files.find(x=>x.path===f.file.path)).toMatchObject({total:0,covered:0,mapped:[],declarationOnly:true});
    expect(result.crates.find(x=>x.crate==="arc-platform")).toMatchObject({total:5,covered:4,pass:true});
    // A real declaration boundary from the new crate also has no native record.
    const actual=readFileSync(resolve(import.meta.dir,"../../crates/arc-platform/src/lib.rs"),"utf8");
    writeFileSync(resolve(f.root,f.file.path),actual);f.file.sha256=hash(actual);f.file.lines=actual.split("\n").length;
    expect(account(f.lcov,f.inventory,f.root,undefined,f.llvm).pass).toBe(true);
    const executable=f.inventory.find(x=>x.crate==="arc-platform" && x!==f.file)!;
    const below=structuredClone(f.llvm), native=below.data[0].files.find(x=>x.filename===resolve(f.root,executable.path))!;
    native.summary.lines.covered=3;native.segments[6][2]=0;
    const lcov=f.lcov.replace(f.record(executable.path),f.record(executable.path,[1,1,1,0,0]));
    expect(account(lcov,f.inventory,f.root,undefined,below).crates.find(x=>x.crate==="arc-platform")).toMatchObject({covered:3,total:5,pass:false});
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("absent native declarations require matching bytes and a complete corroborated report",()=>{
  const f=declarationFixture("pub struct Decl { value: u64 }\n");
  try {
    expect(()=>account(f.lcov,f.inventory,f.root)).toThrow("MISSING_FILE");
    f.file.sha256="0".repeat(64);
    expect(()=>account(f.lcov,f.inventory,f.root,undefined,f.llvm)).toThrow("MISSING_FILE");
    unlinkSync(resolve(f.root,f.file.path));
    expect(()=>account(f.lcov,f.inventory,f.root,undefined,f.llvm)).toThrow("MISSING_FILE");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("missing executable bodies, initializers and expansion hooks remain hard failures",()=>{
  const f=declarationFixture("pub struct Decl;\n");
  try {
    for(const source of [
      "fn missing() {}", "impl Decl { fn missing() {} }", "pub trait T { fn defaulted() {} }",
      "const VALUE: u64 = 1;", "static VALUE: u64 = 1;", "generated!();", "include!(\"body.rs\");",
      "macro_rules! hidden { () => {} }", "#[derive(Custom)] pub struct Decl;",
      "#[custom] pub struct Decl;", "#[cfg_attr(any(), custom)] pub struct Decl;",
      "pub enum E { A = dangerous() }", "mod inline { fn missing() {} }",
      "pub struct Decl { field: [u8; { dangerous() }] }",
      "pub trait T { #[custom] fn signature(); }",
    ]) {
      writeFileSync(resolve(f.root,f.file.path),source);f.file.sha256=hash(source);f.file.lines=source.split("\n").length;
      expect(()=>account(f.lcov,f.inventory,f.root,undefined,f.llvm)).toThrow("MISSING_FILE");
    }
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
