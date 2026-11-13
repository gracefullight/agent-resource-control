import { readdirSync, readFileSync, realpathSync, existsSync, lstatSync } from "node:fs";
import { resolve, relative, dirname, basename, join } from "node:path";
import { createHash } from "node:crypto";
import { CRATES, HISTORICAL_CRATES, CRATE_ROOTS, FROZEN_INPUTS, testPath, type CrateName } from "./config";

export const hash = (data: string | Buffer) => createHash("sha256").update(data).digest("hex");
export function walk(root: string, excluded: readonly string[] = []): string[] {
  return readdirSync(root, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))
    .flatMap(e => {
      if(excluded.includes(join(root,e.name))) return [];
      if(e.isSymbolicLink()) throw Error(`SOURCE: symlink input ${join(root,e.name)}`);
      return e.isDirectory() ? walk(join(root,e.name),excluded) : [join(root,e.name)];
    });
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
    let keyword=i;
    if(["pub","crate"].includes(ts[keyword].text)) {
      keyword++;
      if(ts[keyword]?.text==="(") keyword=closing(ts,keyword)+1;
    }
    // A cfg-disabled visible item is skipped at its `pub` token. Record its
    // external module before skipping, retaining the real path and cfg proof.
    // Production-visible items are recorded once at their later `mod` token.
    if((keyword===i || testOnly) && ts[keyword]?.text==="mod" && ts[keyword+2]?.text===";") {
      const path=attrs.find(a=>a[0]==="path")?.[2];
      modules.push({name:ts[keyword+1].text,path:path?JSON.parse(path):undefined,testOnly});
    }
    if(testOnly) {
      const groupedUse=ts[keyword]?.text==="use";
      let end=i, angles=0;
      for(;end<ts.length;end++) {
        const t=ts[end].text;
        // Braces in a use tree group paths; the item still ends at `;`.
        // Block-bodied items continue ending at their closing brace.
        if(t==="{") {end=closing(ts,end);if(!groupedUse) break;continue;}
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
export type CargoMetadata = {workspace_members:string[];packages:{id:string;name:string;manifest_path:string;targets:{name:string;kind:string[];src_path:string}[]}[]};
// Only Cargo-declared example entries and their explicit Rust module graph are
// diagnostic. Directory names alone never authorize omitting a source file.
export function declaredExampleSources(root:string, metadata?:CargoMetadata, crates:readonly CrateName[]=CRATES) {
  const sources=new Map<string,CrateName>();
  if(metadata===undefined) return sources;
  root=realpathSync(root);
  if(!Array.isArray(metadata.workspace_members) || !Array.isArray(metadata.packages) ||
    new Set(metadata.workspace_members).size!==metadata.workspace_members.length) throw Error("SOURCE: invalid Cargo workspace metadata");
  const members=metadata.packages.filter(p=>metadata.workspace_members.includes(p.id));
  if(members.length!==metadata.workspace_members.length || new Set(members.map(p=>p.id)).size!==members.length ||
    JSON.stringify(members.map(p=>p.name).sort())!==JSON.stringify([...CRATES].sort())) throw Error("SOURCE: invalid Cargo workspace ownership");
  for(const crate of crates) {
    const pkg=members.find(p=>p.name===crate)!;
    const manifest=resolve(root,CRATE_ROOTS[crate].manifest), packageRoot=dirname(manifest);
    if(realpathSync(pkg.manifest_path)!==manifest || realpathSync(manifest)!==manifest || !Array.isArray(pkg.targets)) throw Error(`SOURCE: invalid Cargo package ${crate}`);
    function visit(file:string, entry=false) {
      if(file!==resolve(file)) throw Error(`SOURCE: non-canonical example source ${file}`);
      file=realpathSync(file);
      if(!file.startsWith(packageRoot+"/") || !file.endsWith(".rs") || !lstatSync(file).isFile()) throw Error(`SOURCE: escaping or unsupported example source ${file}`);
      const path=relative(root,file);
      if(sources.has(path)) {
        if(sources.get(path)!==crate) throw Error(`SOURCE: conflicting example ownership ${path}`);
        return;
      }
      sources.set(path,crate);
      const info=analyzeSource(readFileSync(file,"utf8"));
      for(const mod of info.modules) {
        const base=entry || ["main.rs","lib.rs","mod.rs"].includes(basename(file))?dirname(file):file.slice(0,-3);
        const candidates=mod.path?[resolve(dirname(file),mod.path)]:[resolve(base,mod.name+".rs"),resolve(base,mod.name,"mod.rs")];
        const child=candidates.find(existsSync);
        if(!child) throw Error(`SOURCE: missing example module ${path}: ${mod.name}`);
        visit(child);
      }
    }
    for(const target of pkg.targets) {
      if(!Array.isArray(target.kind)) throw Error("SOURCE: invalid Cargo target kind");
      if(!target.kind.includes("example")) continue;
      if(target.kind.length!==1 || typeof target.name!=="string" || !target.name || typeof target.src_path!=="string") throw Error("SOURCE: unsupported Cargo example classification");
      visit(target.src_path,true);
    }
  }
  return sources;
}
export type SourceFile = { path:string; crate:string; sha256:string; lines:number; omitted:number[]; testOnly:boolean; classification?:"cargo_example"; spans:ReturnType<typeof analyzeSource>["spans"] };
export function inventoryForCrates(root: string, crates: readonly CrateName[], metadata?:CargoMetadata): SourceFile[] {
  root=realpathSync(root);
  const owners=new Map<string,CrateName>();
  const files=crates.flatMap(crate=>{
    readFileSync(resolve(root,CRATE_ROOTS[crate].manifest));
    return walk(resolve(root,CRATE_ROOTS[crate].source)).filter(p=>p.endsWith(".rs")).map(p=>{
      owners.set(p,crate); return p;
    });
  });
  const productionFiles=new Set(files), examples=declaredExampleSources(root,metadata,crates);
  for(const [path,crate] of examples) {
    const file=resolve(root,path);
    if(!owners.has(file)) {owners.set(file,crate);files.push(file);}
  }
  const testFiles=crates.flatMap(crate=>{
    const dir=resolve(root,CRATE_ROOTS[crate].tests);
    return existsSync(dir)?walk(dir).filter(p=>p.endsWith(".rs")):[];
  });
  const analyses=new Map([...files,...testFiles].map(p=>[realpathSync(p),analyzeSource(readFileSync(p,"utf8"))]));
  const production=new Set<string>(), tests=new Set<string>();
  function visit(file:string,test:boolean,entry=false) {
    file=realpathSync(file); const seen=test?tests:production;
    if(seen.has(file)) return; seen.add(file);
    const info=analyses.get(file);
    // A partial diagnostic inventory may reference a test fixture from another
    // registered crate. The complete current inventory still classifies that
    // source, and production references outside this inventory remain errors.
    const path=relative(root,file);
    const dependencyTestSource=test && CRATES.some(crate=>!crates.includes(crate) && path.startsWith(CRATE_ROOTS[crate].source+"/"));
    if(!info && test && (testPath(path) || dependencyTestSource)) return;
    if(!info) throw Error(`SOURCE: module outside src: ${file}`);
    if(!test && !productionFiles.has(file)) throw Error(`SOURCE: production module outside inventoried src: ${file}`);
    for(const mod of info.modules) {
      const base=entry || ["main.rs","lib.rs","mod.rs"].includes(basename(file))?dirname(file):file.slice(0,-3);
      const candidates=mod.path?[resolve(dirname(file),mod.path)]:[resolve(base,mod.name+".rs"),resolve(base,mod.name,"mod.rs")];
      const child=candidates.find(existsSync);
      if(!child) throw Error(`SOURCE: missing module ${file}: ${mod.name}`);
      visit(child,test||mod.testOnly);
    }
  }
  for(const crate of crates) {
    const entries=["lib.rs","main.rs"].map(entry=>resolve(root,CRATE_ROOTS[crate].source,entry)).filter(existsSync);
    if(!entries.length) throw Error(`SOURCE: missing crate entry ${crate}`);
    for(const p of entries) visit(p,false,true);
  }
  for(const file of testFiles.filter(p=>crates.some(crate=>dirname(p)===resolve(root,CRATE_ROOTS[crate].tests)))) visit(file,true,true);
  for(const path of examples.keys()) visit(resolve(root,path),true,true);
  return files.map(file=>{
    const path=relative(root,file), info=analyses.get(realpathSync(file))!;
    if(!production.has(realpathSync(file)) && !tests.has(realpathSync(file))) throw Error(`SOURCE: unclassified src file ${path}`);
    const text=readFileSync(file,"utf8");
    return {path,crate:owners.get(file)!,sha256:hash(text),lines:text.split("\n").length,omitted:[...info.omitted],spans:info.spans,testOnly:!production.has(realpathSync(file)),
      ...(!productionFiles.has(file)?{classification:"cargo_example" as const}:{})};
  }).sort((a,b)=>a.path.localeCompare(b.path));
}
// Immutable pre-Traffic baselines retain their original six-crate inventory.
export const sourceInventory = (root: string, metadata?:CargoMetadata) => inventoryForCrates(root, CRATES, metadata);
export const historicalSourceInventory = (root: string) => inventoryForCrates(root, HISTORICAL_CRATES);
export function inputIdentity(root:string) {
  root=realpathSync(root);
  const paths=["Cargo.toml","Cargo.lock","rust-toolchain.toml","mise.toml","package.json","bun.lock","bunfig.toml","biome.json",
    ".arc-verify/rust-coverage/contract/accounting.json",
    ".arc-verify/desktop-foundation/contract.md",
    ...FROZEN_INPUTS,
    "apps/desktop/package.json","apps/desktop/tsconfig.json","apps/desktop/tsconfig.test.json","apps/desktop/vite.config.ts",
    "apps/desktop/index.html","apps/desktop/src-tauri/build.rs","apps/desktop/src-tauri/tauri.conf.json",
    ...CRATES.map(crate=>CRATE_ROOTS[crate].manifest),
    ...[".cargo/config",".cargo/config.toml","build.rs"].filter(p=>existsSync(resolve(root,p))),
    ...["crates","scripts","apps/desktop",".arc-verify/rust-coverage/contract"].flatMap(dir=>
      walk(resolve(root,dir),["apps/desktop/node_modules","apps/desktop/dist","apps/desktop/src-tauri/target","apps/desktop/src-tauri/gen"].map(p=>resolve(root,p))).map(p=>relative(root,p)))];
  for(const crate of CRATES) if(!lstatSync(resolve(root,CRATE_ROOTS[crate].source)).isDirectory()) throw Error(`SOURCE: missing source root ${crate}`);
  const files=Object.fromEntries([...new Set(paths)].sort().map(p=>{
    const path=resolve(root,p);
    if(!lstatSync(path).isFile() || realpathSync(path)!==path) throw Error(`SOURCE: non-canonical input ${p}`);
    return [p,hash(readFileSync(path))];
  }));
  return {sha256:hash(JSON.stringify(files)),files};
}

export function requireIdentityDigest(recorded:ReturnType<typeof inputIdentity>) {
  if(!recorded || !recorded.files || Array.isArray(recorded.files)) throw Error("INPUT_IDENTITY: missing file map");
  const canonical=Object.fromEntries(Object.keys(recorded.files).sort().map(p=>[p,recorded.files[p]]));
  if(hash(JSON.stringify(canonical))!==recorded.sha256) throw Error("INPUT_IDENTITY: corrupt aggregate");
  return canonical;
}
export function requireInputIdentity(root:string, recorded:ReturnType<typeof inputIdentity>) {
  const canonical=requireIdentityDigest(recorded);
  const current=inputIdentity(root);
  if(current.sha256!==recorded.sha256 || JSON.stringify(current.files)!==JSON.stringify(canonical)) throw Error("INPUT_IDENTITY: stale or incomplete source/configuration hashes");
  return current;
}
