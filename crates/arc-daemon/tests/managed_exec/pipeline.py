"""Native macOS foreground job controller; no shell or timing-based signal dedup."""
import ctypes
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import socket
import struct
import subprocess
import sys
import termios
import threading
import time

LIBC = ctypes.CDLL(None, use_errno=True)


class Bsd(ctypes.Structure):
    _fields_ = [('head', ctypes.c_uint32 * 12), ('comm', ctypes.c_char * 16),
                ('name', ctypes.c_char * 32), ('tail', ctypes.c_uint32 * 6),
                ('sec', ctypes.c_uint64), ('usec', ctypes.c_uint64)]


class Timeval(ctypes.Structure):
    _fields_ = [('sec', ctypes.c_long), ('usec', ctypes.c_int)]


class Prefix(ctypes.Structure):
    _fields_ = [('start', Timeval), ('vm', ctypes.c_void_p), ('sa', ctypes.c_void_p),
                ('flags', ctypes.c_int), ('status', ctypes.c_char), ('pid', ctypes.c_int)]


def native(pid):
    value = Bsd()
    size = LIBC.proc_pidinfo(pid, 3, 0, ctypes.byref(value), ctypes.sizeof(value))
    if size == ctypes.sizeof(value):
        return {'pid': pid, 'start_token': str(value.sec * 1000000 + value.usec)}
    mib = (ctypes.c_int * 4)(1, 14, 1, pid)
    data = ctypes.create_string_buffer(4096)
    length = ctypes.c_size_t(len(data))
    rc = LIBC.sysctl(mib, 4, data, ctypes.byref(length), None, 0)
    if rc == 0 and length.value == 0:
        return None
    if rc == 0 and length.value >= ctypes.sizeof(Prefix):
        value = Prefix.from_buffer_copy(data)
        if value.pid == pid:
            return {'pid': pid, 'start_token': str(value.start.sec * 1000000 + value.start.usec)}
    raise RuntimeError(f'native ownership indeterminate for {pid}')


def exact(stream, count):
    result = b''
    while len(result) < count:
        part = stream.recv(count - len(result))
        assert part, 'control EOF'
        result += part
    return result


def receive(stream):
    return json.loads(exact(stream, struct.unpack('>I', exact(stream, 4))[0]))


def send(stream, value):
    data = json.dumps(value).encode()
    stream.sendall(struct.pack('>I', len(data)) + data)


def stopped(identity):
    assert matches(identity)
    value = Bsd()
    size = LIBC.proc_pidinfo(identity["pid"], 3, 0, ctypes.byref(value), ctypes.sizeof(value))
    assert size == ctypes.sizeof(value)
    return value.head[1] == 4  # sys/proc.h SSTOP; proc_info.h pbi_status


def matches(identity):
    return native(identity['pid']) == {key: identity[key] for key in ['pid', 'start_token']}


def owned_signal(identity, number):
    if matches(identity):
        os.kill(identity['pid'], number)


