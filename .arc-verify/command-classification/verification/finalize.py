#!/usr/bin/env python3
"""Bind the independent review to actual command receipts and final source bytes."""
import datetime, hashlib, json, pathlib, re, subprocess
ROOT = pathlib.Path.cwd()
EVIDENCE = ROOT / '.arc-verify/command-classification'
OUT = EVIDENCE / 'verification'
REVIEW = EVIDENCE / 'review'
BASE = '05f0812d0841485bbe07b3f1f8f6fde3861a1d55'
def sha(data):
    return hashlib.sha256(data).hexdigest()
def write(path, obj):
    path.write_text(json.dumps(obj, indent=2) + '\n')
def read(path):
    return json.loads(path.read_text())
# Independent path enumeration, including untracked implementation files.
paths = subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard','--','crates','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo','scripts/harness']).decode().split('\0')
paths += ['.arc-verify/command-classification/' + p for p in ['contract.md','contract.json','cli-requirements.md']]
tracked = set(subprocess.check_output(['git','ls-files','-z','--','crates','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo','scripts/harness']).decode().split('\0'))
inputs = []
for name in sorted(set(paths) - {''}):
    path = ROOT / name
    data = path.read_bytes()
    inputs.append(dict(path=name, sha256=sha(data), bytes=len(data), lines=len(data.splitlines()), tracked=name in tracked, mode=oct(path.stat().st_mode & 0o777)))
assert inputs == read(OUT/'source-inputs-before.json') == read(OUT/'source-inputs-after.json')
head = subprocess.check_output(['git','rev-parse','HEAD']).decode().strip()
assert head == BASE
invariants = {}
for name in ['crates/arc-storage/migrations/0001_phase0.sql','scripts/harness/validate.sh','crates/arc-daemon/src/limits.rs','crates/arc-daemon/src/platform.rs','crates/arc-daemon/src/admission.rs','crates/arc-daemon/src/lifecycle.rs']:
    baseline = subprocess.check_output(['git','show',BASE+':'+name])
    current = (ROOT/name).read_bytes()
    invariants[name] = dict(baselineSha256=sha(baseline),sha256=sha(current),unchanged=baseline==current)
assert all(x['unchanged'] for x in invariants.values())
rust = [x for x in inputs if x['path'].endswith('.rs')]
assert len(rust)==64 and max(x['lines'] for x in rust)==474
assert all(x['lines']<=500 for x in rust)
preceding = read(EVIDENCE/'implementation/repair-6-source-inventory.json')
current_hashes = {x['path']:x['sha256'] for x in inputs}
assert all(current_hashes[x['path']]==x['sha256'] for x in preceding['inputs'])
privacy_path = 'crates/arc-daemon/tests/classification/privacy.rs'
assert current_hashes[privacy_path]=='3c7fd3ba43eded12ddaa071cbce6553c05370fe3898f4034912a6653620eadee'
checks = {}
for name,expected in [('privacy',0),('fmt',0),('clippy',0),('workspace-tests',0),('harness',1),('work-trace',0)]:
    receipt = read(OUT/(name+'.json'))
    assert receipt['exitCode']==expected and not receipt['timedOut']
    assert receipt['inputsBefore']==receipt['inputsAfter']==inputs and receipt['sourceInputsMatch']
    assert sha((ROOT/receipt['log']).read_bytes())==receipt['logSha256']
    checks[name] = dict(argv=receipt['argv'], exitCode=receipt['exitCode'], logSha256=receipt['logSha256'], elapsedSeconds=receipt['elapsedSeconds'])
harness = (OUT/'harness.txt').read_text()
assert 'Script not found "test:coverage"' in harness and 'Total violations: 1' in harness
assert 'skipping coverage gate' not in harness
assert read(OUT/'harness.json')['coverageStatus']=='unresolved'
assert read(OUT/'tcp-prerequisite.json')['permitted']
# Parse the complete test log; do not infer counts from prior Actors' reports.
text = (OUT/'workspace-tests.txt').read_text()
target = None
executed = []
for line in text.splitlines():
    if line.lstrip().startswith('Running ') or line.lstrip().startswith('Doc-tests '):
        target=line.strip()
    match = re.fullmatch(r'test (.+) \.\.\. ok',line)
    if match:
        executed.append(dict(target=target,name=match[1],outcome='passed'))
