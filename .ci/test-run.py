#!/usr/bin/env python3
"""Execute the shipped Bash entrypoint with fault-controlled Cargo commands.

These are orchestration tests, not product or runner qualification.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
CARGO = '''#!/usr/bin/env python3
import json, os, pathlib, signal, sys, time
args = sys.argv[1:]
step = args[0] if args[0] != 'run' else args[args.index('--') + 1]
with open(os.environ['CALLS'], 'a') as stream:
    stream.write(json.dumps(args) + '\\n')
if step == 'fmt' and os.environ.get('SLOW_CORE'):
    time.sleep(5)
if step == 'fmt' and os.environ.get('SIGNAL_CORE'):
    os.kill(os.getpid(), signal.SIGTERM)
if step == 'ci-test':
    root = pathlib.Path(os.environ['UNSAFE_REVIEW_CI_HANDOFF_DIR'])
    root.mkdir(parents=True, exist_ok=True)
    (root / 'test-diagnostics.json').write_text('{"schema":"fixture"}')
if step == 'ci-test-validate' and os.environ.get('REJECT_DIAGNOSTICS'):
    sys.exit(4)
if step == os.environ.get('FAIL_STEP'):
    print('RAW_SECRET_FIXTURE_DO_NOT_RETAIN')
    sys.exit(17)
sys.exit(0)
'''


class ProofContract(unittest.TestCase):
    def run_case(self, fail='', docs=False, bad_target=False, bad_sha=False,
                 reject_diagnostics=False, fail_staging=False,
                 fail_receipt=False, slow_core=False, signal_core=False,
                 deadline_failure=''):
        temp = tempfile.TemporaryDirectory(prefix='unsafe-proof-test-')
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        shutil.copytree(ROOT / '.ci', root / '.ci')
        shutil.copytree(ROOT / '.github/scripts', root / '.github/scripts')
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=root, text=True).strip()
        git('init', '-q')
        git('config', 'user.name', 'CI fixture')
        git('config', 'user.email', 'ci@example.invalid')
        git('add', '.')
        git('commit', '-qm', 'base')
        git('update-ref', 'refs/remotes/origin/main', 'HEAD')
        (root / ('notes.md' if docs else 'proof.rs')).write_text('fixture\n')
        git('add', '.')
        git('commit', '-qm', 'candidate')
        runtime = root / 'runtime'
        runtime.mkdir()
        binary = root / 'bin'
        binary.mkdir()
        cargo = binary / 'cargo'
        cargo.write_text(CARGO)
        cargo.chmod(0o755)
        # Stub only transport/budget for orchestration tests; the real native
        # metadata validator is exercised separately in test-native-deadline.py.
        python = binary / 'python3'
        python.write_text('#!' + sys.executable + '\nimport os, sys\n'
                          "if len(sys.argv) > 1 and sys.argv[1] == '.ci/native_deadline.py':\n"
                          "    phase = sys.argv[2]\n"
                          "    with open(os.environ['DEADLINE_CALLS'], 'a') as f: f.write(phase + '\\n')\n"
                          "    if phase == os.environ.get('DEADLINE_FAILURE'): sys.exit(78)\n"
                          "    print(1000 if phase == 'proof' else 100)\n"
                          "    sys.exit(0)\n"
                          "os.execv(" + repr(sys.executable) + ", [" + repr(sys.executable) + ", *sys.argv[1:]])\n")
        python.chmod(0o755)
        if fail_receipt:
            tool = binary / 'mv'
            tool.write_text("#!/usr/bin/env python3\nimport os, sys\n"
                            "if sys.argv[-1].endswith('/core_exit-123-2'): sys.exit(1)\n"
                            "os.execv('/usr/bin/mv', ['mv', *sys.argv[1:]])\n")
            tool.chmod(0o755)
        if slow_core:
            # Exercise GNU timeout against the real shipped core process group,
            # changing only the test's wall duration, never the candidate file.
            tool = binary / 'timeout'
            tool.write_text("#!/usr/bin/env python3\nimport os, sys\n"
                            "args = ['0.2s' if a == '1000s' else a for a in sys.argv[1:]]\n"
                            "os.execv('/usr/bin/timeout', ['timeout', *args])\n")
            tool.chmod(0o755)
        if fail_staging:
            tool = binary / 'mktemp'
            tool.write_text("#!/usr/bin/env python3\nimport os, sys\n"
                            "with open(os.environ['STAGING_CALLS'], 'a') as f: f.write('called\\n')\n"
                            "if any('staging-' in arg for arg in sys.argv): sys.exit(1)\n"
                            "os.execv('/usr/bin/mktemp', ['mktemp', *sys.argv[1:]])\n")
            tool.chmod(0o755)
        calls = root / 'calls'
        output = root / 'output'
        summary = root / 'summary'
        env = dict(os.environ, PATH=str(binary) + os.pathsep + os.environ['PATH'],
                   RUNNER_TEMP=str(runtime), CARGO_TARGET_DIR=str(runtime / 'cargo-target'),
                   GITHUB_SHA='0' * 40 if bad_sha else git('rev-parse', 'HEAD'),
                   GITHUB_EVENT_NAME='merge_group', GITHUB_RUN_ID='123',
                   GITHUB_RUN_ATTEMPT='2', EM_CI_EXECUTION_SELF_HOSTED='true',
                   GITHUB_OUTPUT=str(output), GITHUB_STEP_SUMMARY=str(summary),
                   CALLS=str(calls), FAIL_STEP=fail, STAGING_CALLS=str(root / 'staging-calls'),
                   DEADLINE_CALLS=str(root / 'deadline-calls'), DEADLINE_FAILURE=deadline_failure)
        if bad_target:
            env['CARGO_TARGET_DIR'] = str(root / 'target')
        if reject_diagnostics:
            env['REJECT_DIAGNOSTICS'] = '1'
        if slow_core:
            env['SLOW_CORE'] = '1'
        if signal_core:
            env['SIGNAL_CORE'] = '1'
        # Seed stale success and raw output; the current run must replace both.
        receipts = root / 'target/ci-core'
        receipts.mkdir(parents=True)
        (receipts / 'core_exit-123-2').write_text('0\n')
        result = subprocess.run(['bash', '.ci/run.sh'], cwd=root, env=env,
                                text=True, capture_output=True, timeout=15)
        commands = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
        return root, result, commands

    def test_full_chain_and_central_target(self):
        root, result, commands = self.run_case()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands, [
            ['fmt', '--all', '--check'],
            ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'],
            ['run', '--locked', '-p', 'xtask', '--', 'ci-test'],
            ['doc', '--workspace', '--no-deps', '--locked'],
            ['run', '--locked', '-p', 'xtask', '--', 'check-pr'],
        ])
        self.assertFalse((root / 'output').exists())
        self.assertEqual((root / 'target/ci-core/core_exit-123-2').read_text(), '0\n')

    def test_every_product_failure_is_authoritative_and_short_circuits(self):
        for step, count in [('fmt', 1), ('clippy', 2), ('ci-test', 3), ('doc', 4), ('check-pr', 5)]:
            with self.subTest(step=step):
                root, result, commands = self.run_case(fail=step)
                self.assertEqual(result.returncode, 17, result.stderr)
                product = [cmd for cmd in commands if cmd[-2:-1] != ['ci-test-validate']]
                self.assertEqual(len(product), count)
                self.assertEqual((root / 'target/ci-core/core_exit-123-2').read_text(), '17\n')
                paths = dict(line.split('=', 1) for line in (root / 'output').read_text().splitlines())
                self.assertIn('summary_path', paths)
                self.assertIn('metadata_path', paths)
                metadata = json.loads(Path(paths['metadata_path']).read_text())
                self.assertEqual(metadata['exit_code'], 17)
                for path in paths.values():
                    self.assertTrue(Path(path).is_relative_to(root / 'runtime'))
                    self.assertFalse(Path(path).is_symlink())
                    self.assertNotIn('RAW_SECRET_FIXTURE', Path(path).read_text())

    def test_markdown_only_skip_keeps_rustdoc_and_policy(self):
        _, result, commands = self.run_case(docs=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(commands), 4)
        self.assertEqual(commands[2][0], 'doc')
        self.assertEqual(commands[-1][-1], 'check-pr')

    def test_rejected_diagnostics_do_not_hide_product_failure(self):
        root, result, _ = self.run_case(fail='ci-test', reject_diagnostics=True)
        self.assertEqual(result.returncode, 17)
        self.assertNotIn('diagnostics_path=', (root / 'output').read_text())

    def test_staging_setup_failure_cannot_expose_artifacts_or_mask_failure(self):
        root, result, _ = self.run_case(fail='ci-test', fail_staging=True)
        self.assertEqual(result.returncode, 17)
        self.assertFalse((root / 'output').exists())
        # One selector allocation and the failing staging allocation, with no
        # subsequent mktemp attempt after the setup failure.
        self.assertEqual((root / 'staging-calls').read_text().count('called'), 2)
        self.assertEqual((root / 'target/ci-core/core_exit-123-2').read_text(), '17\n')

    def test_runtime_and_source_mismatch_reject_before_cargo(self):
        for settings in [{'bad_target': True}, {'bad_sha': True}]:
            _, result, commands = self.run_case(**settings)
            self.assertEqual(result.returncode, 78)
            self.assertEqual(commands, [])

    def test_receipt_publication_failure_cannot_report_success(self):
        for failure, expected in [('', 78), ('clippy', 17)]:
            with self.subTest(failure=failure):
                root, result, _ = self.run_case(fail=failure, fail_receipt=True)
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertFalse((root / 'target/ci-core/core_exit-123-2').exists())
                self.assertFalse((root / 'output').exists())

    def test_deadline_refusal_stops_entry_or_success_and_preserves_failure(self):
        for phase in ('proof', 'finalize', 'result'):
            with self.subTest(phase=phase):
                _, result, commands = self.run_case(deadline_failure=phase)
                self.assertEqual(result.returncode, 78, result.stderr)
                if phase == 'proof':
                    self.assertEqual(commands, [])
                _, result, _ = self.run_case(fail='clippy', deadline_failure=phase)
                self.assertEqual(result.returncode, 78 if phase == 'proof' else 17)


    def test_real_timeout_cannot_reuse_stale_success_or_continue_producers(self):
        root, result, commands = self.run_case(slow_core=True)
        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertEqual(commands, [['fmt', '--all', '--check']])
        self.assertEqual((root / 'target/ci-core/core_exit-123-2').read_text(), '124\n')

    def test_terminated_producer_cannot_report_success_or_continue(self):
        root, result, commands = self.run_case(signal_core=True)
        self.assertEqual(result.returncode, 143, result.stderr)
        self.assertEqual(commands, [['fmt', '--all', '--check']])
        self.assertEqual((root / 'target/ci-core/core_exit-123-2').read_text(), '143\n')


if __name__ == '__main__':
    unittest.main()
