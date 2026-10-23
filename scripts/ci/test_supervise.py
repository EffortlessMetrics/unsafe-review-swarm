#!/usr/bin/env python3
"""Focused Linux process-group, EOF and bounded-output supervision controls."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
SUPERVISOR = ROOT / '.ci/supervise.py'


@unittest.skipUnless(sys.platform.startswith('linux'), 'Linux process-group controls')
class Supervision(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='unsafe-supervise-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def command(self, code, seconds='0.6', grace='0.3'):
        return [sys.executable, str(SUPERVISOR), '--seconds', seconds, '--grace', grace,
                '--', sys.executable, '-c', code]

    def live(self, pid):
        stat = Path('/proc') / str(pid) / 'stat'
        return stat.exists() and stat.read_text().split()[2] != 'Z'

    def kill(self, pid):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    def wait_file(self, name):
        path = self.root / name
        deadline = time.monotonic() + 3
        while not path.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertTrue(path.exists(), name)
        return int(path.read_text())

    def test_ordinary_completion_preserves_both_streams_and_status(self):
        for status in (0, 17):
            with self.subTest(status=status):
                result = subprocess.run(self.command(
                    f'import sys; print("out"); print("err", file=sys.stderr); sys.exit({status})'),
                    capture_output=True, text=True, timeout=3)
                self.assertEqual(result.returncode, status)
                self.assertEqual(result.stdout, 'out\n')
                self.assertEqual(result.stderr, 'err\n')

    def test_leader_exit_kills_resistant_descendant_and_reaches_eof(self):
        child = "import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(30)"
        code = ('import subprocess,sys,pathlib; '
                f'p=subprocess.Popen([sys.executable,"-c",{child!r}]); '
                'pathlib.Path("child").write_text(str(p.pid)); print("leader done")')
        result = subprocess.run(self.command(code), cwd=self.root, capture_output=True,
                                text=True, timeout=3)
        pid = self.wait_file('child')
        self.addCleanup(self.kill, pid)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, 'leader done\n')
        self.assertFalse(self.live(pid), 'owned child survived leader completion')

    def test_timeout_kills_resistant_group_without_harming_other_group(self):
        unrelated = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'],
                                     start_new_session=True)
        self.addCleanup(lambda: (unrelated.kill(), unrelated.wait()))
        code = ('import signal,time,pathlib,os; '
                'signal.signal(signal.SIGTERM, signal.SIG_IGN); '
                'pathlib.Path("child").write_text(str(os.getpid())); time.sleep(30)')
        start = time.monotonic()
        result = subprocess.run(self.command(code), cwd=self.root, capture_output=True,
                                text=True, timeout=3)
        pid = self.wait_file('child')
        self.addCleanup(self.kill, pid)
        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertLess(time.monotonic() - start, 2)
        self.assertFalse(self.live(pid))
        self.assertIsNone(unrelated.poll(), 'unrelated group was signaled')

    def test_supervisor_cancellation_cleans_group_and_returns_signal_status(self):
        code = ('import signal,time,pathlib,os; '
                'signal.signal(signal.SIGTERM, signal.SIG_IGN); '
                'pathlib.Path("child").write_text(str(os.getpid())); time.sleep(30)')
        process = subprocess.Popen(self.command(code, seconds='20'), cwd=self.root,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.addCleanup(lambda: process.poll() is None and process.kill())
        pid = self.wait_file('child')
        self.addCleanup(self.kill, pid)
        process.send_signal(signal.SIGTERM)
        stdout, stderr = process.communicate(timeout=3)
        self.assertEqual(process.returncode, 143, (stdout, stderr))
        self.assertFalse(self.live(pid))

    def test_backpressured_output_does_not_extend_deadline(self):
        code = 'import os; os.write(1, b"x" * 1000000)'
        process = subprocess.Popen(self.command(code), stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE)
        self.addCleanup(lambda: process.poll() is None and process.kill())
        # Deliberately do not consume stdout while waiting for termination.
        status = process.wait(timeout=3)
        stdout, stderr = process.communicate(timeout=1)
        self.assertEqual(status, 124, stderr)
        self.assertGreater(len(stdout), 0)
        self.assertLess(len(stdout), 1000000)

    def test_cancellation_during_output_drain_cannot_report_success(self):
        import fcntl
        for primary, expected in ((0, 143), (17, 17)):
            with self.subTest(primary=primary):
                marker = f'leader-{primary}'
                code = ('import os,pathlib,sys; os.write(1,b"x"*20000); '
                        f'pathlib.Path({marker!r}).write_text(str(os.getpid())); '
                        f'sys.exit({primary})')
                process = subprocess.Popen(self.command(code, seconds='10', grace='3'),
                                           cwd=self.root, stdout=subprocess.PIPE,
                                           stderr=subprocess.PIPE)
                self.addCleanup(lambda p=process: p.poll() is None and p.kill())
                fcntl.fcntl(process.stdout, fcntl.F_SETPIPE_SZ, 4096)
                self.wait_file(marker)
                # Let the supervisor reap the leader, while backpressure keeps
                # it in the drain phase with the primary verdict already set.
                time.sleep(0.2)
                self.assertIsNone(process.poll())
                process.send_signal(signal.SIGTERM)
                time.sleep(0.1)
                stdout, stderr = process.communicate(timeout=2)
                self.assertEqual(process.returncode, expected, stderr)
                self.assertEqual(stdout, b'x' * 20000)

    def test_missing_eof_from_escaped_child_cannot_report_success(self):
        child = 'import time; time.sleep(30)'
        code = ('import subprocess,sys,pathlib; '
                f'p=subprocess.Popen([sys.executable,"-c",{child!r}],start_new_session=True); '
                'pathlib.Path("escaped").write_text(str(p.pid))')
        result = subprocess.run(self.command(code), cwd=self.root, capture_output=True,
                                text=True, timeout=3)
        pid = self.wait_file('escaped')
        self.addCleanup(self.kill, pid)
        self.assertEqual(result.returncode, 78, result.stderr)
        # Group escape is outside this helper's containment contract. The test
        # owns and cleans this intentionally escaped process explicitly.


if __name__ == '__main__':
    unittest.main()
