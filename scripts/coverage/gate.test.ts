import { test, expect } from "bun:test";
import { readFileSync, writeFileSync, utimesSync, unlinkSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fixture } from "./fixtures";
import { commandEvidence, readEvidence } from "./evidence";
import { requireProfiles } from "./coverage-receipts";
import { hash } from "./source";
import { desktopEvidence } from "./desktop-evidence";
import { activeBunEvidence, desktopSectionsEvidence } from "./desktop-operations";

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

function evidenceFixture() {
  const f=fixture();
  const save=(path:string,value:string|object)=>{
    const bytes=Buffer.from(typeof value==="string"?value:JSON.stringify(value));
    mkdirSync(dirname(join(f.root,path)),{recursive:true});writeFileSync(join(f.root,path),bytes);
    return {path,sha256:hash(bytes),bytes:bytes.length};
  };
  const identity:{files:Record<string,string>;sha256:string}={files:{"source.rs":hash("fixture source")},sha256:""};
  identity.sha256=hash(JSON.stringify(identity.files));
  const input={sha256:identity.sha256,manifest:save("target/proof/inputs.json",identity)};
  const command={argv:["cargo","fmt","--all","--","--check"],cwd:".",status:"passed",exitCode:0,termination:null,
    startedAt:"2026-11-11T18:00:00Z",finishedAt:"2026-11-11T18:00:01Z",durationMs:1000,timeoutMs:7_200_000,
    inputsUnchanged:true,inputBefore:input,inputAfter:input,stdout:save("target/proof/stdout.log",""),stderr:save("target/proof/stderr.log","")};
  return {...f,save,identity,command};
}

