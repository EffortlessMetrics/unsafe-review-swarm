"""Deterministic deadline oracles; no sleeps, builds or native CI required."""

from datetime import datetime, timedelta, timezone, tzinfo
import unittest

from qualification_deadline import calculate_budget


ORIGIN = datetime(2026, 10, 15, tzinfo=timezone.utc)


class FoldZone(tzinfo):
    """Two UTC offsets for an ambiguous hour, without a system tz database."""

    def utcoffset(self, dt):
        return timedelta(hours=-5 if dt.fold else -4)

    def dst(self, dt):
        return timedelta(0)


class QualificationDeadlineTests(unittest.TestCase):
    def budget(self, elapsed=0, reserve=60):
        return calculate_budget(
            origin=ORIGIN,
            now=ORIGIN + timedelta(seconds=elapsed),
            cleanup_reserve_seconds=reserve,
        )

    def test_fixed_whole_run_deadline(self):
        budget = self.budget()
        self.assertEqual(budget.deadline, ORIGIN + timedelta(seconds=1200))
        self.assertEqual(budget.proof_seconds, 1140)

    def test_setup_and_cold_build_time_are_already_spent(self):
        self.assertEqual(self.budget(300).proof_seconds, 840)
        # Warm product timing alone leaves only 114 seconds after this reserve;
        # another 120 seconds of setup makes the same workload inadmissible.
        self.assertEqual(self.budget(1026).proof_seconds, 114)
        with self.assertRaises(ValueError):
            self.budget(1146)

    def test_retry_and_recovery_retain_origin_and_shrink_budget(self):
        budgets = [self.budget(t) for t in (0, 180, 600, 1000)]
        self.assertEqual([b.proof_seconds for b in budgets], [1140, 960, 540, 140])
        self.assertEqual(len({b.deadline for b in budgets}), 1)
        self.assertEqual(self.budget(600), self.budget(600))

    def test_reserve_reduces_proof_without_extending_deadline(self):
        small, large = self.budget(300, 60), self.budget(300, 240)
        self.assertEqual(small.deadline, large.deadline)
        self.assertEqual(small.proof_seconds - large.proof_seconds, 180)

    def test_floor_final_remainder_without_granting_partial_seconds(self):
        self.assertEqual(self.budget(0.000001).proof_seconds, 1139)
        self.assertEqual(self.budget(1139).proof_seconds, 1)
        for elapsed in (1139.000001, 1139.999999, 1140, 1199):
            with self.subTest(elapsed=elapsed), self.assertRaises(ValueError):
                self.budget(elapsed)

    def test_exact_deadline_and_later_are_expired(self):
        for elapsed in (1200, 1200.000001, 5000):
            with self.subTest(elapsed=elapsed), self.assertRaisesRegex(
                ValueError, "expired"
            ):
                self.budget(elapsed)

    def test_future_origin_even_one_microsecond_is_refused(self):
        with self.assertRaisesRegex(ValueError, "future"):
            self.budget(-0.000001)

    def test_missing_malformed_and_naive_times_are_refused(self):
        invalid = (None, "2026-10-15T00:00:00Z", "bad", 0, True,
                   float("nan"), float("inf"), datetime(2026, 10, 15))
        for field in ("origin", "now"):
            for value in invalid:
                kwargs = dict(origin=ORIGIN, now=ORIGIN, cleanup_reserve_seconds=60)
                kwargs[field] = value
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    calculate_budget(**kwargs)

    def test_reserve_must_be_explicit_positive_bounded_integer(self):
        for value in (None, 0, -1, 1200, 3900, 4500, True, 60.0,
                      "60", float("nan"), float("inf")):
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.budget(reserve=value)
        self.assertEqual(self.budget(reserve=1199).proof_seconds, 1)

    def test_equivalent_offsets_use_the_same_absolute_deadline(self):
        offset = timezone(timedelta(hours=5, minutes=30))
        result = calculate_budget(
            origin=ORIGIN.astimezone(offset), now=ORIGIN,
            cleanup_reserve_seconds=60,
        )
        self.assertEqual(result, self.budget())
        self.assertIs(result.deadline.tzinfo, timezone.utc)

    def test_same_zone_fold_uses_elapsed_time_not_wall_time(self):
        zone = FoldZone()
        early = datetime(2026, 11, 1, 1, 10, tzinfo=zone, fold=0)
        late = datetime(2026, 11, 1, 1, 20, tzinfo=zone, fold=1)
        with self.assertRaisesRegex(ValueError, "expired"):
            calculate_budget(origin=early, now=late, cleanup_reserve_seconds=60)
        with self.assertRaisesRegex(ValueError, "future"):
            calculate_budget(origin=late, now=early, cleanup_reserve_seconds=60)

    def test_unrepresentable_deadline_is_refused(self):
        maximum = datetime.max.replace(tzinfo=timezone.utc)
        with self.assertRaisesRegex(ValueError, "represent"):
            calculate_budget(origin=maximum, now=maximum, cleanup_reserve_seconds=60)


if __name__ == "__main__":
    unittest.main()
