# Protected-base review-owner bootstrap

This prerequisite branch supplies the three existing owners required by the
reviewed central `native-review-v1` hook. It preserves the migration and product checks, permissions, protection and live
allowance. A temporary legacy-workflow admission guard makes draft publication
cheap while these owners are reviewed for landing. It creates
no new review requirement: the prepared #2380 caller already selects the central
contract, which refuses when these protected-base files are absent.

## Recovered accepted source

All three producers are copied byte-for-byte, with original Git file modes,
from `EffortlessMetrics/perl-lsp-swarm` main
`b3809f0e50d75e7a2aec2e681b2492429fc51736`. They accept an explicit repository
argument; central supplies `EffortlessMetrics/unsafe-review-swarm`. No Perl
repository default or policy-surface transplant is introduced.

| Path under scripts/ci | Git blob | Mode |
| --- | --- | --- |
| check-pr-semantic-review-currentness.py | 9ba85bf6784c626f74f08060d76f0922201fa5eb | 100644 |
| check-pr-review-convergence | fd13074d4761a3fa4ec9c6ea4888965af57c424e | 100755 |
| check-pr-review-convergence-core | 8ba2ef91c9100c78379332fececcedb49a3a92b3 | 100755 |

Accepted source history includes these merged owner repairs:

- [#17493](https://github.com/EffortlessMetrics/perl-lsp-swarm/pull/17493),
  merge e4c0cd5080f7d278a1cfdf910436ddb5de0dca3a: later adverse cumulative reviews revoke currentness.
- [#17575](https://github.com/EffortlessMetrics/perl-lsp-swarm/pull/17575),
  merge 71de63b76d2c0013d6eeb4ef7d40e737f91a4f94: fixture data cannot qualify live currentness.
- [#16698](https://github.com/EffortlessMetrics/perl-lsp-swarm/pull/16698),
  merge b78cffe30144c1da5e155f2818e6d6fed57253f2: canonical semantic stdout schema.
- [#16583](https://github.com/EffortlessMetrics/perl-lsp-swarm/pull/16583),
  merge 6f20f89922e27164c068f0e376ed15874e5f06bf: native review count and wrapper-v2 envelope.
- [#16317](https://github.com/EffortlessMetrics/perl-lsp-swarm/pull/16317),
  merge 4cadcde3af5cf7e6e2fa0b94399db66108ddbb6f: bounded argv through stdin pagination.
- [#16335](https://github.com/EffortlessMetrics/perl-lsp-swarm/pull/16335),
  merge 08836a10a94227e1a1744330b01a55224f710b31: convergence-core-v1 envelope.

The semantic producer returns `semantic_review_currentness.v1`; production
success requires `REVIEW_CURRENT`, live acquisition and matching current head.
Fixture evaluation remains `NOT_PROVEN`. The native-facts wrapper returns
`convergence_core.v1` plus `pr_review_convergence.v2`; convergence does not
itself establish substantive semantic review. Both are required by central.

The accepted standalone semantic regression file is also unchanged (blob
14c8b1dea6c1587434f2c5c40a767906ffaf1e28). The cumulative-review class is retained
verbatim from upstream tests/test_semantic_review_currentness.py (blob
cf7243fb215aa73a111cae4a2405fba893b10b46), with only standalone imports/ROOT adapted.
Upstream Perl-specific policy-surface tests and helper scripts are excluded.

## Focused local checks

```bash
python3 scripts/ci/test_check_pr_semantic_review_currentness.py
python3 scripts/ci/test_review_owner_supersession.py
python3 scripts/ci/test_review_owner_bootstrap.py
python3 scripts/ci/test_bootstrap_admission.py
bash -n scripts/ci/check-pr-review-convergence scripts/ci/check-pr-review-convergence-core
```

The bootstrap integration tests use an explicit Unsafe repository argument and
an offline provider tripwire. They check producer custody/modes, required schema
shapes, clean native facts without a semantic claim, pending users/teams,
current change requests, active/outdated threads and unavailable facts. These
are source/evaluator controls, not authenticated native admission or product
qualification. No test posts review markers or calls live GitHub.

## Landing dependency and ownership

#2380 retains migration ownership at reviewed source
77bea9a8c4303a07da759a68ac02e9b889f6604b. This bootstrap is separately based on
Unsafe main b5355e2f917cad5132f5e1b72e2d8d43510b7d8a; it contains the owners,
tests, source handoff and temporary legacy-workflow admission guard. No existing separate Unsafe owner PR was found;
the original owner fixes above are already merged in Perl.

The bootstrap must land in Unsafe **protected main** through its normal review
and required-check gates before #2380 can use these owners. Copying them into a
candidate, relying on an alternate branch, or accepting fixture success cannot
satisfy that central protected-base requirement. After landing, re-read the
actual base, bind the native integration/result subject, refresh affected
review/currentness evidence and only then propose qualification. The accepted
semantic owner requires a genuine substantive semantic-review:v1 record bound
to the cumulative subject; a generic source-review comment is not that record.
Do not mint a positive marker or admission request from this bootstrap handoff.

The bootstrap retains the existing legacy proof floor for its own normal
required-check qualification. Product execution requires a fresh native PR body
edit by the existing coordinator, with an independent review receipt bound to
head/base/native integration/tree. Draft, Ready and synchronize do not admit
product execution. The router emits hosted metadata-only routing without an
inventory request on those events; the required check fails before checkout,
toolchain setup or Cargo. It cannot treat skipped product proof as green.

The same bounded native authority program runs before setup, immediately before
product entry and after successful proof. It rejects subject drift, revocation,
dismissed/adverse review and unavailable metadata. Checkout explicitly selects
the reviewed native merge SHA; the launched proof confirms that checkout. The
existing real command chains and bounded nonfatal failure publisher remain.
The assertion step runs only after a successful launch, avoiding its long
missing-receipt wait when admission or setup fails. No new token scopes, actions,
runner grants, required checks or dispatch entrypoint are added.

This guard does **not** select central 8ff, establish protected-base owner
readiness, or make a whole-run deadline claim. The existing legacy 60-minute job
cap remains; queue, setup and cleanup timing require actual qualification. The
migration's stricter deadline is a separate consumer contract. No native request
or qualification run is issued by local fixture tests. Draft publication is for
source collaboration and remote-only qualification dependencies; landing still
requires current independent review, real product success and native policy.

Live allowance for the migration's exact 8ff77e58 pin is still unverified.
Immutable runtime tools, cold product fit, artifacts/result binding and cleanup
qualification remain separate gates. There is no dispatch, settings/access,
image activation, main merge, release or deployment in this source handoff.
