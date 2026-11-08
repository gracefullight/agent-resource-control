import { mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { resolve, relative, join, dirname } from "node:path";
import { BASELINE, PINS, EVIDENCE, CRATES } from "./config";
import { hash, sourceInventory, inputIdentity, walk } from "./source";
import { requireExit, requireVersion, validateReport, enforce } from "./gate";
import { command, type Receipt } from "./commands";

const root=resolve(import.meta.dir,"../..");
const id=`${new Date().toISOString().replace(/[:.]/g,"-")}-${crypto.randomUUID()}`;
const dir=resolve(root,EVIDENCE,"baseline/runs",id);
mkdirSync(dir,{recursive:true});
const save=(name:string,value:unknown)=>writeFileSync(resolve(dir,name),JSON.stringify(value,null,2)+"\n");
const receipts:Receipt[]=[];
const env={...process.env,RUSTUP_TOOLCHAIN:PINS.rust,CARGO_NET_OFFLINE:"true",CARGO_TERM_COLOR:"never"};
const rust=["rustup","run",PINS.rust];
async function run(argv:string[],label:string,extra:Record<string,string>={},expected=0,timeout=120000) {
  const receipt=await command(argv,dir,root,{...env,...extra},timeout); receipts.push(receipt);
  if(receipt.exit!==expected) throw Error(`${label}: exited ${receipt.exit}; ${receipt.stderr.trim()}`);
  return receipt;
}
let input:ReturnType<typeof inputIdentity>|undefined;
let inventory:ReturnType<typeof sourceInventory>|undefined;
let measured=false;
try {
  console.log("Coverage: checking pinned tools (setup: mise run coverage:provision)");
  input=inputIdentity(root); inventory=sourceInventory(root);
  save("inputs.json",input);save("source-inventory.json",inventory);
  requireVersion(Bun.version,PINS.bun,"Bun");
  const rustc=await run([...rust,"rustc","-Vv"],"MISSING_TOOL rustc");
  if(!rustc.stdout.includes(`release: ${PINS.rust}\n`)) throw Error("TOOL_VERSION: Rust release mismatch");
  const host=rustc.stdout.match(/^host: (.+)$/m)?.[1];
  if(!host || !/(apple-darwin|unknown-linux)/.test(host)) throw Error(`PLATFORM: unsupported measurement host ${host}`);
  const cov=await run([...rust,"cargo","llvm-cov","--version"],"MISSING_TOOL cargo-llvm-cov");
  requireVersion(cov.stdout,`cargo-llvm-cov ${PINS.cargoLlvmCov}`,"cargo-llvm-cov");
  const sysroot=(await run([...rust,"rustc","--print","sysroot"],"MISSING_TOOL sysroot")).stdout.trim();
  const llvm=resolve(sysroot,"lib/rustlib",host,"bin");
  const versions:Record<string,string>={bun:Bun.version,rustc:rustc.stdout,cargoLlvmCov:cov.stdout,host};
  for(const name of ["llvm-cov","llvm-profdata"]) {
    if(!existsSync(join(llvm,name))) throw Error(`MISSING_TOOL: ${name}; run mise run coverage:provision`);
    const receipt=await run([join(llvm,name),"--version"],`MISSING_TOOL ${name}`);
    const rustLlvm=rustc.stdout.match(/LLVM version: (\S+)/)?.[1];
    if(!rustLlvm || !receipt.stdout.includes(rustLlvm)) throw Error(`TOOL_VERSION: ${name} must match Rust LLVM ${rustLlvm}`);
    versions[name]=receipt.stdout;
  }
  save("versions.json",versions);
  const overrides=["RUSTFLAGS","CARGO_ENCODED_RUSTFLAGS","LLVM_COV_FLAGS","LLVM_PROFDATA_FLAGS","RUSTC_WRAPPER","RUSTC_WORKSPACE_WRAPPER","CARGO_BUILD_TARGET",
    ...Object.keys(process.env).filter(k=>/^CARGO_(?:PROFILE_|TARGET_.*RUSTFLAGS)/.test(k))];
  for(const name of overrides) {
    if(process.env[name]) throw Error(`CONFIGURATION: unset ${name} for the pinned measurement`);
  }
  const metadata=JSON.parse((await run([...rust,"cargo","metadata","--format-version","1","--locked","--offline"],"METADATA_FAILURE")).stdout);
  const names=metadata.packages.filter((p:any)=>metadata.workspace_members.includes(p.id)).map((p:any)=>p.name).sort();
  if(JSON.stringify(names)!==JSON.stringify([...CRATES].sort())) throw Error("MISSING_CRATE: workspace members differ from accounting contract");
  save("metadata.json",metadata);
  // Disable the tool's broad tests-directory filter: path-included production
  // modules may contain tests/../src in their recorded filename. Only exact
  // non-workspace dependency and compiler-source roots are filtered upstream.
  const escape=(s:string)=>s.replace(/[.*+?^${}()|[\]\\]/g,"\\$&");
  const dependencyRoots=metadata.packages.filter((p:any)=>!metadata.workspace_members.includes(p.id)).map((p:any)=>dirname(p.manifest_path));
  for(const dep of dependencyRoots) if(root===dep || root.startsWith(dep+"/")) throw Error(`CONFIGURATION: dependency root overlaps workspace ${dep}`);
  const compilerHash=rustc.stdout.match(/^commit-hash: (.+)$/m)?.[1];
  const ignoredRoots=[sysroot,`/rustc/${compilerHash}`,...dependencyRoots];
  const ignore=ignoredRoots.map(p=>`^${escape(p)}(/|$)`).join("|");
  save("non-workspace-roots.json",ignoredRoots);
  // A new run owns its target and profiles; no prior artifact can satisfy it.
  const target=resolve(root,"target/arc-coverage",id);mkdirSync(target,{recursive:true});
  const coverageEnv={CARGO_LLVM_COV_TARGET_DIR:target,LLVM_PROFILE_FILE_NAME:"arc-%p-%m.profraw",CARGO_LLVM_COV_DENY_WARNINGS:"1",LLVM_COV:join(llvm,"llvm-cov"),LLVM_PROFDATA:join(llvm,"llvm-profdata")};
  console.log("Coverage: running workspace unit, integration and child-binary tests");
  await run([...rust,"cargo","llvm-cov","--workspace","--all-targets","--locked","--offline","--no-report","--no-cfg-coverage","--no-cfg-coverage-nightly"],"TEST_FAILURE",coverageEnv,0,120000);
  // Explicit binary probes supply independently identifiable child PIDs/profile
  // files in addition to the existing integration suites' inherited profiles.
  const childProof=[];
  for(const [binary,args,expected,extra] of [
    ["arcctl",["--help"],0,{}],
    ["arc-daemon",[],1,{ARC_SOCKET_PATH:"relative-invalid-coverage-socket",ARC_DB_PATH:join(dir,"probe.sqlite3")}],
  ] as [string,string[],number,Record<string,string>][]) {
    const receipt=await run([join(target,"debug",binary),...args],`CHILD_FAILURE ${binary}`,{...coverageEnv,...extra,LLVM_PROFILE_FILE:join(target,"arc-%p-%m.profraw")},expected);
    const profiles=readdirSync(target).filter(p=>p.startsWith(`arc-${receipt.pid}-`)&&p.endsWith(".profraw"));
    if(!profiles.length) throw Error(`CHILD_PROFILE: no profile for ${binary} pid ${receipt.pid}`);
    for(const profile of profiles) if(!statSync(join(target,profile)).size) throw Error(`CHILD_PROFILE: empty ${profile}`);
    childProof.push({binary,pid:receipt.pid,profiles,exit:receipt.exit});
  }
  const profiles=readdirSync(target).filter(p=>p.endsWith(".profraw")).sort();
  if(!profiles.length) throw Error("MISSING_REPORT: no raw profiles");
  console.log("Coverage: merging fresh execution profiles and validating reports");
  const report=join(dir,"raw.lcov"), started=Date.now();
  await run([...rust,"cargo","llvm-cov","report","--lcov","--output-path",report,"--failure-mode","any","--disable-default-ignore-filename-regex","--ignore-filename-regex",ignore,"--verbose"],"REPORT_FAILURE",coverageEnv);
  const llvmReport=join(dir,"raw.json");
  await run([...rust,"cargo","llvm-cov","report","--json","--output-path",llvmReport,"--failure-mode","any","--disable-default-ignore-filename-regex","--ignore-filename-regex",ignore,"--verbose"],"REPORT_FAILURE",coverageEnv);
  // The pinned tool writes the exact llvm-profdata -f input list. Check that
  // every collected profile, including explicit child PIDs, entered this merge.
  const lists=readdirSync(target).filter(p=>p.endsWith("-profraw-list"));
  if(lists.length!==1) throw Error("CHILD_PROFILE: missing merge input list");
  const merge=readFileSync(join(target,lists[0]),"utf8").trim().split("\n").sort();
  if(JSON.stringify(merge)!==JSON.stringify(profiles.map(p=>join(target,p)).sort())) throw Error("CHILD_PROFILE: merge input mismatch");
  save("profiles.json",{target,childProof,merge,profiles:profiles.map(p=>({path:p,sha256:hash(readFileSync(join(target,p))),size:statSync(join(target,p)).size}))});
  const after=inputIdentity(root);
  const result=validateReport({path:report,llvmPath:llvmReport,started,before:input.sha256,after:after.sha256,inventory,root,testExit:0});
  measured=true;
  save("summary.json",{...result,runId:id,acceptedBaseline:BASELINE,host,inputs:input,versions,childProof});
  writeFileSync(join(dir,"uncovered.md"),result.files.map(f=>`${f.path}: ${f.covered}/${f.total}; uncovered: ${f.uncovered.join(", ")||"none"}`).join("\n")+"\n");
  for(const crate of result.crates) console.log(`${crate.pass?"PASS":"FAIL"} ${crate.crate}: ${crate.covered}/${crate.total} production lines (${crate.percent.toFixed(2)}%; requires 80%)`);
  console.log(`Coverage reports: ${relative(root,dir)}`);
  enforce(result);
  save("receipt.json",{schemaVersion:1,runId:id,pass:true,exit:0,acceptedBaseline:BASELINE,sourceHash:input.sha256,receipts});
  writeFileSync(resolve(root,EVIDENCE,"baseline/latest.json"),JSON.stringify({runId:id,directory:relative(root,dir),sourceHash:input.sha256,pass:true})+"\n");
} catch(error) {
  const message=String(error);
  save("receipt.json",{schemaVersion:1,runId:id,pass:false,exit:1,error:message,acceptedBaseline:BASELINE,sourceHash:input?.sha256,receipts});
  writeFileSync(resolve(root,EVIDENCE,"baseline/latest.json"),JSON.stringify({runId:id,directory:relative(root,dir),sourceHash:input?.sha256,pass:false})+"\n");
  console.error(`Coverage unavailable or failed: ${message}\nEvidence: ${relative(root,dir)}`);
  if(!measured) for(const crate of CRATES) console.error(`UNAVAILABLE ${crate}: no valid current measurement`);
  process.exitCode=1;
}
