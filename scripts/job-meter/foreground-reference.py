"""Owned native PTY/pipeline fixture. The unrelated peer burns measurable CPU.

The C workload supplies an independent self+waited-descendant reference. This
controller never signals a foreground group and reaps only its direct children.
"""
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import pty
import signal
import subprocess
import sys
import termios

source = Path(__file__).resolve().parents[2] / 'crates/arc-daemon/tests/managed_exec/pipeline.py'
spec = importlib.util.spec_from_file_location('native_pipeline', source)
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def controller(root, arcctl, workload):
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)
    os.tcsetpgrp(0, os.getpgrp())
    signal.signal(signal.SIGTTOU, signal.SIG_IGN)
    children = []
    identities = []
    try:
        managed = subprocess.Popen([arcctl, 'exec', '--', workload, '0.6'],
                                   stdin=0, stdout=subprocess.PIPE,
                                   stderr=open(root / 'managed-stderr', 'wb'),
                                   process_group=0)
        children.append(managed)
        identities.append(native.native(managed.pid))
        peer_code = ('import sys,time; end=time.monotonic()+0.9; x=1\n'
                     'while time.monotonic()<end: x=(x*17+1)%999983\n'
                     'sys.stdout.buffer.write(sys.stdin.buffer.read())\n')
        peer = subprocess.Popen([sys.executable, '-c', peer_code],
                                stdin=managed.stdout, stdout=open(root / 'reference.json', 'wb'),
                                stderr=open(root / 'peer-stderr', 'wb'), process_group=managed.pid)
        managed.stdout.close()
        children.append(peer)
        identities.append(native.native(peer.pid))
        group = os.getpgid(managed.pid)
        assert group == os.getpgid(peer.pid) == managed.pid
        os.tcsetpgrp(0, group)
        assert os.tcgetpgrp(0) == group
        _, status, _ = os.wait4(managed.pid, 0)
        managed.returncode = os.waitstatus_to_exitcode(status)
        _, status, peer_usage = os.wait4(peer.pid, 0)
        peer.returncode = os.waitstatus_to_exitcode(status)
        assert managed.returncode == peer.returncode == 0
        assert peer_usage.ru_utime + peer_usage.ru_stime > 0.1
        os.tcsetpgrp(0, os.getpgrp())
        result = {'wrapper': identities[0], 'unrelatedPeer': identities[1],
                  'sameForegroundGroup': group, 'realOsPipe': True,
                  'peerUserCoreSeconds': peer_usage.ru_utime,
                  'peerSystemCoreSeconds': peer_usage.ru_stime,
                  'outcomes': [managed.returncode, peer.returncode],
                  'reference': json.loads((root / 'reference.json').read_text())}
        (root / 'controller.json').write_text(json.dumps(result))
    finally:
        for child, identity in zip(children, identities):
            if child.returncode is None:
                native.owned_signal(identity, signal.SIGKILL)
                child.wait(timeout=5)


if __name__ == '__main__':
    root, arcctl, workload = sys.argv[2:] if sys.argv[1] == '--controller' else sys.argv[1:]
    root = Path(root)
    if sys.argv[1] == '--controller':
        controller(root, arcctl, workload)
    else:
        root.mkdir()
        master, slave = pty.openpty()
        try:
            result = subprocess.run([sys.executable, __file__, '--controller', str(root), arcctl, workload],
                                    stdin=slave, capture_output=True, timeout=15)
            assert result.returncode == 0, (result.stdout, result.stderr)
            value = json.loads((root / 'controller.json').read_text())
            assert all(native.native(value[key]['pid']) is None for key in ['wrapper', 'unrelatedPeer'])
            value['allOwnedProcessesAbsent'] = True
            print(json.dumps(value))
        finally:
            os.close(master)
            os.close(slave)
