#!/usr/bin/env python3
"""Inventory or compact stopped, identified Swift fixture projects only."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys

CACHES = ("scratch path", "module-cache", "clang-cache", "private-home", "config")
EVIDENCE = {"entry.json", "semantic.json", "bodies", "ARCGenerated.swift"}


def real_directory(path):
    return path.is_dir() and not path.is_symlink()


def roots(workspace):
    return (workspace / "target/traffic-native-tools/swift", workspace / "target/traffic-bun-scripts/native")


def validate(workspace, project, manifest):
    if not project.is_absolute() or project != Path(os.path.normpath(project)):
        raise ValueError(f"project path must be absolute and normalized: {project}")
    relative = project.relative_to(workspace)
    cursor = workspace
    for component in relative.parts:
        cursor = cursor / component
        if not real_directory(cursor):
            raise ValueError(f"missing or symlinked directory: {cursor}")
    native, bun = roots(workspace)
    if project.parent.parent == native:
        if not re.fullmatch(r"[0-9]+-[0-9]+", project.parent.name):
            raise ValueError(f"not a Swift tool-owned directory: {project}")
    elif project.parent.parent == bun:
        if not re.match(r"^(daemon|desktop)-swift-", project.parent.name):
            raise ValueError(f"not a Swift scenario directory: {project}")
    else:
        raise ValueError(f"outside owned Swift project locations: {project}")
    package = project / "package path/Package.swift"
    form = project / "form"
    for path in (package.parent, package, form, project / "sdk-path"):
        if path.is_symlink():
            raise ValueError(f"symlinked fixture marker: {path}")
    if package.read_bytes() != manifest or form.read_text() not in ("swift-build", "swift-test"):
        raise ValueError(f"unrecognized Swift fixture: {project}")
    if not (project / "sdk-path").is_file():
        raise ValueError(f"missing Swift fixture SDK marker: {project}")
    for name in CACHES:
        cache = project / name
        if cache.is_symlink() or (cache.exists() and not cache.is_dir()):
            raise ValueError(f"unexpected cache entry: {cache}")
    return project


def discover(workspace, manifest):
    projects = []
    for base in roots(workspace):
        if not base.exists():
            continue
        if not real_directory(base):
            raise ValueError(f"invalid fixture root: {base}")
        for parent in sorted(base.iterdir()):
            if not real_directory(parent):
                continue
            for project in sorted(parent.iterdir()):
                if not real_directory(project) or not (project / "form").is_file():
                    continue
                try:
                    projects.append(str(validate(workspace, project, manifest)))
                except (OSError, ValueError):
                    continue
    return projects


def plugin_evidence(project):
    base = project / "scratch path/plugins/outputs"
    for component in (project / "scratch path", project / "scratch path/plugins", base):
        if component.is_symlink():
            raise ValueError(f"symlinked plugin evidence path: {component}")
    if not base.exists():
        return []
    if not real_directory(base):
        raise ValueError(f"invalid plugin evidence root: {base}")
    pending = [(base, 0)]
    found = []
    visited = 0
    while pending:
        directory, depth = pending.pop()
        visited += 1
        if visited > 4096 or depth > 12:
            raise ValueError(f"plugin evidence traversal limit: {base}")
        for entry in directory.iterdir():
            if entry.is_symlink():
                continue
            if entry.is_dir():
                pending.append((entry, depth + 1))
            elif entry.is_file() and entry.name in EVIDENCE:
                found.append(entry)
    return sorted(found)


def compact(project):
    base = project / "scratch path/plugins/outputs"
    saved = []
    for source in plugin_evidence(project):
        relative = source.relative_to(base)
        destination = project / "failure-plugin-outputs" / relative
        cursor = project
        for component in destination.relative_to(project).parts[:-1]:
            cursor = cursor / component
            if cursor.is_symlink():
                raise ValueError(f"symlinked evidence destination: {cursor}")
            cursor.mkdir(exist_ok=True)
        if destination.is_symlink():
            raise ValueError(f"symlinked evidence destination: {destination}")
        if destination.exists() and destination.read_bytes() != source.read_bytes():
            raise ValueError(f"conflicting retained evidence: {destination}")
        if not destination.exists():
            shutil.copy2(source, destination)
        saved.append({"path": str(destination), "sha256": hashlib.sha256(destination.read_bytes()).hexdigest()})
    for name in CACHES:
        cache = project / name
        if cache.exists():
            shutil.rmtree(cache)
    return saved


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--apply-file", type=Path, help="apply a previously reviewed discovery JSON")
    args = parser.parse_args()
    workspace = args.workspace.resolve(strict=True)
    manifest = (Path(__file__).parent / "sources/Package.swift.txt").read_bytes()
    if args.apply_file is None:
        print(json.dumps({"version": 1, "workspace": str(workspace), "projects": discover(workspace, manifest)}, indent=2))
        return
    plan = json.loads(args.apply_file.read_text())
    if plan.get("version") != 1 or plan.get("workspace") != str(workspace):
        raise ValueError("discovery file has the wrong version or workspace")
    paths = plan.get("projects")
    if not isinstance(paths, list) or not paths or any(not isinstance(path, str) for path in paths) or len(set(paths)) != len(paths):
        raise ValueError("discovery file must name distinct projects")
    projects = [validate(workspace, Path(path), manifest) for path in paths]
    for project in projects:
        print(json.dumps({"project": str(project), "saved": compact(project)}), flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"Swift recovery stopped: {error}", file=sys.stderr)
        sys.exit(1)
