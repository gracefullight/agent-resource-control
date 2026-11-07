"""Read-only receiving-checkout validation; does not imply independent task review."""
import json
import subprocess
import record

ROOT, OUT = record.ROOT, record.OUT


def verify():
    evidence = json.loads((OUT.parent / 'evidence.json').read_text())
    for item in evidence['sourceHashes'] + evidence['artifacts'] + evidence['historicalArtifacts']:
        path = ROOT / item['path']
        assert record.sha(path.read_bytes()) == item['sha256'], item['path']
        if 'bytes' in item:
            assert path.stat().st_size == item['bytes'], item['path']
        if 'mode' in item:
            assert oct(path.stat().st_mode & 0o777) == item['mode'], item['path']
    for name, check in evidence['finalChecks'].items():
        receipt = json.loads((ROOT / check['receipt']).read_text())
        assert receipt['argv'] == check['argv'] and receipt['exitCode'] == check['exitCode']
        assert receipt['exitCode'] == (1 if name == 'harness' else 0)
        assert receipt['processExitObserved'] and not receipt['timedOut']
        assert receipt['sourceBefore'] == receipt['sourceAfter'] == evidence['sourceFingerprint']
        assert record.sha((OUT / receipt['output']).read_bytes()) == receipt['outputSha256']
        assert receipt['environment']['CARGO_NET_OFFLINE'] == 'true'
        assert receipt['toolchain'] and receipt['cwd'] == evidence['sourceWorkspace']
    for name in ('focused-cache-denied', 'focused-missing-cli'):
        receipt = json.loads((OUT / (name + '.json')).read_text())
        assert receipt['exitCode'] == 101 and receipt['processExitObserved']
        assert not receipt['timedOut']
        assert receipt['sourceBefore'] == receipt['sourceAfter'] == evidence['sourceFingerprint']
        assert record.sha((OUT / receipt['output']).read_bytes()) == receipt['outputSha256']
    summary = json.loads((OUT / 'acceptance-summary.json').read_text())
    red = json.loads((ROOT / summary['historicalRed']['receipt']).read_text())
    assert red['exitCode'] == 101
    red_log = ROOT / summary['historicalRed']['receipt']
    red_log = red_log.with_name(red['output'])
    assert record.sha(red_log.read_bytes()) == red['outputSha256'] == summary['historicalRed']['logSha256']
    assert summary['focusedPassed'] == 52
    assert json.loads((OUT / 'workspace-summary.json').read_text())['passedInvocations'] == 173
    for item in json.loads((OUT / 'before-boundary.json').read_text()):
        path = ROOT / item['path']
        assert record.sha(path.read_bytes()) == item['sha256'], item['path']
        assert oct(path.stat().st_mode & 0o777) == item['mode'], item['path']
    assert record.fingerprint()['sha256'] == evidence['sourceFingerprint']
    for path, item in json.loads((OUT / 'preservation.json').read_text()).items():
        accepted = subprocess.check_output(['git', 'show', evidence['baselineHead'] + ':' + path], cwd=ROOT)
        assert accepted == (ROOT / path).read_bytes() and record.sha(accepted) == item['sha256']
    for commit, tree in [(evidence['baselineHead'], evidence['baselineTree']),
                         (evidence['snapshotCommit'], evidence['snapshotTree'])]:
        assert subprocess.check_output(['git', 'rev-parse', commit + '^{tree}'], cwd=ROOT).decode().strip() == tree
    assert evidence['authoritativeReportChannel'] == 'ARC_RUNTIME_REPORT'
    assert evidence['reportMaxBytes'] == 4096 and evidence['changedProductPaths'] == []
    assert not evidence['independentReviewComplete'] and not evidence['postCollectionVerified']
    assert not evidence['missionCompletionClaimed']
    return {'status': 'matches-current-checkout', 'sourceFiles': evidence['sourceFileCount'],
            'artifactsVerified': len(evidence['artifacts']),
            'historicalEntriesVerified': len(evidence['historicalArtifacts']),
            'checksVerified': len(evidence['finalChecks']),
            'sourceFingerprint': evidence['sourceFingerprint'],
            'independentReviewComplete': False, 'postCollectionVerified': False}


if __name__ == '__main__':
    print(json.dumps(verify(), indent=2))
