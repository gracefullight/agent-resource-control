"""Independent read-only reconciliation of delivered native coverage receipts."""
from pathlib import Path
from collections import defaultdict
import datetime
import hashlib
import json
import re
import shlex
import subprocess

ROOT = Path.cwd()
QA = ROOT / '.arc-verify/rust-coverage/qa'
BASE = 'fbe76a5f15101ad415f355ef06f1deeb1f97d568'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(Path(path).read_text())


def canonical(path):
    return str((ROOT / path).resolve().relative_to(ROOT))


def main():
    live = load(QA / 'receipts/01-command.json')
    identity = json.loads(live['stdout'])['identity']
    assert all(sha((ROOT / p).read_bytes()) == h for p, h in identity['files'].items())
    serialized = json.dumps(dict(sorted(identity['files'].items())), separators=(',', ':'), ensure_ascii=False)
    assert sha(serialized.encode()) == identity['sha256']
    accepted = load(ROOT / '.arc-verify/rust-coverage/integration/repair-4/acceptance.json')
    commands = []
    measurement_dirs = []
    for entry in accepted['commands']:
        p = ROOT / entry['receipt']
        actual = load(p / '00-command.json' if p.is_dir() else p)
        binding = load(p / 'receipt.json' if p.is_dir() else p.parent / 'binding.json')
        assert actual['argv'] == binding['argv'] == entry['argv']
        assert actual['exit'] == binding['exit'] == 0
        assert binding['sourceHash'] == binding['afterHash'] == identity['sha256']
        assert actual['cwd'] == str(ROOT)
        if p.is_dir():
            assert binding['logs'] == {s: sha((p / (s + '.txt')).read_bytes()) for s in ['stdout', 'stderr']}
            assert actual['stdout'] == (p / 'stdout.txt').read_text()
            assert actual['stderr'] == (p / 'stderr.txt').read_text()
        for d in re.findall(r'Coverage reports: (\S+)', actual['stdout']):
            measurement_dirs.append(ROOT / d)
        commands.append(dict(argv=actual['argv'], exit=actual['exit'], sourceHash=binding['sourceHash'], receipt=entry['receipt'], receiptSha256=sha((p / '00-command.json' if p.is_dir() else p).read_bytes())))
    protected = {}
    for p in ['scripts/harness/validate.sh', 'crates/arc-storage/migrations/0001_phase0.sql']:
        original = subprocess.run(['git', 'show', BASE + ':' + p], capture_output=True, check=True).stdout
        assert original == (ROOT / p).read_bytes()
        protected[p] = sha(original)
    runs = []
    for directory in measurement_dirs:
        inputs = load(directory / 'inputs.json')
        summary = load(directory / 'summary.json')
        inventory = load(directory / 'source-inventory.json')
        receipt = load(directory / 'receipt.json')
        native = load(directory / 'raw.json')
        profile = load(directory / 'profiles.json')
        assert inputs == identity
        assert summary['sourceHash'] == receipt['sourceHash'] == identity['sha256']
        assert receipt['pass'] and receipt['exit'] == 0
        assert sha((directory / 'raw.lcov').read_bytes()) == summary['rawSha256']
        assert sha((directory / 'raw.json').read_bytes()) == summary['llvmSha256']
        assert all(sha((ROOT / p['path']).read_bytes()) == p['sha256'] for p in inventory)
        lc = defaultdict(dict)
        exact = defaultdict(dict)
        aliases = defaultdict(list)
        source = None
        for line in (directory / 'raw.lcov').read_text().splitlines():
            if line.startswith('SF:'):
                exact_name = line[3:]
                source = canonical(line[3:])
                aliases[source].append(line[3:])
            if line.startswith('DA:'):
                n, hit = map(int, line[3:].split(',')[:2])
                exact[exact_name][n] = hit
                lc[source][n] = max(lc[source].get(n, 0), hit)
        # Independently read native sparse segments per source line, before
        # any canonical path union or test-only omission.
        for f in native['data'][0]['files']:
            segments = f['segments']
            mapped = {}
            for n in range(1, max((s[0] for s in segments), default=0) + 1):
                starts = [s for s in segments if s[0] == n]
                earlier = [s for s in segments if s[0] < n]
                carry = earlier[-1] if earlier else None
                region_starts = [s for s in starts if s[3] and s[4] and not s[5]]
                skipped = starts and not starts[0][3] and starts[0][4]
                if (not skipped and ((carry and carry[3]) or region_starts)) or any(s[3] and s[4] for s in starts):
                    mapped[n] = max([carry[2] if carry else 0] + [s[2] for s in region_starts])
            assert mapped == exact[f['filename']]
        files = []
        totals = defaultdict(lambda: [0, 0])
        for f in inventory:
            if f['testOnly']:
                continue
            assert f['path'] in lc
            raw = lc[f['path']]
            values = {n: hit for n, hit in raw.items() if n not in f['omitted']}
            uncovered = sorted(n for n, hit in values.items() if hit == 0)
            total, covered = len(values), sum(v > 0 for v in values.values())
            expected = next(x for x in summary['files'] if x['path'] == f['path'])
            assert (expected['covered'], expected['total'], expected['mapped'], expected['uncovered']) == (covered, total, sorted(values), uncovered)
            files.append(dict(path=f['path'], covered=covered, total=total, uncovered=uncovered, omitted=f['omitted'], nativeAliases=aliases[f['path']]))
            totals[f['crate']][0] += covered
            totals[f['crate']][1] += total
        for c in summary['crates']:
            assert [c['covered'], c['total']] == totals[c['crate']]
            assert c['covered'] * 100 >= c['total'] * 80
        assert len(files) == 29
        assert len(native['data'][0]['files']) == sum(len(v) for v in aliases.values())
        for p in profile['profiles']:
            raw = Path(profile['target']) / p['path']
            assert raw.stat().st_size == p['size'] > 0
            assert sha(raw.read_bytes()) == p['sha256']
        assert sorted(profile['merge']) == sorted(str(Path(profile['target']) / p['path']) for p in profile['profiles'])
        merge = Path(profile['target']) / 'workspace-profraw-list'
        assert sorted(merge.read_text().strip().splitlines()) == sorted(profile['merge'])
        for c in profile['childProof']:
            assert c['profiles'] and all(str(Path(profile['target']) / p) in profile['merge'] for p in c['profiles'])
            assert any(r.get('pid') == c['pid'] and r['exit'] == c['exit'] for r in receipt['receipts'])
        test_receipt = next(r for r in receipt['receipts'] if '--no-report' in r['argv'])
        counts = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', test_receipt['stdout'])
        assert sum(int(c[0]) for c in counts) == 275
        assert all(c[1:] == ('0', '0') for c in counts)
        explicit = profile['childProof']
        # libtest captures output for successful tests. Validate its real pass;
        # the review separately reproduces it with --nocapture and owned profiles.
        lifecycle = 'test coverage::startup_failures_release_admission_then_sigint_reaps_instrumented_children ... ok'
        assert lifecycle in test_receipt['stdout']
        runs.append(dict(directory=str(directory.relative_to(ROOT)), sourceHash=inputs['sha256'], files=files, crates=summary['crates'], profiles=len(profile['profiles']), childProof=explicit, lifecycle=lifecycle, nativePassedTests=275, reports={'lcov':summary['rawSha256'], 'llvm':summary['llvmSha256']}))
    assert len(runs) == 2 and runs[0]['directory'] != runs[1]['directory']
    latest = ROOT / runs[-1]['directory']
    receipt = load(latest / 'receipt.json')
    export = receipt['receipts'][-1]
    line = next(s.strip()[9:-1] for s in export['stderr'].splitlines() if s.strip().startswith('Running `') and '/llvm-cov export ' in s)
    argv = shlex.split(line)
    export_start = datetime.datetime.now(datetime.timezone.utc).isoformat()
    reexport = subprocess.run(argv, capture_output=True, timeout=60)
    (QA / 'receipts/native-reexport.json').write_text(json.dumps(dict(argv=argv, cwd=str(ROOT), started=export_start, ended=datetime.datetime.now(datetime.timezone.utc).isoformat(), exit=reexport.returncode, stdoutSha256=sha(reexport.stdout), originalSha256=sha((latest/'raw.json').read_bytes()), stderr=reexport.stderr.decode()), indent=2) + '\n')
    assert reexport.returncode == 0
    # cargo-llvm-cov adds its own metadata and reserializes float percentages.
    # Compare all native fields except those presentation-only differences.
    def native_counts(value):
        if isinstance(value, dict):
            return {k:native_counts(v) for k,v in value.items() if k not in ['cargo_llvm_cov', 'percent']}
        if isinstance(value, list):
            return [native_counts(v) for v in value]
        return value
    assert native_counts(json.loads(reexport.stdout)) == native_counts(load(latest / 'raw.json'))
    argv[1] = 'show'
    argv = [x for x in argv if not x.startswith('-format=')]
    i = argv.index('-object')
    binary = argv[i+1]
    argv[i:i+2] = [binary]
    prod = set(f['path'] for f in runs[-1]['files'])
    aliases = sorted({p for f in runs[-1]['files'] for p in f['nativeAliases']})
    argv += ['-format=text', '-show-line-counts-or-regions', '--dump', '--sources', *aliases]
    start = datetime.datetime.now(datetime.timezone.utc).isoformat()
    process = subprocess.run(argv, capture_output=True, text=True, timeout=60)
    (QA / 'receipts/native-show.json').write_text(json.dumps(dict(argv=argv, cwd=str(ROOT), started=start, ended=datetime.datetime.now(datetime.timezone.utc).isoformat(), exit=process.returncode, stdout=process.stdout, stderr=process.stderr), indent=2) + '\n')
    assert process.returncode == 0
    shown = defaultdict(dict)
    current = None
    for row in process.stdout.splitlines():
        if row.endswith(':') and row.startswith('/'):
            current = canonical(row[:-1])
        match = re.match(r'^\s*(\d+)\|([^|]*)\|', row)
        if current in prod and match and match[2].strip():
            n = int(match[1])
            hit = match[2].strip() not in ['0', '0.0', '0.00']
            shown[current][n] = shown[current].get(n, False) or hit
    differences = []
    alias_view_limits = []
    for f in runs[-1]['files']:
        actual = {n: h for n, h in shown[f['path']].items() if n not in f['omitted']}
        # show normalizes --sources, selecting only the canonical filename
        # when an export also has tests/../src aliases. Check that view against
        # its exact native record, not against the required all-alias union.
        canonical_view = {n: h for n, h in exact[str(ROOT/f['path'])].items() if n not in f['omitted']}
        expected = {'mapped':sorted(canonical_view), 'uncovered':sorted(n for n,h in canonical_view.items() if not h)}
        if expected['uncovered'] != f['uncovered']:
            alias_view_limits.append(dict(path=f['path'], canonicalOnlyUncovered=expected['uncovered'], unionUncovered=f['uncovered'], reason='native show normalizes source aliases; line union uses both unmodified exports'))
        if sorted(actual) != expected['mapped'] or sorted(n for n, h in actual.items() if not h) != expected['uncovered']:
            differences.append(dict(path=f['path'], nativeTotal=len(actual), expectedTotal=f['total'], missing=sorted(set(expected['mapped'])-actual.keys()), extra=sorted(actual.keys()-set(expected['mapped'])), nativeUncovered=sorted(n for n, h in actual.items() if not h), expectedUncovered=expected['uncovered']))
    result = dict(sourceHash=identity['sha256'], inputCount=len(identity['files']), protected=protected, commands=commands, runs=runs, nativeReexportCountsSegmentsFunctionsMatch=True, nativeReexportByteIdentical=False, nativeReexportDifference='cargo_llvm_cov metadata and float percent serialization only', independentJsonSegmentsMatchAllLcovRecords=True, nativeShowDifferences=differences, nativeShowAliasViewLimit=alias_view_limits, nativeShowWarnings=[s for s in process.stderr.splitlines() if 'mismatch' in s or 'warning:' in s], passAll=not differences)
    (QA / 'reconciliation.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'inputs':len(identity['files']), 'runs':len(runs), 'files':len(runs[-1]['files']), 'profiles':[r['profiles'] for r in runs], 'nativeShowDifferences':differences, 'nativeShowWarnings':result['nativeShowWarnings']}, indent=2))
    assert not differences


if __name__ == '__main__':
    main()
