import { inventory, remainingAllocation } from "./exec-fail-open-inventory";
//! Capacity/integrity fixture: real subprocess capture and production orchestration,
//! explicit simulated leaves. Its records are never production acceptance.
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, rmSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { directory, reference, load, hash, requireValue, baseline, baselineTree, boundJSON, canonicalBytes, artifact, root as productRoot } from "./exec-fail-open-inputs";
import { fixtureCatalog, fixtureRecipe } from "./exec-fail-open-fixture-recipe";
import { capturePending } from "./exec-fail-open-receipts";
import { archiveCanonical, archiveBytes, compactReferences, rejectedCollectionSha256, rejectedIntermediateSha256, rejectedIntermediatePath, rejectedAssemblyPath, rejectedAssemblySha256, rejectedLatestSha256, rejectedLatestPath, retainedPredecessor } from "./exec-fail-open-compact";
import { decodeContainer, selectedBytes } from "./exec-fail-open-container";
import { sealCollection } from "./exec-fail-open-collection";
import { assemble } from "./exec-fail-open-assemble";
import { validateGates } from "./exec-fail-open-gates";
import { validateEvidence } from "./exec-fail-open-evidence";
import { scenarios } from "./exec-fail-open-native";
export async function compactFirstRun(run: (root: string, state: any) => void | Promise<void>, options: { combined?: boolean; unrelatedSuccessor?: boolean } = {}) {
  const root = mkdtempSync(resolve(tmpdir(), "arc-compact-flow-"));
  const save = (path: string, value: any) => {
    mkdirSync(resolve(root, path, ".."), { recursive: true });
    writeFileSync(resolve(root, path), typeof value === "string" ? value : JSON.stringify(value,null,2)+"\n"); return reference(root,path);
  };
  const git = (...args: string[]) => { const r=spawnSync("git",args,{cwd:root,encoding:"utf8"}); requireValue(r.status===0,r.stderr); };
  const count = () => spawnSync("git",["ls-files","--",directory],{cwd:root,encoding:"utf8"}).stdout.trim().split("\n").filter(Boolean).length;
  const capture = (argv: string[], name: string, actor = "backend", pending = "target/exec-fail-open/pending") => capturePending(argv,name,pending,actor,root,()=>fixtureRecipe(root).context());
  try {
    git("init","--quiet"); save("inputs.json",{fixtureOnly:true});
    const worker=resolve(root,"fixture-worker.ts"), bun=Bun.which("bun")!;
    save("fixture-worker.ts",`import {readFileSync,mkdirSync,writeFileSync,existsSync} from 'node:fs';import {createHash} from 'node:crypto';import {spawnSync} from 'node:child_process';
if(process.argv[2]==='0'&&!existsSync('target/exec-fail-open/failed-once')){mkdirSync('target/exec-fail-open',{recursive:true});writeFileSync('target/exec-fail-open/failed-once','fixture');process.exit(7);}
const child=spawnSync(process.execPath,['-e',"process.stdout.write('fixture-child-once');process.exit(17)"],{encoding:'utf8'});if(child.status!==17||child.stdout!=='fixture-child-once')throw Error('fixture child');
const s=${JSON.stringify(scenarios)};const input=readFileSync('inputs.json');const out={fixtureOnly:true,inputSha256:createHash('sha256').update(input).digest('hex'),test:'fixture::prerequisite',childExit:child.status,invocations:1,passed:true};
if(process.argv[2]==='0'){const base='target/exec-fail-open/native/'+process.pid;mkdirSync(base,{recursive:true});for(const id of [...s,'negative-control'])writeFileSync(base+'/'+id+'.json',JSON.stringify({fixtureOnly:true,scenario:id,test:out.test,recordedUnixMs:Date.now(),candidateBinaryPath:${JSON.stringify(worker)},candidateSha256:createHash('sha256').update(readFileSync(${JSON.stringify(worker)})).digest('hex'),boundary:'fixture subprocess elapsed observation',samples:{startNs:0,endNs:1000000},parentSpawnToInvocationMs:1,subdivisionReason:'Simulated fixture leaf, never native timing proof',counts:{},events:[],proxyClock:'fixture-only',commitRows:[],invocations:1,recoveryInvocations:1,observedExit:17,passed:true}));}console.log(JSON.stringify(out));`);
    const argvs=Array.from({length:11},(_,i)=>[bun,worker,String(i)]), names=argvs.map((_,i)=>`prerequisite-${i}`);
    save("fixture-recipe.json",{fixtureOnly:true,compact:true,scenarios,workspaceArgv:argvs[0],context:{fixtureOnly:true,schemaVersion:1,baselineHead:baseline,baselineTree,currentInputs:{},observedGit:{fixtureOnly:true},host:{fixtureOnly:true},tools:{fixtureOnly:true}}});
    save(`${directory}/contract.json`,{fixtureOnly:true,receiptDependencyGraph:{nodes:fixtureCatalog},commandPhases:{prerequisiteCommands:argvs}});
    for(const name of ["contract.md","design-contract.md"])save(`${directory}/${name}`,"Fixture-only evidence. ".repeat(12));
    save(`${directory}/claims-review.md`,"Fixture only; desktop, native Windows, vendor hooks and scheduling remain outstanding and business results unmeasured. ".repeat(3));
    for(const n of ["gates","timing","historical","preservation"])save(`${directory}/implementation/${n}.json`,{kind:"retained-prior-failure",passed:false,fixtureOnly:true});
    const initialFiles=8;
    for(let i=initialFiles;i<140;i++)save(`${directory}/retained-${i}.json`,{fixtureOnly:true,retained:true});
    git("add",directory); requireValue(count()===140,"fixture must start at exactly 140 retained files");
    const allocation=inventory(root,remainingAllocation);requireValue(allocation.retained===140&&allocation.added===8&&allocation.projected===148,"compact allocation is not 140+8=148");
    const older=Object.fromEntries(["gates","timing","historical","preservation"].map(n=>{const p=`${directory}/implementation/${n}.json`;return[p,canonicalBytes(root,p)];}));
    const archive=archiveCanonical(root);git("add",directory);
    for(const [path,bytes] of Object.entries(older))requireValue(archiveBytes(root,archive.entries[path]).equals(bytes),"archive round trip failed");
    let previousEntry: any, previousValue: any, previousRefs: any[], prior: any;
    if (options.combined) {
      for (const n of ["gates","timing","historical","preservation"]) save(`${directory}/implementation/${n}.json`, `{"fixtureOnly":true,"kind":"attempt-8-${n}","passed":false}\r\n`);
      save(`${directory}/implementation/task9-compact-repair.json`,{fixtureOnly:true,kind:"repair",passed:true});
      git("add",directory); requireValue(count()===142,"fixture must start with 142 indexed retained files");
      const priorPath=`${directory}/implementation/collection.json`;
      const priorBytes=readFileSync(resolve(productRoot,priorPath));
      requireValue(hash(priorBytes)==="7c791e5cd0dc228962974ac589d329cc54ce9245d42b909ced309d5cad04d529","original collection changed");
      writeFileSync(resolve(root,priorPath),priorBytes,{flag:"wx"});
      const before=inventory(root), priorRef=reference(root,priorPath);
      let rejectedUnindexed=false;
      try { compactReferences(root,priorRef); } catch(e) { rejectedUnindexed=String(e).includes("unstaged container or index bytes/type differ"); }
      requireValue(rejectedUnindexed && before.indexed===142 && before.filesystem===143 && before.retained===143,"unindexed original was accepted or inventory differs");
      const next=["implementation/collection-next.json","qa/review.md","qa/review.json","final.md","final.json","terminal.json"].map(p=>`${directory}/${p}`);
      const projected=inventory(root,next);requireValue(projected.retained===143&&projected.added===6&&projected.projected===149,"combined allocation is not 143+6=149");
      git("add","--",priorPath);
      const staged=spawnSync("git",["show",`:${priorPath}`],{cwd:root,maxBuffer:8*1024*1024});
      const unchanged=staged.status===0 && staged.stdout.equals(priorBytes) && readFileSync(resolve(root,priorPath)).equals(priorBytes);
      previousRefs=compactReferences(root,priorRef);
      const entries=decodeContainer(priorBytes).entries;
      requireValue(unchanged && count()===143 && entries.length===44 && previousRefs.length===44,"staged prior collection differs");
      for(const ref of previousRefs) {
        const entry=entries.find(e=>e.path===ref.selector.path);
        requireValue(entry && artifact(root,ref).equals(Buffer.from(entry.bytes,"base64")),"prior selected entry differs");
      }
      previousEntry=previousRefs[0]; previousValue=JSON.parse(artifact(root,previousEntry).toString());
      prior={indexedBefore:before.indexed,filesystemBefore:before.filesystem,unionBefore:before.retained,rejectedUnindexed,indexedAfter:count(),unchanged,selected:previousRefs.length};
    }
    const originals=Object.fromEntries(["gates","timing","historical","preservation"].map(n=>{const p=`${directory}/implementation/${n}.json`;return[p,canonicalBytes(root,p)];}));
    const retainedBindings=Object.fromEntries(inventory(root).paths.map(path=>[path,hash(canonicalBytes(root,path))]));
    let rejectedPrevious: Buffer | undefined, originalRejected: Buffer | undefined, failedAssembly: Buffer | undefined, latestPrevious: Buffer | undefined;
    if (options.combined) {
      const path=`${directory}/implementation/collection-next.json`, product=retainedPredecessor(productRoot,rejectedLatestSha256,rejectedLatestPath);
      requireValue(hash(product)===rejectedLatestSha256,"latest collection differs"); latestPrevious=product;
      rejectedPrevious=hash(product)===rejectedIntermediateSha256?product:Buffer.from(decodeContainer(product).entries.find(e=>e.scope==="archive"&&e.path===rejectedIntermediatePath&&e.sha256===rejectedIntermediateSha256)!.bytes,"base64");
      requireValue(hash(rejectedPrevious)===rejectedIntermediateSha256,"intermediate container differs");
      originalRejected=selectedBytes(rejectedPrevious,{path,sha256:rejectedCollectionSha256},true);
      const failedPath="target/exec-fail-open/task4-attempt10/post-pending/assemble.json";
      if (existsSync(resolve(productRoot,failedPath))) failedAssembly=Buffer.from(JSON.parse(readFileSync(resolve(productRoot,failedPath),"utf8")).bytes,"base64");
      else failedAssembly=Buffer.from(decodeContainer(product).entries.find(e=>e.scope==="archive"&&e.path===rejectedAssemblyPath&&e.sha256===rejectedAssemblySha256)!.bytes,"base64");
      requireValue(hash(failedAssembly)===rejectedAssemblySha256,"failed assembly receipt differs");
      mkdirSync(resolve(root,"target/exec-fail-open/pending"),{recursive:true});
      writeFileSync(resolve(root,"target/exec-fail-open/pending/rejected-assembly.json"),JSON.stringify({path:rejectedAssemblyPath,sha256:rejectedAssemblySha256,bytes:failedAssembly.toString("base64")}));
      writeFileSync(resolve(root,path),latestPrevious,{flag:"wx"});git("add","--",path);
    }
    const failed=await capture(argvs[0],"failed");requireValue(failed.receipt.exit!==0,"failure unobserved");
    const unrelatedPlain=await capture([bun,"-e","throw Error('unrelated failed fixture attempt')"],"unrelated-plain");requireValue(unrelatedPlain.receipt.exit!==0,"unrelated failure unobserved");
    const extra=await capture(argvs[1],"success-extra");requireValue(extra.receipt.exit===0,"extra success unobserved");
    if (options.unrelatedSuccessor) {
      const unrelated=await capture([bun,"-e","throw Error('unrelated failed fixture attempt')"],"unrelated");
      requireValue(unrelated.receipt.exit!==0,"unrelated failure unobserved");
      const pending=resolve(root,"target/exec-fail-open/pending/unrelated.json"), record=JSON.parse(readFileSync(pending,"utf8"));
      const receipt=JSON.parse(Buffer.from(record.bytes,"base64").toString());
      receipt.successor={path:`${directory}/final.json`,sha256:"0".repeat(64)};
      const bytes=Buffer.from(JSON.stringify(receipt,null,2)+"\n");
      writeFileSync(pending,JSON.stringify({...record,bytes:bytes.toString("base64"),sha256:hash(bytes)}));
    }
    for(let i=0;i<argvs.length;i++){const r=await capture(argvs[i],names[i]);requireValue(r.receipt.exit===0,"fixture prerequisite failed");}
    requireValue(count()===(options.combined?144:141) && inventory(root).retained===(options.combined?144:141),"capture allocated retained command files");
    const collection=sealCollection(root,"target/exec-fail-open/pending",`${directory}/implementation/${options.combined?"collection-next":"collection"}.json`,argvs[0],undefined,options.combined,options.combined);
    const archiveRef=options.combined?collection.ref:archive.ref;
    const manifest=load(root,"fixture-recipe.json");manifest.prerequisite=collection.entries[`${directory}/implementation/commands/${names[0]}.json`];save("fixture-recipe.json",manifest);
    const ctx=fixtureRecipe(root).context(), candidate=hash(canonicalBytes(root,"fixture-worker.ts"));
    save("target/exec-fail-open/history/preparation.json",{fixtureOnly:true,files:{},identity:ctx.sourceHash,tools:[],binary:{path:"fixture-worker.ts",sha256:candidate}});
    const workspace=JSON.parse(Buffer.from(load(root,"target/exec-fail-open/pending/prerequisite-0.json").bytes,"base64").toString());
    save("target/exec-fail-open/history/routes.jsonl",JSON.stringify({fixtureOnly:true,recordedUnixMs:Date.parse(workspace.ended)})+"\n");
    save(".arc-verify/rust-coverage/baseline/latest.json",{pass:true,runId:"fixture-only",directory:".arc-verify/rust-coverage/baseline/runs/fixture-only"});
    save(".arc-verify/rust-coverage/baseline/runs/fixture-only/summary.json",{fixtureOnly:true,sourceHash:ctx.sourceHash,pass:true,childProof:[],crates:["arc-core","arc-detector","arc-storage","arc-ipc","arc-daemon","arc-cli"].map(crate=>({crate,covered:80,total:100}))});
    requireValue(!existsSync(resolve(root,directory,"final.json")),"first-run final preexists");
    let unstagedRejected=false;try{assemble(names,fixtureRecipe(root),{collection:collection.ref,archive:archiveRef},["failed"]);}catch(e){unstagedRejected=String(e).includes("unstaged container");}requireValue(unstagedRejected,"assembler accepted unstaged compact evidence");
    git("add",directory);
    if (options.combined) for(const [path,bytes] of Object.entries(originals))requireValue(archiveBytes(root,collection.entries[path]).equals(bytes),"combined archive round trip failed");
    if (options.unrelatedSuccessor) {
      let rejected=false;
      try { assemble(names,fixtureRecipe(root),{collection:collection.ref,archive:archiveRef},["unrelated"]); }
      catch(e) { rejected=String(e).includes("attempted successor read"); }
      requireValue(rejected,"selected unrelated successor escaped the predecessor guard");
    }
    for (const [attempts,error] of [[["failed","failed"],"duplicate or selected attempt"],[[names[0]],"duplicate or selected attempt"],[["../unsafe"],"unsafe command name"],[["missing"],"missing selected command"],[["success-extra"],"attempt is not a failed command"]] as const) {
      let rejected=false;
      try { assemble(names,fixtureRecipe(root),{collection:collection.ref,archive:archiveRef},[...attempts]); } catch(e) { rejected=String(e).includes(error); }
      requireValue(rejected,"invalid explicit attempts accepted");
    }
    let invalidRejected=false;
    try { assemble(names,{...fixtureRecipe(root),timing:()=>{throw Error("injected failed timing validation");}},{collection:collection.ref,archive:archiveRef},["failed"]); }
    catch(e){if (!String(e).includes("injected failed timing")) throw e; invalidRejected=true;}
    requireValue(invalidRejected,"assembler accepted failed validation");
    for(const [path,bytes] of Object.entries(originals))requireValue(canonicalBytes(root,path).equals(bytes),"failed assembly rewrote original canonical bytes");
    const assembled=assemble(names,fixtureRecipe(root),{collection:collection.ref,archive:archiveRef},["failed"]);
    const ready=validateGates(root,fixtureRecipe(root));
    requireValue(ready.gates.commands.length===11&&boundJSON(root,ready.gates.timings).cases.length===13&&ready.gates.attempts.length===1,"lost compact observations");
    const modulePath=(name:string)=>JSON.stringify(resolve(import.meta.dir,`exec-fail-open-${name}.ts`));
    const prefix=`import {fixtureRecipe} from ${modulePath("fixture-recipe")};const root=${JSON.stringify(root)};const recipe=fixtureRecipe(root);`;
    const terminalArgv=(name:string)=>[bun,"-e",`${prefix}import {${name==='gates'?'validateGates':'validateEvidence'}} from ${modulePath(name)};${name==='gates'?'validateGates':'validateEvidence'}(root,recipe);`];
    const firstGates=await capture(terminalArgv("gates"),"first-gates","backend","target/exec-fail-open/terminal");requireValue(firstGates.receipt.exit===0,"first gates failed");
    const reproduction=await capture(argvs[0],"reproduction","reviewer","target/exec-fail-open/review");
    const report=save(`${directory}/qa/review.md`,`Fixture-only independent reproduction of ${baseline}. `+"This simulated integrity fixture cannot establish product acceptance. ".repeat(5));
    const qa=save(`${directory}/qa/review.json`,{...ctx,kind:"independent-review",actorId:"reviewer",skill:"oma-qa",status:"passed",openBlockers:0,productFilesEdited:false,implementationTestsEdited:false,findings:[],report,reviewedReceipts:[ready.reference,ready.gates.timings,ready.gates.historical,...ready.gates.commands],commands:[reproduction.receipt]});
    const claims=reference(root,`${directory}/claims-review.md`);
    if (!options.combined) save(`${directory}/implementation/task9-compact-repair.json`,{fixtureOnly:true,kind:"repair",passed:true});
    save(`${directory}/final.md`,"Fixture-only final construction; never product acceptance.");
    const final={...ctx,kind:"final-decision",verdict:"pass",openBlockers:0,gates:ready.reference,timings:ready.gates.timings,coverage:ready.gates.coverage,review:qa,reviewReport:report,claimsReview:claims,outstanding:["desktop","native Windows","vendor hooks","scheduling"],limitations:"unmeasured fixture only",evidenceFiles:options.combined?148:147};
    save(`${directory}/final.json`,final);git("add",directory);requireValue(count()===(options.combined?148:147),"sealed compact inventory differs");
    const firstEvidence=await capture(terminalArgv("evidence"),"first-evidence","backend","target/exec-fail-open/terminal");
    requireValue(firstEvidence.receipt.exit===0,canonicalBytes(root,firstEvidence.receipt.output.stderrPath).toString());
    save(`${directory}/terminal.json`,{fixtureOnly:true,commands:[firstGates.receipt,firstEvidence.receipt],passed:true});git("add",directory);
    const terminalGates=await capture(terminalArgv("gates"),"later-gates","backend","target/exec-fail-open/terminal");
    const terminalEvidence=await capture(terminalArgv("evidence"),"later-evidence","backend","target/exec-fail-open/terminal");
    requireValue(terminalGates.receipt.exit===0&&terminalEvidence.receipt.exit===0,"later terminal validation failed");
    const total=options.combined?149:148;
    requireValue(count()===total&&validateEvidence(root,fixtureRecipe(root)).trackedFiles===total,"complete compact inventory differs");
    for (const [path,bytes] of Object.entries(older))requireValue(archiveBytes(root,archive.entries[path]).equals(bytes),"older archive changed");
    for (const [path,digest] of Object.entries(retainedBindings)) if (!(path in originals)) requireValue(hash(canonicalBytes(root,path))===digest,"retained fixture bytes changed");
    await run(root,{git,count,collection,archive,originals,rejectedPrevious,originalRejected,failedAssembly,latestPrevious,previousEntry,previousValue,previousRefs,prior,ready,assembled,names,terminalGates,terminalEvidence,reproduction,final,recipe:fixtureRecipe(root)});
  }finally{rmSync(root,{recursive:true,force:true});}
}
