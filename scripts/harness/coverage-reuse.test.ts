import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { delimiter, resolve } from "node:path";
import { reuseFixture } from "./coverage-reuse-fixture";
import { coverageEvidence, reuseCoverage } from "../coverage/coverage-receipts";
import { coverageLayout } from "../coverage/output-layout";
import { CRATES, METER_INPUTS, MEMORY_PRESSURE_INPUTS, FROZEN_INPUTS, BASELINE } from "../coverage/config";
import { hash, inputIdentity } from "../coverage/source";
import { account } from "../coverage/accounting";
import { enforce } from "../coverage/gate";
import { nativeOutput } from "../coverage/native-output";

function withFixture(run:(f:ReturnType<typeof reuseFixture>)=>void) {
  const f=reuseFixture();
  try {run(f);} finally {rmSync(f.root,{recursive:true,force:true});}
}
const reuse=(f:ReturnType<typeof reuseFixture>)=>reuseCoverage(f.root,f.path);

test("synthetic same-input receipt recomputes all eleven integer floors without collection",()=>withFixture(f=>{
  const result=reuse(f);
  expect(result.recollected).toBe(false);
  expect(result.sourceHash).toBe(inputIdentity(f.root).sha256);
  expect(result.result.threshold).toBe(80);
  expect(result.result.crates.map(c=>c.crate)).toEqual([...CRATES]);
  for(const crate of result.result.crates) expect(crate).toMatchObject({covered:4,total:5,pass:true});
  expect(f.bundle.acceptedBaseline).toBe(BASELINE);
}));

test("reuse rejects consistent source hashes unrelated to the validated input digest",()=>withFixture(f=>{
  const sourceHash="0".repeat(64);
  f.bundle.sourceHash=sourceHash;
  f.rebind("receipt",{...f.receipt,sourceHash});
  f.rebind("summary",{...f.summary,sourceHash});
  expect(()=>coverageEvidence(f.root,f.bundle.evidence,sourceHash)).toThrow("source hash/input digest mismatch");
  expect(()=>reuse(f)).toThrow("source hash/input digest mismatch");
}));

test("memory-pressure frozen inputs bind receipts while generated reports do not enter identity",()=>withFixture(f=>{
  for(const name of ["gates.json","integration.json","qa.json","final.md"]) {
    f.save(`.arc-verify/memory-pressure/${name}`,"Synthetic generated result\n");
  }
  f.save("target/memory-pressure/raw-log.txt","Synthetic raw log\n");
  expect(inputIdentity(f.root).sha256).toBe(f.inputs.sha256);
  expect(reuse(f).recollected).toBe(false);
  const contract=MEMORY_PRESSURE_INPUTS[0];
  delete f.bundle.frozenInputs[contract];f.writeBundle();
  expect(()=>reuse(f)).toThrow(`frozen input mismatch ${contract}`);
}));

test("rebinding current input manifests cannot reuse an earlier source identity",()=>withFixture(f=>{
  for(const path of ["scripts/fixture.test.ts","apps/desktop/tests/fixture.test.ts","apps/desktop/tsconfig.json",
    "scripts/coverage/config.ts","scripts/coverage/declaration-only.ts","Cargo.lock","bun.lock",METER_INPUTS[1],...MEMORY_PRESSURE_INPUTS]) {
    const file=resolve(f.root,path), bytes=readFileSync(file);
    writeFileSync(file,Buffer.concat([bytes,Buffer.from("\n")]));
    const current=inputIdentity(f.root);
    expect(current.sha256).not.toBe(f.inputs.sha256);
    // Keep the manifest and frozen contract current, leaving the collection's
    // source hashes and reports at the earlier inputs.
    f.bundle.frozenInputs[METER_INPUTS[1]]=current.files[METER_INPUTS[1]];
    for(const path of MEMORY_PRESSURE_INPUTS) f.bundle.frozenInputs[path]=current.files[path];
    f.rebind("summary",{...f.summary,inputs:current});
    expect(()=>coverageEvidence(f.root,f.bundle.evidence,f.inputs.sha256)).toThrow("source hash/input digest mismatch");
    expect(()=>reuse(f)).toThrow("source hash/input digest mismatch");
    writeFileSync(file,bytes);
  }
}));

