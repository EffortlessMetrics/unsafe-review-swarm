#!/usr/bin/env python3
"""Cheap Linux process regressions for the shipped consumer entrypoint.

Run: python3 scripts/ci/test_interrupted_receipts.py -v
Only Cargo, metadata transport and timeout durations are fixture-controlled.
Real Bash, process supervision, receipt publication and finalization run.
The harness also supports GNU timeout on the unchanged baseline for red controls.
This does not qualify native GitHub cancellation, uploads or whole-run cleanup.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CARGO = r'''
import json, os, pathlib, signal, sys, time
args = sys.argv[1:]
step = args[0] if args[0] != 'run' else args[args.index('--') + 1]
with open('calls', 'a') as stream:
    stream.write(json.dumps(args) + '\n')
mode = os.environ.get('PROOF_FAULT', '')
if step == 'fmt' and mode:
    receipt = pathlib.Path('target/ci-core/core_exit-123-2')
    receipt.unlink(missing_ok=True)
    if mode == 'partial':
        receipt.with_suffix('.tmp').write_text('0')
        os.kill(os.getpid(), signal.SIGTERM)
    elif mode == 'missing':
        os.kill(os.getpid(), signal.SIGTERM)
    elif mode == 'resist':
        def term(signum, frame):
            pathlib.Path('proof-term').write_text('received')
        signal.signal(signal.SIGTERM, term)
        pathlib.Path('proof-ready').write_text(str(os.getpid()))
        time.sleep(30)
    elif mode == 'cooperate':
        def term(signum, frame):
            pathlib.Path('proof-term').write_text('received')
            sys.exit(42)
        signal.signal(signal.SIGTERM, term)
        time.sleep(30)
if step == 'ci-test':
    directory = pathlib.Path(os.environ['UNSAFE_REVIEW_CI_HANDOFF_DIR'])
    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'test-diagnostics.json').write_text('{"fixture":true}')
if step == 'ci-test-validate' and os.environ.get('FINALIZER_FAULT'):
    mode = os.environ['FINALIZER_FAULT']
    if mode == 'signal':
        os.kill(os.getpid(), signal.SIGTERM)
    if mode == 'resist':
        def term(signum, frame):
            pathlib.Path('finalizer-term').write_text('received')
        signal.signal(signal.SIGTERM, term)
        pathlib.Path('finalizer-ready').write_text(str(os.getpid()))
        time.sleep(30)
if step == os.environ.get('FAIL_STEP'):
    sys.exit(17)
'''


@unittest.skipUnless(sys.platform.startswith('linux') and shutil.which('timeout'),
                     'requires Linux and GNU timeout')
class InterruptedReceipts(unittest.TestCase):
    def run_case(self, **faults):
        temp = tempfile.TemporaryDirectory(prefix='unsafe-interruption-')
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        for path in ('.ci/run.sh', '.ci/core.sh', '.ci/finalize.sh',
                     '.github/scripts/select-core-mode.sh'):
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)
        if (ROOT / '.ci/supervise.py').exists():
            shutil.copyfile(ROOT / '.ci/supervise.py', root / '.ci/supervise.py')
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=root, text=True).strip()
        git('init', '-q')
        git('config', 'user.name', 'CI fixture')
        git('config', 'user.email', 'ci@example.invalid')
        git('add', '.')
        git('commit', '-qm', 'base')
        git('update-ref', 'refs/remotes/origin/main', 'HEAD')
        (root / 'proof.rs').write_text('fixture\n')
        git('add', '.')
        git('commit', '-qm', 'candidate')
        binary = root / 'bin'
        binary.mkdir()
        def tool(name, source):
            path = binary / name
            path.write_text('#!' + sys.executable + '\n' + source)
            path.chmod(0o755)
        tool('cargo', CARGO)
        tool('python3', '''import os, sys
if len(sys.argv) > 1 and sys.argv[1] == '.ci/native_deadline.py':
    with open('deadline-calls', 'a') as stream: stream.write(sys.argv[2] + '\\n')
    print(1000 if sys.argv[2] == 'proof' else 100)
    sys.exit(0)
if len(sys.argv) > 1 and sys.argv[1] == '.ci/supervise.py' and sys.argv[-1] in ('.ci/core.sh', '.ci/finalize.sh'):
    import json, subprocess
    args = sys.argv[1:]
    phase = 'proof' if args[-1] == '.ci/core.sh' else 'finalize'
    duration, grace = args[2], args[4]
    with open('timeouts', 'a') as stream: stream.write(json.dumps(['--kill-after=' + grace + 's', duration + 's']) + '\\n')
    if (phase == 'proof' and os.environ.get('PROOF_FAULT') in ('resist', 'cooperate')) or (phase == 'finalize' and os.environ.get('FINALIZER_FAULT') == 'resist'):
        args[2], args[4] = '0.5', '0.2'
    status = subprocess.call([sys.executable, *args])
    with open('timeout-results', 'a') as stream: stream.write(phase + ':' + str(status) + '\\n')
    sys.exit(status)
os.execv(sys.executable, [sys.executable, *sys.argv[1:]])
''')
        # GNU timeout itself and the actual command are unchanged. Record the
        # production duration/grace arguments, then shorten only fixture time.
        tool('timeout', '''import json, os, subprocess, sys
args = sys.argv[1:]
phase = 'proof' if args[-1] == '.ci/core.sh' else 'finalize'
with open('timeouts', 'a') as stream: stream.write(json.dumps(args) + '\\n')
if (phase == 'proof' and os.environ.get('PROOF_FAULT') in ('resist', 'cooperate')) or (phase == 'finalize' and os.environ.get('FINALIZER_FAULT') == 'resist'):
    args[0] = '--kill-after=0.2s'
    args[1] = '0.5s'
process = subprocess.Popen(['/usr/bin/timeout', *args], start_new_session=True)
with open('process-groups', 'a') as stream: stream.write(str(process.pid) + '\\n')
status = process.wait()
code = 128 - status if status < 0 else status
with open('timeout-results', 'a') as stream: stream.write(phase + ':' + str(code) + '\\n')
sys.exit(code)
''')
        tool('mv', '''import os, sys
from pathlib import Path
fault = os.environ.get('PUBLICATION_FAULT')
if fault == 'receipt' and sys.argv[-1].endswith('/core_exit-123-2'):
    Path('publication-failed').write_text('receipt')
    sys.exit(1)
if fault == 'metadata' and sys.argv[-1].endswith('/metadata.json'):
    Path('publication-failed').write_text('metadata')
    sys.exit(1)
os.execv('/usr/bin/mv', ['mv', *sys.argv[1:]])
''')
        runtime = root / 'runtime'
        runtime.mkdir()
        receipts = root / 'target/ci-core'
        receipts.mkdir(parents=True)
        # Stale success plus an unrelated attempt must never be selected.
        (receipts / 'core_exit-123-2').write_text('0\n')
        (receipts / 'core_exit-123-1').write_text('0\n')
        env = dict(os.environ, PATH=str(binary) + os.pathsep + os.environ['PATH'],
                   RUNNER_TEMP=str(runtime), CARGO_TARGET_DIR=str(runtime / 'cargo-target'),
                   GITHUB_SHA=git('rev-parse', 'HEAD'), GITHUB_EVENT_NAME='merge_group',
                   GITHUB_RUN_ID='123', GITHUB_RUN_ATTEMPT='2',
                   EM_CI_EXECUTION_SELF_HOSTED='true', GITHUB_BASE_REF='main',
                   GITHUB_OUTPUT=str(root / 'output'), GITHUB_STEP_SUMMARY=str(root / 'summary'),
                   PROOF_FAULT='', FINALIZER_FAULT='', PUBLICATION_FAULT='', FAIL_STEP='')
        env.update(faults)
        def cleanup_groups():
            groups = root / 'process-groups'
            for group in groups.read_text().splitlines() if groups.exists() else []:
                try:
                    os.killpg(int(group), signal.SIGKILL)
                except ProcessLookupError:
                    pass
        process = subprocess.Popen(['bash', '.ci/run.sh'], cwd=root, env=env,
                                   text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        try:
            try:
                stdout, stderr = process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                (root / 'outer-guard-fired').write_text('descendant retained pipes or run stalled')
                cleanup_groups()
                stdout, stderr = process.communicate(timeout=2)
            # Snapshot live fixture descendants before cleanup, including the
            # case where timeout returned after its shell died but Cargo did not.
            survivors = []
            for ready in ('proof-ready', 'finalizer-ready'):
                if (root / ready).exists():
                    pid = (root / ready).read_text()
                    stat = Path('/proc') / pid / 'stat'
                    if stat.exists() and stat.read_text().split()[2] != 'Z':
                        survivors.append(ready)
            (root / 'survivors').write_text('\n'.join(survivors))
            result = subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)
        finally:
            cleanup_groups()
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate(timeout=2)
        calls = [json.loads(line) for line in (root / 'calls').read_text().splitlines()]
        return root, result, calls

    def assert_receipt(self, root, status):
        self.assertEqual((root / 'target/ci-core/core_exit-123-2').read_text(), f'{status}\n')
        self.assertFalse((root / 'target/ci-core/core_exit-123-2.tmp').exists())
        self.assertEqual((root / 'target/ci-core/core_exit-123-1').read_text(), '0\n')

    def test_control_finishes_all_producers_and_finalization(self):
        root, result, calls = self.run_case()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 5)
        self.assert_receipt(root, 0)
        self.assertEqual((root / 'deadline-calls').read_text(), 'proof\nfinalize\nresult\n')

    def test_partial_or_missing_receipt_after_interrupt_is_replaced(self):
        for fault in ('partial', 'missing'):
            with self.subTest(fault=fault):
                root, result, calls = self.run_case(PROOF_FAULT=fault)
                self.assertEqual(result.returncode, 143, result.stderr)
                self.assertEqual(calls, [['fmt', '--all', '--check']])
                self.assert_receipt(root, 143)

    def test_proof_termination_grace_preserves_timeout_and_escalation(self):
        for fault in ('cooperate', 'resist'):
            with self.subTest(fault=fault):
                root, result, calls = self.run_case(PROOF_FAULT=fault)
                self.assertIn(result.returncode, (124, 137), result.stderr)
                status = result.returncode
                self.assertEqual(calls, [['fmt', '--all', '--check']])
                self.assertEqual((root / 'proof-term').read_text(), 'received')
                self.assertEqual((root / 'survivors').read_text(), '', 'Cargo survived timeout grace')
                self.assert_receipt(root, status)
                args = json.loads((root / 'timeouts').read_text().splitlines()[0])
                self.assertEqual(args[:2], ['--kill-after=30s', '1000s'])
                self.assertIn(f'proof:{status}\n', (root / 'timeout-results').read_text())

    def test_failed_receipt_publication_preserves_interrupt_and_product_failure(self):
        for faults, status in (({}, 78), ({'FAIL_STEP': 'clippy'}, 17),
                              ({'PROOF_FAULT': 'partial'}, 143),
                              ({'PROOF_FAULT': 'cooperate'}, 124)):
            with self.subTest(faults=faults):
                root, result, _ = self.run_case(PUBLICATION_FAULT='receipt', **faults)
                self.assertEqual(result.returncode, status, result.stderr)
                self.assertEqual((root / 'publication-failed').read_text(), 'receipt')
                self.assertFalse((root / 'target/ci-core/core_exit-123-2').exists())
                self.assertFalse((root / 'output').exists())
                self.assertEqual((root / 'deadline-calls').read_text(), 'proof\n')

    def test_finalizer_sigterm_or_grace_escalation_preserves_original_failure(self):
        for fault in ('signal', 'resist'):
            with self.subTest(fault=fault):
                root, result, calls = self.run_case(FAIL_STEP='ci-test', FINALIZER_FAULT=fault)
                self.assertEqual(result.returncode, 17, result.stderr)
                self.assert_receipt(root, 17)
                self.assertEqual(len(calls), 4)  # fmt, clippy, ci-test, validator
                self.assertNotIn('diagnostics_path=', (root / 'output').read_text()
                                 if (root / 'output').exists() else '')
                self.assertEqual((root / 'deadline-calls').read_text(), 'proof\nfinalize\nresult\n')
                if fault == 'resist':
                    self.assertFalse((root / 'outer-guard-fired').exists(), 'finalizer descendant retained pipes beyond timeout grace')
                    self.assertEqual((root / 'survivors').read_text(), '')
                    self.assertEqual((root / 'finalizer-term').read_text(), 'received')
                    self.assertTrue(any(line in ('finalize:124', 'finalize:137')
                                        for line in (root / 'timeout-results').read_text().splitlines()))
                    self.assertFalse((root / 'output').exists())
                    args = json.loads((root / 'timeouts').read_text().splitlines()[1])
                    self.assertEqual(args[:2], ['--kill-after=10s', '100s'])

    def test_partial_failure_evidence_publication_cannot_mask_product_failure(self):
        root, result, _ = self.run_case(FAIL_STEP='ci-test', PUBLICATION_FAULT='metadata')
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assert_receipt(root, 17)
        self.assertEqual((root / 'publication-failed').read_text(), 'metadata')
        # A summary may already be staged, but no incomplete selector is emitted.
        self.assertEqual(len(list((root / 'runtime').glob('**/summary.md'))), 1)
        self.assertFalse((root / 'output').exists())


if __name__ == '__main__':
    unittest.main()
