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


class GeneratedCommandAdmission(unittest.TestCase):
    """No candidate-returned shell suffix reaches the command boundary."""

    def test_source_literal_quoting_for_both_shell_families(self):
        spec = importlib.util.spec_from_file_location("consumer_probe_under_test", PROBE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        value = "repo's $literal; name"
        with mock.patch.object(module.os, "name", "posix"):
            self.assertEqual(module.init_shell_arg(value), "'repo'\\''s $literal; name'")
        with mock.patch.object(module.os, "name", "nt"):
            self.assertEqual(module.init_shell_arg(value), "'repo''s $literal; name'")

    def test_accepts_source_command_shape_and_rejects_wrong_identity(self):
        spec = importlib.util.spec_from_file_location("consumer_probe_under_test", PROBE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "repo's $literal; name"
            root.mkdir()
            root_text = str(root.resolve())
            artifacts = str(root / "target/unsafe-review")
            if os.name == "nt":
                quoted_root = "'" + root_text.replace("'", "''") + "'"
                quoted_artifacts = "'" + artifacts.replace("'", "''") + "'"
            else:
                quoted_root = "'" + root_text.replace("'", "'\\''") + "'"
                quoted_artifacts = "'" + artifacts.replace("'", "'\\''") + "'"
            doctor = "unsafe-review doctor --root " + quoted_root
            first_pr = ("unsafe-review pr --root " + quoted_root +
                        " --base origin/main --out-dir " + quoted_artifacts)
            proposal = {
                "root": root_text, "repository": {"base_ref": "origin/main"},
                "commands": {"doctor": doctor, "first_pr": first_pr,
                             "first_pr_artifacts": artifacts},
            }
            self.assertEqual(module.admitted_generated_command(proposal, "doctor", root), doctor)
            self.assertEqual(module.admitted_generated_command(proposal, "first_pr", root), first_pr)
            for altered in (
                    {"root": str(Path(directory))},
                    {"repository": {"base_ref": "origin/other"}},
                    {"commands": {**proposal["commands"],
                                  "first_pr_artifacts": str(Path(directory) / "elsewhere")}}):
                with self.subTest(altered=altered):
                    candidate = {**proposal, **altered}
                    with self.assertRaisesRegex(RuntimeError, "generated .* command"):
                        module.admitted_generated_command(candidate, "first_pr", root)

    def test_rejects_doctor_and_first_pr_suffix_before_shell(self):
        spec = importlib.util.spec_from_file_location("consumer_probe_under_test", PROBE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for attacked in ("doctor", "first_pr"):
            with self.subTest(attacked=attacked), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binary = root / "unsafe-review"
                binary.write_bytes(b"fixture candidate; never executed")
                scratch = root / "previously-absent-scratch"
                marker = root / "rejected-command-marker"
                shell_calls = []

                def quote(value):
                    value = str(value)
                    if value and all(ch.isascii() and (ch.isalnum() or ch in "/._-:") for ch in value):
                        return value
                    if os.name == "nt":
                        return "'" + value.replace("'", "''") + "'"
                    return "'" + value.replace("'", "'\\''") + "'"

                def proposal_for(selected):
                    selected = Path(selected)
                    root_text = str(selected.resolve())
                    doctor = "unsafe-review doctor --root " + quote(root_text)
                    base = None if selected.name == "control" else "origin/main"
                    artifacts = str(selected / "target/unsafe-review")
                    first_pr = None if base is None else (
                        "unsafe-review pr --root " + quote(root_text) +
                        " --base origin/main --out-dir " + quote(artifacts))
                    if selected.name == "target repo's $literal;name":
                        suffix = ("; Set-Content -LiteralPath " + quote(marker) + " -Value injected"
                                  if os.name == "nt" else "; printf injected > " + quote(marker))
                        if attacked == "doctor":
                            doctor += suffix
                        else:
                            first_pr += suffix
                    return {
                        "schema_version": "unsafe-review/init/v1", "mode": "preview_only",
                        "writes_repository": False, "root": root_text,
                        "repository": {"base_ref": base},
                        "proposed_files": [{"status": "conflict"}],
                        "commands": {
                            "doctor": doctor, "first_pr": first_pr,
                            "first_pr_artifacts": artifacts,
                            "first_pr_prerequisite": "Provide --base or --diff",
                        },
                    }

                def fake_run(argv, *, cwd=None, env=None, capture_output=None, timeout=None):
                    argv = [str(arg) for arg in argv]
                    if argv[0] == "git":
                        return subprocess.CompletedProcess(argv, 0, b"", b"")
                    if argv[0] == str(binary):
                        if argv[1:] == ["--version"]:
                            return subprocess.CompletedProcess(argv, 0, b"unsafe-review 0.5.0\n", b"")
                        if argv[1:] == ["init", "--help"]:
                            return subprocess.CompletedProcess(
                                argv, 0, b"unsafe-review init:\nUsage: unsafe-review init [--root .]\n", b"")
                        if argv[1] == "init":
                            selected = argv[argv.index("--root") + 1]
                            raw = json.dumps(proposal_for(selected)).encode()
                            if "--out" in argv:
                                out = Path(argv[argv.index("--out") + 1])
                                out.mkdir(parents=True)
                                (out / "unsafe-review-init.json").write_bytes(raw)
                            return subprocess.CompletedProcess(argv, 0, raw, b"")
                    if argv[0] in ("sh", "powershell.exe"):
                        shell_calls.append(argv[-1])
                        if str(marker) in argv[-1]:
                            # Simulate the side effect; never run candidate-returned text.
                            marker.write_text("would have executed")
                        return subprocess.CompletedProcess(
                            argv, 0, ("unsafe-review doctor\nworkspace root: " +
                                      str(scratch / "target repo's $literal;name") + "\n").encode(), b"")
                    raise AssertionError("unexpected command " + repr(argv))

                output = io.StringIO()
                with mock.patch.object(sys, "argv", [str(PROBE), "--binary", str(binary),
                                                      "--scratch", str(scratch)]), \
                        mock.patch.object(module.subprocess, "run", side_effect=fake_run), \
                        contextlib.redirect_stdout(output):
                    exit_code = module.main()
                receipt = json.loads((scratch / "receipt.json").read_text(encoding="utf-8"))
                self.assertEqual(exit_code, 1)
                self.assertFalse(marker.exists(), "injected shell side effect reached the boundary")
                self.assertFalse(any(str(marker) in command for command in shell_calls))
                self.assertIn("generated " + attacked + " command", receipt["stop_reason"])


if __name__ == "__main__":
    unittest.main()
