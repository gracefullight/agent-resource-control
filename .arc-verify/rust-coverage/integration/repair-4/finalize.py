import json, pathlib, hashlib, datetime
base=pathlib.Path('.arc-verify/rust-coverage/integration')
out=base/'repair-4'
load=lambda p:json.loads(pathlib.Path(p).read_text())
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
inputs=load(out/'source.json');source=inputs['sha256']
index=load(base/'commands.json')
wrappers=[e for e in index if e['name'].startswith('repair-4-') and not e['name'].startswith('repair-4-required-')]
for w in wrappers:
    w.update(toolVersionsArtifact=str(out/'versions.json'),toolVersionsSha256=sha(out/'versions.json'),toolVersions=load(out/'versions.json'))
    assert w['sourceHash']==w['afterHash']==source
    assert not w['environment'].get('SKIP_COVERAGE')
nested=[]
for wrapper in wrappers:
    short=wrapper['name'].removeprefix('repair-4-')
    if short not in ['coverage-cli','harness']:continue
    expected=['bun','run','test:coverage'] if short=='coverage-cli' else ['./scripts/harness/validate.sh']
    matches=[]
    for directory in pathlib.Path('.arc-verify/rust-coverage/tooling/acceptance').glob(f'{short}-*'):
        p=directory/'binding.json'
        if not p.exists():continue
        b=load(p)
        if b['sourceHash']!=source:continue
        for r in directory.glob('*-command.json'):
            receipt=load(r)
            if receipt['argv']==expected and receipt['started']>=wrapper['started'] and receipt['ended']<=wrapper['ended']:
                matches.append((r,receipt,b))
    assert len(matches)==1,(short,len(matches))
    path,r,b=matches[0]
    assert r['exit']==b['exit']
    nested.append(dict(name='repair-4-required-'+short,argv=r['argv'],cwd=r['cwd'],pid=r['pid'],started=r['started'],ended=r['ended'],exit=r['exit'],durationMs=(datetime.datetime.fromisoformat(r['ended'].replace('Z','+00:00'))-datetime.datetime.fromisoformat(r['started'].replace('Z','+00:00'))).total_seconds()*1000,sourceHash=b['sourceHash'],afterHash=b['afterHash'],sourceStable=b['unchanged'],environment=wrapper['environment'],environmentProvenance='Inherited realAcceptance environment; SKIP_COVERAGE absent in both parent and child.',receipt=str(path),receiptSha256=sha(path),toolVersionsArtifact=str(out/'versions.json'),toolVersionsSha256=sha(out/'versions.json'),toolVersions=load(out/'versions.json')))
# Preserve all previous rows; append the actual nested commands once.
for e in nested:
    if not any(i.get('receipt')==e['receipt'] for i in index):index.append(e)
(base/'commands.json').write_text(json.dumps(index,indent=2)+'\n')
current=wrappers+nested
(out/'commands.json').write_text(json.dumps(current,indent=2)+'\n')
required=[['bun','run','test:coverage'],['cargo','fmt','--all','--','--check'],['cargo','clippy','--workspace','--all-targets','--','-D','warnings'],['cargo','test','--workspace'],['./scripts/harness/validate.sh']]
selected=[max((e for e in current if e['argv']==a),key=lambda e:e['started']) for a in required]
assert all(e['exit']==0 and e['sourceStable'] and e['sourceHash']==source for e in selected)
assert all(e['exit']==0 for e in current)
latest=load('.arc-verify/rust-coverage/baseline/latest.json');assert latest['sourceHash']==source and latest['pass']
s=load(pathlib.Path(latest['directory'])/'summary.json');assert s['pass'] and len(s['crates'])==6
for x in s['crates']:assert x['covered']*100>=80*x['total'] and x['total']>0
summary=dict(status='ready-for-independent-QA',complete=True,task='task-6',missionComplete=False,sourceHash=source,inputCount=len(inputs['files']),allFiveExactCommandsPass=True,allVerificationTestsPass=True,commands=[dict(argv=e['argv'],exit=e['exit'],durationMs=e['durationMs'],receipt=e.get('receipt',e.get('directory')),sourceHash=e['sourceHash']) for e in selected],verificationWrappers=[dict(argv=e['argv'],exit=e['exit'],durationMs=e['durationMs'],directory=e['directory']) for e in wrappers],measurement=latest,crates=s['crates'],independentQA='Task-7 pending; task-6 implementer does not self-approve',historicalFailures='Previous attempts and all failed receipts retained; only this current-source attempt establishes repair acceptance.',obligations={'desktop':'incomplete','native Windows':'incomplete','remaining Phase 0':'incomplete'})
(out/'acceptance.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({k:summary[k] for k in ['status','complete','sourceHash','allFiveExactCommandsPass','allVerificationTestsPass']}))
