// Pure synthetic checker inputs. No native process or gate executes here and
// these fixtures cannot qualify as mission performance/calibration evidence.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { EvidenceRef } from "./evidence";
import type { nativeEvidence, Observation } from "./host-meter-evidence";
import { HOST_METER_MISSION_CONTEXT, containerTargetsEvidence, evidenceInventory } from "./host-meter-evidence";
import { mkdirSync, rmSync, symlinkSync } from "node:fs";
type Save=(path:string,value:string|object)=>EvidenceRef;
export function nativeFixture(root:string, sourceHash:string, save:Save,
  command:(name:string,argv:string[],stdout?:string)=>EvidenceRef) {
  const file=(name:string,value:string|object)=>save(`target/native-fixture/${name}`,value);
  const daemon=file("daemon","synthetic binary"), cli=file("cli","synthetic CLI");
  // Required actual profile paths are still enforced in the synthetic root.
  daemon.path="target/release/arc-daemon"; Object.assign(daemon,save(daemon.path,"synthetic binary"));
  cli.path="target/release/arcctl"; Object.assign(cli,save(cli.path,"synthetic CLI"));
  const observer=file("observer","synthetic observer"), desktop=file("desktop","synthetic Wry");
  const path=(ref:EvidenceRef)=>resolve(root,ref.path);
  const sample=JSON.parse(readFileSync(resolve(import.meta.dir,"../../crates/arc-core/tests/fixtures/meter-snapshot.json"),"utf8"));
  sample.sequence=2; sample.sampled_monotonic_ms=15000;
  function normalize(node:unknown):void {
    if(!node || typeof node!=="object") return;
    const row=node as Record<string,unknown>;
    if("unit" in row) row.observed_monotonic_ms=15000;
    else for(const child of Object.values(row)) normalize(child);
  }
  normalize(sample);
  sample.host.cpu_utilization.reading={availability:"available",value:25};
  sample.host.cpu_interval={started_at_ms:sample.sampled_at_ms-15000,ended_at_ms:sample.sampled_at_ms,started_monotonic_ms:0,ended_monotonic_ms:15000,elapsed_ms:15000,logical_cpu_count:4,normalization:"host_capacity"};
  sample.host.memory_available_bytes.reading={availability:"available",value:120_000_000};
  sample.host.memory_pressure.reading={availability:"available",value:"normal"};
  sample.host.swap_used_bytes.reading={availability:"available",value:0};
  sample.host.filesystem.identity={path:"/",mount_point:"/",filesystem_type:"apfs",volume_id:"synthetic-volume",is_local:true};
  for(const [name,value] of Object.entries({capacity_bytes:500_000_000,free_bytes:200_000_000,available_bytes:180_000_000,pressure:"normal"})) sample.host.filesystem[name].reading={availability:"available",value};
  for(const [name,value] of Object.entries({cpu_core_seconds:1,rss_bytes:20_000_000,observed_peak_rss_bytes:22_000_000})) sample.daemon[name].reading={availability:"available",value};
  const i=sample.daemon_instance, base=sample.sampled_at_ms;
  const process={pid:i.pid,startToken:i.start_token,bootId:i.boot_id,executable:path(daemon),cpuCoreSeconds:1,rssBytes:20_000_000,
    source:"proc_pid_rusage(RUSAGE_INFO_V4) fixture"};
  const desktopIdentity={...i,pid:i.pid+1};
  const desktopProcess={...process,pid:desktopIdentity.pid,executable:path(desktop)};
  const before={atMs:base-15000,host:{cpu:{user:100,system:100,nice:0,idle:800,logicalCpuCount:4,source:"host_processor_info fixture"}}};
  const after={atMs:base+20,host:{cpu:{...before.host.cpu,user:200,idle:1000},memory:{availableBytes:100_000_000,source:"HOST_VM_INFO64 synthetic fixture"},pressureRaw:1,swapUsedBytes:0,
    filesystem:{path:sample.host.filesystem.identity.path,mountPoint:sample.host.filesystem.identity.mount_point,filesystemType:"apfs",
      capacityBytes:500_000_000,freeBytes:200_000_000,availableBytes:180_000_000,source:"statfs(/) synthetic fixture"}}};
  const measured={hostCpu:sample.host.cpu_utilization.reading.value,memoryAvailable:sample.host.memory_available_bytes.reading.value,
    memoryPressure:sample.host.memory_pressure.reading.value,swapUsed:sample.host.swap_used_bytes.reading.value,
    capacity:sample.host.filesystem.capacity_bytes.reading.value,free:sample.host.filesystem.free_bytes.reading.value,available:sample.host.filesystem.available_bytes.reading.value,
    daemonCpu:sample.daemon.cpu_core_seconds.reading.value,daemonRss:sample.daemon.rss_bytes.reading.value,sampledPeakVsCurrentReference:sample.daemon.observed_peak_rss_bytes.reading.value};
  const reference={hostCpu:100/300*100,memoryAvailable:100_000_000,memoryPressure:"normal",swapUsed:0,capacity:500_000_000,free:200_000_000,available:180_000_000,
    daemonCpu:1,daemonRss:20_000_000,sampledPeakVsCurrentReference:20_000_000};
  const comparisons=Object.fromEntries(Object.entries(measured).map(([name,value])=>[name,{difference:typeof value==="number"?value-(reference[name as keyof typeof reference] as number):null,
    explanation:"Synthetic validator comparison only; sampled peak is compared against current reference RSS, not a native high-water mark."}]));
  const rows:Observation[]=Array.from({length:301},(_,n)=>({atMs:base+n*1000,monotonicMs:n*1000,process:{...process,cpuCoreSeconds:1+n*0.0001},desktopProcess,
    windows:{pid:desktopIdentity.pid,normalOnscreenCount:0,source:"synthetic CGWindow metadata"}}));
  const visible={pid:desktopIdentity.pid,atMs:base-1000,monotonicMs:0,phase:"before_hide",isVisible:true,runtime:"production_wry",windowLabel:"main"};
  const hidden=Array.from({length:302},(_,n)=>({...visible,atMs:base+n*1000,monotonicMs:(n+1)*1000,phase:n?"hidden_interval":"after_hide",isVisible:false}));
  const native:Parameters<typeof nativeEvidence>[1]={sourceHash,artifacts:{daemon,cli,observer,desktop,
    daemonBuild:command("native-build",["cargo","build","--release","--locked","--offline","-p","arc-daemon","-p","arc-cli","--bins"]),
    observerBuild:command("observer-build",["xcrun","clang","scripts/host-meter/native-observer.c","-o",observer.path]),
    desktopBuild:command("desktop-build",["cargo","build","--example","host-meter-desktop","--features","tauri/custom-protocol"]),
    noninstrumentation:command("nm",["nm","-a",path(daemon)],"synthetic Mach-O symbols; not a real binary inspection")},
    calibration:{status:"measured",independentReferences:true,discrepanciesExplained:true,samples:[{
      cli:command("cli",[path(cli),"meter","snapshot","--json"],JSON.stringify(sample)),
      hostBefore:command("host-before",[path(observer),"host"],JSON.stringify(before)),hostAfter:command("host-after",[path(observer),"host"],JSON.stringify(after)),
      process:command("process",[path(observer),"observe",String(i.pid),"1","1000"],JSON.stringify(rows[0])),comparisons}]},
    idle:{status:"measured",warmed:true,noninstrumented:true,desktopHidden:true,durationMeetsMinimum:true,independentObserver:true,warmupMs:30000,
      daemonIdentity:i,desktopIdentity,observerReceipt:command("idle-observer",[path(observer),"observe",String(i.pid),"301","1000",String(desktopIdentity.pid)],rows.map(row=>JSON.stringify(row)).join("\n")),
      visibility:file("visibility.jsonl",[visible,...hidden].map(row=>JSON.stringify(row)).join("\n")),
      durationMs:300_000,cpu:{denominator:"one_core",limitPercent:0.5,belowLimit:true,percent:0.01,deltaCoreSeconds:0.03},rss:{limitBytes:50_000_000,belowLimit:true,maxBytes:20_000_000}}};
  // Rebind every modified stdout and receipt; rejection must reach measurement
  // validation rather than stop at unrelated hash/size tampering.
  let serial=0;
  const recalibrate=(changes:Record<string,unknown>,differences:Record<string,unknown>={},explanations:Record<string,unknown>={})=>{
    const data=structuredClone({sample,before,after,process:rows[0]}), copy=structuredClone(native);
    const set=(object:object,path:string,value:unknown)=>{
      const keys=path.split("."), last=keys.pop()!;
      let parent=object as Record<string,unknown>;
      for(const key of keys) parent=parent[key] as Record<string,unknown>;
      parent[last]=value;
    };
    for(const [key,value] of Object.entries(changes)) set(data,key,value);
    for(const [key,value] of Object.entries(differences)) set(copy.calibration.samples[0].comparisons,`${key}.difference`,value);
    copy.calibration.samples[0].comparisons.memoryPressure.explanation=`Synthetic comparison: sampled ${data.sample.host.memory_pressure.reading.value}, native raw ${String(data.after.host.pressureRaw)}; separate observations can disagree across the interval.`;
    for(const [key,value] of Object.entries(explanations)) set(copy.calibration.samples[0].comparisons,`${key}.explanation`,value);
    for(const [key,value] of Object.entries(data)) {
      const name=({sample:"cli",before:"hostBefore",after:"hostAfter",process:"process"} as const)[key as keyof typeof data];
      const old=JSON.parse(readFileSync(resolve(root,native.calibration.samples[0][name].path),"utf8"));
      const json=JSON.stringify(value,(_,v)=>typeof v==="number" && !Number.isFinite(v)?"__nonfinite_reference__":v).replaceAll('"__nonfinite_reference__"',"1e309");
      copy.calibration.samples[0][name]=command(`rebound-${serial++}`,old.argv,json);
    }
    return copy;
  };
  return {native,rows,visibility:[visible,...hidden],command,file,recalibrate};
}