test("each added crate independently fails when missing or below 80%",()=>withFixture(f=>{
  const lcov=readFileSync(resolve(f.root,f.bundle.evidence.rawReports.lcov.path),"utf8");
  for(const crate of ["arc-platform","arc-meter","arc-policy"]) {
    const file=f.inventory.find(file=>file.crate===crate)!;
    const records=lcov.split("end_of_record\n").filter(Boolean);
    const below=records.map(record=>record.includes(resolve(f.root,file.path))?record.replace("DA:4,1","DA:4,0").replace("LH:4","LH:3"):record).join("end_of_record\n")+"end_of_record\n";
    expect(()=>enforce(account(below,f.inventory,f.root))).toThrow(`BELOW_THRESHOLD: ${crate} 3/5`);
    expect(()=>account(records.filter(record=>!record.includes(resolve(f.root,file.path))).join("end_of_record\n")+"end_of_record\n",f.inventory,f.root)).toThrow(`MISSING_CRATE: ${crate}`);
  }
}));

test("reuse rejects changed source, tests, config, locks, SRS and frozen contract inputs",()=>withFixture(f=>{
  for(const path of ["crates/arc-platform/src/lib.rs","crates/arc-meter/src/lib.rs","crates/arc-policy/src/lib.rs","scripts/fixture.test.ts",
    "apps/desktop/tests/fixture.test.ts","apps/desktop/tsconfig.json","Cargo.toml","Cargo.lock","bun.lock",
    "scripts/coverage/config.ts","scripts/coverage/declaration-only.ts",...FROZEN_INPUTS]) {
    const file=resolve(f.root,path), bytes=readFileSync(file);
    writeFileSync(file,Buffer.concat([bytes,Buffer.from("\n")]));
    expect(()=>reuse(f)).toThrow("stale or incomplete");
    writeFileSync(file,bytes);
  }
  const added="crates/arc-meter/tests/added.rs";
  f.save(added,"// Added test input\n");
  expect(()=>reuse(f)).toThrow("stale or incomplete");unlinkSync(resolve(f.root,added));
  const missing="scripts/fixture.test.ts", bytes=readFileSync(resolve(f.root,missing));
  unlinkSync(resolve(f.root,missing));expect(()=>reuse(f)).toThrow("stale or incomplete");writeFileSync(resolve(f.root,missing),bytes);
  f.save("target/host-meter/later-output.json",{status:"generated"});
  f.save(".arc-verify/host-meter/gates.json",{status:"generated"});
  expect(reuse(f).sourceHash).toBe(f.inputs.sha256);
}));

test("reuse rejects failed, stale, incomplete or mismatched collection identities",()=>withFixture(f=>{
  for(const change of [{pass:false},{exit:1},{sourceHash:"0".repeat(64)},{runId:"older-run"},{acceptedBaseline:"changed"},{receipts:[]}]) {
    f.rebind("receipt",{...f.receipt,...change});expect(()=>reuse(f)).toThrow();
  }
  f.rebind("receipt",f.receipt);
  for(const change of [{sourceHash:"0".repeat(64)},{runId:"older-run"},{acceptedBaseline:"changed"},{threshold:0},{crates:[]},{files:[]}]) {
    f.rebind("summary",{...f.summary,...change});expect(()=>reuse(f)).toThrow();
  }
  for(const change of [{versions:undefined},{versions:{...f.summary.versions,bun:"0.0.0"}},{versions:{...f.summary.versions,"llvm-cov":"incorrect LLVM"}},{host:"unsupported-host"}]) {
    f.rebind("summary",{...f.summary,...change});expect(()=>reuse(f)).toThrow("pinned collection tools");
  }
  const files={...f.inputs.files};delete files[METER_INPUTS[1]];
  f.rebind("summary",{...f.summary,inputs:{files,sha256:hash(JSON.stringify(files))}});
  expect(()=>reuse(f)).toThrow("stale or incomplete");
  f.rebind("summary",{...f.summary,inputs:{...f.inputs,sha256:"0".repeat(64)}});
  expect(()=>reuse(f)).toThrow("corrupt aggregate");
  f.rebind("summary",f.summary);
  f.bundle.frozenInputs[METER_INPUTS[1]]="0".repeat(64);f.writeBundle();
  expect(()=>reuse(f)).toThrow("frozen input mismatch");
}));

test("reuse rejects changed workspace membership and unexpected manifest roots",()=>withFixture(f=>{
  for(const metadata of [
    {...f.metadata,workspace_members:f.metadata.workspace_members.slice(0,-1)},
    {...f.metadata,workspace_members:[...f.metadata.workspace_members,"extra"],packages:[...f.metadata.packages,{...f.metadata.packages[0],id:"extra",name:"extra"}]},
    {...f.metadata,packages:f.metadata.packages.map(p=>p.name==="arc-meter"?{...p,manifest_path:f.metadata.packages[0].manifest_path}:p)},
  ]) {
    f.bundle.metadata=f.save(f.bundle.metadata.path,metadata);f.writeBundle();
    expect(()=>reuse(f)).toThrow("MISSING_CRATE");
  }
}));

