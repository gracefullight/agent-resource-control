// One current-mission checker. No historical report is recertified, no command
// is executed, and no pending/failed gate can stand in for a completed receipt.
import { readFileSync, readdirSync, lstatSync, realpathSync } from "node:fs";
import { resolve, relative } from "node:path";
import { CRATES, METER_SRS_SHA256 } from "./config";
import { commandEvidence, evidenceRefs, readEvidence, type EvidenceRef } from "./evidence";
import { reuseCoverage } from "./coverage-receipts";
import { gitBytes } from "./hygiene";
import { hash, requireInputIdentity, type inputIdentity } from "./source";
import { parseCargoExecution } from "../archive/harness/managed-exec-test-output";
import { isMeterSnapshot } from "../../apps/desktop/src/bridge/meter-contract";

const HEAD = "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de", DIR = ".arc-verify/host-meter";
export const HOST_METER_BUN = [
  ...["bridge-client", "contracts", "credits", "dashboard", "lifecycle", "meter"].map(n=>`apps/desktop/tests/${n}.test.ts`),
  ...["accounting", "desktop-foundation", "gate", "hygiene", "terminal-outcomes", "traffic-accounting"].map(n=>`scripts/coverage/${n}.test.ts`),
  ...["equivalence", "traffic-targets", "coverage-reuse"].map(n=>`scripts/harness/${n}.test.ts`),
];
const COMMANDS = {
  format:["cargo","fmt","--all","--","--check"], clippy:["cargo","clippy","--workspace","--all-targets","--","-D","warnings"],
  workspaceTest:["cargo","test","--workspace"], coverage:["bun","run","test:coverage"],
  architecture:["./scripts/harness/validate.sh"], activeBun:["bun","test",...HOST_METER_BUN],
};
function need(condition:unknown, message:string):asserts condition { if(!condition) throw Error(`HOST_METER: ${message}`); }
const equal = (a:unknown,b:unknown) => JSON.stringify(a)===JSON.stringify(b);
const finite = (n:unknown):n is number => typeof n==="number" && Number.isFinite(n);
const integer = (n:unknown):n is number => finite(n) && Number.isSafeInteger(n) && n>=0;
const close = (a:number,b:number) => finite(a) && finite(b) && Math.abs(a-b)<=1e-6;
type Identity = {pid:number;start_token:string;boot_id:string};
type Process = {pid:number;startToken:string;bootId:string;executable:string;cpuCoreSeconds:number;rssBytes:number;source:string};
export type Observation = {atMs:number;monotonicMs:number;process:Process;desktopProcess?:Process;windows?:{pid:number;normalOnscreenCount:number;source:string}};
const same = (p:Process,i:Identity) => p.pid===i.pid && p.startToken===i.start_token && p.bootId===i.boot_id;
export function idleAccounting(rows:Observation[]) {
  need(rows.length>=301,"at least 301 independent idle observations required");
  const first=rows[0], last=rows.at(-1)!, elapsedMs=last.monotonicMs-first.monotonicMs;
  need(finite(elapsedMs) && elapsedMs>=300_000,"idle interval below 300 seconds");
  for(let i=0;i<rows.length;i++) {
    const row=rows[i], p=row.process, prev=rows[i-1];
    need(finite(row.atMs) && finite(row.monotonicMs) && finite(p.cpuCoreSeconds) && p.cpuCoreSeconds>=0 && Number.isSafeInteger(p.rssBytes) && p.rssBytes>0,"invalid observer values");
    need(p.pid===first.process.pid && p.startToken===first.process.startToken && p.bootId===first.process.bootId &&
      p.source.includes("proc_pid_rusage(RUSAGE_INFO_V4)"),"observer identity/provenance changed");
    if(prev) need(row.monotonicMs>prev.monotonicMs && row.monotonicMs-prev.monotonicMs<=5000 && p.cpuCoreSeconds>=prev.process.cpuCoreSeconds,"observer reset/gap");
  }
  const deltaCoreSeconds=last.process.cpuCoreSeconds-first.process.cpuCoreSeconds;
  const percent=deltaCoreSeconds/(elapsedMs/1000)*100, maxBytes=Math.max(...rows.map(r=>r.process.rssBytes));
  need(percent<0.5 && maxBytes<50_000_000,"idle CPU/RSS numeric bound failed");
  return {elapsedMs,deltaCoreSeconds,percent,maxBytes};
}
type HostReference = {atMs:number;host:{cpu:{user:number;system:number;nice:number;idle:number;logicalCpuCount:number;source:string};memory:{availableBytes:number;source:string};pressureRaw:number;swapUsedBytes:number;filesystem:{capacityBytes:number;freeBytes:number;availableBytes:number;path:string;mountPoint:string;filesystemType:string;source:string}}};
type Calibration = {cli:EvidenceRef;hostBefore:EvidenceRef;hostAfter:EvidenceRef;process:EvidenceRef;comparisons:Record<string,{difference:number|null;explanation:string}>};
type Native = {
  sourceHash:string; artifacts:Record<"daemon"|"cli"|"observer"|"desktop"|"daemonBuild"|"observerBuild"|"desktopBuild"|"noninstrumentation",EvidenceRef>;
  calibration:{status:string;independentReferences:boolean;discrepanciesExplained:boolean;samples:Calibration[]};
  idle:{status:string;warmed:boolean;noninstrumented:boolean;desktopHidden:boolean;durationMeetsMinimum:boolean;independentObserver:boolean;
    warmupMs:number;daemonIdentity:Identity;desktopIdentity:Identity;observerReceipt:EvidenceRef;visibility:EvidenceRef;
    cpu:{denominator:string;limitPercent:number;belowLimit:boolean;percent:number;deltaCoreSeconds:number};rss:{limitBytes:number;belowLimit:boolean;maxBytes:number};durationMs:number};
};
export function nativeEvidence(root:string, native:Native, sourceHash:string) {
  need(native.sourceHash===sourceHash,"stale native inputs");
  const artifact=native.artifacts, path=(name:keyof typeof artifact)=>resolve(root,artifact[name].path);
  for(const ref of Object.values(artifact)) readEvidence(root,ref);
  const command=(ref:EvidenceRef,argv?:string[])=>{
    const result=commandEvidence(root,ref,sourceHash,argv);
    need(realpathSync(resolve(root,result.receipt.cwd))===realpathSync(root),"native command workspace mismatch"); return result;
  };
  const build=command(artifact.daemonBuild,["cargo","build","--release","--locked","--offline","-p","arc-daemon","-p","arc-cli","--bins"]);
  need(!build.receipt.environment?.RUSTFLAGS && !build.receipt.environment?.LLVM_PROFILE_FILE && !build.receipt.environment?.CARGO_ENCODED_RUSTFLAGS,"instrumented daemon build");
  need(artifact.daemon.path==="target/release/arc-daemon" && artifact.cli.path==="target/release/arcctl","unexpected noninstrumented profile");
  const inspection=command(artifact.noninstrumentation,["nm","-a",path("daemon")]);
  need(inspection.stdout.length>0 && !/__llvm_(?:profile|cov|prf)/.test(inspection.stdout),"LLVM-instrumented or uninspected daemon");
  const observerBuild=command(artifact.observerBuild), desktopBuild=command(artifact.desktopBuild);
  need(observerBuild.receipt.argv.includes("scripts/host-meter/native-observer.c") && observerBuild.receipt.argv.at(-1)===artifact.observer.path,"observer build/source mismatch");
  need(desktopBuild.receipt.argv.includes("host-meter-desktop") && desktopBuild.receipt.argv.includes("tauri/custom-protocol"),"actual embedded desktop build missing");
  const calibration=native.calibration;
  need(calibration.status==="measured" && calibration.independentReferences && calibration.discrepanciesExplained && calibration.samples.length>0,"missing calibration");
  for(const row of calibration.samples) {
    const sample=JSON.parse(command(row.cli,[path("cli"),"meter","snapshot","--json"]).stdout);
    need(isMeterSnapshot(sample),"invalid calibrated snapshot");
    const before:HostReference=JSON.parse(command(row.hostBefore,[path("observer"),"host"]).stdout);
    const after:HostReference=JSON.parse(command(row.hostAfter,[path("observer"),"host"]).stdout);
    const proc:Observation=JSON.parse(command(row.process,[path("observer"),"observe",String(sample.daemon_instance.pid),"1","1000"]).stdout);
    need(same(proc.process,sample.daemon_instance) && proc.process.executable===path("daemon"),"calibration process provenance");
    need(sample.sampled_at_ms!==null && before.atMs<=sample.sampled_at_ms && after.atMs>=sample.sampled_at_ms && after.atMs-before.atMs<=40_000,"unbounded calibration timing");
    need(Math.abs(proc.atMs-sample.sampled_at_ms)<=sample.cadence.interval_ms+2000 && after.host.memory.source.includes("HOST_VM_INFO64") && after.host.filesystem.source.includes("statfs(/)"),"unbounded/missing native reference provenance");
    const a=before.host.cpu,b=after.host.cpu;
    for(const c of [a,b]) need([c.user,c.system,c.nice,c.idle,c.logicalCpuCount].every(integer) && c.logicalCpuCount>0 && c.logicalCpuCount<=4_294_967_295,"invalid native CPU counter");
    need(b.user>=a.user && b.system>=a.system && b.nice>=a.nice && b.idle>=a.idle,"invalid independent host counter reset");
    const busy=(b.user-a.user)+(b.system-a.system)+(b.nice-a.nice), total=busy+b.idle-a.idle;
    need(integer(total) && total>0 && integer(busy) && busy<=total && a.logicalCpuCount===b.logicalCpuCount && a.source.includes("host_processor_info") && b.source.includes("host_processor_info"),"invalid independent host interval");
    need(integer(after.host.pressureRaw) && [1,2,4].includes(after.host.pressureRaw),"invalid memory pressure reference");
    need(sample.host.cpu_interval?.logical_cpu_count===b.logicalCpuCount && sample.host.filesystem.identity?.path===after.host.filesystem.path && sample.host.filesystem.identity?.mount_point===after.host.filesystem.mountPoint && sample.host.filesystem.identity?.filesystem_type===after.host.filesystem.filesystemType,"calibration topology/filesystem mismatch");
    const fields:Record<string,[unknown,unknown]>={
      hostCpu:[sample.host.cpu_utilization,busy/total*100], memoryAvailable:[sample.host.memory_available_bytes,after.host.memory.availableBytes],
      memoryPressure:[sample.host.memory_pressure,({1:"normal",2:"warning",4:"critical"} as Record<number,string>)[after.host.pressureRaw]],
      swapUsed:[sample.host.swap_used_bytes,after.host.swapUsedBytes], capacity:[sample.host.filesystem.capacity_bytes,after.host.filesystem.capacityBytes],
      free:[sample.host.filesystem.free_bytes,after.host.filesystem.freeBytes], available:[sample.host.filesystem.available_bytes,after.host.filesystem.availableBytes],
      daemonCpu:[sample.daemon.cpu_core_seconds,proc.process.cpuCoreSeconds], daemonRss:[sample.daemon.rss_bytes,proc.process.rssBytes],
      sampledPeakVsCurrentReference:[sample.daemon.observed_peak_rss_bytes,proc.process.rssBytes],
    };
    for(const [name,[measurement,reference]] of Object.entries(fields)) {
      const field=measurement as {unit:string;reading:{availability:string;value:unknown}}, value=field.reading, comparison=row.comparisons[name];
      need(value.availability==="available" && typeof comparison?.explanation==="string" && comparison.explanation.trim().length>=20,`missing actual comparison ${name}`);
      if(name==="memoryPressure") need(field.unit==="level" && ["normal","warning","critical"].includes(value.value as string) && comparison.difference===null,`incorrect discrepancy ${name}`);
      else {
        need(["bytes","percent","core_seconds"].includes(field.unit) && finite(reference) && reference>=0 && reference<=Number.MAX_SAFE_INTEGER &&
          (field.unit!=="bytes" || integer(reference)) && (field.unit!=="percent" || reference<=100),`invalid numeric reference ${name}`);
        need(finite(value.value) && finite(value.value-reference) && close(comparison.difference!,value.value-reference),`incorrect discrepancy ${name}`);
      }
    }
  }
  const idle=native.idle;
  need(idle.status==="measured" && idle.warmed && idle.noninstrumented && idle.desktopHidden && idle.durationMeetsMinimum && idle.independentObserver && finite(idle.warmupMs) && idle.warmupMs>0,"missing idle evidence");
  const observed=command(idle.observerReceipt), rows:Observation[]=observed.stdout.trim().split("\n").map(line=>JSON.parse(line));
  need(equal(observed.receipt.argv.slice(0,3),[path("observer"),"observe",String(idle.daemonIdentity.pid)]) && observed.receipt.argv[5]===String(idle.desktopIdentity.pid),"independent observer invocation mismatch");
  const actual=idleAccounting(rows);
  need(rows.every(r=>same(r.process,idle.daemonIdentity) && r.process.executable===path("daemon") && r.desktopProcess && same(r.desktopProcess,idle.desktopIdentity) && r.desktopProcess.executable===path("desktop") && r.windows?.pid===idle.desktopIdentity.pid && r.windows.normalOnscreenCount===0),"desktop/daemon native identity or visibility mismatch");
  need(rows[0].atMs-Number(idle.daemonIdentity.start_token)/1000>=idle.warmupMs,"unwarmed daemon interval");
  const visibility=readEvidence(root,idle.visibility)!.toString().trim().split("\n").map(line=>JSON.parse(line));
  need(visibility[0]?.phase==="before_hide" && visibility[0]?.isVisible===true,"actual desktop window never observed");
  const hidden=visibility.slice(1);
  need(hidden.length>=301 && hidden.every((v,i)=>v.pid===idle.desktopIdentity.pid && v.runtime==="production_wry" && v.windowLabel==="main" && v.isVisible===false &&
    finite(v.atMs) && finite(v.monotonicMs) && (!i || (v.monotonicMs>hidden[i-1].monotonicMs && v.monotonicMs-hidden[i-1].monotonicMs<=5000))) && hidden[0].atMs<=rows[0].atMs && hidden.at(-1).atMs>=rows.at(-1)!.atMs,"native hidden lifecycle interval incomplete");
  need(idle.cpu.denominator==="one_core" && idle.cpu.limitPercent===0.5 && idle.cpu.belowLimit && close(idle.cpu.percent,actual.percent) && close(idle.cpu.deltaCoreSeconds,actual.deltaCoreSeconds) && close(idle.durationMs,actual.elapsedMs),"CPU denominator/accounting mismatch");
  need(idle.rss.limitBytes===50_000_000 && idle.rss.belowLimit && idle.rss.maxBytes===actual.maxBytes,"RSS accounting mismatch");
  return actual;
}

