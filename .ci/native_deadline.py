"""Bind deadline arithmetic to public native metadata; grants no admission.

The immutable central owner must still bound phases outside this entrypoint and
revalidate protected-base review authority. No token or caller timestamp is used.
"""
import datetime as dt
import json
import os
from pathlib import Path
import re
import sys
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts/ci'))
from qualification_deadline import calculate_budget

REPO = 'EffortlessMetrics/unsafe-review-swarm'
# Finalization reserves its ten-second termination grace plus forty seconds
# for external phases; GNU timeout's --kill-after is additional to its duration.
RESERVES = {'proof': 180, 'finalize': 50, 'result': 1}


def require(value, message):
    if not value:
        raise ValueError(message)


def native_budget(event, env, run, now, phase):
    require(phase in RESERVES, 'unknown deadline phase')
    require(env['GITHUB_REPOSITORY'] == REPO and env['GITHUB_EVENT_NAME'] == 'pull_request',
            'deadline requires native repository PR event')
    require(re.fullmatch(r'[1-9][0-9]*', env['GITHUB_RUN_ID']), 'invalid native run id')
    require(env['GITHUB_RUN_ATTEMPT'] == '1', 'retry cannot reset whole-run origin')
    pr = event['pull_request']
    require(pr['draft'] is False and event['action'] == 'edited', 'not a proof requesting event')
    require(pr['head']['repo']['full_name'] == REPO and pr['base']['repo']['full_name'] == REPO,
            'deadline subject repository mismatch')
    require(env['GITHUB_REF'] == f"refs/pull/{pr['number']}/merge"
            and re.fullmatch(r'[0-9a-f]{40}', env['GITHUB_SHA'])
            and pr['merge_commit_sha'] == env['GITHUB_SHA'], 'native integration mismatch')
    require(type(run['id']) is int and run['id'] == int(env['GITHUB_RUN_ID'])
            and type(run['run_attempt']) is int and run['run_attempt'] == 1,
            'native run/attempt mismatch')
    require(run['repository']['full_name'] == REPO and run['event'] == 'pull_request'
            and run['path'] == '.github/workflows/ci.yml' and run['status'] == 'in_progress',
            'native run provenance mismatch')
    require(run['head_sha'] == pr['head']['sha'], 'native head mismatch')
    subjects = run['pull_requests']
    require(len(subjects) == 1 and subjects[0]['number'] == pr['number']
            and subjects[0]['head']['sha'] == pr['head']['sha']
            and subjects[0]['base']['sha'] == pr['base']['sha'], 'native PR subject mismatch')
    # created_at includes control/setup/queue time. run_started_at and now at
    # product entry must never become replacement origins on later phases.
    origin = dt.datetime.strptime(run['created_at'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=dt.timezone.utc)
    return calculate_budget(origin=origin, now=now, cleanup_reserve_seconds=RESERVES[phase])


def main():
    env = os.environ
    run_id, attempt = env['GITHUB_RUN_ID'], env['GITHUB_RUN_ATTEMPT']
    require(re.fullmatch(r'[1-9][0-9]*', run_id) and attempt == '1', 'invalid run identity or retry')
    request = urllib.request.Request(
        f'https://api.github.com/repos/{REPO}/actions/runs/{run_id}/attempts/{attempt}',
        headers={'Accept': 'application/vnd.github+json', 'Cache-Control': 'no-cache',
                 'X-GitHub-Api-Version': '2022-11-28'})
    with urllib.request.urlopen(request, timeout=5) as response:
        raw = response.read(2_000_001)
    require(len(raw) <= 2_000_000, 'oversized native metadata')
    event = json.loads(Path(env['GITHUB_EVENT_PATH']).read_text())
    budget = native_budget(event, env, json.loads(raw), dt.datetime.now(dt.timezone.utc), sys.argv[1])
    print(budget.proof_seconds)


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'::error::native whole-run deadline refused: {type(error).__name__}', file=sys.stderr)
        raise SystemExit(78)
