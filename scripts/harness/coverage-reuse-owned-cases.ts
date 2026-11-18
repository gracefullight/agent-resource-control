import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { runOwned } from "./owned-process";
import { identity, OwnedMarker, processGroup } from "./owned-marker";
import { recoverOwned } from "./sync-watchdog";
import { runSync } from "./test-support";
import { retireFixture, withRetiredFixture } from "./fixtures";
import { HARNESS_TEST_WATCHDOG_MS, withAsyncFixture, withFixture } from "./coverage-reuse-test-support";

type CapturedIdentity = NonNullable<ReturnType<typeof identity>>;
function killCaptured(owned: CapturedIdentity | undefined,
  lookup: typeof identity = identity, signal: (pid: number) => void = pid => process.kill(pid,"SIGKILL")) {
  if (owned && lookup(owned.pid)?.start === owned.start) signal(owned.pid);
}

// A fixture publishes its own native start time before the supervisor can
// reap it; a later, reused PID never authorizes test cleanup.
const pythonIdentity = `
def publish_identity(path, pid):
 import ctypes, json, os, sys
 if sys.platform == 'darwin':
  lib = ctypes.CDLL('/usr/lib/libproc.dylib')
  data = ctypes.create_string_buffer(136)
  assert lib.proc_pidinfo(pid, 3, 0, data, 136) == 136
  start = str(int.from_bytes(data.raw[120:128], 'little')) + ':' + str(int.from_bytes(data.raw[128:136], 'little'))
 else:
  stat = open('/proc/%d/stat' % pid).read().rsplit(')', 1)[1].strip().split()
  start = stat[19]
 with open(path + '.pending', 'w') as f: json.dump({'pid': pid, 'start': start}, f)
 os.rename(path + '.pending', path)
`;
const readCaptured = (path: string): CapturedIdentity => JSON.parse(readFileSync(path,"utf8"));

test("process group observation retries an exit transition and bounds persistent errors",()=>{
  const recorded={pid:99999999,start:"recorded"};
  let reads=0, retries=0;
  expect(processGroup(recorded.pid,{
    expectedStart:recorded.start,
    getGroup:()=>{reads++;return -1;},
    observe:()=>reads===1?recorded:{...recorded,zombie:true},
    retry:()=>{retries++;return true;},
  })).toBeUndefined();
  expect(reads).toBe(2);
  expect(retries).toBe(1);

  reads=0;
  expect(processGroup(recorded.pid,{
    expectedStart:recorded.start,
    getGroup:()=>++reads===1?-1:123,
    observe:()=>recorded,
    retry:()=>true,
  })).toBe(123);
  expect(reads).toBe(2);

  retries=0;
  expect(processGroup(recorded.pid,{
    expectedStart:recorded.start,
    getGroup:()=>-1,
    observe:()=>({...recorded,start:"replacement"}),
    retry:()=>{retries++;return true;},
  })).toBeUndefined();
  expect(retries).toBe(0);

  expect(()=>processGroup(recorded.pid,{
    expectedStart:recorded.start,
    getGroup:()=>-1,
    observe:()=>recorded,
    retry:()=>false,
  })).toThrow("cannot verify owned process group");
});

test("cleanup never signals a replaced process identity",()=>{
  const calls:number[]=[];
  const recorded={pid:12345,start:"old"};
  killCaptured(recorded,()=>({pid:recorded.pid,start:"new"}),pid=>calls.push(pid));
  expect(calls).toEqual([]);
  killCaptured(recorded,()=>recorded,pid=>calls.push(pid));
  expect(calls).toEqual([recorded.pid]);
});

