#!/usr/bin/env python3
"""Capture accepted Git bytes and bounded baseline receipts; no product writes."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import signal
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
REV = "a905653d557f4c5246a1f656042cb8d29e46c052"
RESIDENT = "crates/arc-daemon/tests/resident.rs"
IPC = "crates/arc-ipc/src/lib.rs"
SUPPORT = "crates/arc-daemon/tests/support/mod.rs"
SCOPES = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
          "mise.toml", ".cargo", "scripts/harness", ".arc-verify/module-conformance"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def accepted(path):
    return git("show", f"{REV}:{path}")


def write_json(name, value):
    (OUT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def write_rows(name, rows):
    # One complete record per line; valid JSON array, not truncated output.
    (OUT / name).write_text("[\n" + ",\n".join(
        json.dumps(x, ensure_ascii=False, separators=(",", ":")) for x in rows) + "\n]\n")


def source_fingerprint():
    seed = json.loads((OUT / "fingerprint-files.json").read_text())
    paths = {x["path"] for x in seed}
    paths.update(git("ls-files", "--cached", "--others", "--exclude-standard", "--",
                     "crates", "scripts/harness", ".cargo").decode().splitlines())
    rows = []
    for path in sorted(paths):
        file = ROOT / path
        try:
            data = file.read_bytes()
            rows.append({"path": path, "mode": oct(file.stat().st_mode & 0o777),
                         "sha256": sha(data), "bytes": len(data)})
        except FileNotFoundError:
            rows.append({"path": path, "missing": True})
    data = json.dumps(rows, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return sha(data)


def record(name, argv, timeout=120, env=None):
    started = dt.datetime.now(dt.timezone.utc).isoformat()
    before = time.monotonic()
    fingerprint_before = source_fingerprint()
    process = subprocess.Popen(argv, cwd=ROOT, env={**os.environ, **(env or {})},
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               start_new_session=True)
    write_json(name + ".running.json", {"command": argv, "startedAt": started,
                                       "pid": process.pid, "ownedProcessGroup": process.pid,
                                       "timeoutSeconds": timeout,
                                       "sourceFingerprintBefore": fingerprint_before})
    try:
        data, _ = process.communicate(timeout=timeout)
        code, timed_out = process.returncode, False
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            data, _ = process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            data, _ = process.communicate()
        code, timed_out = process.returncode, True
    lines = data.splitlines(keepends=True)
    parts = []
    for index, offset in enumerate(range(0, len(lines), 400), 1):
        part = f"{name}.part-{index:02}.txt"
        chunk = b"".join(lines[offset:offset + 400])
        (OUT / part).write_bytes(chunk)
        parts.append({"path": part, "sha256": sha(chunk), "lines": chunk.count(b"\n")})
    receipt = {"command": argv, "cwd": str(ROOT), "environmentOverrides": env or {},
               "startedAt": started, "elapsedSeconds": round(time.monotonic() - before, 6),
               "exitCode": code, "timedOut": timed_out, "outputSha256": sha(data),
               "outputBytes": len(data), "outputParts": parts, "pid": process.pid,
               "processReaped": process.poll() is not None,
               "acceptedRevision": git("rev-parse", "HEAD").decode().strip(),
               "sourceFingerprintBefore": fingerprint_before,
               "sourceFingerprintAfter": source_fingerprint()}
    write_json(name + ".json", receipt)
    print(json.dumps({"name": name, "exitCode": code, "timedOut": timed_out,
                      "bytes": len(data), "tail": data.decode(errors="replace")[-1600:]}), flush=True)
    return receipt


# Scoped lexical inventory: retain literal bytes as tokens; ignore braces inside
# strings/comments. This is not a general Rust parser or semantic proof.
LEX = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|(?:br|r)(?P<h>\#*)"[\s\S]*?"(?P=h)'
                 r'|b?"(?:\\[\s\S]|[^"\\])*"|b?\'(?:\\.|[^\'\\\n])\''
                 r'|[A-Za-z_][A-Za-z_0-9]*|\d[\w.]*|[^\s]', re.MULTILINE)


def lex(source):
    return [(m.start(), m.end(), m.group()) for m in LEX.finditer(source)
            if not m.group().startswith(("//", "/*"))]


def token_sha(source):
    return sha(json.dumps([t[2] for t in lex(source)], ensure_ascii=False,
                          separators=(",", ":")).encode())


def declarations(path, first_line=1):
    source = accepted(path).decode()
    tokens = lex(source)
    rows = []
    pattern = r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(struct|impl|const|static|type|use|mod)\s+([^\n]+)"
    for match in re.finditer(pattern, source, re.MULTILINE):
        start = match.start()
        line = source.count("\n", 0, start) + 1
        if line < first_line:
            continue
        depth, end = 0, None
        for token_start, token_end, token in tokens:
            if token_start < start:
                continue
            if token in ["{", "(", "["]:
                depth += 1
            elif token in ["}", ")", "]"]:
                depth -= 1
                if depth == 0 and token == "}" and match.group(1) in ["struct", "impl", "mod"]:
                    end = token_end
                    break
            elif token == ";" and depth == 0:
                end = token_end
                break
        assert end is not None, (path, line)
        rows.append({"path": path, "kind": match.group(1), "header": match.group().strip(),
                     "range": [line, source.count("\n", 0, end - 1) + 1],
                     "rawSha256": sha(source[start:end].encode()),
                     "tokenSha256": token_sha(source[start:end])})
    return rows


def inventory(path, first_line=1):
    source = accepted(path).decode()
    tokens = lex(source)
    mask = list(source)
    for start, end, text in tokens:
        if text.startswith(('"', 'b"', 'r"', 'r#', 'br"', 'br#', "'", "b'")):
            mask[start:end] = ["\n" if c == "\n" else " " for c in source[start:end]]
    # Also mask comments (not present in tokens).
    for match in LEX.finditer(source):
        if match.group().startswith(("//", "/*")):
            mask[match.start():match.end()] = ["\n" if c == "\n" else " " for c in match.group()]
    mask = "".join(mask)
    line = lambda offset: source.count("\n", 0, offset) + 1
    pairs, stack = {}, []
    for pos, char in enumerate(mask):
        if char == "{":
            stack.append(pos)
        elif char == "}":
            begin = stack.pop()
            pairs[begin] = pos + 1
    assert not stack, path
    impls = []
    for match in re.finditer(r"^impl\s+([^\n{]+)\s*\{", mask, re.MULTILINE):
        begin = mask.index("{", match.start())
        impls.append((begin, pairs[begin], match.group(1).strip()))
    functions = []
    pattern = r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)"
    for match in re.finditer(pattern, mask, re.MULTILINE):
        if line(match.start()) < first_line:
            continue
        begin = mask.index("{", match.end())
        end = pairs[begin]
        start_line = line(match.start())
        attrs = []
        prior = start_line - 2
        source_lines = source.splitlines()
        while prior >= 0 and source_lines[prior].lstrip().startswith("#["):
            attrs.insert(0, {"line": prior + 1, "text": source_lines[prior].strip()})
            prior -= 1
        name = match.group(1)
        impl = next((i[2] for i in impls if i[0] < match.start() < i[1]), None)
        qualified = ("tests::" if path == IPC else "support::" if path == SUPPORT else "")
        if impl and " for " in impl:
            trait, target = impl.split(" for ", 1)
            qualified += f"<{target} as {trait}>::"
        elif impl:
            qualified += f"{impl}::"
        qualified += name
        parents = [f for f in functions if f["_start"] < match.start() < f["_end"]]
        if parents:
            parent = min(parents, key=lambda f: f["_end"] - f["_start"])
            qualified = parent["qualifiedName"] + "::" + name
        is_test = any(a["text"] == "#[test]" for a in attrs)
        functions.append({"id": f"{path}:{start_line}:{name}", "path": path,
                          "name": name, "qualifiedName": qualified,
                          "kind": "test" if is_test else "helper",
                          "range": [start_line, line(end - 1)],
                          "bodyRange": [line(begin), line(end - 1)],
                          "attributes": attrs, "enclosingImpl": impl,
                          "rawItemSha256": sha(source[match.start():end].encode()),
                          "rawBodySha256": sha(source[begin:end].encode()),
                          "bodyTokenSha256": token_sha(source[begin:end]),
                          "_start": match.start(), "_end": end})
    assertions = []
    for match in re.finditer(r"\b(assert(?:_eq|_ne)?|debug_assert(?:_eq|_ne)?|panic|unreachable)!", mask):
        if line(match.start()) < first_line:
            continue
        owners = [f for f in functions if f["_start"] <= match.start() < f["_end"]]
        owner = min(owners, key=lambda f: f["_end"] - f["_start"]) if owners else None
        assert owner is not None, (path, line(match.start()))
        assertions.append({"path": path, "line": line(match.start()),
                           "column": match.start() - source.rfind("\n", 0, match.start()),
                           "macro": match.group(1), "ownerId": owner["id"]})
    for function in functions:
        function["assertionLines"] = [a["line"] for a in assertions if a["ownerId"] == function["id"]]
        del function["_start"], function["_end"]
    attributes = [{"path": path, "line": n, "text": text.strip()}
                  for n, text in enumerate(source.splitlines(), 1)
                  if n >= first_line and text.lstrip().startswith("#[")]
    return functions, assertions, attributes


def capture():
    assert git("rev-parse", "HEAD").decode().strip() == REV
    # Git tree metadata is the accepted-path inventory, not native content search.
    tree = git("ls-tree", "-r", "--name-only", REV, "--", "crates", "scripts/harness",
               "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", ".cargo",
               "docs/architecture/CONSTRAINTS.md", "docs/harness", ".agents/oma-config.cue").decode().splitlines()
    rows = []
    for path in sorted(tree):
        data = accepted(path)
        try:
            current = (ROOT / path).read_bytes()
            current_hash, error = sha(current), None
        except OSError as exc:
            current_hash, error = None, str(exc)
        rows.append({"path": path, "gitBlob": git("rev-parse", f"{REV}:{path}").decode().strip(),
                     "sha256": sha(data), "bytes": len(data), "lines": data.count(b"\n"),
                     "workingSha256": current_hash, "workingMatchesAccepted": current_hash == sha(data),
                     "readError": error})
    write_rows("source-files.json", rows)
    funcs, asserts, attrs, decls = [], [], [], []
    for path, first in [(RESIDENT, 1), (IPC, 355), (SUPPORT, 1)]:
        f, a, at = inventory(path, first)
        funcs.extend(f)
        asserts.extend(a)
        attrs.extend(at)
        decls.extend(declarations(path, first))
    write_rows("inventory.json", funcs)
    write_rows("assertions.json", asserts)
    write_rows("attributes.json", attrs)
    write_rows("declarations.json", decls)
    ipc_prefix = b"".join(accepted(IPC).splitlines(keepends=True)[:354])
    config_paths = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml",
                    ".cargo/config", ".cargo/config.toml", ".gitignore", ".agents/oma-config.cue",
                    ".agents/oma-config.local.cue", ".agents/oma-config.yaml", ".agents/oma-config.local.yaml"]
    fingerprint_paths = sorted(set(tree + config_paths))
    fingerprint = []
    for path in fingerprint_paths:
        file = ROOT / path
        try:
            content = file.read_bytes()
            fingerprint.append({"path": path, "mode": oct(file.stat().st_mode & 0o777),
                                "sha256": sha(content), "bytes": len(content)})
        except FileNotFoundError:
            fingerprint.append({"path": path, "missing": True})
    write_rows("fingerprint-files.json", fingerprint)
    canonical = json.dumps(fingerprint, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    baseline = {"schemaVersion": 1, "acceptedRevision": REV,
                "capturedAt": dt.datetime.now(dt.timezone.utc).isoformat(),
                "task": "task-1", "actor": "technical-director", "skill": "oma-refactor",
                "workspace": str(ROOT), "scope": "preservation baseline only; product unchanged",
                "sourceFiles": "source-files.json", "inventory": "inventory.json",
                "assertions": "assertions.json", "attributes": "attributes.json",
                "declarations": "declarations.json",
                "targets": [r for r in rows if r["path"] in [RESIDENT, IPC]],
                "testCounts": {p: sum(f["kind"] == "test" for f in funcs if f["path"] == p)
                               for p in [RESIDENT, IPC]},
                "helperCount": sum(f["kind"] == "helper" for f in funcs),
                "assertionSiteCount": len(asserts), "attributeCount": len(attrs),
                "ipcProductionPrefix": {"path": IPC, "range": [1, 354], "sha256": sha(ipc_prefix),
                                        "boundary": "bytes preceding #[cfg(test)] mod tests"},
                "protectedPaths": [r["path"] for r in rows if r["path"] not in [RESIDENT, IPC]],
                "fingerprint": {"algorithm": "sha256", "value": sha(canonical),
                                "records": "fingerprint-files.json", "contract": "fingerprint.md"},
                "mappingContract": "mapping-contract.json", "fixtureInvariants": "baseline.md",
                "codeIntelligence": {"provider": "serena", "mode": "unavailable",
                                     "tools": [], "fallback": "known-path reads and Git object/path metadata only",
                                     "limitation": "No recursive native source text/symbol search; no provider changes"},
                "cleanliness": {"globalVerified": False, "reason": "git status --short reports av.yaml: Operation not permitted",
                                "scopedEvidence": ["scoped-worktree-diff.json", "scoped-staged-diff.json", "scoped-untracked.json"]},
                "environment": {"os": platform.platform(), "uname": list(platform.uname()),
                                "cpuCount": os.cpu_count(),
                                "tools": {k: shutil.which(k) for k in ["cargo", "rustc", "rustup", "node", "bun", "lsof", "cc", "python3"]},
                                "limits": {k: list(resource.getrlimit(getattr(resource, k)))
                                           for k in ["RLIMIT_CPU", "RLIMIT_AS", "RLIMIT_NOFILE"]}},
                "budget": {"missionMaxDurationSec": 86400, "maxParallel": 3, "maxRuns": 200,
                           "maxRetries": 3, "localProcessesAtOnce": 1, "perCommandTimeoutSec": 120,
                           "downloads": False, "installs": False, "monetaryCost": "unknown; local native checks only"},
                "limits": ["One baseline run does not establish flakiness rate, coverage or mutation strength",
                           "No OMA claim/run/session path was injected; do not create or mutate mission state",
                           "Separate final workspace, fmt, clippy and harness receipts belong to later tasks"]}
    write_json("baseline.json", baseline)
    print(json.dumps({"files": len(rows), "tests": baseline["testCounts"], "helpers": baseline["helperCount"],
                      "assertionSites": len(asserts), "attributes": len(attrs),
                      "fingerprint": baseline["fingerprint"]["value"]}), flush=True)


if __name__ == "__main__":
    if sys.argv[1:] == ["capture"]:
        capture()
    elif len(sys.argv) > 3 and sys.argv[1] == "record":
        record(sys.argv[2], sys.argv[3:], env={"CARGO_NET_OFFLINE": "true", "RUSTUP_AUTO_INSTALL": "0"})
    else:
        raise SystemExit("usage: capture.py capture | record NAME COMMAND [ARG...]")
