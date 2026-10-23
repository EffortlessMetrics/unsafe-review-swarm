"""Verify the exact public central program against real consumer staging.

Usage: python3 .ci/test-central-contract.py /path/to/pinned/rust.yml
No API calls, credentials, native admission or product build. The central file
must be fetched from the documented immutable ref; Git blob identity is checked.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import types
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_BLOB = '6107a7dbcb94ce6a5b44b2dbf063c3500eae9129'
RAW = Path(sys.argv.pop(1)).read_bytes()
assert hashlib.sha1(b'blob ' + str(len(RAW)).encode() + b'\0' + RAW).hexdigest() == EXPECTED_BLOB
TEXT = RAW.decode()


def program(marker):
    tail = TEXT.split(marker, 1)[1]
    body = tail.split("          python3 -I - <<'PYTHON'\n", 1)[1].split('          PYTHON', 1)[0]
    module = types.ModuleType('pinned_central')
    exec(compile('\n'.join(row[10:] for row in body.splitlines()), EXPECTED_BLOB, 'exec'), module.__dict__)
    return module


PACKAGE = program('id: failure_diagnostics\n')
ADMISSION = program('id: admission\n')
SPEC = importlib.util.spec_from_file_location('entrypoint_tests', ROOT/'.ci/test-run.py')
ENTRY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ENTRY)


class CentralContract(unittest.TestCase):
    def test_actual_publisher_accepts_producer_and_rejects_tampering(self):
        fixture = ENTRY.ProofContract()
        try:
            root, status, _ = fixture.run_case(fail='fmt')
            self.assertEqual(status.returncode, 17)
            paths = dict(row.split('=', 1) for row in (root/'output').read_text().splitlines())
            sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
            output = root/'publisher-output'
            env = dict(PROOF_OUTCOME='failure', PROOF_ENTERED_AT='2020-01-01T00:00:00Z',
                       SUMMARY_PATH=paths['summary_path'], METADATA_PATH=paths['metadata_path'],
                       DIAGNOSTICS_PATH='', GITHUB_RUN_ID='123', GITHUB_RUN_ATTEMPT='2', GITHUB_SHA=sha,
                       NATIVE_JOB=json.dumps(dict(check_run_id=7)), SELECTED_PROFILE='standard',
                       RUNNER_TEMP=str(root/'runtime'), GITHUB_OUTPUT=str(output))
            with patch.dict(os.environ, env, clear=True):
                PACKAGE.package()
                receipt = dict(row.split('=',1) for row in output.read_text().splitlines())
                self.assertEqual(receipt['upload'], 'true')
                self.assertEqual(set(p.name for p in Path(receipt['path']).iterdir()), {'summary.md','metadata.json'})
                output.unlink()
                metadata = Path(paths['metadata_path'])
                value = json.loads(metadata.read_text()); value['commit'] = '0'*40
                metadata.write_text(json.dumps(value))
                with self.assertRaises(ValueError): PACKAGE.package()
                self.assertFalse(output.exists())
        finally:
            fixture.doCleanups()

    def test_actual_admission_refuses_missing_protected_base_owner(self):
        import tempfile
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root/'workspace/.git/objects').mkdir(parents=True)
            expected = dict(repository='EffortlessMetrics/unsafe-review-swarm', base='b'*40, head='a'*40, pr=2380)
            (root/'owner-check').mkdir()
            env = dict(GITHUB_WORKSPACE=str(root/'workspace'), EM_CI_ADMISSION_TOKEN='fixture-not-a-credential')
            def run(argv, cwd, child, limit=1024**2):
                self.assertNotIn('candidate', str(argv))
                if argv[:2] == ['git', 'init']:
                    (Path(argv[-1])/'.git/objects/info').mkdir(parents=True)
                return b''  # Protected base has no owner blob; never substitute candidate.
            with patch.object(ADMISSION.shutil, 'which', return_value='/fixture/tool'):
                with self.assertRaises(ValueError):
                    ADMISSION.owners(expected, env, root/'owner-check', run=run)

    def test_failure_lane_is_seven_day_nonfatal_and_closed(self):
        lane = TEXT.split('      - name: Upload bounded failure diagnostics\n',1)[1].split('\n      # END ABSOLUTE DEADLINE',1)[0]
        for marker in ('continue-on-error: true', 'timeout-minutes: 2', 'retention-days: 7'):
            self.assertIn(marker, lane)
        consumer = (ROOT/'.github/workflows/ci.yml').read_text()
        for marker in ('proof_admission: native-review-v1', 'whole_run_deadline: native-run-1200s',
                       'artifact_lane: failure-diagnostics', '@8ff77e5876b4a7808a58e053aa6248b1020e02d0'):
            self.assertIn(marker, consumer)


if __name__ == '__main__':
    unittest.main()
