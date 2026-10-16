"""Accepted Perl cumulative-review falsifiers, ported without policy surfaces.

Class body retained verbatim from tests/test_semantic_review_currentness.py at
b3809f0e50d75e7a2aec2e681b2492429fc51736 (blobcf7243fb). Only imports/ROOT
adapt to this standalone Unsafe test location; no producer behavior is changed.
"""
import contextlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]

class LaterCumulativeReviewTests(unittest.TestCase):
    """Exercise the real reducer and Git subject digest, not a duplicate model (#15010)."""

    @classmethod
    def setUpClass(cls) -> None:
        path = ROOT / "scripts/ci/check-pr-semantic-review-currentness.py"
        spec = importlib.util.spec_from_file_location("review_supersession_subject", path)
        assert spec and spec.loader
        cls.checker = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = cls.checker
        spec.loader.exec_module(cls.checker)

    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Review fixture")
        self.git("config", "user.email", "review@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        (self.root / "source.rs").write_text("fn value() -> u8 { 1 }\n", encoding="utf-8")
        self.base = self.commit("base")
        (self.root / "source.rs").write_text("fn value() -> u8 { 2 }\n", encoding="utf-8")
        self.head = self.commit("candidate")

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args], cwd=self.root, check=True, capture_output=True,
            text=True, encoding="utf-8", timeout=15,
        ).stdout.strip()

    def commit(self, message: str) -> str:
        self.git("add", ".")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD")

    def record(self, result="REVIEW_CURRENT", second=1, **overrides):
        head = overrides.pop("commit_oid", self.head)
        sections = "\n\n".join(
            f"{heading}\n- {'no open findings' if heading == '## No material findings' else 'fixture evidence'}"
            for heading in (
                "## Review scope", "## Evidence and falsifiers",
                "## No material findings" if result == "REVIEW_CURRENT" else "## Findings",
                "## What this establishes", "## Residual risk / not proved",
            )
        )
        body = f"{sections}\n\n## Substantive review result\n- {result}\n"
        if result == "REVIEW_CURRENT":
            marker = {
                "head": head, "merge_base": self.base, "pr": 15010,
                "result": result,
                "subject_sha256": self.checker.subject_digest(self.root, self.base, head),
            }
            body += "\n<!-- semantic-review:v1 " + json.dumps(marker) + " -->\n"
        values = dict(
            login="reviewer", user_type="User", state="COMMENTED", body=body,
            commit_oid=head, submitted_at=f"2026-10-09T01:00:{second:02d}Z",
        )
        values.update(overrides)
        return self.checker.Review(**values)

    def verdict(self, *records, head=None):
        return self.checker.evaluate(
            self.root, pr=15010, current_head=head or self.head, reviews=iter(records)
        )

    def test_same_head_later_commented_changes_required_blocks(self):
        value = self.verdict(self.record(), self.record("CHANGES_REQUIRED", 2))
        self.assertEqual("NOT_PROVEN", value["classification"])
        self.assertEqual("later_cumulative_review_changes_required", value["reason"])

    def test_all_adverse_cumulative_outcomes_block(self):
        for result in ("NOT_PROVEN", "BLOCKED_BY_PREREQUISITE", "SUPERSEDED_OR_CLOSE"):
            with self.subTest(result=result):
                value = self.verdict(self.record(), self.record(result, 2))
                self.assertEqual("NOT_PROVEN", value["classification"])
                self.assertEqual("later_cumulative_review_" + result.lower(), value["reason"])

    def test_adverse_same_timestamp_wins_over_api_order(self):
        clean, adverse = self.record(), self.record("CHANGES_REQUIRED")
        for records in ((clean, adverse), (adverse, clean)):
            with self.subTest(order=records):
                self.assertEqual("NOT_PROVEN", self.verdict(*records)["classification"])

    def test_genuine_later_cumulative_rereview_can_restore_currentness(self):
        records = (self.record(), self.record("CHANGES_REQUIRED", 2), self.record(second=3))
        self.assertEqual("REVIEW_CURRENT", self.verdict(*reversed(records))["classification"])

    def test_repaired_head_requires_its_own_valid_positive_marker(self):
        clean, adverse = self.record(), self.record("CHANGES_REQUIRED", 2)
        (self.root / "source.rs").write_text("fn value() -> u8 { 3 }\n", encoding="utf-8")
        repaired = self.commit("repair")
        self.assertEqual("NOT_PROVEN", self.verdict(clean, adverse, head=repaired)["classification"])
        rereview = self.record(second=3, commit_oid=repaired)
        self.assertEqual("REVIEW_CURRENT", self.verdict(clean, adverse, rereview, head=repaired)["classification"])

    def test_unmarked_later_clean_claim_does_not_erase_adverse(self):
        clean, adverse = self.record(), self.record("CHANGES_REQUIRED", 2)
        unmarked = self.record(second=3)
        unmarked = unmarked._replace(body=unmarked.body.split("<!--")[0])
        self.assertEqual("NOT_PROVEN", self.verdict(clean, adverse, unmarked)["classification"])

    def test_bot_dismissed_and_pending_reviews_do_not_become_cumulative_verdicts(self):
        for override in ({"user_type": "Bot"}, {"state": "DISMISSED"}, {"state": "PENDING"}):
            with self.subTest(override=override):
                value = self.verdict(self.record(), self.record("CHANGES_REQUIRED", 2, **override))
                self.assertEqual("REVIEW_CURRENT", value["classification"])

    def test_ordinary_comment_is_not_an_adverse_cumulative_review(self):
        comment = self.record("CHANGES_REQUIRED", 2, body="Could this need CHANGES_REQUIRED later?")
        self.assertEqual("REVIEW_CURRENT", self.verdict(self.record(), comment)["classification"])

    def test_quoted_complete_review_is_not_a_live_declaration(self):
        quoted = self.record("CHANGES_REQUIRED", 2)
        quoted = quoted._replace(body="```markdown\n" + quoted.body + "```\n")
        self.assertEqual("REVIEW_CURRENT", self.verdict(self.record(), quoted)["classification"])

    def test_fenced_required_sections_cannot_pose_as_a_cumulative_review(self):
        quoted = self.record("CHANGES_REQUIRED", 2)
        prefix = quoted.body.split("## Substantive review result")[0]
        quoted = quoted._replace(body="```markdown\n" + prefix + "```\n## Substantive review result\n- CHANGES_REQUIRED\n")
        self.assertEqual("REVIEW_CURRENT", self.verdict(self.record(), quoted)["classification"])

    def test_conflicting_result_declaration_does_not_reuse_older_green(self):
        ambiguous = self.record("CHANGES_REQUIRED", 2)
        ambiguous = ambiguous._replace(body=ambiguous.body + "- REVIEW_CURRENT\n")
        value = self.verdict(self.record(), ambiguous)
        self.assertEqual("NOT_PROVEN", value["classification"])
        self.assertEqual("later_cumulative_review_ambiguous", value["reason"])

    def test_distinct_clean_reviews_and_replayed_publications_remain_valid(self):
        clean = self.record()
        self.assertEqual("REVIEW_CURRENT", self.verdict(clean, clean, self.record(second=2))["classification"])

    def test_timestamp_comparison_uses_instants_not_strings(self):
        # 20:00:02 -05:00 is later than 01:00:01Z on the following date.
        adverse = self.record("CHANGES_REQUIRED", submitted_at="2026-10-08T20:00:02-05:00")
        self.assertEqual("NOT_PROVEN", self.verdict(self.record(), adverse)["classification"])
        rereview = self.record(submitted_at="2026-10-08T20:00:03-05:00")
        self.assertEqual("REVIEW_CURRENT", self.verdict(self.record(), adverse, rereview)["classification"])

    def test_missing_time_or_identity_in_cumulative_review_is_instrument_failure(self):
        for override in ({"submitted_at": ""}, {"submitted_at": "2026-10-09T01:00:02"}, {"user_type": ""}, {"login": ""}, {"commit_oid": "not-a-sha"}):
            with self.subTest(override=override):
                with self.assertRaises(self.checker.CurrentnessError):
                    self.verdict(self.record(), self.record("CHANGES_REQUIRED", 2, **override))

    def test_cli_returns_nonzero_for_later_body_only_adverse(self):
        fixture = self.root / "reviews.json"
        fixture.write_text(json.dumps({"head": self.head, "reviews": [r._asdict() for r in (self.record(), self.record("CHANGES_REQUIRED", 2))]}), encoding="utf-8")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = self.checker.main(["15010", "fixture/repo", "--root", str(self.root), "--fixture", str(fixture)])
        self.assertEqual(1, code)
        value = json.loads(output.getvalue())
        self.assertEqual("semantic_review_currentness.v1", value["schema_version"])
        self.assertEqual("NOT_PROVEN", value["classification"])

    def test_pending_positive_cannot_override_published_adverse(self):
        pending = self.record(second=3, state="PENDING")
        value = self.verdict(self.record(), self.record("CHANGES_REQUIRED", 2), pending)
        self.assertEqual("NOT_PROVEN", value["classification"])

    def test_quoted_positive_sections_cannot_override_published_adverse(self):
        forged = self.record(second=3)
        prefix, rest = forged.body.split("## Substantive review result", 1)
        forged = forged._replace(body="```markdown\n" + prefix + "```\n## Substantive review result" + rest)
        value = self.verdict(self.record(), self.record("CHANGES_REQUIRED", 2), forged)
        self.assertEqual("NOT_PROVEN", value["classification"])

    def test_proven_older_invalid_identity_does_not_block_later_valid_review(self):
        for override in ({"login": ""}, {"user_type": ""}, {"user_type": "Ghost"},
                         {"commit_oid": "not-a-sha"}, {"state": "UNKNOWN"}):
            with self.subTest(override=override):
                historical = self.record("CHANGES_REQUIRED", 1, **override)
                current = self.record(second=2)
                self.assertEqual("REVIEW_CURRENT", self.verdict(historical, current)["classification"])

    def test_equal_time_invalid_identity_still_fails_closed(self):
        for override in ({"login": ""}, {"user_type": "Ghost"}, {"commit_oid": "bad"}):
            with self.subTest(override=override):
                with self.assertRaises(self.checker.CurrentnessError):
                    self.verdict(self.record(), self.record("CHANGES_REQUIRED", **override))

    def test_unorderable_historical_claim_is_not_proven_older(self):
        # A timestamp failure cannot establish the old-record exemption.
        historical = self.record("CHANGES_REQUIRED", submitted_at="not-a-timestamp")
        with self.assertRaises(self.checker.CurrentnessError):
            self.verdict(historical, self.record(second=2))

    def test_no_review_stays_not_proven(self):
        self.assertEqual("NOT_PROVEN", self.verdict()["classification"])


if __name__ == "__main__":
    unittest.main()
