# Consumer interruption supervision

`.ci/supervise.py` supervises the proof and finalizer process groups launched by
`.ci/run.sh`. The same supervisor also bounds every native metadata query to
five seconds plus one second of termination grace. The existing native deadline
adapter still owns origin and budget
calculation. Proof gets its existing 30-second grace; finalization gets its
existing 10-second grace. Cleanup does not receive another budget window.

The supervisor launches a new session, forwards bounded stdout/stderr buffers,
and terminates its owned process group on timeout, cancellation or leader exit.
It does not infer that descendants are gone merely because Bash exited. On
termination it uses the existing grace, reserving its final fraction for kill
and drain. Successful completion requires leader completion and both pipe EOFs;
unavailable EOF or broken output cannot turn success into a green result.
Primary nonzero results remain authoritative. Timeout is 124; caught signals
are 128 plus the signal number; supervision failure after success is 78.

The entrypoint continues to publish its authoritative result atomically and
preserve original proof failure through finalizer or receipt-publication failure.
Native deadline refusal still prevents success after budget expiry.

Focused Linux checks:

    python3 .ci/test-run.py -v
    python3 scripts/ci/test_interrupted_receipts.py -v
    python3 scripts/ci/test_supervise.py -v
    python3 scripts/ci/test_metadata_transport.py -v

The interruption fixtures substitute Cargo and native metadata transport, and
shorten duration/grace for fault cases. The production scripts are unchanged in
those fixtures. Controls include partial/missing receipts, failed publication,
TERM-resistant descendants, finalizer failures, ordinary completion, unrelated
process-group isolation, cancellation, blocked output and missing pipe EOF.

These checks are not native GitHub cancellation, artifact publication/retention,
runner, whole-run deadline or cleanup qualification. A descendant that creates
a new session can escape the owned group; missing EOF fails closed, but this
helper is not a security boundary or a substitute for central cgroup/container
cleanup. Setup, queueing, uploads and cleanup outside the consumer entrypoint
remain the immutable central owner's responsibility. Admission stays refused
until the separate source and runtime prerequisites are satisfied.
