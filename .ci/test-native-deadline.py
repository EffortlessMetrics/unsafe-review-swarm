"""Deterministic native provenance controls; no API calls or CI admission."""
import copy
import datetime as dt
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('native_deadline', Path(__file__).with_name('native_deadline.py'))
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NativeDeadlineTests(unittest.TestCase):
    def setUp(self):
        self.origin = dt.datetime(2026, 10, 15, tzinfo=dt.timezone.utc)
        self.env = dict(GITHUB_REPOSITORY=MODULE.REPO, GITHUB_EVENT_NAME='pull_request',
                        GITHUB_RUN_ID='123', GITHUB_RUN_ATTEMPT='1',
                        GITHUB_REF='refs/pull/2380/merge', GITHUB_SHA='c' * 40)
        self.pr = dict(number=2380, draft=False, merge_commit_sha='c' * 40,
                       head=dict(sha='a' * 40, repo=dict(full_name=MODULE.REPO)),
                       base=dict(sha='b' * 40, repo=dict(full_name=MODULE.REPO)))
        self.event = dict(action='edited', pull_request=self.pr)
        self.run = dict(id=123, run_attempt=1, repository=dict(full_name=MODULE.REPO),
                        event='pull_request', path='.github/workflows/ci.yml', status='in_progress',
                        head_sha='a' * 40, pull_requests=[copy.deepcopy(self.pr)],
                        created_at='2026-10-15T00:00:00Z', run_started_at='2026-10-15T00:10:00Z')

    def budget(self, elapsed=300, phase='proof'):
        return MODULE.native_budget(self.event, self.env, self.run,
                                    self.origin + dt.timedelta(seconds=elapsed), phase)

    def test_origin_includes_setup_and_does_not_use_run_started_at(self):
        self.assertEqual(self.budget().proof_seconds, 720)
        self.assertEqual(self.budget().deadline, self.origin + dt.timedelta(seconds=1200))
        self.run['run_started_at'] = '2026-10-15T00:19:00Z'
        self.assertEqual(self.budget().proof_seconds, 720)

    def test_phases_keep_deadline_and_reserves(self):
        self.assertEqual(self.budget(900, 'finalize').proof_seconds, 244)
        self.assertEqual(self.budget(900, 'result').proof_seconds, 299)
        self.assertEqual(self.budget(900).deadline, self.budget(1000, 'result').deadline)
        with self.assertRaises(ValueError):
            self.budget(1200, 'result')

    def test_finalizer_termination_grace_stays_inside_reserved_time(self):
        # Proof timeout may take 30 extra seconds to terminate its process group.
        proof_end = self.budget(0).proof_seconds + 30
        # Charge six seconds for each post-proof metadata query and its grace.
        finalize = min(self.budget(proof_end + 6, 'finalize').proof_seconds, 110)
        self.assertGreaterEqual(1200 - (proof_end + 6 + finalize + 10 + 6), 40)


    def test_environment_identity_event_and_retries_refused(self):
        original = dict(self.env)
        for field, value in [('GITHUB_REPOSITORY', 'other/repo'), ('GITHUB_EVENT_NAME', 'workflow_dispatch'),
                             ('GITHUB_RUN_ID', '124'), ('GITHUB_RUN_ID', '../x'),
                             ('GITHUB_RUN_ATTEMPT', '2'), ('GITHUB_SHA', 'd' * 40),
                             ('GITHUB_REF', 'refs/heads/main')]:
            self.env = dict(original, **{field: value})
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.budget()

    def test_native_run_provenance_and_subject_mismatch_refused(self):
        original = copy.deepcopy(self.run)
        for mutate in (
            lambda r: r.update(id=124), lambda r: r.update(run_attempt=2),
            lambda r: r.update(event='push'), lambda r: r.update(status='completed'),
            lambda r: r.update(path='.github/workflows/other.yml'),
            lambda r: r['repository'].update(full_name='other/repo'),
            lambda r: r.update(head_sha='d' * 40), lambda r: r.update(pull_requests=[]),
            lambda r: r['pull_requests'][0].update(number=42),
            lambda r: r['pull_requests'][0]['head'].update(sha='d' * 40),
            lambda r: r['pull_requests'][0]['base'].update(sha='d' * 40),
            lambda r: r.update(created_at='bad'),
            lambda r: r.update(created_at='2026-10-15T00:19:00Z'),
        ):
            self.run = copy.deepcopy(original)
            mutate(self.run)
            with self.subTest(run=self.run), self.assertRaises(ValueError):
                self.budget()

    def test_event_draft_unrelated_edit_and_repository_refused(self):
        self.event['action'] = 'synchronize'
        with self.assertRaises(ValueError):
            self.budget()
        self.event['action'] = 'edited'
        self.pr['draft'] = True
        with self.assertRaises(ValueError):
            self.budget()
        self.pr['draft'] = False
        self.pr['head']['repo']['full_name'] = 'fork/repo'
        with self.assertRaises(ValueError):
            self.budget()


if __name__ == '__main__':
    unittest.main()
