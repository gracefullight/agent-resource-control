// Source/host-bound command receipts. Raw outputs stay in ignored local runs.
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve, relative } from "node:path";
import { managedInputs, nativeHost, managedGit } from "../../../scripts/harness/managed-exec-evidence";
import { hash } from "../../../scripts/coverage/source";
import { parseCargoExecution } from "../../../scripts/harness/managed-exec-test-output";
const root=resolve(import.meta.dir,"../../.."), area=resolve(import.meta.dir);
const [label,...argv]=process.argv.slice(2);
if(!label || !argv.length || !/^[a-z0-9-]+$/.test(label)) throw Error("label and exact argv required");
const run=resolve(area,"runs",`${new Date().toISOString().replace(/[:.]/g,"-")}-${label}`);
mkdirSync(run,{recursive:true});
const inputs=managedInputs(root), host=nativeHost(),git=managedGit(root), started=new Date().toISOString();
const child=Bun.spawn(argv,{cwd:root,stdout:"pipe",stderr:"pipe"});
const timer=setTimeout(()=>child.kill("SIGTERM"),3600000);
console.log(`Started ${label}, owned PID ${child.pid}, allowance 3600000ms`);
const [exit,stdout,stderr]=await Promise.all([child.exited,new Response(child.stdout).text(),new Response(child.stderr).text()]);
clearTimeout(timer);
const output={stdoutPath:relative(root,resolve(run,"stdout.txt")),stderrPath:relative(root,resolve(run,"stderr.txt"))};
writeFileSync(resolve(run,"stdout.txt"),stdout);writeFileSync(resolve(run,"stderr.txt"),stderr);
let execution;
try {if(argv[0]==="cargo"&&argv[1]==="test") execution=parseCargoExecution(stdout);}
catch {execution={validation:"failed; inspect actual local libtest output"};}
const receipt={schemaVersion:1,actorId:"backend",sourceHash:inputs.sha256,host,argv,cwd:".",started,ended:new Date().toISOString(),
 exit,status:exit===0?"passed":"failed",timeoutAllowanceMs:3600000,skipFlags:false,pid:child.pid,
 stdoutSha256:hash(stdout),stderrSha256:hash(stderr),output,
 ...(execution?{execution}:{})};
writeFileSync(resolve(run,"receipt.json"),JSON.stringify(receipt,null,2)+"\n");
writeFileSync(resolve(area,`${label}.json`),JSON.stringify(receipt,null,2)+"\n");
writeFileSync(resolve(area,"inputs.json"),JSON.stringify({inputs,git,host},null,2)+"\n");
const unchanged=managedInputs(root).sha256===inputs.sha256;
console.log(`${label}: exit=${exit}, sourceUnchanged=${unchanged}; local output ${relative(root,run)}`);
if(exit!==0) console.log((stdout+stderr).slice(-3500));
process.exit(exit===0&&unchanged?0:1);
