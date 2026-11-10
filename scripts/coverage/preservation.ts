import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { BASELINE, PROTECTED, HARNESS } from "./config";
import { checkHarnessStructure } from "./hygiene";
import { hash, tokens, analyzeSource, historicalSourceInventory } from "./source";

function git(root:string,args:string[]) {
  const p=Bun.spawnSync(["git",...args],{cwd:root,stdout:"pipe",stderr:"pipe"});
  if(p.exitCode) throw Error(`PRESERVATION: git ${args.join(" ")}: ${p.stderr}`);
  return p.stdout.toString();
}
function productionTokens(source:string) {
  const omitted=analyzeSource(source).omitted;
  return tokens(source).filter(t=>!omitted.has(t.line)).map(t=>t.text);
}
export function preservationReferences(root:string) {
  const files=git(root,["ls-tree","-r","--name-only",BASELINE,"--","crates"]).trim().split("\n");
  return {
    baselineHead:BASELINE,baselineTree:git(root,["rev-parse",`${BASELINE}^{tree}`]).trim(),
    protected:Object.fromEntries(PROTECTED.map(p=>[p,hash(git(root,["show",`${BASELINE}:${p}`]))])),
    files:files.filter(p=>p.endsWith(".rs")).map(path=>{
      const source=git(root,["show",`${BASELINE}:${path}`]);
      return {path,sha256:hash(source),tokens:tokens(source).map(t=>t.text),productionTokenHash:hash(JSON.stringify(productionTokens(source)))};
    }),
  };
}
export function checkHistoricalPreservation(root:string) {
  const refs=preservationReferences(root);
  checkHarnessStructure(root);
  const production=new Set(historicalSourceInventory(root).filter(f=>!f.testOnly).map(f=>f.path));
  for(const [path,expected] of Object.entries(refs.protected)) {
    if(path!==HARNESS && hash(readFileSync(resolve(root,path)))!==expected) throw Error(`PRESERVATION: protected bytes changed: ${path}`);
  }
  // Existing tokens must be an ordered subsequence: additions of tests are
  // allowed, but deleting/changing existing assertion or helper tokens is not.
  // The independent reviewer also checks additions for weakened assertions.
  for(const file of refs.files) {
    const source=readFileSync(resolve(root,file.path),"utf8"), current=tokens(source).map(t=>t.text);
    let index=0;
    for(const token of current) if(token===file.tokens[index]) index++;
    if(index!==file.tokens.length) throw Error(`PRESERVATION: existing Rust tokens changed or removed: ${file.path}`);
    if(production.has(file.path)) {
      if(hash(JSON.stringify(productionTokens(source)))!==file.productionTokenHash) throw Error(`PRESERVATION: production tokens changed: ${file.path}`);
    }
  }
  return {baselineHead:refs.baselineHead,baselineTree:refs.baselineTree,protected:refs.protected,existingRustFiles:refs.files.length,pass:true};
}

// Live feature checks preserve regression semantics rather than production bytes.
export { checkCurrentPreservation as checkPreservation } from "./current-preservation";