def controller(root, managed, fixture, arcctl, ignored_cont=False):
    """Own two siblings and a real OS pipe as a non-orphaned shell job."""
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)
    os.tcsetpgrp(0, os.getpgrp())
    stream = socket.socket(socket.AF_UNIX)
    stream.connect(str(root / 'controller.sock'))
    for number in [signal.SIGTTOU, signal.SIGTTIN, signal.SIGTSTP]:
        signal.signal(number, signal.SIG_IGN)
    children = []
    identities = []

    def setup(group):
        def child():
            os.setpgid(0, group)
            for number in [signal.SIGINT, signal.SIGQUIT, signal.SIGTERM, signal.SIGHUP,
                           signal.SIGTSTP, signal.SIGTTIN, signal.SIGTTOU]:
                signal.signal(number, signal.SIG_DFL)
            signal.signal(signal.SIGCONT, signal.SIG_IGN if ignored_cont else signal.SIG_DFL)
        return child

    try:
        env = dict(os.environ, ARC_FIXTURE_PIPE_ROLE='producer',
                   ARC_FIXTURE_PIPE_CONTROL=str(root / 'producer.sock'),
                   ARC_FIXTURE_MARKER=str(root / 'calls'))
        argv = [fixture] if not managed else [arcctl, 'exec', '--', fixture]
        producer = subprocess.Popen(argv, env=env, stdout=subprocess.PIPE, preexec_fn=setup(0))
        children.append(producer)
        identities.append(native(producer.pid))
        env.update(ARC_FIXTURE_PIPE_ROLE='peer', ARC_FIXTURE_PIPE_CONTROL=str(root / 'peer.sock'))
        peer = subprocess.Popen([fixture], env=env, stdin=producer.stdout, preexec_fn=setup(producer.pid))
        producer.stdout.close()
        children.append(peer)
        identities.append(native(peer.pid))
        os.tcsetpgrp(0, producer.pid)
        send(stream, {'identities': identities, 'group': producer.pid,
                      'controller': native(os.getpid())})
        while True:
            command = receive(stream)
            if command['op'] == 'events':
                events = []
                for child in children:
                    if child.returncode is not None:
                        continue
                    while True:
                        pid, status = os.waitpid(child.pid, os.WUNTRACED | os.WCONTINUED | os.WNOHANG)
                        if not pid:
                            break
                        event = {'pid': pid, 'raw': status}
                        if os.WIFSTOPPED(status):
                            event.update(kind='stopped', signal=os.WSTOPSIG(status))
                        elif os.WIFCONTINUED(status):
                            event.update(kind='continued')
                        else:
                            child.returncode = os.waitstatus_to_exitcode(status)
                            event.update(kind='exited', outcome=child.returncode)
                        events.append(event)
                        if child.returncode is not None:
                            break
                send(stream, {'events': events, 'foreground': os.tcgetpgrp(0),
                              'outcomes': [c.returncode for c in children]})
            elif command['op'] == 'foreground':
                os.tcsetpgrp(0, producer.pid if command['job'] else os.getpgrp())
                send(stream, {'foreground': os.tcgetpgrp(0)})
            elif command['op'] == 'group':
                assert all(c.returncode is None for c in children)
                assert all(matches(i) for i in identities)
                os.killpg(producer.pid, command['signal'])
                send(stream, {'sent': command['signal']})
            elif command['op'] == 'pid':
                owned_signal(identities[0], command['signal'])
                send(stream, {'sent': command['signal']})
            elif command['op'] == 'quit':
                break
            else:
                raise AssertionError(command)
    finally:
        for identity, child in zip(identities, children):
            if child.returncode is None:
                owned_signal(identity, signal.SIGKILL)
                child.wait(timeout=5)
        os.tcsetpgrp(0, os.getpgrp())
        send(stream, {'cleanup': True, 'foreground': os.tcgetpgrp(0)})
        stream.close()


def listen(path):
    value = socket.socket(socket.AF_UNIX)
    value.bind(str(path))
    value.listen(1)
    value.settimeout(8)
    return value


def accept(listener):
    value, _ = listener.accept()
    value.settimeout(8)
    return value, receive(value)


