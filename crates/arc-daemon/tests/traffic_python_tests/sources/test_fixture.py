"""Offline real pytest/unittest body, with explicit entry/release synchronization."""
import json
import ctypes
import os
import pathlib
import select
import signal
import subprocess
import sys
import unittest

import fixture_dependency

# Only unittest's human duration rendering is made deterministic. Native ARC
# deadlines, fixture monotonic watchdogs, and child CPU accounting are untouched.
unittest.runner.time.perf_counter = lambda: 0.0


def wait_for_release(project):
    # Register the directory before checking for release so publication
    # cannot be missed between observation and the kernel wait.
    fd = os.open(project, os.O_RDONLY)
    queue = select.kqueue()
    try:
        event = select.kevent(
            fd,
            filter=select.KQ_FILTER_VNODE,
            flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR,
            fflags=select.KQ_NOTE_WRITE | select.KQ_NOTE_EXTEND | select.KQ_NOTE_ATTRIB,
        )
        queue.control([event], 0)
        release = pathlib.Path(os.environ["TBS_RELEASE"])
        while not release.exists():
            queue.control(None, 1)
    finally:
        queue.close()
        os.close(fd)


class TestFixture(unittest.TestCase):
    def test_selected(self):
        project = pathlib.Path(os.environ["TBS_PROJECT"])
        if os.environ.get("TBS_SIGINT"):
            # Block before publishing readiness. sigsuspend atomically unblocks
            # the signal at the one instruction shared by both executions.
            signal.signal(signal.SIGINT, signal.default_int_handler)
            old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT})
        with (project / "bodies").open("a") as marker:
            marker.write("body\n")
        data = {
            "cwd": os.getcwd(),
            "env": os.environ["TBS_VALUE"],
            "argv": sys.argv,
            "orig_argv": sys.orig_argv,
            "executable": sys.executable,
            "prefix": sys.prefix,
            "base_prefix": sys.base_prefix,
            "path": sys.path,
            "dependency": fixture_dependency.VALUE,
            "module": __name__,
            "input": list(sys.stdin.buffer.read()) if os.environ.get("TBS_STDIN") else None,
        }
        (project / "native.json").write_text(json.dumps(data, sort_keys=True))
        print("PYTHON_STDOUT 日本語", flush=True)
        print("PYTHON_STDERR", file=sys.stderr, flush=True)
        if os.environ.get("TBS_INNER"):
            inner = pathlib.Path(os.environ["TBS_INNER"])
            env = dict(os.environ, TBS_PROJECT=str(inner), TBS_BODY=str(inner / "bodies"), TBS_RELEASE=str(inner / "release"), PYTHONPATH=str(inner / "imports"))
            env.pop("TBS_INNER")
            child = subprocess.Popen([os.environ["TBS_ARCCTL"], "exec", "--", "python", "-m", "pytest"], cwd=inner, env=env, stdin=subprocess.DEVNULL)
            try:
                if child.wait() != 0:
                    raise AssertionError("nested native Python failed")
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
        if os.environ.get("TBS_SIGINT"):
            libc = ctypes.CDLL(None, use_errno=True)
            mask = ctypes.c_uint32()  # Darwin sigset_t
            assert libc.sigemptyset(ctypes.byref(mask)) == 0
            for blocked in old_mask - {signal.SIGINT}:
                assert libc.sigaddset(ctypes.byref(mask), int(blocked)) == 0
            ready = project / "sigint-ready"
            temporary = project / "sigint-ready.tmp"
            temporary.write_text(str(os.getpid()))
            os.replace(temporary, ready)
            try:
                libc.sigsuspend(ctypes.byref(mask))
                raise AssertionError("default SIGINT handler did not interrupt")
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
        wait_for_release(project)
        if os.environ.get("TBS_SIGNAL"):
            os.kill(os.getpid(), signal.SIGTERM)
        if os.environ.get("TBS_EXIT"):
            os._exit(int(os.environ["TBS_EXIT"]))
        self.assertNotEqual(os.environ.get("TBS_FAIL"), "1", "fixture requested native test failure")
