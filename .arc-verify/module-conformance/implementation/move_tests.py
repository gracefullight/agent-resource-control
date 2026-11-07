#!/usr/bin/env python3
"""Two exact source-slice moves; refuse a changed input or existing destination."""
import json
from pathlib import Path
import runpy
import sys

HERE = Path(__file__).resolve().parent
api = runpy.run_path(str(HERE.parent / 'baseline/capture.py'))
ROOT = api['ROOT']
REV = api['REV']
RESIDENT = api['RESIDENT']
IPC = api['IPC']


def original(path):
    data = api['accepted'](path)
    assert (ROOT / path).read_bytes() == data, path
    return data.decode().splitlines(keepends=True)


def section(lines, first, last):
    return ''.join(lines[first - 1:last])


def create(path, text):
    target = ROOT / path
    assert not target.exists(), path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)


def resident():
    lines = original(RESIDENT)
    groups = {
        'lifecycle': [(265, 408)],
        'provisioning': [(535, 669), (833, 860)],
        'identity': [(685, 831)],
        'coordination': [(872, 901)],
    }
    for module, ranges in groups.items():
        text = 'use super::*;\n\n' + '\n'.join(section(lines, a, b) for a, b in ranges)
        create(f'crates/arc-daemon/tests/resident/{module}.rs', text)
    wiring = ''.join(f'#[path = "resident/{name}.rs"]\nmod {name};\n' for name in groups)
    root = section(lines, 1, 4) + '\n' + wiring + section(lines, 5, 264)
    root += section(lines, 409, 534) + section(lines, 670, 684) + section(lines, 861, 871)
    (ROOT / RESIDENT).write_text(root)


def ipc():
    lines = original(IPC)
    # Remove only the enclosing module's indentation; preserve literal contents.
    def dedent(first, last):
        return ''.join(line[4:] if line.startswith('    ') else line
                       for line in lines[first - 1:last])
    create('crates/arc-ipc/src/tests/mod.rs',
           'use super::*;\n\nmod dispatch;\nmod framing;\n\n' + dedent(360, 362))
    create('crates/arc-ipc/src/tests/framing.rs',
           'use super::*;\nuse std::io::Cursor;\n\n' + dedent(364, 482))
    create('crates/arc-ipc/src/tests/dispatch.rs',
           'use super::*;\n\n' + dedent(484, 550))
    (ROOT / IPC).write_text(section(lines, 1, 355) + 'mod tests;\n')


if __name__ == '__main__':
    {'resident': resident, 'ipc': ipc}[sys.argv[1]]()
