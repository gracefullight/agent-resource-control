import { readFileSync, writeFileSync, readdirSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { managedInputs, managedGit, nativeHost, validateCommandReceipt } from "../../../scripts/harness/managed-exec-evidence";
import { gitBytes } from "../../../scripts/coverage/hygiene";
import { hash, inputIdentity } from "../../../scripts/coverage/source";
const root=resolve(import.meta.dir,"../../.."),inputs=managedInputs(root);
const baseline="19d7a507044cbf83918f58cd41056905c275c8d4";
const protectedPaths=["crates/arc-storage/migrations/0001_phase0.sql","scripts/coverage/accounting.ts","scripts/coverage/config.ts","scripts/coverage/source.ts","scripts/coverage/gate.ts",".arc-verify/rust-coverage/contract/accounting.json"];
const protection=Object.fromEntries(protectedPaths.map(path=>{
 const bytes=readFileSync(resolve(root,path)),prior=gitBytes(root,["show",`${baseline}:${path}`]);
 if(!bytes.equals(prior))throw Error(`Protected bytes differ: ${path}`);
 return [path,hash(bytes)];
}));
const lengths=Object.keys(inputs.files).filter(path=>/\.(rs|sql)$/.test(path)||path.startsWith("scripts/coverage/")||path.startsWith("scripts/harness/"))
 .map(path=>({path,lines:readFileSync(resolve(root,path),"utf8").split("\n").length-Number(readFileSync(resolve(root,path),"utf8").endsWith("\n"))}));
for(const file of lengths)if(file.lines>500)throw Error(`Source exceeds limit: ${file.path}`);
const files=gitBytes(root,["ls-files","--cached","--others","--exclude-standard","-z","--",".arc-verify/managed-exec"])
 .toString().split("\0").filter(Boolean);
const unique=[...new Set(files)];if(unique.length>=150)throw Error("Managed evidence inventory limit exceeded");
const receipts=readdirSync(import.meta.dir).filter(name=>name.endsWith(".json")).flatMap(name=>{
 const data=JSON.parse(readFileSync(resolve(import.meta.dir,name),"utf8"));
 if(data.schemaVersion===1&&data.actorId==="backend"&&Array.isArray(data.argv)) {
  for(const stream of ["stdout","stderr"]) if(hash(readFileSync(resolve(root,data.output[`${stream}Path`])))!==data[`${stream}Sha256`]) throw Error(`Output digest differs: ${name} ${stream}`);
  if(data.sourceHash===inputs.sha256) validateCommandReceipt(data,inputs.sha256,nativeHost());
 }
 return data.schemaVersion===1&&data.actorId==="backend"&&Array.isArray(data.argv)?[{path:`.arc-verify/managed-exec/implementation/${name}`,sha256:hash(readFileSync(resolve(import.meta.dir,name))),argv:data.argv,exit:data.exit,currentSource:data.sourceHash===inputs.sha256}]:[];
});
const latestPath=resolve(root,".arc-verify/rust-coverage/baseline/latest.json");
let coverage:unknown={available:false};
if(existsSync(latestPath)) {
 const latest=JSON.parse(readFileSync(latestPath,"utf8")), identity=inputIdentity(root);
 if(latest.pass===true&&latest.sourceHash===identity.sha256) {
  const summaryPath=resolve(root,latest.directory,"summary.json"),summary=JSON.parse(readFileSync(summaryPath,"utf8"));
  if(summary.inputs.sha256!==identity.sha256||!summary.pass||!summary.crates.every((c:any)=>c.covered*100>=c.total*80))throw Error("Invalid current native coverage");
  coverage={available:true,nativeInputHash:identity.sha256,latest:latestPath,summary:{path:summaryPath,sha256:hash(readFileSync(summaryPath))},crates:summary.crates,childProof:summary.childProof};
 }
}
const result={schemaVersion:1,baseline,host:nativeHost(),sourceHash:inputs.sha256,inputCount:Object.keys(inputs.files).length,git:managedGit(root),protectedSha256:protection,
 sourceLimit:500,maxSource:lengths.sort((a,b)=>b.lines-a.lines)[0],managedEvidenceFiles:unique.length,coverage,receipts};
writeFileSync(resolve(import.meta.dir,"audit.json"),JSON.stringify(result,null,2)+"\n");
console.log(JSON.stringify({sourceHash:inputs.sha256,inputCount:result.inputCount,maxSource:result.maxSource,evidenceFiles:unique.length,receipts:receipts.map(r=>({argv:r.argv,exit:r.exit,current:r.currentSource}))}));