export const invalidReferences:unknown[]=["not-a-number","123",null,true,false,[],{},NaN,Infinity,-Infinity,-1,0.5,Number.MAX_SAFE_INTEGER+1];
export const numericReferencePaths={
  memoryAvailable:"after.host.memory.availableBytes",swapUsed:"after.host.swapUsedBytes",capacity:"after.host.filesystem.capacityBytes",
  free:"after.host.filesystem.freeBytes",available:"after.host.filesystem.availableBytes",daemonRss:"process.process.rssBytes",
  sampledPeakVsCurrentReference:"process.process.rssBytes",
};
export function policyCases(root:string,sourceHash:string,save:Save,command:(name:string,argv:string[],stdout?:string)=>EvidenceRef) {
  const context=HOST_METER_MISSION_CONTEXT, required={...context,observationPolicy:{targets:[{id:"api",immutableIdentity:"sha256:fixture-image"}]}};
  let serial=0;
  const config=(trusted:object)=>({completionRequirementsSatisfied:true,applicability:Object.hasOwn(trusted,"observationPolicy")?"required":"not_applicable",
    policyContext:save(`target/policy-${serial++}.json`,trusted),targets:[] as object[]});
  const absent=config(context);
  const observation=(kind:string)=>({targetId:"api",immutableIdentity:"sha256:fixture-image",kind,atMs:Date.parse("2026-11-11T18:00:00.500Z"),value:kind==="recentErrors"?0:true});
  const observed={...config(required),configuration:{authority:required.authority,receipt:command("config",["synthetic","configuration"],JSON.stringify(required.observationPolicy))},targets:[{
    ...required.observationPolicy.targets[0],...Object.fromEntries(["health","availability","recentErrors"].map(name=>[name,{status:"passed",receipt:command(`target-${name}`,["synthetic",name],JSON.stringify(observation(name)))}]))}]};
  const run=(value:object,trusted:object=context)=>()=>containerTargetsEvidence(root,value as Parameters<typeof containerTargetsEvidence>[1],sourceHash,trusted);
  const failedReceipt=save("target/failed-policy-receipt.json",{...JSON.parse(readFileSync(resolve(root,observed.configuration.receipt.path),"utf8")),exitCode:17,status:"failed"});
  return [
    {name:"authoritative absent policy",valid:true,run:run(absent)},
    {name:"required targets with complete receipts",valid:true,run:run(observed,required)},
    {name:"caller cannot replace pinned live context",valid:false,run:run(observed)},
    {name:"unknown context without policy",valid:false,run:run(config({}),{})},
    ...[
      ["empty targets without context",{completionRequirementsSatisfied:true,targets:[]}],
      ["unsupported caller absence assertion",{...absent,policyContext:save("target/caller.json",{authority:"caller",observationPolicy:null})}],
      ["EPERM is not absence",{...absent,policyContext:save("target/denial.json",{error:"EPERM"})}],
      ["missing context",{...absent,policyContext:undefined}],
      ["applicable policy claimed absent",{...observed,applicability:"not_applicable"}],
      ["missing required targets",{...observed,targets:[]}],
      ["missing configuration",{...observed,configuration:undefined}],
      ["wrong configuration authority",{...observed,configuration:{...observed.configuration,authority:"caller"}}],
      ["failed configuration receipt",{...observed,configuration:{...observed.configuration,receipt:failedReceipt}}],
      ["wrong configuration inventory",{...observed,configuration:{...observed.configuration,receipt:command("wrong-config",["synthetic","configuration"],JSON.stringify({targets:[]}))}}],
      ["changed immutable identity",{...observed,targets:[{...observed.targets[0],immutableIdentity:"other"}]}],
      ["duplicate target",{...observed,targets:[observed.targets[0],observed.targets[0]]}],
      ...["health","availability","recentErrors"].flatMap(name=>[
        [`missing ${name}`,{...observed,targets:[{...observed.targets[0],[name]:undefined}]}],
        [`failed ${name}`,{...observed,targets:[{...observed.targets[0],[name]:{status:"failed"}}]}],
        [`failed ${name} command`,{...observed,targets:[{...observed.targets[0],[name]:{status:"passed",receipt:failedReceipt}}]}],
        ...[{targetId:"other"},{immutableIdentity:"other"},{atMs:0},{kind:"unrelated"},{value:null}].map((change,i)=>[
          `wrong ${name} result ${i}`,{...observed,targets:[{...observed.targets[0],[name]:{status:"passed",receipt:command(`invalid-${name}-${i}`,["synthetic",name],JSON.stringify({...observation(name),...change}))}}]},
        ]),
      ]),
    ].map(([name,value])=>({name:String(name),valid:false,run:run(value as object,String(name).includes("context") || ["unsupported caller absence assertion","EPERM is not absence"].includes(String(name))?context:required)})),
    ...[null,{},"unknown",{targets:[]}].map(observationPolicy=>{const trusted={...context,observationPolicy};return {name:`unknown policy ${JSON.stringify(observationPolicy)}`,valid:false,run:run(config(trusted),trusted)};}),
  ];
}

