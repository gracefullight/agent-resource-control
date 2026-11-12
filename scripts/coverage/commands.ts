import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { nativeOutput } from "./native-output";
export type Receipt = {argv:string[];cwd:string;started:string;ended:string;exit:number;stdout:string;stderr:string;pid?:number;output?:ReturnType<typeof nativeOutput>};
export async function command(argv:string[],dir:string,cwd:string,env:Record<string,string|undefined>=process.env,timeout=7200000,nativeDirectory?:string):Promise<Receipt> {
  const started=new Date().toISOString();
  let receipt:Receipt;
  try {
    const child=Bun.spawn(argv,{cwd,env,stdout:"pipe",stderr:"pipe"});
    const timer=setTimeout(()=>child.kill("SIGTERM"),timeout);
    const [out,err,exit]=await Promise.all([new Response(child.stdout).arrayBuffer(),new Response(child.stderr).arrayBuffer(),child.exited]);
    clearTimeout(timer);
    const rawOut=Buffer.from(out),rawErr=Buffer.from(err),stdout=rawOut.toString(),stderr=rawErr.toString();
    receipt={argv,cwd,started,ended:new Date().toISOString(),exit,stdout,stderr,pid:child.pid,output:nativeOutput(argv,cwd,rawOut,rawErr,nativeDirectory)};
  } catch(error) {
    receipt={argv,cwd,started,ended:new Date().toISOString(),exit:127,stdout:"",stderr:String(error)};
  }
  const sequence=commandSequence++;
  writeFileSync(resolve(dir,`${String(sequence).padStart(2,"0")}-command.json`),JSON.stringify(receipt,null,2)+"\n");
  return receipt;
}
let commandSequence=0;
