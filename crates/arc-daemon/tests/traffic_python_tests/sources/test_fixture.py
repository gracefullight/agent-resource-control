"""Offline real pytest/unittest body, with explicit entry/release synchronization."""
import json
import os
import pathlib
import signal
import subprocess
import sys
import time
import unittest

import fixture_dependency

# Only unittest's human duration rendering is made deterministic. Native ARC
# deadlines, fixture monotonic watchdogs, and child CPU accounting are untouched.
unittest.runner.time.perf_counter = lambda: 0.0


class TestFixture(unittest.TestCase):
    def test_selected(self):
        project = pathlib.Path(os.environ["TBS_PROJECT"])
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
                if child.wait(timeout=120) != 0:
                    raise AssertionError("nested native Python failed")
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
        deadline = time.monotonic() + 120
        while not pathlib.Path(os.environ["TBS_RELEASE"]).exists():
            if time.monotonic() > deadline:
                raise AssertionError("explicit fixture release was not received")
            time.sleep(0.01)
        if os.environ.get("TBS_SIGNAL"):
            os.kill(os.getpid(), signal.SIGTERM)
        if os.environ.get("TBS_EXIT"):
            os._exit(int(os.environ["TBS_EXIT"]))
        self.assertNotEqual(os.environ.get("TBS_FAIL"), "1", "fixture requested native test failure")
