import { realpathSync, readFileSync } from "node:fs";
import { resolve, relative } from "node:path";
import { CRATES } from "./config";
import type { SourceFile } from "./source";

export type FileCoverage = { path:string; crate:string; covered:number; total:number; uncovered:number[]; mapped:number[]; omitted:number[] };
// Pinned LLVM 22.1.2: CoverageMapping.cpp LineCoverageStats/Iterator and
// CoverageReport.cpp prepareSingleFileReport use different line aggregations.
// https://github.com/llvm/llvm-project/tree/llvmorg-22.1.2/llvm/tools/llvm-cov
function llvmFiles(report:any) {
  if(report?.type!=="llvm.coverage.json.export" || report.version!=="3.1.0" ||
    !Array.isArray(report.data) || report.data.length!==1 || !Array.isArray(report.data[0]?.files)) {
    throw Error("MALFORMED_REPORT: unsupported LLVM JSON export");
  }
  const files=new Map<string,any>();
  for(const file of report.data[0].files) {
    const summary=file?.summary?.lines;
    if(typeof file?.filename!=="string" || files.has(file.filename) || !Array.isArray(file.segments) ||
      !Number.isSafeInteger(summary?.count) || !Number.isSafeInteger(summary?.covered) ||
      summary.count<0 || summary.covered<0 || summary.covered>summary.count) {
      throw Error("MALFORMED_REPORT: invalid LLVM file/summary");
    }
    files.set(file.filename,file);
  }
  return files;
}
function llvmLines(segments:any[],maxLine:number) {
  let prior=[0,0];
  for(const s of segments) {
    if(!Array.isArray(s) || s.length!==6 || !s.slice(0,3).every(Number.isSafeInteger) ||
      s[0]<1 || s[0]>maxLine || s[1]<1 || s[2]<0 || !s.slice(3).every(x=>typeof x==="boolean") ||
      s[0]<prior[0] || (s[0]===prior[0] && s[1]<prior[1])) throw Error("MALFORMED_REPORT: invalid LLVM segment");
    prior=s;
  }
  const lines=new Map<number,number>();let next=0,wrapped:any;
  for(let line=1;next<segments.length;line++) {
    const onLine=[];
    while(next<segments.length && segments[next][0]===line) onLine.push(segments[next++]);
    const starts=onLine.filter(s=>!s[5] && s[3] && s[4]);
    const skipped=onLine.length && !onLine[0][3] && onLine[0][4];
    const mapped=(!skipped && (wrapped?.[3] || starts.length)) || onLine.some(s=>s[3] && s[4]);
    if(mapped) lines.set(line,Math.max(wrapped?.[2]??0,...starts.map(s=>s[2])));
    if(onLine.length) wrapped=onLine.at(-1);
  }
  return lines;
}
export function account(lcov:string, inventory:SourceFile[], root:string, canonical=(p:string)=>realpathSync(resolve(root,p)), llvmJson?:unknown) {
  root=realpathSync(root);
  if(!lcov.trim()) throw Error("MALFORMED_REPORT: empty LCOV");
  const known=new Map(inventory.map(f=>[f.path,f]));
  const lines=new Map<string,Map<number,number>>();
  const corroborated=llvmJson===undefined?undefined:llvmFiles(llvmJson), seen=new Set<string>();
  let nativeFile:any;
  let current:string|undefined, records=0;
  let entries=new Map<number,number>();
  let summaries:Partial<Record<"LF"|"LH",number>>={};
  for(const row of lcov.split(/\r?\n/)) {
    if(row.startsWith("SF:")) {
      if(current) throw Error("MALFORMED_REPORT: unterminated record");
      entries=new Map(); summaries={};
      try {current=relative(root,canonical(row.slice(3)));}
      catch {throw Error(`UNMAPPED_FILE: ${row.slice(3)}`);}
      if(!known.has(current) && !/^crates\/[^/]+\/tests\//.test(current)) throw Error(`UNMAPPED_FILE: ${current}`);
      if(corroborated) {
        const filename=row.slice(3);nativeFile=corroborated.get(filename);
        if(!nativeFile || seen.has(filename)) throw Error("MALFORMED_REPORT: LLVM/LCOV file set differs");
        seen.add(filename);
      }
      if(known.has(current) && !lines.has(current)) lines.set(current,new Map());
    } else if(row.startsWith("DA:")) {
      if(!current) throw Error("MALFORMED_REPORT: DA without SF");
      const match=row.match(/^DA:([1-9]\d*),(\d+)(?:,[^,]+)?$/);
      if(!match) throw Error("MALFORMED_REPORT: invalid DA");
      const n=Number(match[1]), hit=Number(match[2]);
      if(!Number.isSafeInteger(n)||!Number.isSafeInteger(hit)) throw Error("MALFORMED_REPORT: unsafe integer");
      const file=known.get(current);
      if(file && n>file.lines) throw Error(`MALFORMED_REPORT: line out of bounds ${current}:${n}`);
      if(entries.has(n)) throw Error("MALFORMED_REPORT: duplicate DA line in record");
      entries.set(n,hit);
    } else if(/^(?:LF|LH):/.test(row)) {
      if(!current) throw Error("MALFORMED_REPORT: summary without SF");
      const match=row.match(/^(LF|LH):(0|[1-9]\d*)$/);
      if(!match || !Number.isSafeInteger(Number(match[2]))) throw Error("MALFORMED_REPORT: invalid line summary");
      const key=match[1] as "LF"|"LH";
      if(summaries[key]!==undefined) throw Error("MALFORMED_REPORT: duplicate line summary");
      summaries[key]=Number(match[2]);
    } else if(row==="end_of_record") {
      if(!current) throw Error("MALFORMED_REPORT: end without SF");
      const covered=[...entries.values()].filter(hit=>hit>0).length;
      if(corroborated) {
        const expected=llvmLines(nativeFile.segments,known.get(current)?.lines??readFileSync(resolve(root,current),"utf8").split("\n").length);
        if(summaries.LF!==nativeFile.summary.lines.count || summaries.LH!==nativeFile.summary.lines.covered ||
          entries.size!==expected.size || [...expected].some(([n,hit])=>entries.get(n)!==hit)) {
          throw Error("MALFORMED_REPORT: LLVM segments/summary disagree with LCOV");
        }
      } else if(summaries.LF!==entries.size || summaries.LH!==covered) throw Error("MALFORMED_REPORT: LF/LH missing or inconsistent with DA entries");
      // Validate each original record, including test-only records, before
      // canonical union can hide an invalid instance's summary or duplicate.
      const map=lines.get(current);
      if(map) for(const [n,hit] of entries) map.set(n,Math.max(map.get(n)??0,hit));
      current=undefined;records++;
    } else if(row && !/^(?:TN|FN|FNDA|FNF|FNH|BRDA|BRF|BRH):/.test(row)) throw Error(`MALFORMED_REPORT: unknown row ${row}`);
  }
  if(current || !records) throw Error("MALFORMED_REPORT: incomplete LCOV");
  if(corroborated && seen.size!==corroborated.size) throw Error("MALFORMED_REPORT: LLVM/LCOV file set differs");
  for(const crate of CRATES) {
    if(!inventory.some(f=>f.crate===crate && !f.testOnly && lines.has(f.path))) throw Error(`MISSING_CRATE: ${crate}`);
  }
  const files:FileCoverage[]=[];
  for(const file of inventory) {
    if(file.testOnly) continue;
    const map=lines.get(file.path);
    if(!map) throw Error(`MISSING_FILE: ${file.path}`);
    const omitted=new Set(file.omitted);
    const mapped=[...map.keys()].filter(n=>!omitted.has(n)).sort((a,b)=>a-b);
    const uncovered=mapped.filter(n=>map.get(n)===0);
    files.push({path:file.path,crate:file.crate,total:mapped.length,covered:mapped.length-uncovered.length,uncovered,mapped,omitted:file.omitted});
  }
  const crates=CRATES.map(crate=>{
    const own=files.filter(f=>f.crate===crate);
    const total=own.reduce((n,f)=>n+f.total,0), covered=own.reduce((n,f)=>n+f.covered,0);
    if(!own.length || !total) throw Error(`MISSING_CRATE: invalid denominator for ${crate}`);
    return {crate,covered,total,percent:100*covered/total,pass:BigInt(covered)*100n>=BigInt(total)*80n};
  });
  return {schemaVersion:1,threshold:80,files,crates,pass:crates.every(c=>c.pass)};
}