type Gate = {status:string;receipt:EvidenceRef};
// Exact operator-supplied correction, not a discovered container inventory.
export const HOST_METER_MISSION_CONTEXT = Object.freeze({
  authority:"operator-supplied task-13 mission context", acceptedHead:HEAD,
  originalInputIdentity:"1548cd6c23598e1081ceed7602f805ac41730cdc60df9aad0241af54e7797851",
  directive:"The supplied mission context has no container/service observation policy.",
});
type ContainerConfig = {completionRequirementsSatisfied:boolean;applicability:string;policyContext:EvidenceRef;configuration?:{authority:string;receipt:EvidenceRef};
  targets:{id:string;immutableIdentity:string;[key:string]:unknown}[]};
// The live caller supplies this fixed operator context, never a report-provided
// applicability assertion. Alternate trusted contexts are deterministic fixtures.
export function containerTargetsEvidence(root:string, config:ContainerConfig, sourceHash:string, trustedMissionContext:object=HOST_METER_MISSION_CONTEXT) {
  need(equal(JSON.parse(readEvidence(root,config.policyContext)!.toString()),trustedMissionContext),"container policy authority mismatch");
  need(config.completionRequirementsSatisfied===true && Array.isArray(config.targets),"container completion unresolved");
  if(!Object.hasOwn(trustedMissionContext,"observationPolicy")) {
    need(equal(trustedMissionContext,HOST_METER_MISSION_CONTEXT) && config.applicability==="not_applicable" && config.targets.length===0 && !config.configuration,"unknown/contradictory absent container policy");
    return {applicability:"not_applicable",basis:HOST_METER_MISSION_CONTEXT.directive};
  }
  const policy=(trustedMissionContext as {observationPolicy?:{targets:{id:string;immutableIdentity:string}[]}}).observationPolicy;
  need(policy && Array.isArray(policy.targets) && policy.targets.length>0 && policy.targets.every(t=>typeof t.id==="string" && t.id.length>0 && typeof t.immutableIdentity==="string" && t.immutableIdentity.length>0) && new Set(policy.targets.map(t=>t.id)).size===policy.targets.length,"unknown container policy");
  const authority=(trustedMissionContext as {authority?:string}).authority;
  need(config.applicability==="required" && authority && config.configuration?.authority===authority && config.configuration.receipt,"container configuration unresolved");
  const command=(ref:EvidenceRef)=>{const result=commandEvidence(root,ref,sourceHash);need(realpathSync(resolve(root,result.receipt.cwd))===realpathSync(root),"container observation workspace mismatch");return result;};
  need(equal(JSON.parse(command(config.configuration.receipt).stdout),policy),"container configuration mismatch");
  need(equal(config.targets.map(({id,immutableIdentity})=>({id,immutableIdentity})),policy.targets),"container identity/inventory mismatch");
  for(const target of config.targets) for(const name of ["health","availability","recentErrors"]) {
    const observed=target[name] as Gate|undefined;need(observed?.status==="passed",`container ${name} incomplete`);
    const result=command(observed.receipt), value=JSON.parse(result.stdout);
    need(value.targetId===target.id && value.immutableIdentity===target.immutableIdentity && value.kind===name && integer(value.atMs) &&
      value.atMs>=Date.parse(result.receipt.startedAt) && value.atMs<=Date.parse(result.receipt.finishedAt),`container ${name} identity/time mismatch`);
    need(name==="recentErrors"?integer(value.value):typeof value.value==="boolean",`invalid container ${name} observation`);
  }
  return {applicability:"required",targetCount:config.targets.length};
}
export function evidenceInventory(root:string, candidate=`${DIR}/gates.json`) {
  root=realpathSync(root);
  need(candidate===`${DIR}/gates.json` || (candidate.startsWith("target/host-meter/") && !candidate.includes("\\") && !candidate.split("/").some(p=>!p || p===".." || p===".")),"invalid candidate path");
  const directory=resolve(root,DIR), candidatePath=resolve(root,candidate);
  need(lstatSync(directory).isDirectory() && realpathSync(directory)===directory,"non-canonical evidence directory");
  need(lstatSync(candidatePath).isFile() && realpathSync(candidatePath)===candidatePath,"non-canonical evidence candidate");
  const allowed=["contract.md","experience.md","value.md","native.json","gates.json","qa.json","final.md"];
  const files=readdirSync(resolve(root,DIR)); let bytes=0;
  for(const name of files) {const path=resolve(root,DIR,name); need(allowed.includes(name) && lstatSync(path).isFile() && realpathSync(path)===path,`unexpected evidence ${name}`);bytes+=readFileSync(path).length;}
  const prospective={fileCount:files.length+(files.includes("gates.json")?0:1),bytes:bytes-(files.includes("gates.json")?readFileSync(resolve(directory,"gates.json")).length:0)+readFileSync(candidatePath).length};
  need(prospective.fileCount<=7 && prospective.bytes<=5_000_000,"evidence budget exceeded");
  return {observed:{fileCount:files.length,bytes},prospective};
}
export function hostMeterEvidence(root:string, candidate=`${DIR}/gates.json`) {
  root=realpathSync(root);
  const inventory=evidenceInventory(root,candidate);
  const report=JSON.parse(readFileSync(resolve(root,candidate),"utf8"));
  const inputs=requireInputIdentity(root,report.inputs as ReturnType<typeof inputIdentity>);
  need(report.baseline.acceptedHead===HEAD && inputs.files["docs/requirements/ARC_SRS_2026-11-06.md"]===METER_SRS_SHA256,"baseline/SRS mismatch");
  need(report.requirements.sourceSha256===METER_SRS_SHA256 && report.requirements.destinationSha256===METER_SRS_SHA256 && report.requirements.actualBytesVerified===true,"unverified SRS");
  need(hash(readFileSync("/Users/gracefullight/Downloads/Agent_Resource_Control_ARC_Requirements_SRS_2026-11-06.md"))===METER_SRS_SHA256,"authoritative SRS changed/unavailable");
  const completed:Record<string,ReturnType<typeof commandEvidence>>={};
  for(const [name,argv] of Object.entries(COMMANDS)) {
    const gate=report.gates[name] as Gate; need(gate.status==="passed",`unfinished gate ${name}`);
    completed[name]=commandEvidence(root,gate.receipt,inputs.sha256,argv);
    need(resolve(root,completed[name].receipt.cwd)===root,`gate workspace mismatch ${name}`);
  }
  need(report.gates.allSixPassed===true,"six gates incomplete");
  need(report.contract.satisfied===true && report.tests.deterministic.status==="passed" && equal(report.tests.deterministic.requiredCases,
    ["counter-deltas","normalization","reset-wrap","clock-changes","partial-failures","sampling-bounds"]),"deterministic contract incomplete");
  need(report.tests.components.status==="passed" && equal(report.tests.components.requiredStates,["fresh","stale","unavailable"]) && report.dashboard.preservedBehaviorsVerified===true,"Dashboard evidence incomplete");
  need(report.sampling.boundsVerified===true && report.sampling.headlessAndAdaptiveVerified===true && report.compatibility.protocol1FailOpenAndExecutionVerified===true,"sampler/compatibility evidence incomplete");
  need(report.tests.native.successiveNativeSamplesAtLeast10===true && ["disconnectReconnect","daemonRestart","oldDaemonUnsupported"].every(name=>report.tests.native[name]==="passed"),"native recovery evidence incomplete");
  const rust=parseCargoExecution(completed.workspaceTest.stdout);
  need(rust.ignored===0 && rust.tests.every(t=>t.status==="ok"),"workspace tests incomplete");
  for(const name of ["successive_native_samples_headless_cli_router_and_restart","accepted_old_daemon_is_unsupported_without_disabling_health_or_jobs","partial_native_failures_preserve_available_fields_cli_health_and_jobs","registered_meter_preserves_partial_failures_and_working_legacy_reads"])
    need(rust.tests.some(t=>t.name.endsWith(name)),`missing executed native case ${name}`);
  for(const path of HOST_METER_BUN) need((completed.activeBun.stdout+completed.activeBun.stderr).includes(`${path}:`),`missing Bun suite ${path}`);
  need(/\b0 fail\b/.test(completed.activeBun.stdout+completed.activeBun.stderr),"Bun failures/unexecuted tests");
  const reused=reuseCoverage(root,report.coverage.reuse.receipt.path);
  need(report.coverage.productionLineFloorPercent===80 && equal(report.coverage.crates,CRATES) && report.coverage.allCratesMeetFloor===true && report.coverage.collectionCount===1 &&
    report.coverage.reuse.currentInputMatch===true && report.coverage.reuse.harnessValidated===true && report.coverage.reuse.recollected===false &&
    completed.architecture.stdout.includes(`Coverage: validated fresh receipt ${reused.runId}; no collection`) && completed.architecture.receipt.environment?.ARC_COVERAGE_REUSE_RECEIPT===reused.receipt.path,"coverage reuse/collection proof mismatch");
  for(const name of ["typecheck","check","build","smoke:tauri"]) {
    const check=report.desktopCompatibility.commands[name] as Gate; need(check.status==="passed",`desktop ${name} incomplete`);
    commandEvidence(root,check.receipt,inputs.sha256,["bun","run",`desktop:${name}`]);
  }
  need(report.desktopCompatibility.allPassed===true,"desktop compatibility incomplete");
  const native:Native=JSON.parse(readFileSync(resolve(root,`${DIR}/native.json`),"utf8"));
  const idle=nativeEvidence(root,native,inputs.sha256);
  for(const artifact of report.tests.native.artifacts as EvidenceRef[]) readEvidence(root,artifact);
  const legacy=JSON.parse(readEvidence(root,report.tests.native.legacyArtifact)!.toString()).legacyBuild;
  need(legacy.acceptedHead===HEAD && legacy.exitCode===0 && equal(legacy.argv,["cargo","build","--locked","--offline","-p","arc-daemon","-p","arc-cli","--bins"]),"invalid legacy compatibility build");
  for(const ref of evidenceRefs(legacy)) readEvidence(root,ref);
  need(legacy.archive.sha256===hash(gitBytes(root,["archive","--format=tar",HEAD,"Cargo.toml","Cargo.lock","rust-toolchain.toml","crates","apps/desktop"])),"legacy peer source differs from accepted HEAD");
  const successive=JSON.parse(readEvidence(root,report.tests.native.successiveArtifact)!.toString());
  need(successive.successiveSamples.length>=10 && successive.headlessObservedBeforeRouter===true && successive.defaultProductionLimits===true,"missing successive/headless evidence");
  successive.successiveSamples.forEach((row:{cli:unknown},i:number)=>{ need(isMeterSnapshot(row.cli),"invalid native case sample"); if(i) {const prev=successive.successiveSamples[i-1].cli; need(row.cli.sequence===prev.sequence+1 && row.cli.sampled_monotonic_ms>prev.sampled_monotonic_ms,"cached/skipped native samples");} });
  const migrations=gitBytes(root,["ls-tree","-r","--name-only",HEAD,"--","crates/arc-storage/migrations"]).toString().trim().split("\n");
  for(const path of migrations) need(hash(readFileSync(resolve(root,path)))===hash(gitBytes(root,["show",`${HEAD}:${path}`])),`migration changed ${path}`);
  need(report.compatibility.migrationChecksumsPreserved===true,"migration receipt missing");
  const baseline=new Set(gitBytes(root,["ls-tree","-r","--name-only",HEAD,"--","crates","scripts","apps/desktop"]).toString().trim().split("\n"));
  for(const path of Object.keys(inputs.files).filter(p=>!baseline.has(p) && /\.(rs|ts|tsx|jsx|css|scss|c|h)$/.test(p)))
    need(readFileSync(resolve(root,path),"utf8").trimEnd().split("\n").length<500,`new source/test >=500 lines ${path}`);
  containerTargetsEvidence(root,report.containerTargets,inputs.sha256);
  for(const ref of evidenceRefs({report,native})) readEvidence(root,ref);
  return {sourceHash:inputs.sha256,gateCount:6,nativeCases:rust.executed,coverageRun:reused.runId,idle,evidence:{...inventory.prospective,observed:inventory.observed},candidate:relative(root,resolve(root,candidate))};
}
if(import.meta.main) {
  try { console.log(JSON.stringify(hostMeterEvidence(process.cwd(),process.argv[2]),null,2)); }
  catch(error) { console.error(String(error)); process.exitCode=1; }
}
