"""Index task-2 artifacts and report actual pre-collection observations."""
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("recorder", HERE / "record.py")
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
FRESH = HERE / "resume-3"
REVIEW = r.ROOT.parents[3]
provenance = json.loads((FRESH / "provenance-before.json").read_text())
checks = []
for name in ("fmt", "clippy", "workspace-test"):
    receipt = json.loads((FRESH / (name + ".json")).read_text())
    log = FRESH / receipt["output"]
    assert r.sha(log.read_bytes()) == receipt["outputSha256"]
    assert receipt["exitCode"] == 0 and not receipt["timedOut"]
    assert receipt["productInputsUnchanged"]
    assert receipt["sourceBefore"] == provenance["productFingerprint"]["sha256"]
    checks.append(dict(receipt=log.relative_to(HERE).with_suffix(".json").as_posix(),
                       **receipt))
test_log = (FRESH / "workspace-test.txt").read_text()
transport = test_log.split("Running tests/transport_unit.rs", 1)[1].split("Running unittests", 1)[0]
assert "running 18 tests" in transport
assert "18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out" in transport
assert sum(line.startswith("test ") and line.endswith(" ... ok")
           for line in transport.splitlines()) == 18
current = r.fingerprint()
assert current == provenance["productFingerprint"]
for p in r.FILES:
    expected = next(x["sha256"] for x in provenance["sourceHashes"] if x["path"] == p)
    assert r.sha((r.ROOT / p).read_bytes()) == expected

paths = [".arc-verify/bounded-ipc/ipc/delivery.md", ".arc-verify/bounded-ipc/ipc/evidence.json"]
collection = {
    "recordedAt": r.now(), "phase": "before Actor return / framework collection",
    "actorWorkspace": str(r.ROOT), "reviewWorkspace": str(REVIEW),
    "ignoreReceipt": r.command(["git", "check-ignore", "--", *paths]),
    "scopedStatus": r.command(["git", "status", "--porcelain=v1", "--untracked-files=all",
                               "--", ".arc-verify/bounded-ipc/ipc"]),
    "reviewArtifacts": [{"path": p, "present": (REVIEW / p).exists(),
                         "sha256": r.sha((REVIEW / p).read_bytes())
                         if (REVIEW / p).is_file() else None} for p in paths],
    "postCollectionVerified": False,
    "requiredNextStep": "Chief/framework collects normally, then verifies delivery/index and all artifact hashes in review checkout. Actor does not manually copy across checkouts or infer collection from non-ignored status.",
}
assert collection["ignoreReceipt"]["exitCode"] == 1
r.save("collection-check.json", collection)
r.OUT = FRESH
r.finish()
assert r.fingerprint() == current
r.OUT = HERE
artifacts = []
for file in sorted(HERE.rglob("*")):
    if not file.is_file() or "__pycache__" in file.parts or file.name == "evidence.json":
        continue
    artifacts.append({"path": file.relative_to(r.ROOT).as_posix(),
                      "bytes": file.stat().st_size, "sha256": r.sha(file.read_bytes())})
evidence = {
    "version": 1, "recordedAt": r.now(), "status": "ready-for-independent-review",
    "identity": {"actorId": "backend", "taskId": "task-2", "attempt": 3,
                 "missionId": "b137e253-def6-46ef-a75d-25ab414784ea",
                 "codexSessionId": "01a1b5e7-ee84-7902-99e0-7f9bf5e2ef8c",
                 "omaRunId": None, "injectedClaimPath": None},
    "skillsUsed": ["oma-backend"],
    "changedPathsScope": ".arc-verify/bounded-ipc/ipc/ only; no product inputs changed",
    "delivery": ".arc-verify/bounded-ipc/ipc/delivery.md",
    "baselineHead": r.BASE, "baselineTree": "c802f71ac4d91ed6b8a602d5c83bf9cf57116b76",
    "originalActorSnapshotCommit": r.OLD,
    "originalActorSnapshotTree": "e79ff4ed85c94059275bbb4992579a2d55f51eb3",
    "repairSnapshotCommit": "eee0c01750bcf852d691395774215b18628e89d7",
    "repairSnapshotTree": "90ce687dd5407eff881a03c666ebe89d79ca66da",
    "currentWorkerRefResolvedInReview": False,
    "sourceHashes": provenance["sourceHashes"],
    "sourceFingerprint": current["sha256"],
    "allNineOriginalHashesMatch": all(x["matchesOriginal"] for x in provenance["sourceHashes"]),
    "reviewOverlayHashesMatchAtObservation": all(x["matchesActor"] for x in provenance["reviewSourceHashes"]),
    "migration": provenance["migration"],
    "checks": checks, "transportUnit": {"passed": 18, "failed": 0, "ignored": 0, "filtered": 0},
    "scope": {"testTargetDeadCodeAllowance": True, "productionModulesRegistered": False,
              "productionBehaviorVerified": False, "combinedWorkerVerified": False,
              "nativePerformanceMeasured": False, "missionAcceptanceComplete": False,
              "postCollectionVerified": False, "independentReviewExecutedByThisActor": False},
    "unresolved": [
        "Framework normal collection plus review-tree artifact/hash validation pending.",
        "Independent reviewer must probe its own noninteractive runner; Codex probe does not prove qwen grantpt recovery.",
        "Task-3 metrics helper/driver repair and independent acceptance remain separate prerequisites.",
        "Task-4 production declarations, lifecycle/worker adaptation, public ERROR_DB_BUSY and actual CLI mapping test remain required.",
        "Later 100000-attempt, shutdown, native measurements, unchanged harness and Phase 0/Linux/Windows obligations remain intact.",
    ],
    "artifacts": artifacts,
    "artifactIndexScope": "All non-pycache files under owned ipc directory except evidence.json itself; no self-hash cycle.",
}
r.save("evidence.json", evidence)
for artifact in evidence["artifacts"]:
    assert r.sha((r.ROOT / artifact["path"]).read_bytes()) == artifact["sha256"]
print(json.dumps({"indexedArtifacts": len(artifacts), "transportTestsPassed": 18,
                  "allFreshGatesExitZero": True, "productInputsUnchanged": True,
                  "postCollectionVerified": False, "evidence": str(HERE / "evidence.json")}))
