"""Exercise the actual receipt writer and standalone consumer admission.

Command execution is mocked at the consumer boundary; no Git, Cargo or candidate
command runs. Positive controls prove admission, not the full consumer workflow.
"""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest import mock


WORKFLOW = Path(os.environ.get("QUALIFICATION_WORKFLOW", ".github/workflows/windows-qual.yml"))
PROBE = Path(os.environ.get("QUALIFICATION_PROBE", ".github/scripts/installed_consumer_probe.py"))

# Sixteen upstream assertions emitted before the writer adds its three identity rows.
PASSING_UPSTREAM_ROWS = (
    "identity", "help:doctor", "help:pr", "help:repo", "help:context",
    "help:lsp", "help:init", "help:pr-setup", "init:exit", "init:json-parse",
    "init:no-mutation", "init:no-tracked-writes", "bundle:exit", "bundle:verifier",
    "fail:invalid-flag", "fail:malformed-diff",
)


class QualificationReceipt(unittest.TestCase):
    def receipt(self, observed="0.3.8"):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        block = workflow.split("      - name: Write bounded receipt\n", 1)[1]
        script = textwrap.dedent(block.split("python - <<'EOF'\n", 1)[1].split("          EOF", 1)[0])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / ("qual-prefix/bin/unsafe-review" + (".exe" if sys.platform == "win32" else ""))
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"fixture source-built binary")
            source = root / "source-candidate"
            source.mkdir()
            (source / "Cargo.lock").write_bytes(b"fixture lockfile\n")
            lock = hashlib.sha256((source / "Cargo.lock").read_bytes()).hexdigest()
            values = {"rows.tsv": "".join(name + "\t0\t-\n" for name in PASSING_UPSTREAM_ROWS),
                      "candidate-sha.txt": "a" * 40 + "\n",
                      "candidate-version.txt": "0.3.8\n",
                      "observed-version.txt": "unsafe-review " + observed + "\n",
                      "lockfile-sha.txt": lock + "  Cargo.lock\n",
                      "lockfile-git-sha.txt": lock + "  -\n",
                      "toolchain.txt": "rustc 1.98.1 (fixture)\n",
                      "source-tree.txt": "b" * 40 + "\n"}
            for name, content in values.items():
                (root / name).write_text(content, encoding="utf-8")
            for package in ("unsafe-review-core", "unsafe-review-cli", "unsafe-review"):
                (root / ("pkg-list-" + package + ".txt")).write_text("Cargo.toml\nsrc/lib.rs\n", encoding="utf-8")
            (root / "source-identity.json").write_text(json.dumps({
                "candidate_sha": "a" * 40, "source_tree": "b" * 40,
                "package_versions": {p: "0.3.8" for p in ("unsafe-review-core", "unsafe-review-cli", "unsafe-review")},
                "tracked_lockfile_sha256": lock}), encoding="utf-8")
            (root / "init-consumer-receipt.json").write_text(json.dumps({
                "result": "pass", "source_binding": "verified_against_supplied_install_receipt",
                "qualification_status": "qualified_for_exercised_cases", "rows": [{"result": "pass"}]}), encoding="utf-8")
            result = subprocess.run([sys.executable, "-c", script], cwd=root,
                                    env={**os.environ, "GITHUB_WORKSPACE": str(root)}, capture_output=True)
            receipt_path = root / "windows-qual-receipt.json"
            receipt = json.loads(receipt_path.read_text()) if receipt_path.exists() else None
            return result, receipt

    def test_receipt_uses_actual_candidate_version(self):
        result, receipt = self.receipt()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(receipt["candidate_version"], "0.3.8")

    def test_receipt_rejects_manifest_executable_version_mismatch(self):
        result, receipt = self.receipt(observed="0.4.0")
        self.assertNotEqual(result.returncode, 0, "a false version receipt must fail qualification")
        self.assertIsNotNone(receipt, result.stderr.decode())
        failures = [row["row"] for row in receipt["rows"] if row["exit"] != 0]
        self.assertEqual(failures, ["identity:manifest-executable-version"])

    def test_candidate_sha_and_binary_hash_are_retained(self):
        result, receipt = self.receipt()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(receipt["candidate_sha"], "a" * 40)
        self.assertEqual(receipt["installed_binary_sha256"], hashlib.sha256(b"fixture source-built binary").hexdigest())