for (const mode of ["success", "watchdog"] as const) {
  test(`owned supervisor cleans a descriptor-closing detached child on ${mode}`,()=>withAsyncFixture(async f=>{
    const ready=resolve(f.root,"closed-descriptor-child");
    const program=`import os, signal
${pythonIdentity}
pid=os.fork()
if pid == 0:
 os.setsid()
 n=os.open('/dev/null',os.O_RDWR);os.dup2(n,1);os.dup2(n,2)
 os.closerange(3,1024)
 publish_identity('closed-descriptor-child',os.getpid())
 while True: signal.pause()
while not os.path.exists('closed-descriptor-child'): pass
os.write(1,b'READY')
${mode === "success" ? "os._exit(0)" : "while True: signal.pause()"}`;
    const watchdog=new AbortController();
    let captured:CapturedIdentity|undefined;
    try {
      const run=runOwned(["/usr/bin/python3","-c",program],f.root,process.env,{
        signal:watchdog.signal,
        onStdout:bytes=>{
          if(!bytes.toString().includes("READY")) return;
          captured=readCaptured(ready);
          if(mode==="watchdog") watchdog.abort();
        },
      });
      if(mode==="watchdog") await expect(run).rejects.toThrow("watchdog expired");
      else expect((await run).exit).toBe(0);
      expect(captured).toBeDefined();
      expect(identity(captured!.pid)?.start).not.toBe(captured!.start);
    } finally {
      watchdog.abort();
      if(!captured&&existsSync(ready)) captured=readCaptured(ready);
      try {killCaptured(captured);} catch {}
    }
  }),HARNESS_TEST_WATCHDOG_MS);
}

test("passing harness fixtures are removed and failed fixtures are retained",()=>withFixture(f=>{
  const success=resolve(f.root,"successful-case"), failure=resolve(f.root,"failed-case");
  mkdirSync(success);mkdirSync(failure);
  writeFileSync(resolve(success,"evidence"),"success");
  writeFileSync(resolve(failure,"evidence"),"failure");
  retireFixture(success,true);retireFixture(failure,false);
  expect(existsSync(success)).toBe(false);
  expect(readFileSync(resolve(failure,"evidence"),"utf8")).toBe("failure");
}),HARNESS_TEST_WATCHDOG_MS);

test("a later failed scaling case retains only its own diagnostics",()=>withFixture(f=>{
  const first=resolve(f.root,"first-case"),second=resolve(f.root,"second-case");
  mkdirSync(first);mkdirSync(second);
  expect(()=>{
    withRetiredFixture(first,()=>writeFileSync(resolve(first,"evidence"),"passed"));
    withRetiredFixture(second,()=>{writeFileSync(resolve(second,"evidence"),"failed");throw Error("second case failed");});
  }).toThrow("second case failed");
  expect(existsSync(first)).toBe(false);
  expect(readFileSync(resolve(second,"evidence"),"utf8")).toBe("failed");
}),HARNESS_TEST_WATCHDOG_MS);

test("synchronous fixture commands use a readiness-injected watchdog and clean descendants",()=>withFixture(f=>{
  const ready=resolve(f.root,"sync-child-ready");
  const command=`/usr/bin/mkfifo sync-gate
exec 3<>sync-gate
/bin/cat <&3 >/dev/null 2>&1 &
printf '%s' "$!" > sync-pending
/bin/mv sync-pending sync-child-ready
wait`;
  expect(()=>runSync(["/bin/sh","-c",command],f.root,undefined,{forceWatchdogOn:ready})).toThrow("watchdog expired");
  const pid=Number(readFileSync(ready,"utf8"));
  expect(Number.isInteger(pid)&&pid>0).toBe(true);
  expect(()=>process.kill(pid,0)).toThrow();
}),HARNESS_TEST_WATCHDOG_MS);

test("owned supervisor cleans a detached redirected descendant after the root exits",()=>withAsyncFixture(async f=>{
  const ready=resolve(f.root,"detached-ready");
  const program=`import os, signal
${pythonIdentity}
read, write = os.pipe()
pid = os.fork()
if pid == 0:
    os.close(read)
    os.setsid()
    devnull = os.open('/dev/null', os.O_RDWR)
    os.dup2(devnull, 1); os.dup2(devnull, 2)
    publish_identity('detached-ready', os.getpid())
    os.write(write, b'R'); os.close(write)
    while True: signal.pause()
os.close(write)
assert os.read(read, 1) == b'R'
os.write(1, b'READY')
os._exit(0)`;
  let pid=0, captured:ReturnType<typeof identity>;
  try {
    const result=await runOwned(["/usr/bin/python3","-c",program],f.root,process.env,{
      onStdout:()=>{if(existsSync(ready)){captured=readCaptured(ready);pid=captured.pid;}},
    });
    expect(result.exit).toBe(0);
    pid=readCaptured(ready).pid;
    expect(Number.isInteger(pid)&&pid>0).toBe(true);
    expect(identity(pid)).toBeUndefined();
  } finally {
    try {killCaptured(captured);} catch {}
  }
}),HARNESS_TEST_WATCHDOG_MS);