assert len(executed)==205
assert all('0 failed; 0 ignored;' in x for x in re.findall(r'test result:.*',text))
write(OUT/'executed-tests.json',dict(command='cargo test --workspace',passingTests=205,tests=executed))
# The exact identifiers in the matrix must have passed in the retained log.
matrix = (OUT/'test-matrix.md').read_text()
names = {x['name'] for x in executed}
for candidate in re.findall(r'`([^`]+)`',matrix):
    if re.fullmatch(r'(?:[a-z_]+::)*[a-z_]+',candidate) and ('_' in candidate or '::' in candidate):
        assert candidate in names, candidate
fixtures = read(EVIDENCE/'contract.json')
assert len(fixtures['fixtures'])==45 and len(fixtures['dispositionFixtures'])==7 and len(fixtures['malformedPayloadFixtures'])==10
saturation = read(OUT/'runtime/workspace-tests/classification-saturation.json')
accounting = read(OUT/'runtime/workspace-tests/classification-accounting.json')
assert saturation['sameConnectionRecovery'] and saturation['classificationLoadContinued']
assert saturation['signalToExitUs']<=3500000
assert all(accounting[x] for x in ['accounted','worker_joined','handlers_reaped','gauges_zero','socket_removed','deadline_met'])
for name in ['runtime/privacy/classification-privacy.json','runtime/workspace-tests/classification-privacy.json']:
    privacy = read(OUT/name)
    assert privacy['cases']==12 and privacy['liveSqliteAndWalChecked']
    assert all(privacy[k]==0 for k in ['canariesPersisted','endpointContacts','substitutionMarkers','toolTrapsStarted'])
assert 'work: 60203, tokens: 6, invocations: 2, max_depth: 1' in (OUT/'work-trace.txt').read_text()
helpers = []
for path in sorted(OUT.glob('*.py')):
    data=path.read_bytes()
    helpers.append(dict(path=str(path.relative_to(ROOT)),sha256=sha(data),lines=len(data.splitlines())))
