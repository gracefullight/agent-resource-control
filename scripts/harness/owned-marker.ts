import { dlopen, FFIType, ptr } from "bun:ffi";
import { closeSync, mkdtempSync, openSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

type Identity = { pid: number; start: string; zombie?: boolean };
const proc = process.platform === "darwin" ? dlopen("/usr/lib/libproc.dylib", {
  proc_listpidspath: { args: [FFIType.u32, FFIType.u32, FFIType.ptr, FFIType.u32, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  proc_listpids: { args: [FFIType.u32, FFIType.u32, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
  proc_pidinfo: { args: [FFIType.i32, FFIType.i32, FFIType.u64, FFIType.ptr, FFIType.i32], returns: FFIType.i32 },
}) : undefined;
const libc = dlopen(process.platform === "darwin" ? "/usr/lib/libSystem.B.dylib" : "libc.so.6", {
  flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 },
  getpgid: { args: [FFIType.i32], returns: FFIType.i32 },
});
const darwinSysctl = process.platform === "darwin" ? dlopen("/usr/lib/libSystem.B.dylib", {
  sysctl: { args: [FFIType.ptr, FFIType.u32, FFIType.ptr, FFIType.ptr, FFIType.ptr, FFIType.u64], returns: FFIType.i32 },
}) : undefined;
const ownerVariable = "ARC_TEST_OWNERSHIP";
const processArguments = Buffer.alloc(1024 * 1024);

// The marker is also inherited as an environment value. Unlike a descriptor,
// this survives a fixture closing all inherited FDs or starting a new session.
// Only an exact private marker plus a stable native start identity grants
// ownership; command names and numeric PIDs alone never do.
function hasEnvironmentMarker(pid: number, path: string, requireKnown = false): boolean {
  const marker = `${ownerVariable}=${path}`;
  let bytes: Buffer;
  if (process.platform === "darwin") {
    const info = Buffer.alloc(136);
    if (proc!.symbols.proc_pidinfo(pid, 3, 0, ptr(info), info.length) !== info.length ||
        info.readUInt32LE(20) !== process.getuid?.()) return false;
    const mib = new Int32Array([1, 49, pid]), length = Buffer.alloc(8);
    bytes = processArguments;
    length.writeBigUInt64LE(BigInt(bytes.length));
    if (darwinSysctl!.symbols.sysctl(ptr(mib), mib.length, ptr(bytes), ptr(length), null, 0) !== 0) {
      if (!requireKnown) return false;
      const current = identity(pid);
      if (!current || current.zombie) return false;
      throw Error(`cannot inspect ownership environment of live pid ${pid}`);
    }
    bytes = bytes.subarray(0, Number(length.readBigUInt64LE()));
    if (bytes.length < 4) return false;
    const argc = bytes.readUInt32LE(0);
    let index = bytes.indexOf(0, 4) + 1;
    if (!index) return false;
    while (index < bytes.length && bytes[index] === 0) index++;
    for (let arg = 0; arg < argc; arg++) {
      index = bytes.indexOf(0, index) + 1;
      if (!index) return false;
    }
    bytes = bytes.subarray(index);
  } else if (process.platform === "linux") {
    try {
      if (statSync(`/proc/${pid}`).uid !== process.getuid?.()) return false;
      bytes = readFileSync(`/proc/${pid}/environ`);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
      if (!requireKnown) return false;
      const current = identity(pid);
      if (!current || current.zombie) return false;
      throw Error(`cannot inspect ownership environment of live pid ${pid}: ${error}`);
    }
  } else throw Error(`owned fixture marker unsupported on ${process.platform}`);
  return bytes.toString().split("\0").includes(marker);
}

function environmentMembers(path: string): number[] {
  let ids: number[];
  if (process.platform === "darwin") {
    for (let count = 128; count <= 16_384; count *= 2) {
      const buffer = Buffer.alloc(count * 4);
      const bytes = proc!.symbols.proc_listpids(1, 0, ptr(buffer), buffer.length);
      if (bytes < 0) throw Error("cannot enumerate ownership environments");
      if (bytes < buffer.length) {
        ids = Array.from({ length: bytes / 4 }, (_, i) => buffer.readInt32LE(i * 4)).filter(pid => pid > 0);
        return ids.filter(pid => pid !== process.pid && hasEnvironmentMarker(pid, path));
      }
    }
    throw Error("ownership environment process limit exceeded");
  }
  if (process.platform !== "linux") throw Error(`owned fixture marker unsupported on ${process.platform}`);
  ids = readdirSync("/proc").filter(name => /^\d+$/.test(name)).map(Number);
  return ids.filter(pid => pid !== process.pid && hasEnvironmentMarker(pid, path));
}

export function processGroup(pid: number, options: {
  expectedStart?: string; getGroup?: (pid: number) => number;
  observe?: (pid: number) => Identity | undefined; retry?: () => boolean;
} = {}): number | undefined {
  const getGroup = options.getGroup ?? ((value: number) => libc.symbols.getpgid(value));
  const observe = options.observe ?? identity;
  const deadline = performance.now() + 60_000, pause = new Int32Array(new SharedArrayBuffer(4));
  const retry = options.retry ?? (() => {
    if (performance.now() >= deadline) return false;
    Atomics.wait(pause, 0, 0, 5);
    return true;
  });
  for (;;) {
    const group = getGroup(pid);
    if (group >= 0) return group;
    const current = observe(pid);
    if (!current || current.zombie || (options.expectedStart && current.start !== options.expectedStart)) return undefined;
    // Darwin can report ESRCH while the identity is transitioning to zombie.
    // Re-observe it until the shared cleanup deadline, rather than treating a
    // transient lookup failure as an unverified surviving process.
    if (!retry()) throw Error(`cannot verify owned process group for pid ${pid}`);
  }
}

export function groupMembers(group: number): number[] {
  if (process.platform !== "darwin") {
    return readdirSync("/proc").filter(name => /^\d+$/.test(name)).map(Number).filter(pid => {
      try {
        const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
        return Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[2]) === group;
      } catch { return false; }
    });
  }
  for (let count = 128; count <= 16_384; count *= 2) {
    const ids = Buffer.alloc(count * 4);
    const bytes = proc!.symbols.proc_listpids(2, group, ptr(ids), ids.length);
    if (bytes < 0) throw Error(`cannot enumerate owned process group ${group}`);
    if (bytes < ids.length) return Array.from({ length: bytes / 4 }, (_, index) => ids.readInt32LE(index * 4)).filter(pid => pid > 0);
  }
  throw Error("owned process group limit exceeded");
}

export function identity(pid: number): Identity | undefined {
  if (process.platform === "darwin") {
    const info = Buffer.alloc(136);
    if (proc!.symbols.proc_pidinfo(pid, 3, 0, ptr(info), info.length) === info.length && info.readUInt32LE(12) === pid) {
      return { pid, start: `${info.readBigUInt64LE(120)}:${info.readBigUInt64LE(128)}`, zombie: info.readUInt32LE(4) === 5 };
    }
    // Darwin's live proc_pidinfo can disappear before a zombie is reaped.
    // Read the native kinfo_proc prefix to retain its exact start identity.
    const mib = new Int32Array([1, 14, 1, pid]), processInfo = Buffer.alloc(4096), length = Buffer.alloc(8);
    length.writeBigUInt64LE(BigInt(processInfo.length));
    const result = darwinSysctl!.symbols.sysctl(ptr(mib), mib.length, ptr(processInfo), ptr(length), null, 0);
    if (result === 0 && length.readBigUInt64LE() >= 44n && processInfo.readInt32LE(40) === pid) {
      const seconds = processInfo.readBigInt64LE(0), micros = processInfo.readInt32LE(8);
      if (seconds > 0n && micros >= 0 && micros < 1_000_000) {
        return { pid, start: `${seconds}:${micros}`, zombie: processInfo.readUInt8(36) === 5 };
      }
    }
  } else if (process.platform === "linux") {
    try {
      const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
      const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
      return { pid, start: fields[19], zombie: fields[0] === "Z" };
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
  } else throw Error(`owned fixture marker unsupported on ${process.platform}`);
  try { process.kill(pid, 0); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ESRCH") return undefined; throw error; }
  throw Error(`cannot verify owned process identity ${pid}`);
}

export class OwnedMarker {
  readonly directory: string;
  readonly path: string;
  readonly fd: number;
  constructor(path?: string) {
    this.directory = path ? dirname(path) : mkdtempSync(join(tmpdir(), "arc-owned-"));
    this.path = path ?? join(this.directory, "marker");
    this.fd = openSync(this.path, path ? "r" : "wx+", 0o600);
    if (libc.symbols.flock(this.fd, 2 | 4) !== 0) {
      closeSync(this.fd);
      if (!path) rmSync(this.directory, { recursive: true, force: true });
      throw Error("cannot acquire private owned marker lock");
    }
  }
  closeParent() { closeSync(this.fd); }
  remove() { rmSync(this.directory, { recursive: true, force: true }); }

  private pids(): number[] {
    if (process.platform === "darwin") {
      const path = Buffer.from(`${this.path}\0`);
      for (let count = 128; count <= 16_384; count *= 2) {
        const ids = Buffer.alloc(count * 4);
        const bytes = proc!.symbols.proc_listpidspath(1, 0, ptr(path), 0, ptr(ids), ids.length);
        if (bytes < 0) throw Error("cannot enumerate owned process marker");
        if (bytes < ids.length) {
          return Array.from({ length: bytes / 4 }, (_, index) => ids.readInt32LE(index * 4)).filter(pid => pid > 0);
        }
      }
      throw Error("owned process marker limit exceeded");
    }
    if (process.platform !== "linux") throw Error(`owned fixture marker unsupported on ${process.platform}`);
    const marker = statSync(this.path), result: number[] = [];
    for (const entry of readdirSync("/proc")) {
      if (!/^\d+$/.test(entry)) continue;
      try {
        if (readdirSync(`/proc/${entry}/fd`).some(fd => {
          try { const file = statSync(`/proc/${entry}/fd/${fd}`); return file.dev === marker.dev && file.ino === marker.ino; }
          catch { return false; }
        })) result.push(Number(entry));
      } catch { /* Other users' descriptors are not accessible. */ }
    }
    return result;
  }

  identities(): Identity[] {
    const probe = openSync(this.path, "r");
    let unowned: boolean;
    try { unowned = libc.symbols.flock(probe, 2 | 4) === 0; }
    finally { closeSync(probe); }
    const descriptors = unowned ? [] : this.pids();
    const first = [...new Set([...descriptors, ...environmentMembers(this.path)])]
      .filter(pid => pid !== process.pid).map(identity).filter((value): value is Identity => !!value);
    const second = unowned ? [] : this.pids();
    return first.filter(value => (second.includes(value.pid) || hasEnvironmentMarker(value.pid, this.path, true)) &&
      identity(value.pid)?.start === value.start);
  }
}
