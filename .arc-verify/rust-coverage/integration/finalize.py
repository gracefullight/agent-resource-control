import json, pathlib, hashlib, datetime
base = pathlib.Path('.arc-verify/rust-coverage/integration')
root = pathlib.Path.cwd()
load = lambda p: json.loads(pathlib.Path(p).read_text())
entries = [e for e in load(base / 'commands.json') if not e['name'].startswith('required-')]
versions = load(base / 'versions.json')
source_hash = entries[-1]['sourceHash']
# Add the actual nested exact commands, retaining wrapper and native receipts.
for wrapper in list(entries):
    if wrapper['name'] not in ['coverage-cli', 'harness']:
        continue
    name = wrapper['name']
    expected = ['bun', 'run', 'test:coverage'] if name == 'coverage-cli' else ['./scripts/harness/validate.sh']
    matches = []
    for directory in pathlib.Path('.arc-verify/rust-coverage/tooling/acceptance').glob(f'{name}-*'):
        binding_path = directory / 'binding.json'
        if not binding_path.exists():
            continue
        binding = load(binding_path)
        if binding['sourceHash'] != source_hash:
            continue
        for path in directory.glob('*-command.json'):
            receipt = load(path)
            if receipt['argv'] == expected and receipt['started'] >= wrapper['started'] and receipt['ended'] <= wrapper['ended']:
                matches.append((path, receipt, binding))
    assert len(matches) == 1, (name, len(matches))
    path, receipt, binding = matches[0]
    assert receipt['exit'] == binding['exit']
    entries.append(dict(name='required-' + name, argv=receipt['argv'], cwd=receipt['cwd'], pid=receipt['pid'],
        started=receipt['started'], ended=receipt['ended'], exit=receipt['exit'],
        durationMs=(datetime.datetime.fromisoformat(receipt['ended'].replace('Z', '+00:00')) - datetime.datetime.fromisoformat(receipt['started'].replace('Z', '+00:00'))).total_seconds()*1000,
        sourceHash=binding['sourceHash'], afterHash=binding['afterHash'], sourceStable=binding['unchanged'],
        environment=wrapper['environment'], environmentProvenance='Inherited realAcceptance environment; SKIP_COVERAGE absent in parent and child.', receipt=str(path)))
for entry in entries:
    entry['toolVersionsArtifact'] = str(base / 'versions.json')
    entry['toolVersionsSha256'] = hashlib.sha256((base / 'versions.json').read_bytes()).hexdigest()
    entry['toolVersions'] = versions
(base / 'commands.json').write_text(json.dumps(entries, indent=2) + '\n')
required = [['bun','run','test:coverage'], ['cargo','fmt','--all','--','--check'],
    ['cargo','clippy','--workspace','--all-targets','--','-D','warnings'], ['cargo','test','--workspace'], ['./scripts/harness/validate.sh']]
selected = [max((e for e in entries if e['argv'] == argv), key=lambda e:e['started']) for argv in required]
exact_pass = all(e['exit'] == 0 and e['sourceStable'] and e['sourceHash'] == source_hash for e in selected)
failed_checks = [dict(name=e['name'],argv=e['argv'],exit=e['exit'],directory=e.get('directory'),receipt=e.get('receipt')) for e in entries if e['exit'] != 0]
complete = exact_pass and not failed_checks
assert all(not e['environment'].get('SKIP_COVERAGE') for e in selected)
(base / 'acceptance.json').write_text(json.dumps(dict(status='ready-for-independent-QA' if complete else 'partial', complete=complete, allFiveExactCommandsPass=exact_pass, failedChecks=failed_checks, sourceHash=source_hash,
    commands=[dict(argv=e['argv'], exit=e['exit'], durationMs=e['durationMs'], sourceHash=e['sourceHash']) for e in selected],
    independentQA='task-7 pending; this integration does not self-approve', obligations={'desktop':'incomplete','native Windows':'incomplete','remaining Phase 0':'incomplete'}), indent=2)+'\n')
print(json.dumps(dict(allFiveExactCommandsPass=exact_pass,complete=complete,failedChecks=failed_checks,sourceHash=source_hash)))