test("synchronous outer watchdog recovers a stalled supervisor and its detached child",()=>withFixture(f=>{
  const ready=resolve(f.root,"outer-detached-ready");
  const program=`import os, signal
${pythonIdentity}
pid = os.fork()
if pid == 0:
    os.setsid()
    publish_identity('outer-detached-ready', os.getpid())
while True: signal.pause()`;
  let captured:CapturedIdentity|undefined;
  try {
    expect(()=>runSync(["/usr/bin/python3","-c",program],f.root,undefined,{forceOuterWatchdogOn:ready})).toThrow("owned supervisor watchdog or abnormal exit");
    captured=readCaptured(ready);
    expect(Number.isInteger(captured.pid)&&captured.pid>0).toBe(true);
    expect(identity(captured.pid)).toBeUndefined();
  } finally {
    if(!captured&&existsSync(ready)) captured=readCaptured(ready);
    try {killCaptured(captured);} catch {}
  }
}),HARNESS_TEST_WATCHDOG_MS);

test("outer watchdog recovers before anchor identity registration",()=>withFixture(f=>{
  const ready=resolve(f.root,"unregistered-ready");
  const program=`import os, signal, json
${pythonIdentity}
anchor=os.getppid(); pid=os.fork()
if pid == 0:
    os.setsid()
    publish_identity('unregistered-child', os.getpid())
    publish_identity('unregistered-anchor', anchor)
    with open('unregistered-ready', 'w') as f: f.write('ready')
while True: signal.pause()`;
  let captured:CapturedIdentity[]=[];
  try {
    expect(()=>runSync(["/usr/bin/python3","-c",program],f.root,undefined,{
      forceOuterWatchdogOn:ready,forceBeforeRegistration:true,
    })).toThrow("owned supervisor watchdog or abnormal exit");
    captured=[readCaptured(resolve(f.root,"unregistered-child")),readCaptured(resolve(f.root,"unregistered-anchor"))];
    expect(captured).toHaveLength(2);
    for(const child of captured) {expect(Number.isInteger(child.pid)&&child.pid>0).toBe(true);expect(identity(child.pid)).toBeUndefined();}
  } finally {
    if(!captured.length) for(const name of ["unregistered-child","unregistered-anchor"]) {
      const path=resolve(f.root,name); if(existsSync(path)) captured.push(readCaptured(path));
    }
    for(const child of captured) {try {killCaptured(child);} catch {}}
  }
}),HARNESS_TEST_WATCHDOG_MS);

test("a supervisor killed after writing success is not accepted",()=>withFixture(f=>{
  expect(()=>runSync(["/usr/bin/true"],f.root,undefined,{forceAfterCompletion:true}))
    .toThrow("owned supervisor watchdog or abnormal exit");
}),HARNESS_TEST_WATCHDOG_MS);

test("failed supervisor recovery retains ownership evidence for a later cleanup",()=>withFixture(f=>{
  const ready=resolve(f.root,"recovery-ready");
  const program=`import os, signal
pid=os.fork()
if pid == 0:
    os.setsid()
    with open('recovery-pending', 'w') as f: f.write(str(os.getpid()))
    os.rename('recovery-pending', 'recovery-ready')
while True: signal.pause()`;
  const observe=OwnedMarker.prototype.identities;
  let markerPath="", statusDir="", pid=0;
  OwnedMarker.prototype.identities=function(){throw Error("injected ownership observation failure");};
  try {
    let failure="";
    try {runSync(["/usr/bin/python3","-c",program],f.root,undefined,{forceOuterWatchdogOn:ready});}
    catch(error) {failure=String(error);}
    expect(failure).toContain("injected ownership observation failure");
    const retained=/retained (.+\/marker) and (.+): Error: injected ownership observation failure/.exec(failure);
    expect(retained).not.toBeNull();
    markerPath=retained![1];statusDir=retained![2];
    expect(existsSync(markerPath)).toBe(true);
    expect(existsSync(statusDir)).toBe(true);
    pid=Number(readFileSync(ready,"utf8"));
  } finally {
    OwnedMarker.prototype.identities=observe;
    if(markerPath&&statusDir) {
      const marker=Object.create(OwnedMarker.prototype) as OwnedMarker;
      Object.defineProperty(marker,"path",{value:markerPath});
      recoverOwned(marker,resolve(statusDir,"status.json"));
      rmSync(dirname(markerPath),{recursive:true,force:true});
      rmSync(statusDir,{recursive:true,force:true});
    }
  }
  expect(pid).toBeGreaterThan(0);
  expect(identity(pid)).toBeUndefined();
}),HARNESS_TEST_WATCHDOG_MS);