assert all(x['lines']<=500 for x in helpers)
write(OUT/'source-inventory.json',dict(baselineHead=BASE,currentHead=head,inputs=inputs,invariants=invariants,rustSourceFiles=len(rust),trackedRustFiles=sum(x['tracked'] for x in rust),untrackedRustFiles=sum(not x['tracked'] for x in rust),maximumRustLines=max(x['lines'] for x in rust),sourceLineLimitPassed=True,reviewHelpers=helpers,precedingImplementationInputsChecked=len(preceding['inputs']),precedingImplementationInputsMatch=True,privacyTestUnchanged=True,inventoryScope='All tracked and nonignored untracked files in crates/, Cargo manifests/lockfile, rust toolchain, .cargo, harness scripts, and amended specification inputs. Review outputs are separately hashed.'))
probe = read(OUT/'ssh-post-destination.json')
assert probe['inputsBefore']==probe['inputsAfter']==inputs and probe['sourceInputsMatch']
assert probe['exitCode']==1 and probe['mismatchCount']==11 and len(probe['cases'])==17
assert all(x['ipcAndBothCliModesAgree'] for x in probe['cases'])
probe['log'] = '.arc-verify/command-classification/verification/ssh-post-destination.txt'
probe['logSha256'] = sha((OUT/'ssh-post-destination.txt').read_bytes())
probe['helperSha256'] = sha((OUT/'ssh-post-destination.py').read_bytes())
write(OUT/'ssh-post-destination.json',probe)
finding = dict(id='SSH-POST-DESTINATION',severity='HIGH',path='crates/arc-detector/src/rules.rs',line=69,relatedLines=[79,90],description='SSH options immediately after the destination are accepted as a remote command, producing false REMOTE_EXEC and MIXED with uncertainty NONE.',evidence='../verification/ssh-post-destination.json',mismatches=11,remediation='Implementation owner must conservatively reject unresolved post-destination options with UNKNOWN/OBSERVE, honor explicit -- and preserve opaque words after a real remote command. Add detector, IPC, daemon and CLI regressions, then refresh affected receipts.')
summary = dict(baselineHead=BASE,currentHead=head,fmtPassed=True,clippyPassed=True,workspaceTestsPassed=True,behaviorVerified=False,compatibilityVerified=True,migrationUnchanged=True,harnessUnchanged=True,sourceLineLimitPassed=True,finalSourcesMatch=True,independent=True,passingWorkspaceTests=205,harnessPassed=False,harnessExitCode=1,coverageStatus='unresolved',sourceInputCount=len(inputs),sourceInventory='source-inventory.json',testMatrix='test-matrix.md',checks=checks,classificationRuntime=saturation,blockingFindings=[finding],remainingObligations=['SSH post-destination option repair and regressions','fresh affected verification after repair','harness test:coverage','wider detector coverage and actual locality resolution','scheduling','managed-execution','desktop','native Windows','Chief final mission verification'],recordedAt=datetime.datetime.now(datetime.timezone.utc).isoformat())
write(OUT/'summary.json',summary)
review = dict(independent=True,actorId='reviewer',taskId='task-5',skill='oma-qa',status='failed',verdict='FAIL',verdictScope='documented Phase 0 command-classification slice and task-5 required verification',completeness='completed',blockersResolved=False,findings=[finding],implementationDefects=[finding],sourceInputsMatch=True,verificationSummary='../verification/summary.json',report='report.md',harnessPassed=False,coverageStatus='unresolved',currentExecutionPrerequisite='TCP bind/listen and unchanged required TCP test pass after operator environment note',codeIntelligence=dict(provider='Serena',mode='unavailable',tools=[],fallback='direct known-path reads and Git diff; no native project code-content search'),remainingObligations=summary['remainingObligations'],inspectionLimitations=['global av.yaml access denied; no globally clean claim','mise tracking-link warning; checks complete','no live foreign-account peer impersonation','macOS execution only; native Windows not verified'],changedPaths=['.arc-verify/command-classification/review/','.arc-verify/command-classification/verification/'],interfaceCompatibility='protocol 1 and dispatch(request, doctor), status/doctor, same-user checks and operational bounds preserved; classifier semantic blocker remains',productOrTestEdits=[])
write(REVIEW/'summary.json',review)
claim=dict(status='failed',changedFiles=sorted({str(x.relative_to(ROOT)) for directory in [REVIEW,OUT] for x in directory.rglob('*') if x.is_file()} | {'.arc-verify/command-classification/review/claim.json', '.arc-verify/command-classification/review/artifact-inventory.json'}),unresolved=[finding['description'],'The implementation owner must repair SSH handling and add regressions; affected receipts then require a fresh review.','The unchanged harness exits 1: test:coverage is missing. Coverage remains open as required by this assignment.'],artifacts=['.arc-verify/command-classification/review/report.md','.arc-verify/command-classification/review/summary.json','.arc-verify/command-classification/verification/summary.json','.arc-verify/command-classification/verification/test-matrix.md','.arc-verify/command-classification/verification/source-inventory.json','.arc-verify/command-classification/verification/ssh-post-destination.json'],independent=True,blockersResolved=False)
write(REVIEW/'claim.json',claim)
artifacts = []
for directory in [REVIEW,OUT]:
    for path in sorted(directory.rglob('*')):
        if path.is_file() and path.name!='artifact-inventory.json':
            data=path.read_bytes()
            artifacts.append(dict(path=str(path.relative_to(ROOT)),sha256=sha(data),bytes=len(data)))
write(REVIEW/'artifact-inventory.json',dict(artifacts=artifacts,sourceInputsMatch=True))
print(json.dumps(dict(status='failed',verdict='FAIL',blockersResolved=False,independent=True,sourceInputs=len(inputs),passingWorkspaceTests=205,rustSourceFiles=len(rust),maxRustLines=474,trackedRustFiles=sum(x['tracked'] for x in rust),untrackedRustFiles=sum(not x['tracked'] for x in rust),sshMismatches=probe['mismatchCount'],harnessExitCode=1,artifacts=len(artifacts))))
