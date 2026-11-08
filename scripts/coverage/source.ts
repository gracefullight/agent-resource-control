import { readdirSync, readFileSync, realpathSync, existsSync } from "node:fs";
import { resolve, relative, dirname, basename, join } from "node:path";
import { createHash } from "node:crypto";
import { CRATES } from "./config";

export const hash = (data: string | Buffer) => createHash("sha256").update(data).digest("hex");
export function walk(root: string): string[] {
  return readdirSync(root, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))
    .flatMap(e => e.isDirectory() ? walk(join(root,e.name)) : [join(root,e.name)]);
}
type Token = { text: string; start: number; end: number; line: number };
// Skip Rust comments and literals, retaining literal text as one opaque token.
// Positions stay in the original source; braces inside text never affect balancing.
export function tokens(source: string): Token[] {
  const result: Token[] = [];
  let i = 0, line = 1;
  const advance = (end: number) => { line += (source.slice(i,end).match(/\n/g) ?? []).length; i = end; };
  while (i < source.length) {
    if (/\s/.test(source[i])) { advance(i+1); continue; }
    if (source.startsWith("//",i)) { const end=source.indexOf("\n",i); advance(end<0?source.length:end); continue; }
    if (source.startsWith("/*",i)) {
      let end=i+2, depth=1;
      while(end<source.length && depth) {
        if(source.startsWith("/*",end)) {depth++;end+=2;}
        else if(source.startsWith("*/",end)) {depth--;end+=2;} else end++;
      }
      if(depth) throw Error("SOURCE: unclosed block comment");
      advance(end); continue;
    }
    const start=i, startLine=line;
    const raw=source.slice(i).match(/^(?:br|r)(#*)"/);
    if(raw) {
      const end=source.indexOf('"'+raw[1],i+raw[0].length);
      if(end<0) throw Error("SOURCE: unclosed raw string");
      advance(end+1+raw[1].length);
    } else if(source[i]==='"' || (source[i]==="'" && /^'(?:\\.|[^'\\\n])'/.test(source.slice(i)))) {
      const quote=source[i]; let end=i+1;
      while(end<source.length) { if(source[end]==="\\") {end+=2;continue;} if(source[end++]===quote) break; }
      advance(end);
    } else {
      const word=source.slice(i).match(/^[A-Za-z_][A-Za-z_0-9]*/);
      advance(i+(word?.[0].length ?? 1));
    }
    result.push({text:source.slice(start,i),start,end:i,line:startLine});
  }
  return result;
}
function closing(ts: Token[], start: number): number {
  const pairs: Record<string,string> = {"[":"]","(":")","{":"}"};
  const end=pairs[ts[start]?.text];
  if(!end) throw Error("SOURCE: expected opening delimiter");
  for(let i=start+1;i<ts.length;i++) {
    if(pairs[ts[i].text]) {i=closing(ts,i);continue;}
    if(ts[i].text===end) return i;
  }
  throw Error("SOURCE: unbalanced delimiters");
}
// Three-valued cfg evaluation: only predicates provably false without `test`
// are omitted. Platform/features stay unknown and therefore in production scope.
function cfgWithoutTest(ts: string[]): boolean | undefined {
  if(ts.length===1 && ts[0]==="test") return false;
  if(["all","any","not"].includes(ts[0]) && ts[1]==="(" && ts.at(-1)===")") {
    const args: string[][]=[]; let depth=0, start=2;
    for(let i=2;i<ts.length-1;i++) {
      if(ts[i]==="(") depth++; if(ts[i]===")") depth--;
      if(ts[i]==="," && depth===0) {args.push(ts.slice(start,i));start=i+1;}
    }
    if(start<ts.length-1) args.push(ts.slice(start,-1));
    const values=args.map(cfgWithoutTest);
    if(ts[0]==="not") return values.length===1 && values[0]!==undefined ? !values[0] : undefined;
    if(ts[0]==="all") return values.includes(false)?false:values.every(x=>x===true)?true:undefined;
    return values.includes(true)?true:values.every(x=>x===false)?false:undefined;
  }
  return undefined;
}
export function analyzeSource(source: string) {
  const ts=tokens(source), omitted=new Set<number>();
  const modules: {name:string;path?:string;testOnly:boolean}[]=[];
  const spans: {start:number;end:number;reason:string}[]=[];
  let attrs: string[][]=[], attrStart=0;
  for(let i=0;i<ts.length;i++) {
    if(ts[i].text==="#" && ts[i+1]?.text==="[") {
      const end=closing(ts,i+1);
      if(!attrs.length) attrStart=i;
      attrs.push(ts.slice(i+2,end).map(t=>t.text)); i=end; continue;
    }
    const testOnly=attrs.some(a=>a[0]==="cfg" && cfgWithoutTest(a.slice(2,-1))===false);
    if(ts[i].text==="mod" && ts[i+2]?.text===";") {
      const path=attrs.find(a=>a[0]==="path")?.[2];
      modules.push({name:ts[i+1].text,path:path?JSON.parse(path):undefined,testOnly});
    }
    if(testOnly) {
      let end=i, angles=0;
      for(;end<ts.length;end++) {
        const t=ts[end].text;
        if(t==="{") {end=closing(ts,end);break;}
        if(t==="<") angles++;
        if(t===">" && ts[end-1]?.text!=="-" && angles) angles--;
        if(t===";" || (t==="," && !angles)) break;
        if(t==="[" || t==="(") end=closing(ts,end);
      }
      if(end>=ts.length) throw Error("SOURCE: unsupported test-only item");
      if(ts[attrStart-1]?.line===ts[attrStart].line || ts[end+1]?.line===ts[end].line) {
        throw Error(`SOURCE: test-only item at ${ts[attrStart].line}-${ts[end].line} shares a line with other code; line accounting is ambiguous`);
      }
      spans.push({start:ts[attrStart].line,end:ts[end].line,reason:"cfg false when test is disabled"});
      for(let l=ts[attrStart].line;l<=ts[end].line;l++) omitted.add(l);
      // External test module declarations are recorded above; inline children
      // cannot be production when their enclosing item is disabled.
      i=end;
    }
    // Visibility can appear between the attributes and the item keyword.
    if(!["pub","crate"].includes(ts[i].text) && !(ts[i].text==="(" && ts[i-1]?.text==="pub")) attrs=[];
    if(ts[i].text==="(" && ts[i-1]?.text==="pub") i=closing(ts,i);
  }
  return {omitted,modules,spans};
}
export type SourceFile = { path:string; crate:string; sha256:string; lines:number; omitted:number[]; testOnly:boolean; spans:ReturnType<typeof analyzeSource>["spans"] };
export function sourceInventory(root: string): SourceFile[] {
  const files=CRATES.flatMap(crate=>walk(resolve(root,"crates",crate,"src"))).filter(p=>p.endsWith(".rs"));
  const testFiles=CRATES.flatMap(crate=>{
    const dir=resolve(root,"crates",crate,"tests");
    return existsSync(dir)?walk(dir).filter(p=>p.endsWith(".rs")):[];
  });
  const analyses=new Map([...files,...testFiles].map(p=>[realpathSync(p),analyzeSource(readFileSync(p,"utf8"))]));
  const production=new Set<string>(), tests=new Set<string>();
  function visit(file:string,test:boolean,entry=false) {
    file=realpathSync(file); const seen=test?tests:production;
    if(seen.has(file)) return; seen.add(file);
    const info=analyses.get(file);
    if(!info && test && /^crates\/[^/]+\/tests\//.test(relative(root,file))) return;
    if(!info) throw Error(`SOURCE: module outside src: ${file}`);
    if(!test && !files.includes(file)) throw Error(`SOURCE: production module outside inventoried src: ${file}`);
    for(const mod of info.modules) {
      const base=entry || ["main.rs","lib.rs","mod.rs"].includes(basename(file))?dirname(file):file.slice(0,-3);
      const candidates=mod.path?[resolve(dirname(file),mod.path)]:[resolve(base,mod.name+".rs"),resolve(base,mod.name,"mod.rs")];
      const child=candidates.find(existsSync);
      if(!child) throw Error(`SOURCE: missing module ${file}: ${mod.name}`);
      visit(child,test||mod.testOnly);
    }
  }
  for(const crate of CRATES) for(const entry of ["lib.rs","main.rs"]) {
    const p=resolve(root,"crates",crate,"src",entry); if(existsSync(p)) visit(p,false,true);
  }
  for(const file of testFiles.filter(p=>dirname(p)===resolve(root,"crates",relative(root,p).split("/")[1],"tests"))) visit(file,true,true);
  return files.map(file=>{
    const path=relative(root,file), info=analyses.get(realpathSync(file))!;
    if(!production.has(realpathSync(file)) && !tests.has(realpathSync(file))) throw Error(`SOURCE: unclassified src file ${path}`);
    const text=readFileSync(file,"utf8");
    return {path,crate:path.split("/")[1],sha256:hash(text),lines:text.split("\n").length,omitted:[...info.omitted],spans:info.spans,testOnly:!production.has(realpathSync(file))};
  }).sort((a,b)=>a.path.localeCompare(b.path));
}
export function inputIdentity(root:string) {
  const paths=["Cargo.toml","Cargo.lock","rust-toolchain.toml","mise.toml","package.json",
    ".arc-verify/rust-coverage/contract/accounting.json",
    ...[".cargo/config",".cargo/config.toml","build.rs"].filter(p=>existsSync(resolve(root,p))),
    ...walk(resolve(root,"crates")).map(p=>relative(root,p)),...walk(resolve(root,"scripts/coverage")).map(p=>relative(root,p))];
  const files=Object.fromEntries([...new Set(paths)].sort().map(p=>[p,hash(readFileSync(resolve(root,p)))]));
  return {sha256:hash(JSON.stringify(files)),files};
}
