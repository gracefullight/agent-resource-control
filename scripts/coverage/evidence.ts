import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { isAbsolute, resolve } from "node:path";
import { hash, requireIdentityDigest } from "./source";

export type EvidenceRef = { path: string; sha256: string; bytes?: number };
export const OUTPUT_ROOTS = ["target/", ".arc-verify/rust-coverage/baseline/runs/"];

// References are portable workspace-relative paths. Inspect canonical bytes
// even when the caller supplies the path (for example the reuse entry receipt).
export function referenceFile(root:string, path:string):EvidenceRef {
  requireEvidenceRef({path,sha256:"0".repeat(64),bytes:0});
  const file=resolve(realpathSync(root),path), stat=lstatSync(file);
  if(!stat.isFile() || stat.isSymbolicLink() || realpathSync(file)!==file) throw Error(`EVIDENCE: non-canonical regular file ${path}`);
  const bytes=readFileSync(file);
  return {path,sha256:hash(bytes),bytes:bytes.length};
}

// Missing historical outputs are an availability result, never authenticated
// bytes. Callers deciding current acceptance must require a present result.
export function requireEvidenceRef(ref: EvidenceRef, options: { sizeRequired?: boolean; prefixes?: string[] } = {}) {
  if(!ref || typeof ref.path!=="string" || isAbsolute(ref.path) || ref.path.includes("\\") ||
    ref.path.split("/").some(p=>!p || p===".." || p===".") ||
    !(options.prefixes ?? OUTPUT_ROOTS).some(prefix=>ref.path.startsWith(prefix)) || !/^[a-f0-9]{64}$/.test(ref.sha256)) {
    throw Error(`EVIDENCE: invalid reference ${ref?.path}`);
  }
  if((options.sizeRequired ?? true) && (!Number.isSafeInteger(ref.bytes) || (ref.bytes??-1)<0)) throw Error(`EVIDENCE: missing byte size ${ref.path}`);
}
export function readEvidence(root: string, ref: EvidenceRef, options: { optional?: boolean; sizeRequired?: boolean; prefixes?: string[] } = {}): Buffer | undefined {
  requireEvidenceRef(ref,options);
  const path=resolve(realpathSync(root),ref.path);
  let stat;
  try {stat=lstatSync(path);} catch(error) {
    if(options.optional && (error as NodeJS.ErrnoException).code==="ENOENT") return undefined;
    throw error;
  }
  if(!stat.isFile() || stat.isSymbolicLink() || realpathSync(path)!==path) throw Error(`EVIDENCE: non-canonical regular file ${ref.path}`);
  const bytes=readFileSync(path);
  if(hash(bytes)!==ref.sha256 || (ref.bytes!==undefined && bytes.length!==ref.bytes)) throw Error(`EVIDENCE: hash/size mismatch ${ref.path}`);
  return bytes;
}

export function evidenceRefs(value: unknown): EvidenceRef[] {
  if(!value || typeof value!=="object") return [];
  const record=value as Record<string,unknown>;
  const own=typeof record.path==="string" && typeof record.sha256==="string" ? [record as EvidenceRef] : [];
  return [...own,...Object.values(record).flatMap(evidenceRefs)];
}

export function commandEvidence(root: string, ref: EvidenceRef, sourceHash: string, expectedArgv?: readonly string[]) {
  const receipt=JSON.parse(readEvidence(root,ref)!.toString());
  if(receipt.status!=="passed" || receipt.exitCode!==0 || receipt.termination || !Array.isArray(receipt.argv) ||
    !receipt.argv.length || !receipt.argv.every((arg:unknown)=>typeof arg==="string") || typeof receipt.cwd!=="string" || !receipt.cwd ||
    !Number.isSafeInteger(receipt.timeoutMs) || receipt.timeoutMs<7_200_000 ||
    !Number.isFinite(Date.parse(receipt.startedAt)) || !Number.isFinite(Date.parse(receipt.finishedAt)) ||
    Date.parse(receipt.finishedAt)<Date.parse(receipt.startedAt) || !Number.isFinite(receipt.durationMs) || receipt.durationMs<0) throw Error(`EVIDENCE: unsuccessful/incomplete command ${ref.path}`);
  if(expectedArgv && JSON.stringify(receipt.argv)!==JSON.stringify(expectedArgv)) throw Error(`EVIDENCE: unexpected argv ${ref.path}`);
  if(receipt.inputsUnchanged!==true) throw Error(`INPUT_IDENTITY: changed command inputs ${ref.path}`);
  for(const input of [receipt.inputBefore,receipt.inputAfter]) {
    if(input?.sha256!==sourceHash) throw Error(`INPUT_IDENTITY: stale command ${ref.path}`);
    const identity=JSON.parse(readEvidence(root,input.manifest)!.toString());
    requireIdentityDigest(identity);
    if(identity.sha256!==sourceHash) throw Error(`INPUT_IDENTITY: stale command manifest ${ref.path}`);
  }
  return {receipt,stdout:readEvidence(root,receipt.stdout)!.toString(),stderr:readEvidence(root,receipt.stderr)!.toString()};
}