test("reuse requires present hash-matching raw reports, profiles and collection artifacts",()=>withFixture(f=>{
  const refs=[f.bundle.evidence.receipt,f.bundle.evidence.summary,f.bundle.evidence.profiles,f.bundle.metadata,
    ...Object.values(f.bundle.evidence.rawReports),f.proof.mergeList,
    ...f.proof.profiles.map(profile=>({path:resolve(f.proof.target,profile.path).slice(f.root.length+1)})),
    ...Object.values(f.commands[1].output!)];
  for(const ref of refs) {
    const path=resolve(f.root,ref.path), bytes=readFileSync(path);
    writeFileSync(path,Buffer.concat([bytes,Buffer.from("altered")]));
    expect(()=>reuse(f)).toThrow("hash/size mismatch");
    unlinkSync(path);expect(()=>reuse(f)).toThrow();writeFileSync(path,bytes);
  }
  unlinkSync(resolve(f.root,f.path));expect(()=>reuse(f)).toThrow();
}));

test("reuse rechecks exact merge membership and mandatory native child profiles",()=>withFixture(f=>{
  for(const change of [
    {merge:f.proof.merge.slice(1)}, {merge:[...f.proof.merge,f.proof.merge[0]]},
    {profiles:[...f.proof.profiles,f.proof.profiles[0]]}, {profiles:[]}, {childProof:[]},
    {childProof:f.proof.childProof.map(child=>({...child,pid:999}))},
    {childProof:f.proof.childProof.map(child=>({...child,exit:0}))},
    {childProof:f.proof.childProof.map(child=>({...child,profiles:[]}))},
    {profiles:f.proof.profiles.map(profile=>({...profile,path:"../escape.profraw"}))},
  ]) {
    f.rebind("profiles",{...f.proof,...change});expect(()=>reuse(f)).toThrow("CHILD_PROFILE");
  }
  f.rebind("profiles",f.proof);
  f.proof.mergeList=f.save(f.proof.mergeList.path,f.proof.merge.slice(1).join("\n"));
  f.rebind("profiles",f.proof);expect(()=>reuse(f)).toThrow("merge input mismatch");
}));

test("reuse requires successful complete native collection, exports, metadata and probe receipts",()=>withFixture(f=>{
  for(const index of f.commands.keys()) {
    const commands=structuredClone(f.commands);commands[index].exit=9;
    f.rebind("receipt",{...f.receipt,receipts:commands});expect(()=>reuse(f)).toThrow();
  }
  for(const index of [0,1,2,3,4,5]) {
    f.rebind("receipt",{...f.receipt,receipts:f.commands.filter((_,i)=>i!==index)});expect(()=>reuse(f)).toThrow();
  }
  for(const change of [
    {index:1,patch:{argv:[...f.commands[1].argv,"--","filtered_selector"]}},
    {index:1,patch:{cwd:"/different-workspace"}},
    {index:1,patch:{started:"invalid-time"}},
    {index:0,patch:{stdout:JSON.stringify({workspace_members:[],packages:[]})}},
    {index:2,patch:{pid:999}},
    {index:4,patch:{started:"2026-11-11T17:00:00Z"}},
    {index:5,patch:{argv:f.commands[5].argv.filter(arg=>arg!=="--disable-default-ignore-filename-regex")}},
  ]) {
    const commands=structuredClone(f.commands);Object.assign(commands[change.index],change.patch);
    f.rebind("receipt",{...f.receipt,receipts:commands});expect(()=>reuse(f)).toThrow();
  }
}));

test("reuse cannot replace native tests with empty, failing, ignored or filtered output",()=>withFixture(f=>{
  for(const text of ["", "running 1 test\ntest fixture ... FAILED\n",
    "running 1 test\ntest fixture ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1s\n",
    f.commands[1].stdout.replace("0 filtered out","1 filtered out")]) {
    const commands=structuredClone(f.commands);
    commands[1].output!.stdout=f.save(commands[1].output!.stdout.path,text);
    f.rebind("receipt",{...f.receipt,receipts:commands});expect(()=>reuse(f)).toThrow();
  }
}));