def scenario(base, managed, mode, fixture, arcctl):
    root = base / f'{managed}-{mode}'
    root.mkdir()
    listeners = [listen(root / f'{role}.sock') for role in ['controller', 'producer', 'peer']]
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    data = bytearray()
    stop = threading.Event()
    os.set_blocking(master, False)

    def drain():
        while not stop.is_set():
            if select.select([master], [], [], .01)[0]:
                try:
                    data.extend(os.read(master, 4096))
                except OSError:
                    pass

    reader = threading.Thread(target=drain)
    reader.start()
    proc = subprocess.Popen([sys.executable, __file__, '--controller', str(root), str(int(managed)),
                             fixture, arcctl, str(int(mode == 'ignored-cont'))], stdin=slave, stdout=slave, stderr=slave)
    streams, identities, events = [], [], []
    result = {'managed': managed, 'mode': mode, 'pass': False}
    try:
        control, job = accept(listeners[0])
        streams.append(control)
        identities.extend(job['identities'])
        identities.append(job['controller'])
        producer, producer_info = accept(listeners[1])
        streams.append(producer)
        identities.append(producer_info['identity'])
        peer, peer_info = accept(listeners[2])
        streams.append(peer)
        result.update(job=job, producer=producer_info, peer=peer_info)

        assert producer_info['pgid'] == peer_info['pgid'] == job['group'], (producer_info, peer_info, job)

        def request(value):
            send(control, value)
            reply = receive(control)
            events.extend(reply.get('events', []))
            return reply

        def poll(predicate, message):
            end = time.monotonic() + 6
            while True:
                reply = request({'op': 'events'})
                if predicate(reply):
                    return reply
                assert time.monotonic() < end, (message, events, bytes(data).decode(errors='replace'))
                select.select([], [], [], .001)

        def tty_input(stream=peer):
            stream.sendall(b't')
            os.write(master, b'pipeline-input\n')
            end = time.monotonic() + 6
            while not select.select([stream], [], [], .005)[0]:
                state = request({'op': 'events'})
                stopped = [e for e in events if e['pid'] == peer_info['identity']['pid'] and e['kind'] == 'stopped']
                assert not stopped, ('pipeline peer lost terminal input', state, stopped)
                assert time.monotonic() < end, ('peer input timeout', state)
            value = receive(stream)
            result.setdefault('input', []).append(value)
            assert value['input'] == list(b'pipeline-input\n'), value
            assert value['pgid'] == value['foreground'], value

        producer.sendall(b'p')
        assert receive(producer)['pipeline']
        peer.sendall(b'p')
        assert receive(peer)['pipeline']
        if mode == 'input':
            tty_input(producer)
            tty_input()
        elif mode in ['terminal-int', 'group-int', 'wrapper-term']:
            if mode == 'terminal-int':
                os.write(master, b'\x03')
            elif mode == 'group-int':
                request({'op': 'group', 'signal': signal.SIGINT})
            else:
                owned_signal(job['identities'][0], signal.SIGTERM)
            wanted = [1, 0 if mode == 'wrapper-term' else 1]
            counts = []
            end = time.monotonic() + 6
            while True:
                counts = []
                for stream in [producer, peer]:
                    stream.sendall(b'c')
                    counts.append(receive(stream))
                actual = [v['signals'] for v in counts]
                assert all(a <= b for a, b in zip(actual, wanted)), ('duplicate signal', counts)
                if actual == wanted:
                    break
                assert time.monotonic() < end, ('signal delivery timeout', counts)
            result['signalObservations'] = counts
            tty_input()
        elif mode == 'provenance':
            assert not managed
            observations = []
            for operation, expected in [('pid', [1, 0]), ('group', [2, 1])]:
                request({'op': operation, 'signal': signal.SIGINT})
                counts = []
                end = time.monotonic() + 6
                while True:
                    counts = []
                    for stream in [producer, peer]:
                        stream.sendall(b'c')
                        counts.append(receive(stream))
                    actual = [v['signals'] for v in counts]
                    assert all(a <= b for a, b in zip(actual, expected)), counts
                    if actual == expected:
                        break
                    assert time.monotonic() < end, counts
                observations.append(counts)
            first, second = [value[0] for value in observations]
            assert first['code'] == second['code'] and first['sender'] == second['sender'], observations
            result['pidVsGroup'] = observations
            result['sameNativeProvenance'] = True
            tty_input()
        elif mode in ['default-cont', 'ignored-cont']:
            assert managed
            assert producer_info['sigcont_ignored'] == (mode == 'ignored-cont'), producer_info
            owned_signal(producer_info['identity'], signal.SIGSTOP)
            end = time.monotonic() + 6
            while not stopped(producer_info['identity']):
                assert time.monotonic() < end, 'target stop not observed'
                select.select([], [], [], .001)
            result['targetStopObserved'] = True
            poll(lambda _: any(e['pid'] == job['group'] and e['kind'] == 'stopped'
                              for e in events), 'wrapper did not mirror target stop')
            assert not stopped(peer_info['identity']), 'unrelated peer stopped'
            owned_signal(job['identities'][0], signal.SIGCONT)
            poll(lambda _: any(e['pid'] == job['group'] and e['kind'] == 'continued'
                              for e in events), 'wrapper-only continuation failed')
            producer.sendall(b'c')
            result['targetAfterWrapperContinue'] = receive(producer)
            assert not stopped(producer_info['identity']), 'target stayed stopped'
            tty_input(producer)
            tty_input()
        elif mode == 'resume-before-wait':
            # Freeze the wrapper before queuing a target-only stop/continue.
            # Its eventual consuming wait must not mirror the stale stop.
            owned_signal(job['identities'][0], signal.SIGSTOP)
            poll(lambda _: any(e['kind'] == 'stopped' for e in events), 'wrapper freeze failed')
            if managed:
                owned_signal(producer_info['identity'], signal.SIGSTOP)
                end = time.monotonic() + 6
                while not stopped(producer_info['identity']):
                    assert time.monotonic() < end, 'target stop not observed'
                owned_signal(producer_info['identity'], signal.SIGCONT)
                producer.sendall(b'c')
                result['targetContinuedBeforeWrapper'] = receive(producer)
            owned_signal(job['identities'][0], signal.SIGCONT)
            poll(lambda _: any(e['kind'] == 'continued' for e in events), 'wrapper continue failed')
            tty_input(producer)
            tty_input()
        elif mode == 'stop-resume':
            os.write(master, b'\x1a')
            poll(lambda _: len({e['pid'] for e in events if e['kind'] == 'stopped'}) == 2,
                 'whole pipeline did not stop')
            request({'op': 'foreground', 'job': False})
            request({'op': 'group', 'signal': signal.SIGCONT})
            poll(lambda _: len({e['pid'] for e in events if e['kind'] == 'continued'}) == 2,
                 'background continuation failed')
            peer.sendall(b't')
            poll(lambda _: any(e['kind'] == 'stopped' and e.get('signal') == signal.SIGTTIN for e in events),
                 'background terminal read must stop')
            request({'op': 'foreground', 'job': True})
            request({'op': 'group', 'signal': signal.SIGCONT})
            os.write(master, b'pipeline-input\n')
            value = receive(peer)
            assert value['input'] == list(b'pipeline-input\n'), value
            result['resumedInput'] = value
        producer.sendall(b'x')
        peer.sendall(b'x')
        final = poll(lambda value: all(v is not None for v in value['outcomes']), 'job exit timeout')
        assert final['outcomes'] == [17, 0], final
        assert (root / 'calls').read_text().splitlines() == [str(producer_info['identity']['pid'])]
        request({'op': 'foreground', 'job': False})
        assert termios.tcgetattr(slave) == before, 'terminal modes changed'
        result['outcomes'] = final['outcomes']
        result['pass'] = True
    except BaseException as error:
        result['error'] = f'{type(error).__name__}: {error}'
        result['failureStates'] = [{'identity': i, 'stopped': stopped(i)}
                                   for i in identities if matches(i)]
    finally:
        # Never signal a name or unverified reused PID. Leave the controller
        # alive until its own direct children have been reaped.
        for identity in identities:
            if identity.get('pid') != proc.pid:
                owned_signal(identity, signal.SIGKILL)
        if streams:
            try:
                send(streams[0], {'op': 'quit'})
                result['cleanup'] = receive(streams[0])
            except (OSError, AssertionError) as error:
                result['cleanupError'] = str(error)
        try:
            proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            original = next((i for i in identities if i['pid'] == proc.pid), None)
            if original:
                owned_signal(original, signal.SIGKILL)
            proc.wait(timeout=5)
        end = time.monotonic() + 5
        while True:
            remaining = [i for i in identities if matches(i)]
            if not remaining or time.monotonic() >= end:
                break
            select.select([], [], [], .005)
        result.update(events=events, allRecordedIdentitiesAbsent=not remaining,
                      remainingIdentities=remaining, controllerOutcome=proc.returncode)
        stop.set()
        reader.join(timeout=2)
        result['ptyOutput'] = data.decode(errors='replace')
        for stream in streams + listeners:
            stream.close()
        os.close(master)
        os.close(slave)
    print(json.dumps(result), flush=True)
    return result['pass'] and result['allRecordedIdentitiesAbsent'] and proc.returncode == 0


if __name__ == '__main__':
    if sys.argv[1] == '--controller':
        controller(Path(sys.argv[2]), bool(int(sys.argv[3])), sys.argv[4], sys.argv[5], bool(int(sys.argv[6])))
    elif sys.argv[1] == '--continuation':
        root, fixture, arcctl = sys.argv[2:]
        results = [scenario(Path(root), True, mode, fixture, arcctl)
                   for mode in ['default-cont', 'ignored-cont']]
        raise SystemExit(0 if all(results) else 1)
    else:
        root, fixture, arcctl = sys.argv[1:]
        passed = True
        for managed in [False, True]:
            for mode in ['input', 'terminal-int', 'group-int', 'wrapper-term', 'stop-resume', 'resume-before-wait'] + ([] if managed else ['provenance']):
                passed = scenario(Path(root), managed, mode, fixture, arcctl) and passed
        raise SystemExit(0 if passed else 1)
