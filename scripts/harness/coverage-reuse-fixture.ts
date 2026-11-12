// Small synthetic receipts/reports exercise validation only. No fixture bytes
// are native measurement evidence and no Rust compilation/coverage is invoked.
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { BASELINE, CRATES, CRATE_ROOTS, METER_INPUTS, FROZEN_INPUTS, PINS } from "../coverage/config";
import { account } from "../coverage/accounting";
import { inputIdentity, sourceInventory, hash } from "../coverage/source";
import { referenceFile } from "../coverage/evidence";
import type { CoverageReuse, ProfileProof } from "../coverage/coverage-receipts";
import type { Receipt } from "../coverage/commands";

const project=resolve(import.meta.dir,"../..");
export function reuseFixture(withExample=false) {
  const directory=resolve(project,"target/host-meter/task-7/fixtures");
  mkdirSync(directory,{recursive:true});
  const root=mkdtempSync(resolve(directory,"case-"));
  const save=(path:string,data:string|object)=>{
    mkdirSync(dirname(resolve(root,path)),{recursive:true});
    const bytes=typeof data==="string"?data:JSON.stringify(data);
    writeFileSync(resolve(root,path),bytes);
    return {path,sha256:hash(bytes),bytes:Buffer.byteLength(bytes)};
  };
  for(const path of ["Cargo.lock","rust-toolchain.toml","mise.toml","package.json","bun.lock","bunfig.toml","biome.json",
    ".arc-verify/rust-coverage/contract/accounting.json",".arc-verify/desktop-foundation/contract.md",
    "apps/desktop/package.json","apps/desktop/tsconfig.json","apps/desktop/tsconfig.test.json","apps/desktop/vite.config.ts",
    "apps/desktop/index.html","apps/desktop/src-tauri/build.rs","apps/desktop/src-tauri/tauri.conf.json",
    ...FROZEN_INPUTS,"scripts/fixture.test.ts","apps/desktop/tests/fixture.test.ts"]) save(path,"Synthetic input fixture\n");
  save("Cargo.toml",`[workspace]\nmembers = ${JSON.stringify(CRATES.map(name=>dirname(CRATE_ROOTS[name].manifest)))}\n`);
  save("bunfig.toml","# Synthetic executable fixture\n");
  save("package.json",{private:true,type:"module"});
  save("apps/desktop/package.json",{private:true,type:"module"});
  save(METER_INPUTS[0],readFileSync(resolve(project,METER_INPUTS[0]),"utf8"));
  for(const name of CRATES) {
    save(CRATE_ROOTS[name].manifest,`[package]\nname = "${name}"\nversion = "0.1.0"\n`);
    save(`${CRATE_ROOTS[name].source}/lib.rs`, "pub fn fixture() {\n    let a = 1;\n    let b = 2;\n    let _c = a + b;\n}\n");
  }
  // Exercise the real executable boundary with only its maintained modules.
  for(const path of ["scripts/coverage/reuse.ts","scripts/coverage/coverage-receipts.ts","scripts/coverage/source.ts",
    "scripts/coverage/config.ts","scripts/coverage/accounting.ts","scripts/coverage/declaration-only.ts","scripts/coverage/gate.ts","scripts/coverage/evidence.ts",
    "scripts/archive/harness/managed-exec-test-output.ts","scripts/harness/validate.sh"]) save(path,readFileSync(resolve(project,path),"utf8"));
  chmodSync(resolve(root,"scripts/harness/validate.sh"),0o755);
  const metadata={workspace_members:[...CRATES],packages:CRATES.map(name=>({id:name,name,manifest_path:resolve(root,CRATE_ROOTS[name].manifest),targets:[] as {name:string;kind:string[];src_path:string}[]}))};
  if(withExample) {
    const path="crates/arc-platform/examples/diagnostic.rs";
    save(path,'#[path = "../src/lib.rs"]\nmod reused;\nfn main() {\n    reused::fixture();\n}\n');
    metadata.packages.find(p=>p.name==="arc-platform")!.targets.push({name:"diagnostic",kind:["example"],src_path:resolve(root,path)});
  }
  const inputs=inputIdentity(root), inventory=sourceInventory(root,metadata);
  const runId="synthetic-reuse-fixture", base="target/host-meter/coverage/runs/synthetic";
  const target=resolve(root,"target/host-meter/coverage/profiles/synthetic");
  const path=`${base}/reuse.json`;
  const nativeLog="Synthetic validator fixture; never executed Rust\nrunning 1 test\ntest fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
  const stdout=save(`${base}/native.stdout.log`,nativeLog), stderr=save(`${base}/native.stderr.log`,"");
  const lcov=inventory.map(file=>`SF:${resolve(root,file.path)}\nDA:1,1\nDA:2,1\nDA:3,1\nDA:4,1\nDA:5,0\nLF:5\nLH:4\nend_of_record\n`).join("");
  const llvm={type:"llvm.coverage.json.export",version:"3.1.0",data:[{files:inventory.map(file=>({
    filename:resolve(root,file.path),summary:{lines:{count:5,covered:4}},
    segments:[1,1,1,1,0].flatMap((hit,i)=>[[i+1,1,hit,true,true,false],[i+1,2,0,false,false,false]]),
  }))}]};
  const lcovRef=save(`${base}/raw.lcov`,lcov), llvmRef=save(`${base}/raw.json`,llvm);
  const profiles=[111,222,333].map(pid=>{
    const path=`arc-${pid}-fixture_0.profraw`, ref=save(`${relative(root,target)}/${path}`,"Synthetic profile fixture\n");
    return {path,sha256:ref.sha256,size:ref.bytes};
  });
  const merge=profiles.map(profile=>resolve(target,profile.path)).sort();
  const proof:ProfileProof={target,profiles,merge,mergeList:save(`${relative(root,target)}/fixture-profraw-list`,merge.join("\n")+"\n"),
    childProof:[{binary:"arcctl",pid:111,exit:0,profiles:[profiles[0].path]},{binary:"arc-daemon",pid:222,exit:1,profiles:[profiles[1].path]}]};
  const command=(argv:string[],extra:Partial<Receipt>={}):Receipt=>({argv,cwd:root,started:"2026-11-11T18:00:00Z",ended:"2026-11-11T18:00:01Z",exit:0,stdout:"",stderr:"",...extra});
  const rust=["rustup","run",PINS.rust,"cargo"];
  const commands=[
    command([...rust,"metadata","--format-version","1","--locked","--offline"],{stdout:JSON.stringify(metadata)}),
    command([...rust,"llvm-cov","--workspace","--all-targets","--locked","--offline","--no-report","--no-cfg-coverage","--no-cfg-coverage-nightly"],{output:{stdout,stderr},stdout:nativeLog}),
    ...proof.childProof.map(child=>command([resolve(target,"debug",child.binary),...(child.binary==="arcctl"?["--help"]:[])],{pid:child.pid,exit:child.exit})),
    ...[["--lcov",lcovRef],["--json",llvmRef]].map(([format,ref])=>command([...rust,"llvm-cov","report",format as string,"--output-path",resolve(root,(ref as typeof lcovRef).path),
      "--failure-mode","any","--disable-default-ignore-filename-regex","--ignore-filename-regex","^/synthetic-dependency(/|$)","--verbose"],{started:"2026-11-11T18:00:02Z",ended:"2026-11-11T18:00:03Z"})),
  ];
  const result=account(lcov,inventory,root,undefined,llvm,metadata);
  const host="aarch64-apple-darwin";
  const versions={bun:PINS.bun,rustc:`Synthetic tool fixture\nrelease: ${PINS.rust}\nLLVM version: 22.1.2\n`,host,
    cargoLlvmCov:`cargo-llvm-cov ${PINS.cargoLlvmCov}`,"llvm-cov":"Synthetic LLVM 22.1.2", "llvm-profdata":"Synthetic LLVM 22.1.2"};
  const summary={...result,runId,acceptedBaseline:BASELINE,sourceHash:inputs.sha256,inputs,rawSha256:lcovRef.sha256,llvmSha256:llvmRef.sha256,versions,host};
  const receipt={schemaVersion:1,runId,pass:true,exit:0,acceptedBaseline:BASELINE,sourceHash:inputs.sha256,receipts:commands};
  const bundle:CoverageReuse={schemaVersion:1,runId,sourceHash:inputs.sha256,acceptedBaseline:BASELINE,
    frozenInputs:Object.fromEntries(FROZEN_INPUTS.map(path=>[path,inputs.files[path]])),metadata:save(`${base}/metadata.json`,metadata),
    evidence:{receipt:save(`${base}/receipt.json`,receipt),summary:save(`${base}/summary.json`,summary),profiles:save(`${base}/profiles.json`,proof),rawReports:{lcov:lcovRef,llvmJson:llvmRef}}};
  const writeBundle=()=>save(path,bundle);
  writeBundle();
  const rebind=(key:"receipt"|"summary"|"profiles",data:object)=>{
    bundle.evidence[key]=save(bundle.evidence[key].path,data);writeBundle();
  };
  const sync=(argv:string[])=>{
    const child=Bun.spawnSync(argv,{cwd:root,stdout:"pipe",stderr:"pipe"});
    if(child.exitCode!==0) throw Error(`Fixture setup failed: ${argv.join(" ")}: ${child.stderr}`);
  };
  sync(["git","-c","init.templateDir=","init","--quiet"]);
  sync(["git","-c","core.hooksPath=/dev/null","add","--",...Object.keys(inputs.files)]);
  return {root,path,save,inputs,inventory,receipt,summary,proof,bundle,metadata,commands,rebind,writeBundle,
    ref:(path:string)=>referenceFile(root,path)};
}
