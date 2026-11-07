"""Collect task-local receipts without editing product, old evidence or mission state."""
import json
from pathlib import Path
import record

ROOT, OUT = record.ROOT, record.OUT
REPORT = '.agents/results/result-backend-task-4-attempt-5-b137e253-def6-46ef-a75d-25ab414784ea.md'
CHECKS = ('cli-prerequisite', 'focused', 'fmt', 'clippy', 'workspace-test', 'harness', 'diff-check')


def collect():
    before = json.loads((OUT / 'before-boundary.json').read_text())
    for item in before:
        path = ROOT / item['path']
        assert record.sha(path.read_bytes()) == item['sha256'], item['path']
        assert oct(path.stat().st_mode & 0o777) == item['mode'], item['path']
    record.save('boundary-check.json', {'checkedFiles': len(before), 'productFilesChanged': [],
                                      'sourceDocsHarnessUnchanged': True})
    fingerprint = record.fingerprint()
    checks = {}
    for name in CHECKS:
        receipt = json.loads((OUT / (name + '.json')).read_text())
        assert receipt['exitCode'] == (1 if name == 'harness' else 0), name
        assert receipt['processExitObserved'] and not receipt['timedOut'], name
        assert receipt['sourceBefore'] == receipt['sourceAfter'] == fingerprint['sha256'], name
        assert record.sha((OUT / receipt['output']).read_bytes()) == receipt['outputSha256'], name
        checks[name] = {'receipt': str((OUT / (name + '.json')).relative_to(ROOT)),
                        'argv': receipt['argv'], 'exitCode': receipt['exitCode']}
    previous = json.loads((OUT / 'previous-evidence.json').read_text())
    historical = {}
    remap = {str((OUT.parent / name).relative_to(ROOT)):
             str((OUT / ('previous-' + name)).relative_to(ROOT))
             for name in ('delivery.md', 'evidence.json', 'claim.json')}
    # Older source manifests remain in their original receipts. They do not
    # substitute for current source bindings where downstream fixtures changed.
    for item in previous['artifacts'] + previous['historicalArtifacts']:
        retained = dict(item)
        retained['path'] = remap.get(item['path'], item['path'])
        if retained['path'] != item['path']:
            retained['originalPath'] = item['path']
        assert record.sha((ROOT / retained['path']).read_bytes()) == item['sha256'], item['path']
        historical[(retained['path'], retained['sha256'])] = retained
    paths = {p for p in OUT.iterdir() if p.is_file() and p.name not in
             ('collection-check.json', 'collection-check.txt')}
    paths.update(OUT.parent / name for name in ('delivery.md', 'claim.json'))
    paths.add(ROOT / REPORT)
    artifacts = [{'path': str(p.relative_to(ROOT)), 'sha256': record.sha(p.read_bytes()),
                  'bytes': p.stat().st_size} for p in sorted(paths)]
    evidence = {'version': 3, 'recordedAt': record.now(), 'actorId': 'backend',
        'taskId': 'task-4', 'attempt': 5, 'status': 'ready-for-independent-task-review',
        'skillsUsed': ['oma-backend'], 'changedProductPaths': [],
        'sourceWorkspace': str(ROOT), 'receiptScope': 'Actual isolated Actor checkout; not receiving-checkout execution',
        'baselineHead': '42f0178ec8c7542b010a7d43c1fb1d6daed8d8a0',
        'baselineTree': 'c802f71ac4d91ed6b8a602d5c83bf9cf57116b76',
        'snapshotCommit': '2fe61708aeffcd3638c7e9331a3ef3b4f8394ddf',
        'snapshotTree': '327fef63bace20053064f9662e5b1a5f7a2ea761',
        'sourceFileCount': len(fingerprint['files']), 'sourceFingerprint': fingerprint['sha256'],
        'sourceHashes': fingerprint['files'], 'finalChecks': checks,
        'historicalArtifacts': list(historical.values()), 'artifacts': artifacts,
        'authoritativeReportChannel': 'ARC_RUNTIME_REPORT', 'reportMaxBytes': 4096,
        'stdoutSubstitutionSuperseded': True, 'migrationUnchanged': True,
        'harnessUnchanged': True, 'harnessExitCode': 1,
        'unresolvedHarnessFailures': ['resident.rs: 901 lines', 'arc-ipc/src/lib.rs: 551 lines',
                                      'missing test:coverage'],
        'independentReviewComplete': False, 'postCollectionVerified': False,
        'missionCompletionClaimed': False}
    (OUT.parent / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'artifacts': len(artifacts), 'historicalEntries': len(historical),
                      'sourceFingerprint': fingerprint['sha256']}))


if __name__ == '__main__':
    collect()
