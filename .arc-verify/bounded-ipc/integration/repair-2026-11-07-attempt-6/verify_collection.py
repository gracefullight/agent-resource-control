"""Read-only supplemental collection verification; no runtime workloads."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent


def main():
    index = json.loads((OUT / 'index.json').read_text())
    for item in index['artifacts']:
        data = (ROOT / item['path']).read_bytes()
        assert len(data) == item['bytes'], item['path']
        assert hashlib.sha256(data).hexdigest() == item['sha256'], item['path']
    for item in index['preservedEvidence']:
        assert hashlib.sha256((ROOT / item['path']).read_bytes()).hexdigest() == item['sha256']
    result = subprocess.run([sys.executable, '-B',
        '.arc-verify/bounded-ipc/integration/repair-2026-11-07-attempt-5/verify.py'],
        cwd=ROOT, capture_output=True, timeout=60)
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    assert result.returncode == 0, 'Original collection checker failed'
    print(json.dumps({'supplementalArtifactsVerified': len(index['artifacts']),
        'receivingCheckout': str(ROOT), 'productChanged': False,
        'independentReviewClaimed': False, 'missionCompletionClaimed': False}))


if __name__ == '__main__':
    main()
