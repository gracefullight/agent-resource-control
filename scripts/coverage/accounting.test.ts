import { test, expect } from "bun:test";
import { account } from "./accounting";
import { analyzeSource, tokens, sourceInventory } from "./source";
import { fixture } from "./fixtures";
import { resolve } from "node:path";

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
  expect(result.files).toHaveLength(6);
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
test("live inventory classifies all six crate roots and test-only module trees",()=>{
  const inventory=sourceInventory(resolve(import.meta.dir,"../.."));
  expect(new Set(inventory.filter(x=>!x.testOnly).map(x=>x.crate)).size).toBe(6);
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