export function inventoryCases(root:string,save:Save) {
  const dir=".arc-verify/host-meter", candidate="target/host-meter/candidate.json";
  const run=(files:Record<string,number>,bytes:number,path=candidate,extra?:()=>void)=>()=>{
    rmSync(resolve(root,dir),{recursive:true,force:true});mkdirSync(resolve(root,dir),{recursive:true});
    for(const [name,count] of Object.entries(files)) save(`${dir}/${name}`,"x".repeat(count));
    save(candidate,"x".repeat(bytes));extra?.();return evidenceInventory(root,path);
  };
  return [
    {name:"absent gates adds candidate",valid:true,expected:[1,7,2,18],run:run({"contract.md":7},11)},
    {name:"replacement counted once",valid:true,expected:[2,10,2,18],run:run({"contract.md":7,"gates.json":3},11)},
    {name:"canonical candidate counted once",valid:true,expected:[2,10,2,10],run:run({"contract.md":7,"gates.json":3},11,`${dir}/gates.json`)},
    {name:"UTF8 bytes rather than characters",valid:true,expected:[1,7,2,9],run:run({"contract.md":7},11,candidate,()=>{save(candidate,"é");})},
    {name:"replacement can reduce prior bytes",valid:true,expected:[2,5_000_001,2,18],run:run({"contract.md":7,"gates.json":4_999_994},11)},
    {name:"exactly five million bytes",valid:true,expected:[1,4_999_989,2,5_000_000],run:run({"contract.md":4_999_989},11)},
    {name:"all seven reserved files",valid:true,expected:[7,21,7,29],run:run(Object.fromEntries(["contract.md","experience.md","value.md","native.json","gates.json","qa.json","final.md"].map(n=>[n,3])),11)},
    {name:"one byte over",valid:false,run:run({"contract.md":4_999_990},11)},
    {name:"reported 4999999 plus 54799",valid:false,run:run({"contract.md":4_999_999},54_799)},
    {name:"unexpected eighth file",valid:false,run:run(Object.fromEntries(["contract.md","experience.md","value.md","native.json","gates.json","qa.json","final.md","unexpected.md"].map(n=>[n,3])),11)},
    {name:"unexpected file before gates publication",valid:false,run:run({"unexpected.md":1},11)},
    {name:"candidate path traversal",valid:false,run:run({},11,"target/host-meter/../host-meter/candidate.json")},
    {name:"symlinked evidence",valid:false,run:run({},11,candidate,()=>symlinkSync(resolve(root,candidate),resolve(root,dir,"contract.md")))},
    {name:"symlinked candidate",valid:false,run:run({},11,"target/host-meter/link.json",()=>symlinkSync(resolve(root,candidate),resolve(root,"target/host-meter/link.json")))},
    {name:"evidence directory entry is not a regular file",valid:false,run:run({},11,candidate,()=>mkdirSync(resolve(root,dir,"contract.md")))},
  ];
}
