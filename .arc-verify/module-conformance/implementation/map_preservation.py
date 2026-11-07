#!/usr/bin/env python3
"""Map immutable baseline items to actual destinations; assert frozen bodies."""
import json
from pathlib import Path
import re
import runpy

HERE = Path(__file__).resolve().parent
BASE = HERE.parent / 'baseline'
api = runpy.run_path(str(BASE / 'capture.py'))
ROOT, sha = api['ROOT'], api['sha']
load = lambda name: json.loads((BASE / name).read_text())


def rows(name, values):
    (HERE / name).write_text('[\n' + ',\n'.join(json.dumps(v, ensure_ascii=False,
                         separators=(',', ':')) for v in values) + '\n]\n')


def save(name, value):
    (HERE / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


source_paths = [api['RESIDENT'], api['IPC'], api['SUPPORT']]
modules = {'crates/arc-daemon/tests/resident/' + name + '.rs': name
           for name in ['lifecycle', 'provisioning', 'identity', 'coordination']}
modules.update({'crates/arc-ipc/src/tests/' + name + '.rs': 'tests::' + name
                for name in ['framing', 'dispatch']})
modules['crates/arc-ipc/src/tests/mod.rs'] = 'tests'
source_paths += list(modules)
original_functions = load('inventory.json')
original_assertions = load('assertions.json')
original_attributes = load('attributes.json')
original_declarations = load('declarations.json')
# Run the same scoped collector over current bytes without writing baseline data.
api['inventory'].__globals__['accepted'] = lambda path: (ROOT / path).read_bytes()
current, assertions, attributes, declarations = [], [], [], []
for path in source_paths:
    if path == api['IPC']:
        # Its production prefix is verified separately, with no test items left.
        continue
    functions, sites, attrs = api['inventory'](path)
    for f in functions:
        if path in modules:
            f['qualifiedName'] = modules[path] + '::' + f['qualifiedName']
    current += functions
    assertions += sites
    attributes += attrs
    declarations += api['declarations'](path)


def body_text(source, item):
    lines = source.splitlines(keepends=True)
    text = ''.join(lines[item['bodyRange'][0] - 1:item['bodyRange'][1]])
    # The function header may share the opening-brace line.
    begin = next(start for start, _, token in api['lex'](text) if token == '{')
    return text[begin:].rstrip('\n')


entries = []
for old in original_functions:
    matches = [f for f in current if f['name'] == old['name'] and
               f['enclosingImpl'] == old['enclosingImpl'] and
               (f['path'] == api['SUPPORT']) == (old['path'] == api['SUPPORT'])]
    assert len(matches) == 1, (old['id'], matches)
    new = matches[0]
    assert old['bodyTokenSha256'] == new['bodyTokenSha256'], old['id']
    assert [a['text'] for a in old['attributes']] == [a['text'] for a in new['attributes']]
    before = api['git']('show', f"{api['REV']}:{old['path']}").decode()
    after = (ROOT / new['path']).read_text()
    old_body, new_body = body_text(before, old), body_text(after, new)
    if old['path'] == api['IPC']:
        old_body = '\n'.join(line[4:] if i and line.startswith('    ') else line
                             for i, line in enumerate(old_body.split('\n')))
    assert old_body == new_body, ('raw body/comments/literals differ', old['id'])
    a = [s for s in original_assertions if s['ownerId'] == old['id']]
    b = [s for s in assertions if s['ownerId'] == new['id']]
    assert len(a) == len(b)
    pairs = []
    for left, right in zip(a, b):
        assert left['macro'] == right['macro']
        pairs.append({'originalLine': left['line'], 'destinationLine': right['line'],
                      'macro': left['macro'], 'tokensEqual': True})
    fields = ['path', 'name', 'qualifiedName', 'range', 'bodyRange', 'attributes',
              'rawBodySha256', 'bodyTokenSha256', 'assertionLines']
    entries.append({'originalId': old['id'], 'kind': old['kind'],
        'original': {k: old[k] for k in fields + ['rawItemSha256']},
        'destination': {k: new[k] for k in fields},
        'comparison': {'bodyTokensEqual': True, 'attributesEqual': True,
            'assertions': pairs, 'rawBodyEqualAfterIndentationOnly': True,
            'mechanicalDifferences': [x for x, yes in [
                ('relocation/module qualification', old['path'] != new['path']),
                ('remove enclosing module indentation', old['path'] == api['IPC'])] if yes]}})
assert len(entries) == len(current) == 51
assert sum(len(e['comparison']['assertions']) for e in entries) == 137

mapped_declarations = []
for old in original_declarations:
    if old['path'] == api['IPC'] and old['kind'] == 'mod':
        new = {'path': api['IPC'], 'range': [356, 356]}
        comparison = 'inline module replaced by external mod tests; all contents mapped below'
        differences = ['module_wiring', 'relocation', 'indentation']
    else:
        matches = [d for d in declarations if d['kind'] == old['kind'] and
                   d['tokenSha256'] == old['tokenSha256'] and
                   (d['path'] == api['SUPPORT']) == (old['path'] == api['SUPPORT'])]
        if old['path'] == api['IPC'] and old['kind'] == 'use':
            matches = [d for d in matches if d['path'] == (
                'crates/arc-ipc/src/tests/framing.rs' if 'Cursor' in old['header']
                else 'crates/arc-ipc/src/tests/mod.rs')]
        assert len(matches) == 1, (old, matches)
        new = matches[0]
        comparison = 'identical declaration tokens; source slices preserve comments and literals'
        differences = ['relocation', 'indentation'] if old['path'] != new['path'] else []
    mapped_declarations.append({'originalPath': old['path'], 'originalRange': old['range'],
        'destinationPath': new['path'], 'destinationRange': new['range'],
        'comparison': comparison, 'mechanicalDifferences': differences,
        'originalHeader': old['header']})

mapped_attributes = []
for old in original_attributes:
    if old['path'] == api['IPC'] and old['text'] == '#[cfg(test)]':
        path, line = api['IPC'], 355
    else:
        owners = [e for e in entries if any(a['line'] == old['line'] for a in e['original']['attributes'])
                  and e['original']['path'] == old['path']]
        if owners:
            dest = owners[0]['destination']
            path = dest['path']
            line = next(a['line'] for a in dest['attributes'] if a['text'] == old['text'])
        else:
            # The nonfunction derives and support allow(dead_code) remain at root.
            matches = [a for a in attributes if a['path'] == old['path'] and a['text'] == old['text']]
            assert len(matches) == 1, old
            path, line = matches[0]['path'], matches[0]['line']
    assert (ROOT / path).read_text().splitlines()[line - 1].strip() == old['text']
    mapped_attributes.append({'original': old, 'destination': {'path': path, 'line': line,
                                                             'text': old['text']}})
assert len(mapped_attributes) == 28
rows('entries.json', entries)
# Compact rows keep the required complete mapping below the file line ceiling.
mapping = {'schemaVersion': 1, 'baselineRevision': api['REV'], 'entries': entries,
           'declarations': mapped_declarations, 'attributes': mapped_attributes,
           'sourceFingerprint': api['source_fingerprint'](),
           'moduleWiring': modules,
           'probeSelector': {'before': 'fixture_serialization_probe',
                             'after': 'fixture_serialization_probe', 'changed': False}}
(HERE / 'mapping.json').write_text('{\n' + ',\n'.join(
    json.dumps(key) + ':' + json.dumps(value, ensure_ascii=False, separators=(',', ':'))
    for key, value in mapping.items()) + '\n}\n')

baseline = load('baseline.json')
source_rows = load('source-files.json')
protected = []
for item in source_rows:
    if item['path'] in [api['IPC'], api['RESIDENT']]:
        continue
    actual = sha((ROOT / item['path']).read_bytes())
    assert actual == item['sha256'], item['path']
    protected.append({'path': item['path'], 'baselineSha256': item['sha256'],
                      'actualSha256': actual, 'equal': True})
prefix = b''.join((ROOT / api['IPC']).read_bytes().splitlines(keepends=True)[:354])
assert sha(prefix) == baseline['ipcProductionPrefix']['sha256']
rows('protected-paths.json', protected)
save('preservation-result.json', {'baselineRevision': api['REV'],
     'sourceFingerprint': mapping['sourceFingerprint'], 'mappedTests': 24,
     'mappedHelpers': 27, 'mappedAssertionSites': 137, 'mappedAttributes': 28,
     'mappedDeclarations': len(mapped_declarations), 'protectedFilesByteIdentical': len(protected),
     'ipcProductionPrefixByteIdentical': True, 'privateVisibilityChanges': [],
     'probeRemainsAtIntegrationRoot': True})
paths = [p for p in source_paths if p != api['SUPPORT']]
lengths = []
for path in paths:
    data = (ROOT / path).read_bytes()
    assert data.count(b'\n') <= 500 and len(data.splitlines()) <= 500, path
    lengths.append({'path': path, 'lines': data.count(b'\n'),
                    'logicalLines': len(data.splitlines()), 'sha256': sha(data)})
rows('source-lengths.json', lengths)
print(json.dumps({'mappedItems': len(entries), 'assertionSites': 137,
                  'attributes': 28, 'lengths': lengths}))
