#!/usr/bin/env python3
"""Discriminating tests of the actual metadata-only workflow source.

No live admission request, label, review approval or product run is produced.
"""
import copy
import datetime as dt
import json
import tomllib
from pathlib import Path
import types
import unittest

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (ROOT / '.github/workflows/ci.yml').read_text()
SCRIPTS = [part.split('          PYTHON', 1)[0]
           for part in WORKFLOW.split("          python3 - <<'PYTHON'\n")[1:]]
SOURCE = '\n'.join(line[10:] for line in SCRIPTS[0].splitlines())
RESULT = '\n'.join(line[10:] for line in SCRIPTS[1].splitlines())
ADMISSION = types.ModuleType('admission')
exec(compile(SOURCE, 'ci.yml/admission', 'exec'), ADMISSION.__dict__)


def marker(kind, value):
    return '<!-- unsafe-review-' + kind + ': ' + json.dumps(value, separators=(',', ':')) + ' -->'


class AdmissionTests(unittest.TestCase):
    def setUp(self):
        self.now = dt.datetime(2026, 10, 14, 12, 0, 30, tzinfo=dt.timezone.utc)
        self.request = dict(repository=ADMISSION.REPO, pr=2380, head='a' * 40,
                            base='b' * 40, integration='c' * 40, tree='d' * 40,
                            coordinator=ADMISSION.COORDINATOR, request_id='e' * 32,
                            review_id=42, issued_at='2026-10-14T12:00:00Z')
        self.receipt = {k: self.request[k] for k in ('repository', 'pr', 'head', 'base', 'integration', 'tree')}
        self.receipt.update(verdict='approved-for-proof', review_kind='independent')
        self.pr = dict(number=2380, state='open', draft=False,
                       user=dict(type='User'), mergeable=True, merge_commit_sha='c' * 40,
                       head=dict(sha='a' * 40, repo=dict(full_name=ADMISSION.REPO, fork=False)),
                       base=dict(sha='b' * 40, ref='main', repo=dict(full_name=ADMISSION.REPO)),
                       updated_at='2026-10-14T12:00:01Z', body=marker('admission-v1', self.request))
        self.event = dict(action='edited', pull_request=copy.deepcopy(self.pr),
                          sender=dict(login=ADMISSION.COORDINATOR, type='User'),
                          changes=dict(body=dict(from_='original body')))
        self.event['changes']['body'] = {'from': 'original body'}
        self.env = dict(GITHUB_EVENT_NAME='pull_request', GITHUB_REPOSITORY=ADMISSION.REPO,
                        GITHUB_ACTOR=ADMISSION.COORDINATOR, GITHUB_TRIGGERING_ACTOR=ADMISSION.COORDINATOR,
                        GITHUB_RUN_ATTEMPT='1', GITHUB_REF='refs/pull/2380/merge', GITHUB_SHA='c' * 40)
        self.review = dict(id=42, commit_id='a' * 40, state='COMMENTED',
                           user=dict(login=ADMISSION.COORDINATOR, type='User'),
                           submitted_at='2026-10-14T11:59:59Z', body=marker('independent-review-v1', self.receipt))
        self.merge = dict(sha='c' * 40, tree=dict(sha='d' * 40), parents=[dict(sha='b' * 40), dict(sha='a' * 40)])
        self.reviews = [self.review]
        self.reads = 0
        self.owner_fault = None

    def api(self, path):
        if '/contents/' in path:
            owner = path.split('/contents/', 1)[1].split('?ref=', 1)[0]
            self.assertTrue(path.endswith('?ref=' + self.request['base']))
            result = dict(type='file', path=owner, size=100, sha='f' * 40)
            if self.owner_fault:
                result.update(self.owner_fault)
            return result
        if '/reviews?' in path:
            return copy.deepcopy(self.reviews)
        if '/git/commits/' in path:
            return self.merge
        self.reads += 1
        return copy.deepcopy(self.pr)

    def run_admission(self, api=None):
        return ADMISSION.admit(self.event, self.env, api or self.api, self.now)

    def rewrite_request(self):
        self.pr['body'] = marker('admission-v1', self.request)
        self.event['pull_request']['body'] = self.pr['body']

    def test_read_scope_expansion_is_only_on_governed_proof(self):
        before, after = WORKFLOW.split('\n  proof:\n', 1)
        proof, result = after.split('\n  unsafe-review-rust-result:\n', 1)
        expected = ('    permissions:\n      contents: read\n      actions: read\n'
                    '      checks: read\n      pull-requests: read\n')
        self.assertIn(expected, proof)
        self.assertEqual(proof.count('    permissions:'), 1)
        self.assertNotIn('    permissions:', before)
        self.assertNotIn('    permissions:', result)
        self.assertIn('permissions:\n  contents: read\n', before)
        self.assertNotIn(': write', WORKFLOW)
        with (ROOT / 'policy/workflow-allowlist.toml').open('rb') as stream:
            policies = tomllib.load(stream)['workflow']
        entry = next(p for p in policies if p['path'] == '.github/workflows/ci.yml')
        self.assertEqual(set(entry['permissions'].split(', ')),
                         {'contents: read', 'actions: read', 'checks: read', 'pull-requests: read'})

    def test_positive_exact_coordinator_review_and_integration(self):
        self.assertEqual(self.run_admission(), self.request)
        self.assertEqual(self.reads, 2)

    def test_unrelated_events_rejected_before_api(self):
        for action in ('opened', 'synchronize', 'ready_for_review', 'labeled', 'reopened', 'closed'):
            with self.subTest(action=action):
                self.event['action'] = action
                with self.assertRaises(ValueError):
                    self.run_admission(lambda _: self.fail('must reject before API'))
        self.event['action'] = 'edited'
        for name in ('merge_group', 'workflow_dispatch', 'push'):
            self.env['GITHUB_EVENT_NAME'] = name
            with self.assertRaises(ValueError):
                self.run_admission(lambda _: self.fail('must reject before API'))

    def test_draft_fork_bot_or_wrong_actor_rejected_before_api(self):
        original = copy.deepcopy(self.event)
        for mutate in (
            lambda e: e['pull_request'].update(draft=True),
            lambda e: e['pull_request']['head']['repo'].update(full_name='other/fork'),
            lambda e: e['pull_request']['head']['repo'].update(fork=True),
            lambda e: e['pull_request']['user'].update(type='Bot'),
            lambda e: e['sender'].update(login='other'),
            lambda e: e['sender'].update(type='Bot'),
        ):
            self.event = copy.deepcopy(original)
            mutate(self.event)
            with self.assertRaises(ValueError):
                self.run_admission(lambda _: self.fail('must reject before API'))

    def test_protected_base_owner_readiness_is_required(self):
        for fault in (dict(type='dir'), dict(path='candidate/owner'), dict(size=0),
                      dict(size=524289), dict(sha='missing')):
            self.owner_fault = fault
            with self.subTest(fault=fault), self.assertRaises(ValueError):
                self.run_admission()

    def test_current_native_driver_emits_only_after_real_checks(self):
        import os, tempfile, urllib.request
        from unittest.mock import patch
        now = self.now
        class Clock(dt.datetime):
            @classmethod
            def now(cls, tz=None):
                return now
        class Response:
            def __init__(self, value):
                self.raw = json.dumps(value).encode()
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def read(self, limit): return self.raw[:limit]
        def transport(request, timeout):
            self.assertTrue(request.full_url.startswith('https://api.github.com/repos/' + ADMISSION.REPO))
            return Response(self.api(request.full_url.removeprefix('https://api.github.com')))
        with tempfile.TemporaryDirectory(prefix='unsafe-admission-driver-') as temporary:
            event_path, output_path = Path(temporary)/'event.json', Path(temporary)/'output'
            event_path.write_text(json.dumps(self.event))
            env = dict(os.environ, **self.env, GITHUB_EVENT_PATH=str(event_path), GITHUB_OUTPUT=str(output_path))
            for fault in (dict(size=0), None):
                self.owner_fault = fault
                with patch.dict(os.environ, env, clear=True), patch.object(dt, 'datetime', Clock), \
                     patch.object(urllib.request, 'urlopen', transport):
                    if fault:
                        with self.assertRaises(SystemExit):
                            exec(compile(SOURCE, 'ci.yml/actual-main', 'exec'), {'__name__':'__main__'})
                        self.assertFalse(output_path.exists())
                    else:
                        exec(compile(SOURCE, 'ci.yml/actual-main', 'exec'), {'__name__':'__main__'})
                        lines = output_path.read_text().splitlines()
                        self.assertEqual(lines[0], 'admitted=true')
                        self.assertEqual(json.loads(lines[1].removeprefix('request=')), self.request)

    def test_pinned_closed_selectors_cannot_be_omitted(self):
        proof = WORKFLOW.split('\n  proof:\n')[1].split('\n  unsafe-review-rust-result:')[0]
        for marker in ('@8ff77e5876b4a7808a58e053aa6248b1020e02d0',
                       'proof_admission: native-review-v1', 'whole_run_deadline: native-run-1200s',
                       'upload_artifacts: true', 'artifact_lane: failure-diagnostics'):
            self.assertEqual(proof.count(marker), 1)

    def test_explicit_restoration_is_not_a_global_nonce_grant(self):
        # No global nonce registry: deliberate restoration remains subject to
        # authenticated coordinator intent, fresh time and current exact review.
        self.event['changes']['body']['from'] = 'marker was removed'
        self.assertEqual(self.run_admission(), self.request)
        self.env['GITHUB_ACTOR'] = 'other'
        with self.assertRaises(ValueError):
            self.run_admission()
        self.env['GITHUB_ACTOR'] = ADMISSION.COORDINATOR
        self.review['state'] = 'DISMISSED'
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_review_dismissal_during_admission_is_refreshed(self):
        def racing(path):
            value = self.api(path)
            if self.reads == 2 and '/reviews?' not in path and '/git/commits/' not in path:
                self.review['state'] = 'DISMISSED'
            return value
        with self.assertRaises(ValueError):
            self.run_admission(racing)

    def test_shared_review_helpers_do_not_drift(self):
        shared = SOURCE[:SOURCE.index('def admit(')] + SOURCE[SOURCE.index('def public_api('):SOURCE.index("if __name__ == '__main__':")]
        self.assertTrue(RESULT.startswith(shared.rstrip()))

    def test_missing_or_persistent_request_is_not_authority(self):
        for old in (self.pr['body'], self.pr['body'] + '\nunrelated edit'):
            self.event['changes']['body']['from'] = old
            with self.assertRaises(ValueError):
                self.run_admission()
        self.event['changes'] = {}
        with self.assertRaises(ValueError):
            self.run_admission()
        self.event['changes'] = {'body': {'from': 'old'}}
        self.event['pull_request']['body'] = 'admit=true'
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_missing_stale_or_dismissed_review_rejected(self):
        original = copy.deepcopy(self.review)
        for mutate in (
            lambda r: r.update(commit_id='f' * 40),
            lambda r: r.update(id=41),
            lambda r: r.update(state='DISMISSED'),
            lambda r: r['user'].update(login='other'),
            lambda r: r.update(body='unbound approval'),
            lambda r: r.update(submitted_at='2026-10-14T12:00:01Z'),
        ):
            self.review = copy.deepcopy(original)
            mutate(self.review)
            self.reviews = [self.review]
            with self.assertRaises(ValueError):
                self.run_admission()
        self.reviews = []
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_adverse_latest_receipt_invalidates_earlier_approval(self):
        newer = copy.deepcopy(self.review)
        receipt = dict(self.receipt, verdict='blocked')
        newer.update(id=43, body=marker('independent-review-v1', receipt), submitted_at='2026-10-14T12:00:00Z')
        self.reviews.append(newer)
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_later_native_request_changes_revokes_receipt(self):
        newer = copy.deepcopy(self.review)
        newer.update(id=43, body='Changes required', state='CHANGES_REQUESTED',
                     submitted_at='2026-10-14T12:00:00Z')
        self.reviews.append(newer)
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_head_base_merge_tree_and_parent_drift_rejected(self):
        original = copy.deepcopy(self.pr)
        for key in ('head', 'base'):
            self.pr = copy.deepcopy(original)
            self.pr[key]['sha'] = 'f' * 40
            with self.assertRaises(ValueError):
                self.run_admission()
        self.pr = copy.deepcopy(original)
        self.pr['merge_commit_sha'] = 'f' * 40
        with self.assertRaises(ValueError):
            self.run_admission()
        self.pr = copy.deepcopy(original)
        self.merge['tree']['sha'] = 'f' * 40
        with self.assertRaises(ValueError):
            self.run_admission()
        self.merge['tree']['sha'] = 'd' * 40
        self.merge['parents'].reverse()
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_expired_request_attempt_replay_and_racing_change_rejected(self):
        self.env['GITHUB_RUN_ATTEMPT'] = '2'
        with self.assertRaises(ValueError):
            self.run_admission()
        self.env['GITHUB_RUN_ATTEMPT'] = '1'
        self.now += dt.timedelta(minutes=11)
        with self.assertRaises(ValueError):
            self.run_admission()
        self.now -= dt.timedelta(minutes=11)
        def racing(path):
            value = self.api(path)
            if self.reads == 2 and '/reviews?' not in path and '/git/commits/' not in path:
                value['draft'] = True
            return value
        with self.assertRaises(ValueError):
            self.run_admission(racing)

    def test_unavailable_api_and_ambiguous_records_rejected(self):
        def unavailable(_):
            raise OSError('metadata unavailable')
        with self.assertRaises(OSError):
            self.run_admission(unavailable)
        self.event['pull_request']['body'] += '\n' + self.pr['body']
        with self.assertRaises(ValueError):
            self.run_admission()

    def test_review_must_bind_base_merge_and_tree_not_just_head(self):
        for key in ('base', 'integration', 'tree'):
            receipt = dict(self.receipt)
            receipt[key] = 'f' * 40
            self.review['body'] = marker('independent-review-v1', receipt)
            with self.assertRaises(ValueError):
                self.run_admission()

    def test_unadmitted_or_wrong_native_result_cannot_turn_green(self):
        import os
        from unittest.mock import patch
        evidence = dict(schema='em-ci-rust-result.v1', repository=ADMISSION.REPO, sha='c' * 40,
                        run_id='7', run_attempt='1', profile='standard', result='success',
                        proof_outcome='success', proof_entered_at='2026-10-14T12:00:30Z')
        env = dict(self.env, ADMISSION_RESULT='success', ADMITTED='true', PROOF_RESULT='success',
                   EVIDENCE=json.dumps(evidence), RECEIPT_NAME='native-central-receipt', GITHUB_RUN_ID='7',
                   ADMISSION_REQUEST=json.dumps(self.request))
        with patch.dict(os.environ, env, clear=True):
            exec(compile(RESULT[RESULT.index("assert os.environ['ADMISSION_RESULT']"):], 'ci.yml/result', 'exec'), dict(ADMISSION.__dict__, public_api=self.api))
        with patch.dict(os.environ, env, clear=True):
            self.review['state'] = 'DISMISSED'
            with self.assertRaises(ValueError):
                exec(compile(RESULT[RESULT.index("assert os.environ['ADMISSION_RESULT']"):], 'ci.yml/result', 'exec'), dict(ADMISSION.__dict__, public_api=self.api))
            self.review['state'] = 'COMMENTED'
            saved = self.pr['body']
            self.pr['body'] = 'request revoked'
            with self.assertRaises(ValueError):
                exec(compile(RESULT[RESULT.index("assert os.environ['ADMISSION_RESULT']"):], 'ci.yml/result', 'exec'), dict(ADMISSION.__dict__, public_api=self.api))
            self.pr['body'] = saved
        for changed in (dict(ADMISSION_RESULT='skipped'), dict(ADMITTED=''), dict(PROOF_RESULT='skipped'),
                        dict(RECEIPT_NAME=''), dict(GITHUB_SHA='f' * 40), dict(GITHUB_RUN_ID='8'),
                        dict(GITHUB_RUN_ATTEMPT='2')):
            with patch.dict(os.environ, dict(env, **changed), clear=True):
                with self.assertRaises(AssertionError):
                    exec(compile(RESULT[RESULT.index("assert os.environ['ADMISSION_RESULT']"):], 'ci.yml/result', 'exec'), dict(ADMISSION.__dict__, public_api=self.api))


if __name__ == '__main__':
    unittest.main()
