import { closeSync, constants, fstatSync, openSync, statSync } from "node:fs";
import { join } from "node:path";
import { dlopen, FFIType } from "bun:ffi";

// The Rust consumer uses the same stable inode, held through pointer lookup,
// preparation, lease acquisition, and last-consumer retirement.
export function legacyCoordinationPath(root: string): string {
  const stat = statSync(root, { bigint: true });
  return join("/tmp", `arc-legacy-${process.geteuid!()}-${stat.dev}-${stat.ino}.lock`);
}

export function lockLegacyPreparation(root: string, nonblocking = false): () => void {
  const path = legacyCoordinationPath(root);
  const inherited = process.env.ARC_LEGACY_COORDINATION_FD;
  if (inherited !== undefined && !/^[0-9]+$/.test(inherited)) throw Error("LEGACY: invalid coordination descriptor");
  const fd = inherited === undefined
    ? openSync(path, constants.O_RDWR | constants.O_CREAT | constants.O_NOFOLLOW, 0o600)
    : Number(inherited);
  if (!Number.isSafeInteger(fd) || fd < 3) throw Error("LEGACY: invalid coordination descriptor");
  const library = dlopen(process.platform === "darwin" ? "/usr/lib/libSystem.B.dylib" : "libc.so.6", {
    flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 },
  });
  try {
    const file = fstatSync(fd, { bigint: true });
    const named = statSync(path, { bigint: true });
    if (!file.isFile() || file.uid !== BigInt(process.geteuid!()) || (file.mode & 0o7777n) !== 0o600n ||
        file.dev !== named.dev || file.ino !== named.ino) throw Error("LEGACY: invalid coordination lock");
    // An inherited descriptor refers to the parent's already-locked open file
    // description. Never wait on a different descriptor from inside that parent.
    if (library.symbols.flock(fd, inherited === undefined && !nonblocking ? 2 : 2 | 4) !== 0)
      throw Error("LEGACY: cannot acquire coordination lock");
    return () => { closeSync(fd); library.close(); };
  } catch (error) {
    closeSync(fd); library.close(); throw error;
  }
}
