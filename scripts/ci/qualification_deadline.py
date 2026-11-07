"""Pure whole-run budget arithmetic; this module grants no proof admission.

The caller must authenticate the native run/attempt/source and original start,
retain that origin across setup, routing, retries and recovery, and supply fresh
trusted current time without clock rollback. Parsing native metadata belongs to
that caller. This helper neither supervises processes nor bounds setup, cleanup
or cancellation. Integration must enforce the returned deadline and reserve.
"""

from dataclasses import dataclass
from datetime import datetime, timedelta, timezone


WHOLE_RUN_SECONDS = 1200
_SECOND = timedelta(seconds=1)
_WHOLE_RUN = timedelta(seconds=WHOLE_RUN_SECONDS)


@dataclass(frozen=True)
class DeadlineBudget:
    """Absolute UTC whole-run deadline and available whole proof seconds."""

    deadline: datetime
    proof_seconds: int


def _utc(value: datetime, name: str) -> datetime:
    if not isinstance(value, datetime):
        raise ValueError(f"{name} must be a timezone-aware datetime")
    try:
        if value.utcoffset() is None:
            raise ValueError(f"{name} must be timezone-aware")
        return value.astimezone(timezone.utc)
    except (OverflowError, TypeError, ValueError) as error:
        raise ValueError(f"{name} is not a valid aware timestamp") from error


def calculate_budget(
    *, origin: datetime, now: datetime, cleanup_reserve_seconds: int
) -> DeadlineBudget:
    """Refuse invalid/exhausted input; floor only the final proof remainder.

    The fixed 1200 seconds include all time since origin. The positive reserve
    covers diagnostics, finalization, termination grace and cleanup; it never
    extends the deadline. For an unchanged origin/reserve and advancing current
    time, proof_seconds cannot increase. Replacing origin or rolling time back
    is a caller contract violation this stateless calculator cannot detect.
    """
    start = _utc(origin, "origin")
    current = _utc(now, "now")
    if type(cleanup_reserve_seconds) is not int or not (
        0 < cleanup_reserve_seconds < WHOLE_RUN_SECONDS
    ):
        raise ValueError("cleanup reserve must be integer seconds in (0, 1200)")
    if start > current:
        raise ValueError("origin must not be in the future")
    try:
        deadline = start + _WHOLE_RUN
    except OverflowError as error:
        raise ValueError("origin cannot represent a whole-run deadline") from error
    remaining = deadline - current
    if remaining <= timedelta(0):
        raise ValueError("whole-run deadline has expired")
    proof_seconds = (remaining - cleanup_reserve_seconds * _SECOND) // _SECOND
    if proof_seconds < 1:
        raise ValueError("no whole proof second remains after cleanup reserve")
    return DeadlineBudget(deadline=deadline, proof_seconds=proof_seconds)
