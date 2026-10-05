"""Exercise the actual embedded workflow receipt writer with candidate inputs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest


WORKFLOW = Path(os.environ.get("QUALIFICATION_WORKFLOW", ".github/workflows/windows-qual.yml"))


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
            values = {"rows.tsv": "identity\t0\tunsafe-review " + observed + "\n",
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


if __name__ == "__main__":
    unittest.main()
