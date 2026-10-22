#!/usr/bin/env python3
"""Discriminating tests of the actual metadata-only workflow source.

No live admission request, label, review approval or product run is produced.
"""
import copy
import datetime as dt
import json
from pathlib import Path
import types
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = (ROOT / '.github/workflows/ci.yml').read_text()
SCRIPTS = [part.split('          PYTHON', 1)[0]
           for part in WORKFLOW.split("          python3 -I - <<'PYTHON'\n")[1:]]
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

    def api(self, path):
        self.assertNotIn('/contents/', path, 'legacy bootstrap must not claim protected-base owners')
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




    def test_native_guard_is_reused_before_setup_before_proof_and_after_proof(self):
        job = WORKFLOW.split('  unsafe-review-rust-result:\n', 1)[1]
        self.assertLess(job.index('run: &native_subject_check'), job.index('uses: actions/checkout'))
        self.assertEqual(job.count('run: *native_subject_check'), 2)
        self.assertLess(job.index('Refresh native authority immediately'), job.index('id: launch'))
        self.assertIn("if: ${{ always() && needs.admission.outputs.admitted == 'true' && steps.launch.outcome == 'success' }}", job)
        self.assertIn('NATIVE_PHASE: final', job)
        self.assertNotIn(': write', WORKFLOW)
        self.assertNotIn('workflow_dispatch:', WORKFLOW)
        self.assertNotIn('merge_group:', WORKFLOW)
        router = WORKFLOW.split('  route:\n',1)[1].split('  unsafe-review-rust-result:',1)[0]
        self.assertLess(router.index('no explicit native admission'), router.index('gh api'))
        self.assertIn('exit 78', job)

    def test_actual_router_never_reads_inventory_without_admission(self):
        import os, subprocess, tempfile
        router = WORKFLOW.split('  route:\n',1)[1].split('  unsafe-review-rust-result:',1)[0]
        script = router.split('        run: |\n',1)[1]
        script = '\n'.join(line[10:] for line in script.splitlines())
        script = script.replace('${{ github.event_name }}', 'pull_request').replace('${{ github.event.pull_request.head.repo.fork }}', 'false')
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            gh = directory/'gh'
            gh.write_text('#!/bin/sh\necho called > "$TRIPWIRE"\nexit 1\n')
            gh.chmod(0o755)
            for state in ('skipped', 'failure', 'success'):
                output = directory/'output'
                output.unlink(missing_ok=True)
                env = dict(os.environ, PATH=temporary+':'+os.environ['PATH'], GH_TOKEN='fixture', ADMISSION_RESULT=state, ADMITTED='', GITHUB_OUTPUT=str(output), TRIPWIRE=str(directory/'tripwire'))
                result = subprocess.run(['bash','-c',script], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertFalse((directory/'tripwire').exists())
                self.assertIn('runner="ubuntu-latest"', output.read_text())

    def test_actual_refresh_revokes_draft_changed_subject_and_dismissed_review(self):
        import os
        from unittest.mock import patch
        code = RESULT[RESULT.index('def expired('):]
        # alarm wiring is tested through the actual program; fake API only.
        env = dict(self.env, ADMISSION_REQUEST=json.dumps(self.request))
        with patch.dict(os.environ, env, clear=True):
            exec(compile(code, 'ci.yml/refresh', 'exec'), dict(ADMISSION.__dict__, public_api=self.api))
            for mutate, restore in (
                (lambda: self.pr.update(draft=True), lambda: self.pr.update(draft=False)),
                (lambda: self.pr.update(body='revoked'), lambda: self.rewrite_request()),
                (lambda: self.review.update(state='DISMISSED'), lambda: self.review.update(state='COMMENTED')),
                (lambda: self.pr['base'].update(sha='f'*40), lambda: self.pr['base'].update(sha='b'*40)),
            ):
                mutate()
                try:
                    with self.assertRaises(ValueError):
                        exec(compile(code, 'ci.yml/refresh', 'exec'), dict(ADMISSION.__dict__, public_api=self.api))
                finally:
                    import signal
                    signal.alarm(0)
                    restore()

    def test_partial_rerun_cannot_reuse_prior_admission_outputs(self):
        import os, signal
        from unittest.mock import patch
        code = RESULT[RESULT.index('def expired('):]
        env = dict(self.env, ADMISSION_REQUEST=json.dumps(self.request))
        for changed in (dict(GITHUB_RUN_ATTEMPT='2'), dict(GITHUB_ACTOR='other'),
                        dict(GITHUB_TRIGGERING_ACTOR='other'), dict(GITHUB_REPOSITORY='other/repo')):
            with self.subTest(changed=changed), patch.dict(os.environ, dict(env, **changed), clear=True):
                try:
                    with self.assertRaises(ValueError):
                        exec(compile(code, 'ci.yml/refresh', 'exec'), dict(ADMISSION.__dict__, public_api=lambda _: self.fail('must reject before API')))
                finally:
                    signal.alarm(0)

    def test_metadata_token_is_required_without_permission_expansion(self):
        import os
        from unittest.mock import patch
        with patch.dict(os.environ, {}, clear=True), patch('urllib.request.urlopen') as transport:
            with self.assertRaisesRegex(ValueError, 'missing metadata token'):
                ADMISSION.public_api('/repos/' + ADMISSION.REPO + '/pulls/2396/reviews')
            transport.assert_not_called()
        self.assertEqual(WORKFLOW.count('GITHUB_TOKEN: ${{ github.token }}'), 4)
        self.assertNotIn('pull-requests:', WORKFLOW)
        self.assertNotIn(': write', WORKFLOW)
        admission = WORKFLOW.split('  admission:\n', 1)[1].split('  route:\n', 1)[0]
        gate = WORKFLOW.split('  unsafe-review-rust-result:\n', 1)[1]
        self.assertNotIn('    permissions:', admission)
        self.assertIn('    permissions:\n      contents: read\n', gate)
        expected = {
            'Verify coordinator request and exact independent review',
            'Require explicit native bootstrap admission',
            'Refresh native authority immediately before product proof',
            'Revalidate exact native subject after product proof',
        }
        actual = set()
        for step in WORKFLOW.split('      - name: ')[1:]:
            if '          GITHUB_TOKEN: ${{ github.token }}' in step:
                actual.add(step.splitlines()[0])
        self.assertEqual(actual, expected)
        self.assertFalse(any(line.startswith('      GITHUB_TOKEN:') for line in WORKFLOW.splitlines()))

    def test_metadata_python_ignores_candidate_import_paths(self):
        import os, subprocess, tempfile
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            poison = "raise RuntimeError('candidate import executed')\n"
            (directory / 'json.py').write_text(poison)
            (directory / 'urllib').mkdir()
            (directory / 'urllib' / '__init__.py').write_text(poison)
            env = dict(os.environ, PYTHONPATH=temporary)
            probe = 'import json, urllib.request; print("isolated")'
            negative = subprocess.run(['python3', '-'], input=probe, cwd=directory,
                                      env=env, capture_output=True, text=True)
            self.assertNotEqual(negative.returncode, 0)
            isolated = subprocess.run(['python3', '-I', '-'], input=probe, cwd=directory,
                                      env=env, capture_output=True, text=True)
            self.assertEqual(isolated.returncode, 0, isolated.stderr)
            self.assertEqual(isolated.stdout.strip(), 'isolated')
        self.assertEqual(WORKFLOW.count("python3 -I - <<'PYTHON'"), 2)

    def test_actual_driver_emits_only_after_authority_checks(self):
        import os, tempfile, urllib.request
        from unittest.mock import patch
        now = self.now
        class Clock(dt.datetime):
            @classmethod
            def now(cls, tz=None): return now
        class Response:
            def __init__(self, value): self.raw = json.dumps(value).encode()
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def read(self, limit): return self.raw[:limit]
        def transport(request, timeout):
            self.assertEqual(request.get_header('Authorization'), 'Bearer fixture-token')
            self.assertTrue(request.full_url.startswith('https://api.github.com/repos/' + ADMISSION.REPO))
            return Response(self.api(request.full_url.removeprefix('https://api.github.com')))
        with tempfile.TemporaryDirectory() as temporary:
            event, output = Path(temporary)/'event', Path(temporary)/'output'
            event.write_text(json.dumps(self.event))
            env = dict(os.environ, **self.env, GITHUB_EVENT_PATH=str(event), GITHUB_OUTPUT=str(output), GITHUB_TOKEN='fixture-token')
            with patch.dict(os.environ, env, clear=True), patch.object(dt, 'datetime', Clock), patch.object(urllib.request, 'urlopen', transport):
                self.review['state'] = 'DISMISSED'
                with self.assertRaises(SystemExit):
                    exec(compile(SOURCE, 'ci.yml/main', 'exec'), {'__name__':'__main__'})
                self.assertFalse(output.exists())
                self.review['state'] = 'COMMENTED'
                exec(compile(SOURCE, 'ci.yml/main', 'exec'), {'__name__':'__main__'})
                self.assertEqual(output.read_text().splitlines()[0], 'admitted=true')


if __name__ == '__main__':
    unittest.main()

