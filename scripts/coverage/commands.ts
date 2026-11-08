import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
export type Receipt = {argv:string[];cwd:string;started:string;ended:string;exit:number;stdout:string;stderr:string;pid?:number};
export async function command(argv:string[],dir:string,cwd:string,env:Record<string,string|undefined>=process.env,timeout=3600000):Promise<Receipt> {
  const started=new Date().toISOString();
  let receipt:Receipt;
  try {
    const child=Bun.spawn(argv,{cwd,env,stdout:"pipe",stderr:"pipe"});
    const timer=setTimeout(()=>child.kill("SIGTERM"),timeout);
    const [stdout,stderr,exit]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
    clearTimeout(timer);
    receipt={argv,cwd,started,ended:new Date().toISOString(),exit,stdout,stderr,pid:child.pid};
  } catch(error) {
    receipt={argv,cwd,started,ended:new Date().toISOString(),exit:127,stdout:"",stderr:String(error)};
  }
  const sequence=commandSequence++;
  writeFileSync(resolve(dir,`${String(sequence).padStart(2,"0")}-command.json`),JSON.stringify(receipt,null,2)+"\n");
  return receipt;
}
let commandSequence=0;