test("watchdog cleans a detached pipe holder after its root exits",()=>withAsyncFixture(async f=>{
  const ready=resolve(f.root,"detached-pipe-ready"), watchdog=new AbortController();
  const program=`import os, signal, sys
${pythonIdentity}
read, write = os.pipe(); parent = os.getpid(); pid = os.fork()
if pid == 0:
    os.close(read); os.setsid(); os.write(write,b'R'); os.close(write)
    while os.getppid() == parent: pass
    publish_identity('detached-pipe-ready', os.getpid())
    print('READY', flush=True)
    while True: signal.pause()
os.close(write); assert os.read(read,1) == b'R'; os._exit(0)`;
  let pid=0, captured:ReturnType<typeof identity>;
  const running=runOwned(["/usr/bin/python3","-c",program],f.root,process.env,{
    signal:watchdog.signal, onStdout:chunk=>{if(chunk.toString().includes("READY")) {captured=readCaptured(ready);pid=captured.pid;watchdog.abort();}},
  });
  try {
    await expect(running).rejects.toThrow("watchdog expired");
    pid=readCaptured(ready).pid;
    expect(Number.isInteger(pid)&&pid>0).toBe(true);
    expect(identity(pid)).toBeUndefined();
  } finally {
    watchdog.abort();
    try {killCaptured(captured);} catch {}
    await running.catch(()=>{});
  }
}),HARNESS_TEST_WATCHDOG_MS);

test("unexpected anchor exit cleans its marked descendants",()=>withAsyncFixture(async f=>{
  const program=`import os,signal
${pythonIdentity}
read,write=os.pipe(); pid=os.fork()
if pid==0:
 os.close(read);os.setsid();n=os.open('/dev/null',os.O_RDWR);os.dup2(n,1);os.dup2(n,2)
 publish_identity('anchor-exit-child',os.getpid())
 os.write(write,b'R');os.close(write)
 while True: signal.pause()
os.close(write);assert os.read(read,1)==b'R'
publish_identity('anchor-exit-root',os.getpid())
with open('anchor-exit-ready','w') as f: f.write('ready')
os.write(1,b'READY');n=os.open('/dev/null',os.O_RDWR);os.dup2(n,1);os.dup2(n,2)
while True: signal.pause()`;
  let anchor:ReturnType<typeof identity>, owned:NonNullable<ReturnType<typeof identity>>[]=[];
  let markerPath="";
  try {
    await expect(runOwned(["/usr/bin/python3","-c",program],f.root,process.env,{
      onAnchor:(pid,start)=>{anchor={pid,start};},
      onStdout:chunk=>{
        if(!chunk.toString().includes("READY")) return;
        owned=[readCaptured(resolve(f.root,"anchor-exit-root")),readCaptured(resolve(f.root,"anchor-exit-child"))];
        if(!anchor||identity(anchor.pid)?.start!==anchor.start) throw Error("anchor identity was lost");
        process.kill(anchor.pid,"SIGKILL");
      },
    }).catch(error=>{markerPath=/marker retained at ([^:]+)/.exec(String(error))?.[1]??"";throw error;})).rejects.toThrow();
    expect(owned).toHaveLength(2);
    for(const child of owned) expect(identity(child.pid)).toBeUndefined();
  } finally {
    for(const child of owned) {
      try {killCaptured(child);} catch {}
    }
    const deadline=Date.now()+60_000;
    for(const child of owned) while(identity(child.pid)?.start===child.start) {
      if(Date.now()>=deadline) throw Error(`test cleanup could not reap ${child.pid}`);
      await new Promise(resolve=>setTimeout(resolve,5));
    }
    if(markerPath) rmSync(dirname(markerPath),{recursive:true,force:true});
  }
}),HARNESS_TEST_WATCHDOG_MS);