test("raw evidence rejects tampering, escapes, symlinks and missing current logs",()=>{
  const f=evidenceFixture();
  try {
    const ref=f.save("target/proof/data","native bytes");
    expect(readEvidence(f.root,ref)?.toString()).toBe("native bytes");
    expect(()=>readEvidence(f.root,{...ref,path:"target/../data"})).toThrow("invalid reference");
    expect(()=>readEvidence(f.root,{...ref,path:join(f.root,ref.path)})).toThrow("invalid reference");
    expect(()=>readEvidence(f.root,{...ref,bytes:undefined})).toThrow("missing byte size");
    symlinkSync(join(f.root,ref.path),join(f.root,"target/proof/link"));
    expect(()=>readEvidence(f.root,{...ref,path:"target/proof/link"})).toThrow("non-canonical");
    symlinkSync(join(f.root,"target/proof"),join(f.root,"target/alias"));
    expect(()=>readEvidence(f.root,{...ref,path:"target/alias/data"})).toThrow("non-canonical");
    writeFileSync(join(f.root,ref.path),"altered");
    expect(()=>readEvidence(f.root,ref)).toThrow("hash/size mismatch");
    unlinkSync(join(f.root,ref.path));
    expect(readEvidence(f.root,ref,{optional:true})).toBeUndefined();
    expect(()=>readEvidence(f.root,ref)).toThrow();
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("completed receipts reject failed commands, short timeouts, altered argv and stale input manifests",()=>{
  const f=evidenceFixture(), ref=()=>f.save("target/proof/check.receipt.json",f.command);
  try {
    expect(commandEvidence(f.root,ref(),f.identity.sha256,f.command.argv).receipt.exitCode).toBe(0);
    for(const change of [{exitCode:17},{status:"failed"},{timeoutMs:120000},{finishedAt:"invalid"},{inputsUnchanged:false}]) {
      expect(()=>commandEvidence(f.root,f.save("target/proof/bad.receipt.json",{...f.command,...change}),f.identity.sha256)).toThrow();
    }
    expect(()=>commandEvidence(f.root,ref(),f.identity.sha256,["cargo","test"])).toThrow("unexpected argv");
    expect(()=>commandEvidence(f.root,ref(),"0".repeat(64))).toThrow("stale command");
    const input=f.save("target/proof/bad-inputs.json",{...f.identity,files:{"source.rs":"0".repeat(64)}});
    expect(()=>commandEvidence(f.root,f.save("target/proof/bad.receipt.json",{...f.command,inputAfter:{sha256:f.identity.sha256,manifest:input}}),f.identity.sha256)).toThrow("corrupt aggregate");
    writeFileSync(join(f.root,f.command.stdout.path),"tampered");
    expect(()=>commandEvidence(f.root,ref(),f.identity.sha256)).toThrow("hash/size mismatch");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

// All logs below are synthetic validator inputs. They never establish mission
// acceptance or execute/relabel a native, smoke, or component gate.
function desktopSectionsFixture() {
  const f=evidenceFixture(), project=resolve(import.meta.dir,"../..");
  for(const path of ["package.json","apps/desktop/package.json","apps/desktop/src-tauri/tauri.conf.json","bun.lock",
    ...["dashboard","credits","contracts","lifecycle"].map(name=>`apps/desktop/tests/${name}.test.ts`)]) {
    const bytes=readFileSync(join(project,path));
    mkdirSync(dirname(join(f.root,path)),{recursive:true});writeFileSync(join(f.root,path),bytes);
    f.identity.files[path]=hash(bytes);
  }
  f.identity.files=Object.fromEntries(Object.entries(f.identity.files).sort(([a],[b])=>a.localeCompare(b)));
  f.identity.sha256=hash(JSON.stringify(f.identity.files));
  const input={sha256:f.identity.sha256,manifest:f.save("target/proof/desktop-inputs.json",f.identity)};
  const command=(id:string,argv:string[],cwd:string,stdout="",stderr="")=>f.save(`target/proof/${id}.receipt.json`,{
    ...f.command,argv,cwd,inputBefore:input,inputAfter:input,
    stdout:f.save(`target/proof/${id}.stdout.log`,stdout),stderr:f.save(`target/proof/${id}.stderr.log`,stderr),
  });
  const cargo=(names:string[],filtered=0)=>`Synthetic validator fixture; not an executed native test\nrunning ${names.length} tests\n${names.map(name=>`test ${name} ... ok`).join("\n")}\ntest result: ok. ${names.length} passed; 0 failed; 0 ignored; 0 measured; ${filtered} filtered out; finished in 0.01s\n`;
  const cases=(path:string)=>[...readFileSync(join(project,path),"utf8").matchAll(/#\[test\]\n(?:#\[[^\n]*\]\n)*fn (\w+)\(/g)].map(match=>match[1]);
  const native=[...cases("apps/desktop/src-tauri/tests/lifecycle.rs"),
    ...["input_validation","local_lifecycle","protocol_validation","status_validation","wire_projection"]
      .flatMap(name=>cases(`apps/desktop/src-tauri/tests/registered_commands_support/${name}.rs`))];
  const daemon=["records_exit_zero","records_exit_seventeen","disconnect_reconnect","rejects_malformed_response","rejects_mismatched_job_id","daemon_survives_desktop_exit","active_child_survives_desktop_exit"];
  const compatibility=[...cases("crates/arc-daemon/tests/traffic_cli/terminal_outcomes.rs"),...cases("crates/arc-ipc/tests/jobs.rs"),
    ...cases("crates/arc-storage/tests/managed_exec_migration.rs"),...cases("crates/arc-ipc/src/client/access_tests.rs")];
  const files=["dashboard","credits","contracts","lifecycle"].map(name=>`apps/desktop/tests/${name}.test.ts`);
  const componentCases=files.map(path=>{
    const source=readFileSync(join(project,path),"utf8");
    const describe=source.match(/describe\("([^"]+)"/)![1];
    // Formatting may place the test name on the next line. Include that
    // identity so fabricated logs still exercise the unchanged role validator.
    return {path,names:[...source.matchAll(/^\s*test\(\s*"([^"]+)"/gm)].map(match=>`${describe} > ${match[1]}`)};
  });
  const bun=(groups=componentCases)=>`Synthetic validator fixture; not executed components\n${groups.map(group=>`${group.path}:\n${group.names.map(name=>`(pass) ${name} [1.00ms]`).join("\n")}`).join("\n")}\n ${groups.reduce((sum,g)=>sum+g.names.length,0)} pass\n 0 fail\nRan ${groups.reduce((sum,g)=>sum+g.names.length,0)} tests across ${groups.length} files. [1.00s]\n`;
  const report={smoke:{
    frozenInstall:{status:"passed",receipt:command("frozen",["bun","install","--frozen-lockfile"],".","Checked 77 installs across 81 packages (no changes)"),
      bunLockBefore:f.identity.files["bun.lock"],bunLockAfter:f.identity.files["bun.lock"],lockUnchanged:true},
    typescript:{status:"passed",major:7,version:"7.0.2",receipt:command("ts",["bun","run","desktop:typecheck"],".")},
    vite:{status:"passed",receipt:command("vite",["bun","run","desktop:build"],".","vite v8.3.8 building client environment for production...\n✓ built in 100ms\n")},
    tauri:{status:"passed",receipt:command("tauri",["bun","run","desktop:smoke:tauri"],".","",'Finished `dev` profile [unoptimized + debuginfo] target(s) in 1s\nBuilt application at: target/debug/arc-desktop\n')},
  },tests:{
    components:{status:"passed",receipt:command("components",["bun","test","apps/desktop/tests"],".","",bun())},
    tauriHandlers:{status:"passed",receipt:command("handlers",["cargo","test","-p","arc-desktop"],".",cargo(native))},
    daemonBridge:{status:"passed",realDaemon:true,cases:Object.fromEntries(["exit0","exit17","disconnectReconnect","malformedResponse","mismatchedJobId","daemonSurvival","activeChildSurvival"].map(name=>[name,"passed"])),
      receipt:command("daemon",["cargo","test","-p","arc-desktop","--test","daemon_bridge"],".",cargo(daemon))},
  },compatibility:{status:"passed",receipt:command("compatibility",["cargo","test","--workspace"],".",cargo(compatibility))}};
  return {...f,command,cargo,bun,native,daemon,compatibility,componentCases,report};
}
test("synthetic valid role-specific and shared-workspace receipts qualify without duplicate native runs",()=>{
  const f=desktopSectionsFixture();
  try {
    expect(()=>desktopSectionsEvidence(f.root,f.report,f.identity.sha256,true)).not.toThrow();
    const report=structuredClone(f.report);
    report.tests.components.receipt=f.command("app-components",["bun","run","test"],"apps/desktop","",f.bun(f.componentCases.map(group=>({...group,path:group.path.replace("apps/desktop/","")}))));
    expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).not.toThrow();
    const shared=f.command("shared-workspace",["cargo","test","--workspace"],".",f.cargo([...new Set([...f.native,...f.daemon,...f.compatibility])]));
    for(const section of [f.report.tests.tauriHandlers,f.report.tests.daemonBridge,f.report.compatibility]) section.receipt=shared;
    expect(()=>desktopSectionsEvidence(f.root,f.report,f.identity.sha256,true)).not.toThrow();
    // Other exact script entry points resolve to the same delivered operations.
    f.report.smoke.typescript.receipt=f.command("app-typecheck",["bun","run","typecheck"],"apps/desktop");
    f.report.smoke.vite.receipt=f.command("forward-vite",["bun","run","--cwd","apps/desktop","build"],".","vite v8.3.8 building production...\n✓ built in 10ms\n");
    expect(()=>desktopSectionsEvidence(f.root,f.report,f.identity.sha256,true)).not.toThrow();
    const components=f.report.tests.components as any;
    delete components.receipt;
    components.receipts=[
      f.command("dashboard-credits",["bun","test",f.componentCases[0].path,f.componentCases[1].path],".","",f.bun(f.componentCases.slice(0,2))),
      ...f.componentCases.slice(2).map(group=>f.command(group.path.split("/").at(-1)!,["bun","test",group.path],".","",f.bun([group]))),
    ];
    expect(()=>desktopSectionsEvidence(f.root,f.report,f.identity.sha256,true)).not.toThrow();
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("synthetic receipt substitutions cannot turn successful bun test into smoke, native or compatibility proof",()=>{
  const f=desktopSectionsFixture();
  try {
    const substitute=f.command("unrelated-success",["bun","test"],".","",f.bun());
    for(const [group,key] of [["smoke","frozenInstall"],["smoke","typescript"],["smoke","vite"],["smoke","tauri"],["tests","tauriHandlers"],["tests","daemonBridge"],["self","compatibility"]]) {
      const report:any=structuredClone(f.report), parent=group==="self"?report:report[group];
      parent[key].receipt=substitute;
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
    }
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("completed synthetic evidence requires every mandatory section and both parent groups",()=>{
  const f=desktopSectionsFixture();
  try {
    for(const path of ["smoke","tests","smoke.frozenInstall","smoke.typescript","smoke.vite","smoke.tauri","tests.components","tests.tauriHandlers","tests.daemonBridge","compatibility"]) {
      const report:any=structuredClone(f.report), keys=path.split("."), key=keys.pop()!;
      delete (keys.length?report[keys[0]]:report)[key];
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
    }
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("synthetic role evidence rejects wrong cwd/argv, compile/list-only and omitted executed identities",()=>{
  const f=desktopSectionsFixture();
  try {
    for(const [group,key] of [["smoke","frozenInstall"],["smoke","typescript"],["smoke","vite"],["smoke","tauri"],["tests","components"],["tests","tauriHandlers"],["tests","daemonBridge"],["self","compatibility"]]) {
      const report:any=structuredClone(f.report), section=(group==="self"?report:report[group])[key];
      const command=JSON.parse(readFileSync(join(f.root,section.receipt.path),"utf8"));
      section.receipt=f.save("target/proof/wrong-cwd.receipt.json",{...command,cwd:"unrelated"});
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow("argv/cwd");
    }
    for(const change of [
      {group:"smoke",key:"typescript",argv:["bun","run","desktop:typecheck"],cwd:"apps/desktop"},
      {group:"smoke",key:"vite",argv:["bun","run","desktop:typecheck"],cwd:"."},
      {group:"smoke",key:"tauri",argv:["cargo","check","-p","arc-desktop"],cwd:"."},
      {group:"tests",key:"tauriHandlers",argv:["cargo","test","-p","arc-desktop","--no-run"],cwd:"."},
      {group:"tests",key:"daemonBridge",argv:["cargo","test","--workspace","--","--list"],cwd:"."},
      {group:"self",key:"compatibility",argv:["cargo","test","-p","arc-desktop"],cwd:"."},
    ]) {
      const report:any=structuredClone(f.report);
      (change.group==="self"?report:report[change.group])[change.key].receipt=f.command("wrong-role",change.argv,change.cwd,f.cargo([...f.native,...f.daemon,...f.compatibility]));
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
    }
    for(const [key,names] of [["tauriHandlers",f.native],["daemonBridge",f.daemon],["compatibility",f.compatibility]] as const) {
      const report:any=structuredClone(f.report), section=key==="compatibility"?report.compatibility:report.tests[key];
      section.receipt=f.command("omitted-tests",["cargo","test","--workspace"],".",f.cargo(["unrelated_test"]));
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
      section.receipt=f.command("filtered-tests",["cargo","test","--workspace"],".",f.cargo([...names],1));
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
    }
    const report:any=structuredClone(f.report);
    report.tests.components.receipt=f.command("unrelated-components",["bun","test"],".","",f.bun([{path:"unrelated.test.ts",names:["unrelated test"]}]));
    expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("synthetic receipts must execute every registered lifecycle, daemon and component case",()=>{
  const f=desktopSectionsFixture();
  try {
    for(const [key,names] of [["tauriHandlers",f.native],["daemonBridge",f.daemon],["compatibility",f.compatibility.slice(0,6)]] as const) {
      for(const omitted of names) {
        const report:any=structuredClone(f.report), section=key==="compatibility"?report.compatibility:report.tests[key];
        const all=[...new Set([...f.native,...f.daemon,...f.compatibility])].filter(name=>name!==omitted);
        section.receipt=f.command("missing-identity",["cargo","test","--workspace"],".",f.cargo(all));
        expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow("required native test did not execute");
      }
    }
    for(const group of f.componentCases) for(const omitted of group.names) {
      const report=structuredClone(f.report), cases=f.componentCases.map(g=>({...g,names:g===group?g.names.filter(name=>name!==omitted):g.names}));
      report.tests.components.receipt=f.command("missing-component",["bun","test","apps/desktop/tests"],".","",f.bun(cases));
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow("required component did not execute");
    }
    for(const mutation of [
      (r:any)=>{r.smoke.frozenInstall.bunLockAfter="0".repeat(64)},
      (r:any)=>{r.smoke.frozenInstall.bunLockBefore=r.smoke.frozenInstall.bunLockAfter="0".repeat(64)},
      (r:any)=>{r.smoke.typescript.major=6},
      (r:any)=>{r.tests.daemonBridge.realDaemon=false},
      (r:any)=>{delete r.tests.daemonBridge.cases.exit17},
      (r:any)=>{r.smoke.vite.receipt=f.command("no-vite-summary",["bun","run","desktop:build"],".")},
      (r:any)=>{r.smoke.tauri.receipt=f.command("no-native-summary",["bun","run","desktop:smoke:tauri"],".","vite v8.3.8 built in 1s")},
    ]) {
      const report=structuredClone(f.report);mutation(report);
      expect(()=>desktopSectionsEvidence(f.root,report,f.identity.sha256,true)).toThrow();
    }
    writeFileSync(join(f.root,"apps/desktop/package.json"),JSON.stringify({scripts:{typecheck:"bun test"},devDependencies:{typescript:"7.0.2"}}));
    expect(()=>desktopSectionsEvidence(f.root,f.report,f.identity.sha256,true)).toThrow("unsupported delivered script");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
test("completed synthetic active Bun evidence requires finished exact full-discovery executed results",()=>{
  const f=desktopSectionsFixture();
  try {
    const gate={status:"passed",fullDiscovery:true,receipt:f.command("active-bun",["bun","test"],".","",f.bun())};
    expect(()=>activeBunEvidence(f.root,gate,f.identity)).not.toThrow();
    for(const change of [{status:"pending"},{status:"running"},{status:"failed"},{receipt:undefined},{fullDiscovery:false}]) {
      expect(()=>activeBunEvidence(f.root,{...gate,...change},f.identity)).toThrow();
    }
    for(const [argv,cwd,output] of [
      [["bun","test","apps/desktop/tests"],".",f.bun()],
      [["bun","test"],"apps/desktop",f.bun()],
      [["bun","test"],".",f.bun(f.componentCases.slice(1))],
      [["bun","test"],".",f.bun().replace(/\d+ pass/,"1 pass")],
      [["bun","test"],".",f.bun().replace("0 fail","1 fail")],
      [["bun","test"],".",f.bun()+"\n 1 skip\n"],
    ] as [string[],string,string][]) {
      expect(()=>activeBunEvidence(f.root,{...gate,receipt:f.command("bad-active",argv,cwd,"",output)},f.identity)).toThrow();
    }
    const finished=JSON.parse(readFileSync(join(f.root,gate.receipt.path),"utf8"));
    for(const change of [{status:"running",exitCode:null,finishedAt:null},{status:"failed",exitCode:17}]) {
      const receipt=f.save("target/proof/unfinished-active.receipt.json",{...finished,...change});
      expect(()=>activeBunEvidence(f.root,{...gate,receipt},f.identity)).toThrow();
    }
    writeFileSync(join(f.root,"target/proof/active-bun.stderr.log"),"tampered");
    expect(()=>activeBunEvidence(f.root,gate,f.identity)).toThrow("hash/size mismatch");
    f.save(finished.stderr.path,f.bun());
    unlinkSync(join(f.root,finished.stdout.path));
    expect(()=>activeBunEvidence(f.root,gate,f.identity)).toThrow();
    f.save(finished.stdout.path,"");
    writeFileSync(join(f.root,gate.receipt.path),"tampered final receipt");
    expect(()=>activeBunEvidence(f.root,gate,f.identity)).toThrow("hash/size mismatch");
    unlinkSync(join(f.root,gate.receipt.path));
    expect(()=>activeBunEvidence(f.root,gate,f.identity)).toThrow();
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("child-profile receipts require both native child PIDs, every raw hash and the exact merge list",()=>{
  const f=evidenceFixture();
  try {
    // Synthetic binder fixture, never a measured production profile.
    const target=join(f.root,"target/arc-coverage/fixture");
    const profiles=[11,22].map(pid=>{
      const path=`arc-${pid}-fixture.profraw`, ref=f.save(`target/arc-coverage/fixture/${path}`,"synthetic profile");
      return {path,sha256:ref.sha256,size:ref.bytes};
    });
    const merge=profiles.map(p=>join(target,p.path));
    const proof={target,profiles,merge,mergeList:f.save("target/arc-coverage/fixture/list-profraw-list",merge.join("\n")+"\n"),childProof:[
      {binary:"arcctl",pid:11,profiles:[profiles[0].path],exit:0},{binary:"arc-daemon",pid:22,profiles:[profiles[1].path],exit:1}]};
    expect(()=>requireProfiles(f.root,proof)).not.toThrow();
    expect(()=>requireProfiles(f.root,{...proof,merge:merge.slice(1)})).toThrow("merge input mismatch");
    expect(()=>requireProfiles(f.root,{...proof,childProof:proof.childProof.slice(1)})).toThrow("missing owned binary");
    expect(()=>requireProfiles(f.root,{...proof,childProof:proof.childProof.map(c=>({...c,pid:33}))})).toThrow("PID profiles");
    writeFileSync(merge[0],"tampered profile");
    expect(()=>requireProfiles(f.root,proof)).toThrow("hash/size mismatch");
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

// Synthetic small-checker fixtures. Fabricated outcomes and logs below never
// establish native gate execution or mission acceptance.
function desktopLogFixture() {
  const f=evidenceFixture();
  const report={
    gates:Object.fromEntries(["format","clippy","workspaceTest","coverage","architecture","activeBun"]
      .map(name=>[name,{status:"pending"}])),
    rawLogs:[f.save("target/proof/native.stdout.log","Synthetic output; not an executed gate\n")],
    // Receipts and registries are opaque to the small checker. Their command
    // validation remains covered independently above.
    commands:{path:"target/proof/commands.json",sha256:"0".repeat(64),bytes:0},
  };
  const saveReport=()=>f.save(".arc-verify/desktop-foundation/gates.json",report);
  saveReport();
  return {...f,report,saveReport};
}

test("synthetic small desktop checker verifies direct logs without opening receipts or registries",()=>{
  const f=desktopLogFixture();
  try {
    const inspected=desktopEvidence(f.root);
    expect(inspected.verifiedLogs).toEqual([f.report.rawLogs[0].path]);
    expect(inspected.gateOutcomes).toEqual(Object.fromEntries(Object.keys(f.report.gates).map(name=>[name,"pending"])));
    for(const status of ["running","passed","failed"]) {
      f.report.gates.activeBun.status=status;f.saveReport();
      expect(desktopEvidence(f.root).gateOutcomes.activeBun).toBe(status);
    }
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("synthetic small desktop checker requires gates.json and every recorded gate outcome",()=>{
  const f=desktopLogFixture();
  try {
    for(const name of Object.keys(f.report.gates)) {
      const gate=f.report.gates[name];
      delete f.report.gates[name];f.saveReport();
      expect(()=>desktopEvidence(f.root)).toThrow(`missing gate outcome ${name}`);
      f.report.gates[name]=gate;
    }
    for(const status of ["",undefined,null,17]) {
      f.save(".arc-verify/desktop-foundation/gates.json",{
        ...f.report,gates:{...f.report.gates,format:{status}},
      });
      expect(()=>desktopEvidence(f.root)).toThrow("missing gate outcome format");
    }
    f.save(".arc-verify/desktop-foundation/gates.json","malformed JSON");
    expect(()=>desktopEvidence(f.root)).toThrow();
    unlinkSync(join(f.root,".arc-verify/desktop-foundation/gates.json"));
    expect(()=>desktopEvidence(f.root)).toThrow();
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("synthetic small desktop checker rejects absent or tampered logs even while active Bun runs",()=>{
  const f=desktopLogFixture(), ref=f.report.rawLogs[0];
  try {
    f.report.gates.activeBun.status="running";f.saveReport();
    writeFileSync(join(f.root,ref.path),"tampered raw output");
    expect(()=>desktopEvidence(f.root)).toThrow("hash/size mismatch");
    unlinkSync(join(f.root,ref.path));
    expect(()=>desktopEvidence(f.root)).toThrow();
  } finally {rmSync(f.root,{recursive:true,force:true});}
});

test("synthetic small desktop checker checks nested raw logs and enforces target paths and SHA-256",()=>{
  const f=desktopLogFixture();
  try {
    const stderr=f.save("target/proof/stderr.txt","Synthetic non-JSON stderr");
    f.save(".arc-verify/desktop-foundation/gates.json",{
      ...f.report,gates:{...f.report.gates,format:{status:"passed",output:{stderr}}},
    });
    expect(desktopEvidence(f.root).verifiedLogs).toContain(stderr.path);
    writeFileSync(join(f.root,stderr.path),"tampered stderr");
    expect(()=>desktopEvidence(f.root)).toThrow("hash/size mismatch");
    for(const ref of [
      f.save("outside.stdout.log","outside target"),
      {...f.report.rawLogs[0],path:"target/../outside.stdout.log"},
      {...f.report.rawLogs[0],sha256:undefined},
    ]) {
      f.save(".arc-verify/desktop-foundation/gates.json",{...f.report,rawLogs:[ref]});
      expect(()=>desktopEvidence(f.root)).toThrow("invalid reference");
    }
    const {bytes,...withoutSize}=f.report.rawLogs[0];
    f.save(".arc-verify/desktop-foundation/gates.json",{...f.report,rawLogs:[withoutSize]});
    expect(desktopEvidence(f.root).verifiedLogs).toEqual([withoutSize.path]);
  } finally {rmSync(f.root,{recursive:true,force:true});}
});
