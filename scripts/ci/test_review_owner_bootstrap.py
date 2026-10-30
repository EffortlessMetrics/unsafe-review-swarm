"""Offline bootstrap integration controls for the accepted review owners."""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SLUG = 'EffortlessMetrics/unsafe-review-swarm'
HEAD = 'a'*40


class OwnerBootstrap(unittest.TestCase):
    def test_producers_preserve_accepted_bytes_and_modes(self):
        entries = {
            'check-pr-semantic-review-currentness.py': ('9ba85bf6784c626f74f08060d76f0922201fa5eb', 0o644),
            'check-pr-review-convergence': ('fd13074d4761a3fa4ec9c6ea4888965af57c424e', 0o755),
            'check-pr-review-convergence-core': ('8ba2ef91c9100c78379332fececcedb49a3a92b3', 0o755),
            'test_check_pr_semantic_review_currentness.py': ('14c8b1dea6c1587434f2c5c40a767906ffaf1e28', 0o644),
        }
        for name, (oid, mode) in entries.items():
            path = ROOT/'scripts/ci'/name
            raw = path.read_bytes()
            with self.subTest(name=name):
                self.assertEqual(hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest(), oid)
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), mode)

    def run_facts(self, metadata=None, reviews=None, threads=None, missing=False):
        with tempfile.TemporaryDirectory(prefix='unsafe-owner-facts-') as temporary:
            root = Path(temporary)
            meta = dict(headRefOid=HEAD, reviewRequests=[], author=dict(login='writer'),
                        reviewDecision='', isDraft=False)
            meta.update(metadata or {})
            facts = dict(pr_view=meta, latestReviews=reviews or [], reviewThreads=threads or [])
            if not missing:
                for name, value in facts.items():
                    (root/(name+'.json')).write_text(json.dumps(value))
            binary = root/'bin'; binary.mkdir()
            gh = binary/'gh'
            gh.write_text('#!/bin/sh\ntouch "'+str(root/'unexpected-gh')+'"\nexit 77\n')
            gh.chmod(0o755)
            env = dict(os.environ, CONVERGENCE_TEST_FIXTURE_DIR=str(root),
                       PATH=str(binary)+os.pathsep+os.environ['PATH'])
            result = subprocess.run(['bash',str(ROOT/'scripts/ci/check-pr-review-convergence'),'2380',SLUG],
                                    env=env, capture_output=True, text=True, timeout=10)
            self.assertFalse((root/'unexpected-gh').exists(), 'offline fixture called live provider')
            payload = json.loads(result.stdout[result.stdout.index('{'):])
            return result.returncode, payload

    def test_native_fact_positive_has_central_schemas_but_no_semantic_claim(self):
        code, value = self.run_facts()
        self.assertEqual(code, 0)
        self.assertEqual(value['schema_version'], 'convergence_core.v1')
        self.assertEqual(value['wrapper_schema_version'], 'pr_review_convergence.v2')
        self.assertEqual((value['pr'], value['headRefOid']), (2380, HEAD))
        self.assertTrue(value['converged'] and value['native_review_facts_converged'])
        self.assertEqual(value['review_currentness'], 'NOT_PROVEN')
        self.assertTrue(value['semantic_currentness_required'])

    def test_draft_and_requested_user_or_team_block(self):
        for meta in (dict(isDraft=True), dict(reviewRequests=[dict(login='reviewer')]),
                     dict(reviewRequests=[dict(name='review-team')])):
            with self.subTest(meta=meta):
                code, value = self.run_facts(metadata=meta)
                self.assertEqual(code, 1)
                self.assertFalse(value['converged'])

    def test_current_change_request_blocks(self):
        review = dict(author=dict(login='reviewer', __typename='User'),
                      state='CHANGES_REQUESTED', commit=dict(oid=HEAD))
        code, value = self.run_facts(reviews=[review])
        self.assertEqual(code, 1)
        self.assertFalse(value['native_review_facts_converged'])

    def test_active_and_outdated_threads_block(self):
        for outdated in (False, True):
            thread = dict(id='thread1', isResolved=False, isOutdated=outdated, path='source.rs',
                          comments=dict(totalCount=1, nodes=[dict(author=dict(login='reviewer'), body='finding')]))
            with self.subTest(outdated=outdated):
                code, value = self.run_facts(threads=[thread])
                self.assertEqual(code, 1)
                self.assertFalse(value['converged'])

    def test_missing_facts_are_not_provider_fallback_or_success(self):
        code, value = self.run_facts(missing=True)
        self.assertEqual(code, 2)
        self.assertFalse(value['converged'])
        self.assertEqual(value['formal_review']['classification'], 'NOT_PROVEN')


if __name__ == '__main__':
    unittest.main()
