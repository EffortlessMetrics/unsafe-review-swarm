"""Loopback-only native metadata faults; no GitHub requests or product work."""
import contextlib
import datetime as dt
import http.server
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]


class MetadataTransport(unittest.TestCase):
    @contextlib.contextmanager
    def transport(self, mode):
        stop = threading.Event()
        reached = threading.Event()
        origin = dt.datetime.now(dt.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
        repo = 'EffortlessMetrics/unsafe-review-swarm'
        run = dict(id=123, run_attempt=1, repository=dict(full_name=repo),
                   event='pull_request', path='.github/workflows/ci.yml', status='in_progress',
                   head_sha='a'*40, pull_requests=[dict(number=2380, head=dict(sha='a'*40),
                                                      base=dict(sha='b'*40))], created_at=origin)
        payload = json.dumps(run).encode()
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args): pass
            def do_GET(self):
                reached.set()
                if mode == 'stall':
                    stop.wait(3)
                    return
                body = b'x'*2_000_001 if mode == 'oversized' else payload
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                try:
                    if mode == 'trickle':
                        for byte in body:
                            if stop.is_set(): break
                            self.wfile.write(bytes([byte])); self.wfile.flush()
                            stop.wait(.03)
                    else:
                        self.wfile.write(body); self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError): pass
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        worker = threading.Thread(target=server.serve_forever, kwargs={'poll_interval': .01}, daemon=True)
        worker.start()
        try:
            yield f'http://127.0.0.1:{server.server_port}', reached, repo
        finally:
            stop.set(); server.shutdown(); server.server_close(); worker.join(timeout=1)

    def run_phase(self, phase, mode, cancel=False):
        with self.transport(mode) as (url, reached, repo), tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            event = dict(action='edited', pull_request=dict(number=2380, draft=False,
                         merge_commit_sha='c'*40, head=dict(sha='a'*40, repo=dict(full_name=repo)),
                         base=dict(sha='b'*40, repo=dict(full_name=repo))))
            event_path = root/'event.json'; event_path.write_text(json.dumps(event))
            # Redirect only urlopen's transport to loopback; the shipped adapter
            # still constructs and validates its fixed native API request.
            driver = root/'driver.py'
            driver.write_text('import runpy,sys,urllib.request\n'
                'original=urllib.request.urlopen\n'
                'def local(request, timeout):\n'
                " assert request.full_url == 'https://api.github.com/repos/EffortlessMetrics/unsafe-review-swarm/actions/runs/123/attempts/1'\n"
                f' return original({url!r}, timeout=timeout)\n'
                'urllib.request.urlopen=local\n'
                f'runpy.run_path({str(ROOT/".ci/native_deadline.py")!r},run_name="__main__")\n')
            source = (ROOT/'.ci/run.sh').read_text()
            if 'metadata_budget()' in source:
                function = source.split('metadata_budget() {', 1)[1].split('\n}', 1)[0]
                self.assertIn('--seconds 5 --grace 1', function)
                function = function.replace('--seconds 5 --grace 1', '--seconds 0.4 --grace 0.2')
                function = function.replace('python3 .ci/native_deadline.py', f'python3 {driver}')
                command = ['bash', '-c', 'metadata_budget() {'+function+'\n}\nmetadata_budget "$1"', 'fixture', phase]
            else:
                command = [sys.executable, str(driver), phase]
            env = dict(os.environ, GITHUB_REPOSITORY=repo, GITHUB_EVENT_NAME='pull_request',
                       GITHUB_RUN_ID='123', GITHUB_RUN_ATTEMPT='1', GITHUB_REF='refs/pull/2380/merge',
                       GITHUB_SHA='c'*40, GITHUB_EVENT_PATH=str(event_path))
            started = time.monotonic()
            process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, text=True, start_new_session=True)
            try:
                if cancel:
                    self.assertTrue(reached.wait(1), 'local transport was not reached')
                    os.killpg(process.pid, signal.SIGTERM)
                try:
                    stdout, stderr = process.communicate(timeout=1.6)
                except subprocess.TimeoutExpired:
                    self.fail('metadata transport exceeded total-time guard')
                return process.returncode, stdout, stderr, time.monotonic()-started
            finally:
                try: os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError: pass
                process.wait(timeout=1)
                process.stdout.close(); process.stderr.close()

    def test_trickle_and_stall_are_bounded_in_every_phase(self):
        for mode in ('trickle', 'stall'):
            for phase in ('proof', 'finalize', 'result'):
                with self.subTest(mode=mode, phase=phase):
                    status, output, _, elapsed = self.run_phase(phase, mode)
                    self.assertEqual(status, 124)
                    self.assertEqual(output, '')
                    self.assertLess(elapsed, 1.5)

    def test_cancellation_releases_capture_pipes_in_every_phase(self):
        for phase in ('proof', 'finalize', 'result'):
            with self.subTest(phase=phase):
                status, output, _, elapsed = self.run_phase(phase, 'trickle', cancel=True)
                self.assertIn(status, (-signal.SIGTERM, 143))
                self.assertEqual(output, '')
                self.assertLess(elapsed, 1.5)

    def test_oversized_response_is_refused_without_budget_output(self):
        for phase in ('proof', 'finalize', 'result'):
            with self.subTest(phase=phase):
                status, output, error, _ = self.run_phase(phase, 'oversized')
                self.assertEqual(status, 78, error)
                self.assertEqual(output, '')

    def test_ordinary_native_response_retains_phase_budget(self):
        for phase, maximum in [('proof', 1020), ('finalize', 1144), ('result', 1199)]:
            with self.subTest(phase=phase):
                status, output, error, _ = self.run_phase(phase, 'ordinary')
                self.assertEqual(status, 0, error)
                self.assertGreater(int(output), maximum-5)
                self.assertLessEqual(int(output), maximum)


if __name__ == '__main__':
    unittest.main()
