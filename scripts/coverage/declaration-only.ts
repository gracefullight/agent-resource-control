import { tokens } from "./source";

// LLVM need not emit a file record for declarations with no authored executable
// body. This deliberately recognizes a small Rust subset; unknown syntax,
// expansion hooks and initializers remain missing-file errors, never exclusions.
export function declarationOnly(source:string):boolean {
  const ts=tokens(source).map(token=>token.text);
  const pairs:Record<string,string>={"[":"]","(":")","{":"}"};
  function close(start:number):number {
    const end=pairs[ts[start]];
    if(!end) return -1;
    for(let i=start+1;i<ts.length;i++) {
      if(pairs[ts[i]]) {i=close(i);if(i<0) return -1;}
      else if(ts[i]===end) return i;
    }
    return -1;
  }
  function attribute(start:number):number {
    if(ts[start]!=="#" || ts[start+1]!=="[") return -1;
    const end=close(start+1);
    if(end<0) return -1;
    const body=ts.slice(start+2,end);
    if(body.includes("!")) return -1;
    if(body[0]==="derive") {
      if(body[1]!=="(" || body.at(-1)!==")" || body.length<4 ||
        body.slice(2,-1).some(t=>![",","Clone","Copy","Debug","Eq","PartialEq"].includes(t))) return -1;
    } else if(!["cfg","repr","doc","path","allow","warn","deny","forbid"].includes(body[0])) return -1;
    return end+1;
  }
  function safe(start:number,end:number,trait=false):boolean {
    for(let i=start;i<end;i++) {
      if(ts[i]==="#") {const next=attribute(i);if(next<0 || next>end) return false;i=next-1;continue;}
      if(["{","}","!","=","impl","const"].includes(ts[i]) ||
        (ts[i]==="static" && ts[i-1]!=="'") || (ts[i]==="fn" && !trait)) return false;
    }
    return true;
  }
  let i=0;
  while(i<ts.length) {
    if(ts[i]==="#") {i=attribute(i);if(i<0) return false;continue;}
    if(ts[i]==="pub") {
      i++;
      if(ts[i]==="(") {const end=close(i);if(end<0) return false;i=end+1;}
    }
    const kind=ts[i++];
    if(kind==="mod") {
      if(!/^[A-Za-z_][A-Za-z_0-9]*$/.test(ts[i]??"") || ts[i+1]!==";") return false;
      i+=2;continue;
    }
    if(kind==="use") {
      const start=i;
      while(i<ts.length && ts[i]!==";") {
        if(["!","=","fn","impl","const","static"].includes(ts[i])) return false;
        i++;
      }
      if(i===start || ts[i++]!==";") return false;
      continue;
    }
    if(!["struct","enum","union","trait"].includes(kind)) return false;
    const header=i;
    while(i<ts.length && !["{",";"].includes(ts[i])) {
      if(ts[i]==="(" || ts[i]==="[") {const end=close(i);if(end<0) return false;i=end;}
      i++;
    }
    if(!safe(header,i)) return false;
    if(ts[i]===";") {if(kind==="trait") return false;i++;continue;}
    if(ts[i]!=="{") return false;
    const end=close(i);
    if(end<0 || !safe(i+1,end,kind==="trait")) return false;
    i=end+1;
    if(ts[i]===";") i++;
  }
  return true;
}
