"""Independent native probes; no product writes or external endpoints."""
import json
import os
from pathlib import Path
import select
import signal
import socket
import sqlite3
import struct
import subprocess
import sys
import tempfile
import uuid

BASE = Path.cwd()
BIN = BASE / 'target/debug'


def exact(stream, count):
    data = b''
    while len(data) < count:
        part = stream.recv(count - len(data))
        assert part
        data += part
    return data


def call(path, operation, payload):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(5)
        stream.connect(str(path))
        body = json.dumps(dict(protocol_version=1, request_id='independent-qa',
                               operation=operation, payload=payload)).encode()
        stream.sendall(struct.pack('>I', len(body)) + body)
        return json.loads(exact(stream, struct.unpack('>I', exact(stream, 4))[0]))


result = {'nativeMacos': sys.platform == 'darwin', 'productIssues': []}
with tempfile.TemporaryDirectory(prefix='arc-qa-', dir='/tmp') as directory:
    root = Path(directory)
    os.chmod(root, 0o700)
    env = dict(os.environ, ARC_DB_PATH=str(root / 'db'), ARC_SOCKET_PATH=str(root / 'socket'))
    env.pop('ARC_RUNTIME_REPORT', None)
    daemon = None
    with (root / 'daemon-stderr').open('wb') as err:
        try:
            daemon = subprocess.Popen([str(BIN / 'arc-daemon')], env=env,
                                      stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=err)
            assert select.select([daemon.stdout], [], [], 10)[0], 'daemon readiness deadline'
            assert b'arc-daemon ready.' in daemon.stdout.readline()
            reader = sqlite3.connect(root / 'db')
            reader.execute('BEGIN')
            reader.execute('SELECT * FROM sqlite_master').fetchall()
            arg_secret, env_secret = ['qa-synthetic-' + uuid.uuid4().hex for _ in range(2)]
            childenv = dict(env, PRIVATE_ENV=env_secret)
            source = '''import os,sys,json
data=sys.stdin.buffer.read()
fds=[]
for n in range(64):
 try: os.fstat(n); fds.append(n)
 except OSError: pass
sys.stdout.buffer.write(data)
print(json.dumps(dict(args=[os.fsencode(x).hex() for x in sys.argv[1:]],cwd=os.fsencode(os.getcwd()).hex(),environment=dict(os.environ),fds=fds),sort_keys=True))
sys.stderr.write("independent-child-stderr\\n")
sys.exit(17)
'''
            argv = [sys.executable, '-c', source, '', 'two words', '--leading', '|;$(not-executed)',
                    '한글', os.fsdecode(b'\xff\xfe'), arg_secret]
            outputs = []
            for managed in [False, True]:
                command = ([str(BIN / 'arcctl'), 'exec', '--'] if managed else []) + argv
                output = subprocess.run(command, input=b'independent-input\0\n', env=childenv,
                                        cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
                assert output.returncode == 17
                outputs.append((output.returncode, output.stdout, output.stderr))
            assert outputs[0] == outputs[1], 'native argv/full environment/cwd/FDs/streams mismatch'
            assert outputs[1][2] == b'independent-child-stderr\n'
            fresh = sqlite3.connect(root / 'db')
            assert fresh.execute('SELECT count(*) FROM job_lifecycle').fetchone()[0] == 1
            assert fresh.execute('SELECT count(*) FROM permits').fetchone()[0] == 0
            dump = '\n'.join(fresh.iterdump()).encode()
            assert (root / 'db-wal').stat().st_size > 0
            checked = []
            for p in root.iterdir():
                if p.is_file():
                    data = p.read_bytes()
                    assert all(s.encode() not in data for s in [arg_secret, env_secret])
                    checked.append(p.name)
            assert all(s.encode() not in dump for s in [arg_secret, env_secret])
            result.update(fullEnvironmentAndNativeArgv=True, identicalStreamsCwdAndDescriptors=True,
                          exit17=True, pinnedLiveWalSecretFree=True, databaseRowsSecretFree=True,
                          diagnosticsSecretFree=True, privacyCheckedFiles=sorted(checked), permits=0)
            reader.rollback()
            reader.close()
            # A real native termination, not a fabricated child outcome.
            terminated = subprocess.run([str(BIN / 'arcctl'), 'exec', '--', str(BIN / 'arc-exec-fixture')],
                                        env=dict(env, ARC_FIXTURE_MODE='term'), stdin=subprocess.DEVNULL,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
            assert terminated.returncode == -signal.SIGTERM
            row = fresh.execute('SELECT job_id,observer_pid,observer_start_token,observer_boot_id,signal,core_dumped '
                                'FROM job_lifecycle WHERE outcome_kind=\'SIGNALED\'').fetchone()
            job_id, pid, token, boot, number, core = row
            observer = dict(pid=pid, start_token=token, boot_id=boot)
            before = call(root / 'socket', 'job.status', dict(query='BY_ID', job_id=job_id))['result']['job']
            completion = dict(job_id=job_id, observer=observer,
                              outcome=dict(kind='SIGNALED', signal=number, core_dumped=bool(core)))
            repeated = call(root / 'socket', 'job.complete', completion)
            assert repeated['result']['disposition'] == 'UNCHANGED'
            assert repeated['result']['job'] == before
            for field, value in [('signal', signal.SIGHUP), ('core_dumped', not bool(core))]:
                conflict = json.loads(json.dumps(completion))
                conflict['outcome'][field] = value
                reply = call(root / 'socket', 'job.complete', conflict)
                assert reply['error']['code'] == 'JOB_CONFLICT'
            after = call(root / 'socket', 'job.status', dict(query='BY_ID', job_id=job_id))['result']['job']
            assert after == before
            status = subprocess.run([str(BIN / 'arcctl'), 'job', 'status', job_id], env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
            assert status.returncode == 0 and not status.stderr
            text = status.stdout.decode()
            assert 'State: terminated by signal' in text and 'Signal: 15' in text and 'Exit code:' not in text
            result.update(nativeSignal=-terminated.returncode, persistedCoreDumped=bool(core),
                          duplicateSignalCompletionUnchanged=True, conflictingSignalAndCoreRejected=True,
                          signaledStatusOutput=text)
            if 'core' not in text.lower():
                result['productIssues'].append(dict(id='QA-001', kind='adopted-cli-contract',
                    actual='Signaled by-ID status omits the recorded core_dumped flag.',
                    expected='contract.json /cli/by_id_labels/SIGNALED requires numeric signal/core flag.'))
            fresh.close()
        finally:
            if daemon and daemon.poll() is None:
                daemon.send_signal(signal.SIGTERM)
            if daemon:
                daemon.wait(timeout=10)
                result['ownedDaemonExit'] = daemon.returncode
                assert daemon.returncode == 0
result['probeCompleted'] = True
print(json.dumps(result, sort_keys=True))