test("coverage scratch layout moves reports, logs and profiles without replacing historical identities",()=>withFixture(f=>{
  const normal=coverageLayout(f.root,"fixture-run"), scratch=coverageLayout(f.root,"fixture-run","target/host-meter");
  expect(normal.directory).toContain(".arc-verify/rust-coverage/baseline/runs/fixture-run");
  expect(normal.target).toContain("target/arc-coverage/fixture-run");
  for(const path of Object.values(scratch)) expect(path).toContain("target/host-meter/coverage/");
  for(const root of ["", "/tmp", "target/../escape", "target/host-meter/", "target//host-meter"]) expect(()=>coverageLayout(f.root,"run",root)).toThrow("COVERAGE_OUTPUT");
  const output=nativeOutput(f.commands[1].argv,f.root,Buffer.from("synthetic stdout"),Buffer.from("synthetic stderr"),scratch.nativeDirectory)!;
  expect(output.stdout.path).toContain("target/host-meter/coverage/");
  expect(readFileSync(resolve(f.root,output.stdout.path),"utf8")).toBe("synthetic stdout");
}));

function harness(f:ReturnType<typeof reuseFixture>, receipt:string|undefined) {
  const bin=resolve(f.root,".fixture-bin"), calls=resolve(f.root,".fixture-coverage-calls");
  mkdirSync(bin,{recursive:true});
  const quote=(value:string)=>"'"+value.replaceAll("'","'\\''")+"'";
  writeFileSync(resolve(bin,"bun"),`#!/bin/bash\nif [ "$1" = run ]; then printf 'standalone fixture collection\\n' >> ${quote(calls)}; exit 0; fi\nexec ${quote(process.execPath)} "$@"\n`);
  chmodSync(resolve(bin,"bun"),0o755);
  const env={...process.env,PATH:bin+delimiter+process.env.PATH,ARC_COVERAGE_RECEIPT:receipt};
  const result=Bun.spawnSync(["/bin/bash","scripts/harness/validate.sh"],{cwd:f.root,env,stdout:"pipe",stderr:"pipe"});
  let collected="";try {collected=readFileSync(calls,"utf8");} catch {}
  return {exit:result.exitCode,stdout:result.stdout.toString(),stderr:result.stderr.toString(),collected};
}

test("harness validates a good receipt, executes all architecture checks and never recollects",()=>withFixture(f=>{
  const result=harness(f,f.path);
  expect(result.exit).toBe(0);expect(result.collected).toBe("");
  expect(result.stdout).toContain("validated fresh receipt");
  for(let i=1;i<=5;i++) expect(result.stdout).toContain(`Check ${i}/5:`);
  expect(result.stdout).toContain("All checks passed (0 violations)");
  f.save("domain/violation.ts",'import x from "../infrastructure/x";\n');
  const tracked=Bun.spawnSync(["git","add","--","domain/violation.ts"],{cwd:f.root});expect(tracked.exitCode).toBe(0);
  const bad=harness(f,f.path);
  expect(bad.exit).not.toBe(0);expect(bad.stderr).toContain("Architecture violation");expect(bad.collected).toBe("");
}));

test("explicit invalid, missing, empty and stale harness receipts fail without hidden recollection",()=>withFixture(f=>{
  for(const path of ["", "target/host-meter/missing.json", "../outside.json"]) {
    const result=harness(f,path);expect(result.exit).not.toBe(0);expect(result.collected).toBe("");
    expect(result.stderr).toContain("Coverage receipt validation failed");
  }
  f.save("scripts/fixture.test.ts","// changed test\n");
  const result=harness(f,f.path);expect(result.exit).not.toBe(0);expect(result.collected).toBe("");expect(result.stderr).toContain("stale or incomplete");
}));

test("harness rejects a rebound current manifest with an earlier source identity",()=>withFixture(f=>{
  f.save("scripts/fixture.test.ts","// changed test input\n");
  f.rebind("summary",{...f.summary,inputs:inputIdentity(f.root)});
  const result=harness(f,f.path);
  expect(result.exit).not.toBe(0);expect(result.collected).toBe("");
  expect(result.stderr).toContain("source hash/input digest mismatch");
  expect(result.stderr).toContain("Coverage receipt validation failed");
}));

test("no harness receipt retains normal standalone collection",()=>withFixture(f=>{
  const result=harness(f,undefined);
  expect(result.exit).toBe(0);expect(result.collected).toBe("standalone fixture collection\n");
  expect(result.stdout).toContain("Coverage thresholds met");
}));