class InstalledConsumerAdmission(unittest.TestCase):
    def passing_receipt(self):
        result, receipt = QualificationReceipt().receipt()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(receipt["schema"], "unsafe-review/installed-qualification/v1")
        self.assertEqual(len(receipt["rows"]), 19)
        return receipt

    def probe(self, provenance, raw=None, supply=True):
        spec = importlib.util.spec_from_file_location("consumer_probe_under_test", PROBE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "fixture-binary"
            binary.write_bytes(b"fixture source-built binary")
            install = root / "installation.json"
            install.write_text(raw if raw is not None else json.dumps(provenance), encoding="utf-8")
            scratch = root / "previously-absent-scratch"
            argv = [str(PROBE), "--binary", str(binary), "--scratch", str(scratch)]
            if supply:
                argv += ["--provenance", str(install), "--candidate", "a" * 40,
                         "--lockfile-sha256", hashlib.sha256(b"fixture lockfile\n").hexdigest(),
                         "--expected-version", "0.3.8"]
            output = io.StringIO()
            # A reached command boundary is observable without executing any command.
            with mock.patch.object(sys, "argv", argv), \
                    mock.patch.object(module.subprocess, "run",
                                      side_effect=RuntimeError("command observation boundary")) as commands, \
                    contextlib.redirect_stdout(output):
                exit_code = module.main()
            receipt = json.loads((scratch / "receipt.json").read_text(encoding="utf-8"))
            return exit_code, receipt, json.loads(output.getvalue()), commands.call_count

    def assert_rejected(self, provenance, state, diagnostic, raw=None):
        exit_code, receipt, summary, calls = self.probe(provenance, raw=raw)
        self.assertEqual(receipt["source_binding"], "unknown",
                         "invalid upstream qualification must not acquire verified source binding")
        self.assertEqual(exit_code, 1)
        self.assertEqual(receipt["qualification_status"], "not_qualified")
        self.assertEqual(calls, 0, "reject before any candidate or fixture command")
        self.assertEqual(receipt["upstream_qualification_status"], state)
        self.assertEqual(receipt["rows"][0]["name"], "upstream-install-qualification")
        self.assertEqual(receipt["rows"][0]["result"], "fail")
        self.assertIn(diagnostic, receipt["stop_reason"])
        self.assertEqual(summary["stop_reason"], receipt["stop_reason"])
        self.assertEqual(summary["upstream_qualification_status"], state)

    def test_complete_actual_writer_receipt_admits_identity_checks(self):
        exit_code, receipt, _, calls = self.probe(self.passing_receipt())
        self.assertEqual(receipt["source_binding"], "verified_against_supplied_install_receipt")
        identity = next(row for row in receipt["rows"] if row["name"] == "exact-source-binary-provenance")
        self.assertEqual(identity["result"], "pass")
        self.assertEqual(calls, 1)
        self.assertEqual(receipt["stop_reason"], "command observation boundary")
        self.assertEqual(exit_code, 1)  # Deliberate stop; this is not full consumer qualification.

    def test_matching_identities_failed_upstream_rows_do_not_admit(self):
        for failed in ("identity:source-sha", "help:init", "bundle:verifier"):
            with self.subTest(failed=failed):
                receipt = self.passing_receipt()
                next(row for row in receipt["rows"] if row["row"] == failed)["exit"] = 1
                self.assert_rejected(receipt, "failed", failed)

    def test_wrong_or_missing_schema_does_not_admit(self):
        for schema in ("other/v1", "unsafe-review/installed-qualification/v2", None):
            with self.subTest(schema=schema):
                receipt = self.passing_receipt()
                if schema is None:
                    del receipt["schema"]
                else:
                    receipt["schema"] = schema
                self.assert_rejected(receipt, "invalid", "schema")

    def test_missing_upstream_rows_do_not_admit(self):
        for missing in ("help:init", "identity:source-sha"):
            with self.subTest(missing=missing):
                receipt = self.passing_receipt()
                receipt["rows"] = [row for row in receipt["rows"] if row["row"] != missing]
                self.assert_rejected(receipt, "invalid", missing)

    def test_missing_empty_or_non_array_population_does_not_admit(self):
        for rows in (None, [], {}, "passed"):
            with self.subTest(rows=rows):
                receipt = self.passing_receipt()
                if rows is None:
                    del receipt["rows"]
                else:
                    receipt["rows"] = rows
                self.assert_rejected(receipt, "invalid", "rows")

    def test_duplicate_upstream_rows_do_not_admit(self):
        receipt = self.passing_receipt()
        receipt["rows"].append(copy.deepcopy(receipt["rows"][0]))
        self.assert_rejected(receipt, "invalid", "duplicate")

    def test_malformed_rows_and_non_integer_exits_do_not_admit(self):
        for bad in (None, [], {"row": ["identity"], "exit": 0},
                    {"row": "", "exit": 0}, {"row": "identity"},
                    {"row": "identity", "exit": False}, {"row": "identity", "exit": 0.0},
                    {"row": "identity", "exit": "0"}):
            with self.subTest(bad=bad):
                receipt = self.passing_receipt()
                receipt["rows"][0] = bad
                self.assert_rejected(receipt, "invalid", "row")

    def test_failed_additive_row_does_not_admit(self):
        receipt = self.passing_receipt()
        receipt["rows"].append({"row": "future:control", "exit": 1, "detail": "failed"})
        self.assert_rejected(receipt, "failed", "future:control")

    def test_passing_additive_metadata_and_row_remain_compatible(self):
        receipt = self.passing_receipt()
        receipt["extra_metadata"] = {"future": True}
        receipt["rows"].append({"row": "future:control", "exit": 0})
        _, observed, _, calls = self.probe(receipt)
        self.assertEqual(observed["source_binding"], "verified_against_supplied_install_receipt")
        self.assertEqual(calls, 1)

    def test_malformed_json_gets_explicit_invalid_diagnostic(self):
        self.assert_rejected({}, "invalid", "cannot read upstream", raw="{invalid")

    def test_non_object_receipt_does_not_admit(self):
        for receipt in ([], None, "passed"):
            with self.subTest(receipt=receipt):
                self.assert_rejected(receipt, "invalid", "object")

    def test_passing_rows_cannot_rescue_wrong_identity(self):
        receipt = self.passing_receipt()
        receipt["candidate_sha"] = "c" * 40
        exit_code, observed, _, calls = self.probe(receipt)
        self.assertEqual(observed["source_binding"], "unknown")
        self.assertEqual(observed["qualification_status"], "not_qualified")
        self.assertEqual(exit_code, 1)
        self.assertEqual(calls, 0)
        self.assertIn("does not bind", observed["stop_reason"])

    def test_without_provenance_remains_capability_observation(self):
        _, receipt, _, calls = self.probe(None, supply=False)
        self.assertEqual(receipt["source_binding"], "unknown")
        self.assertEqual(receipt["qualification_status"], "not_qualified")
        self.assertEqual(receipt["execution_class"], "capability_observation")
        self.assertEqual(calls, 1)


if __name__ == "__main__":
    unittest.main()
