import datetime, hashlib, json, pathlib, subprocess
root = pathlib.Path.cwd()
base = pathlib.Path('.arc-verify/rust-coverage/integration')
out = base / 'repair-3'
load = lambda p: json.loads(pathlib.Path(p).read_text())
hash_bytes = lambda b: hashlib.sha256(b).hexdigest()
entries = load(base / 'commands.json')
versions = load(out / 'versions.json')
wrappers = [e for e in entries if e['name'].startswith('repair-3-')]
assert len(wrappers) == 3
wrapper = next(e for e in wrappers if e['name'] == 'repair-3-harness')
child_matches = []
for directory in pathlib.Path('.arc-verify/rust-coverage/tooling/acceptance').glob('harness-*'):
    path = directory / 'binding.json'
    if not path.exists():
        continue
    binding = load(path)
    for receipt_path in directory.glob('*-command.json'):
        receipt = load(receipt_path)
        if receipt['argv'] == ['./scripts/harness/validate.sh'] and wrapper['started'] <= receipt['started'] <= receipt['ended'] <= wrapper['ended']:
            child_matches.append((receipt_path, receipt, binding))
assert len(child_matches) == 1
path, child, binding = child_matches[0]
assert child['exit'] == binding['exit'] == 143
assert binding['unchanged'] and wrapper['sourceStable']
assert 'Files to check: 3702' in child['stdout']
assert 'Check 5/5' not in child['stdout']
assert not wrapper['environment'].get('SKIP_COVERAGE')
child_entry = dict(name='required-repair-3-harness', argv=child['argv'], cwd=child['cwd'], pid=child['pid'],
    started=child['started'], ended=child['ended'], exit=child['exit'],
    durationMs=(datetime.datetime.fromisoformat(child['ended'].replace('Z', '+00:00'))-datetime.datetime.fromisoformat(child['started'].replace('Z', '+00:00'))).total_seconds()*1000,
    sourceHash=binding['sourceHash'], afterHash=binding['afterHash'], sourceStable=binding['unchanged'],
    environment=wrapper['environment'], receipt=str(path), environmentProvenance='Inherited realAcceptance environment; no skip flags or verification substitutions.')
entries.append(child_entry)
for entry in wrappers + [child_entry]:
    entry['toolVersionsArtifact'] = str(out / 'versions.json')
    entry['toolVersionsSha256'] = hash_bytes((out / 'versions.json').read_bytes())
    entry['toolVersions'] = versions
(base / 'commands.json').write_text(json.dumps(entries, indent=2)+'\n')
inputs = wrapper['inputs']
assert all(e['sourceHash'] == inputs['sha256'] and e['sourceStable'] for e in wrappers)
current = {p: hash_bytes(pathlib.Path(p).read_bytes()) for p in inputs['files']}
assert current == inputs['files']
prior = load(base / 'repair-2/current-preservation.json')['binding']
assert inputs == prior
protected = {}
accepted = 'fbe76a5f15101ad415f355ef06f1deeb1f97d568'
for p in ['scripts/harness/validate.sh','crates/arc-storage/migrations/0001_phase0.sql']:
    original = subprocess.run(['git','show',f'{accepted}:{p}'],capture_output=True,check=True).stdout
    actual = pathlib.Path(p).read_bytes()
    protected[p] = dict(baselineSha256=hash_bytes(original),currentSha256=hash_bytes(actual),identical=original == actual)
assert all(p['identical'] for p in protected.values())
identity = dict(createdAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),acceptedHead=accepted,
    head=subprocess.run(['git','rev-parse','HEAD'],capture_output=True,text=True,check=True).stdout.strip(),
    binding=inputs,inputCount=len(current),allInputsMatch=True,identicalToPriorIntegratedInputs=True,protectedFiles=protected,
    implementationEditsThisAttempt=[],skills=['oma-dev-workflow'],independentReview='Not performed by this implementer; task-7 remains pending.')
(out / 'source-preservation.json').write_text(json.dumps(identity,indent=2)+'\n')
required = [['bun','run','test:coverage'],['cargo','fmt','--all','--','--check'],['cargo','clippy','--workspace','--all-targets','--','-D','warnings'],['cargo','test','--workspace'],['./scripts/harness/validate.sh']]
selected = [max((e for e in entries if e['argv']==argv),key=lambda e:e['started']) for argv in required]
latest = load('.arc-verify/rust-coverage/baseline/latest.json')
summary = load(pathlib.Path(latest['directory']) / 'summary.json')
assert latest['pass'] and latest['sourceHash'] == inputs['sha256']
acceptance = dict(status='partial',complete=False,allFiveExactCommandsPass=False,sourceHash=inputs['sha256'],
    reason='Required real-command harness verification exceeds its preserved 115000-ms timer during Check 1, before coverage.',
    latestRequiredCommands=[{k:e[k] for k in ['name','argv','started','exit','durationMs','sourceHash']} for e in selected],
    freshChecks=[{k:e[k] for k in ['name','argv','started','exit','durationMs','sourceHash','directory']} for e in wrappers],
    nestedHarness=dict(receipt=str(path),**{k:child_entry[k] for k in ['argv','exit','durationMs','sourceHash']}),
    retainedCoverage=dict(directory=latest['directory'],sourceHash=summary['sourceHash'],crates=summary['crates'],files=len(summary['files']),freshMeasurementThisAttempt=False),
    independentQA='task-7 pending',obligations={'desktop':'incomplete','native Windows':'incomplete','remaining Phase 0':'incomplete'})
(out / 'acceptance.json').write_text(json.dumps(acceptance,indent=2)+'\n')
print(json.dumps(dict(sourceHash=inputs['sha256'],inputCount=len(current),protectedFilesMatch=True,childReceipt=str(path),childExit=child['exit'],durationMs=child_entry['durationMs'],crates=summary['crates'],files=len(summary['files'])),indent=2))
