"""Probe a known executable; never build, install, or alter a user's PATH.

With --provenance, require a receipt binding the binary to the selected source
SHA/lockfile. Without it, report capability observations only. All fixture and
receipt writes stay in an explicitly supplied, previously absent scratch root.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(root):
    return {str(p.relative_to(root)): sha256(p) for p in root.rglob("*")
            if p.is_file() and ".git" not in p.relative_to(root).parts}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--provenance", type=Path)
    parser.add_argument("--candidate")
    parser.add_argument("--lockfile-sha256")
    parser.add_argument("--expected-version")
    args = parser.parse_args()
    if args.provenance and not all((args.candidate, args.lockfile_sha256, args.expected_version)):
        parser.error("candidate, lockfile-sha256 and expected-version are required with provenance")
    binary = args.binary.resolve(strict=True)
    scratch = args.scratch.resolve()
    if scratch.exists():
        parser.error("scratch must be previously absent; existing work is preserved")
    scratch.mkdir(parents=True)
    rows = []
    receipt = {"schema": "unsafe-review/init-consumer-probe/v1",
               "probe_sha256": sha256(Path(__file__).resolve()),
               "binary": str(binary), "binary_sha256": sha256(binary),
               "requested_candidate": args.candidate,
               "source_binding": "unknown", "rows": rows,
               "cpu_time": "not_measured", "peak_memory": "not_measured",
               "platform": sys.platform,
               "execution_class": "source_prefix" if args.provenance else "capability_observation",
               "claim_boundary": "Capability/fixture observations only unless provenance is verified; no package publication, actual provider, witness, accuracy or safety claim."}

    def run(argv, cwd=scratch, env=None):
        start = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd, env=env,
                                capture_output=True, timeout=120)
        return result, round((time.monotonic() - start) * 1000)

    def row(name, passed, result=None, elapsed=None, detail=""):
        rows.append({"name": name, "result": "pass" if passed else "fail",
                     "command_exit": result.returncode if result else None,
                     "elapsed_ms": elapsed, "detail": detail[:400]})

    def git(root, *argv):
        result, _ = run(["git", "-C", root, *argv])
        if result.returncode:
            raise RuntimeError("fixture Git command failed: " + result.stderr.decode(errors="replace")[:400])
        return result.stdout

    def repo(root):
        (root / "src").mkdir(parents=True)
        (root / "Cargo.toml").write_text('[package]\nname="init-probe"\nversion="0.0.0"\nedition="2024"\n')
        (root / ".gitignore").write_text("target/\n")
        (root / "src/lib.rs").write_text("pub fn base() {}\n")
        (root / "src/inherited.rs").write_text("pub unsafe fn inherited(p:*const u8)->u8 { unsafe { *p } }\n")
        git(root, "init", "-q")
        git(root, "config", "user.name", "init probe fixture")
        git(root, "config", "user.email", "init-probe@example.test")
        git(root, "add", ".")
        git(root, "commit", "-qm", "base")

    failed = False
    try:
        if args.provenance:
            provenance = json.loads(args.provenance.read_text(encoding="utf-8"))
            ok = (provenance.get("candidate_sha") == args.candidate
                  and provenance.get("lockfile_sha256") == args.lockfile_sha256
                  and provenance.get("installed_binary_sha256") == receipt["binary_sha256"]
                  and provenance.get("candidate_version") == args.expected_version
                  and bool(provenance.get("install_method"))
                  and provenance.get("toolchain", "").startswith("rustc 1.98"))
            row("exact-source-binary-provenance", ok)
            if not ok:
                raise RuntimeError("provenance does not bind this binary to the selected source/lockfile/toolchain")
            receipt["source_binding"] = "verified_against_supplied_install_receipt"
            receipt["install_method"] = provenance["install_method"]
        else:
            receipt["limitation"] = "No installation receipt supplied; no source-candidate qualification claim."
        result, elapsed = run([binary, "--version"])
        receipt["observed_version"] = result.stdout.decode(errors="replace").strip()
        row("version-observed", result.returncode == 0, result, elapsed, receipt["observed_version"])
        if args.provenance:
            version_matches = receipt["observed_version"] == "unsafe-review " + args.expected_version
            row("version-matches-candidate-receipt", version_matches)
            if not version_matches:
                raise RuntimeError("executable version contradicts the candidate receipt")
        # A successful global-help fallback must not count as init capability.
        result, elapsed = run([binary, "init", "--help"])
        real_help = result.returncode == 0 and b"unsafe-review init [--root .]" in result.stdout
        row("init-specific-help", real_help, result, elapsed)
        control = scratch / "control"
        (control / "src").mkdir(parents=True)
        (control / "Cargo.toml").write_text('[package]\nname="control"\nversion="0.0.0"\nedition="2024"\n')
        (control / "src/lib.rs").write_text("pub fn safe_control() {}\n")
        before = snapshot(control)
        result, elapsed = run([binary, "init", "--root", control, "--format", "json"])
        try:
            proposal = json.loads(result.stdout)
        except (ValueError, UnicodeError):
            proposal = {}
        real_init = (result.returncode == 0 and proposal.get("schema_version") == "unsafe-review/init/v1"
                     and proposal.get("mode") == "preview_only" and proposal.get("writes_repository") is False)
        row("actual-init-contract", real_init, result, elapsed, result.stderr.decode(errors="replace"))
        row("default-preview-no-writes", before == snapshot(control))
        if not (real_help and real_init):
            raise RuntimeError("actual init capability unavailable; later consumer rows remain not run")
        # Own the control's Git boundary even if the scratch root sits inside a
        # candidate checkout. This prevents inherited parent refs selecting a base.
        git(control, "init", "-q")
        result, elapsed = run([binary, "init", "--root", control, "--format", "json"])
        proposal = json.loads(result.stdout)
        no_base = proposal.get("commands", {}).get("first_pr") is None
        required = proposal.get("commands", {}).get("first_pr_prerequisite", "")
        row("missing-base-explicit-recovery", result.returncode == 0 and no_base
            and "--base" in required and "--diff" in required, result, elapsed)
        envelope = scratch / "proposal-envelope"
        result, elapsed = run([binary, "init", "--root", control, "--format", "json", "--out", envelope])
        output_files = list(envelope.iterdir())
        row("explicit-proposal-envelope-only", result.returncode == 0
            and [p.name for p in output_files] == ["unsafe-review-init.json"]
            and json.loads(output_files[0].read_bytes()) == json.loads(result.stdout), result, elapsed)
        row("explicit-envelope-preserves-root", snapshot(control) == before)

        caller = scratch / "caller"
        target = scratch / "target repo's $literal;name"
        repo(caller)
        repo(target)
        git(target, "update-ref", "refs/remotes/origin/main", "HEAD")
        (target / "src/lib.rs").write_text("pub unsafe fn changed_byte(p:*const u8)->u8 { unsafe { *p } }\n")
        git(target, "add", "src/lib.rs")
        git(target, "commit", "-qm", "changed seam")
        workflow = target / ".github/workflows/unsafe-review-first-pr.yml"
        workflow.parent.mkdir(parents=True)
        workflow.write_text("name: owner-managed\n")
        caller_before, target_before = snapshot(caller), snapshot(target)
        result, elapsed = run([binary, "init", "--root", target, "--format", "json"], cwd=caller)
        proposal = json.loads(result.stdout)
        commands = proposal["commands"]
        row("foreign-root-preview", result.returncode == 0 and proposal["writes_repository"] is False
            and proposal["proposed_files"][0]["status"] == "conflict", result, elapsed)
        row("preview-preserves-caller-and-target", snapshot(caller) == caller_before and snapshot(target) == target_before)
        output = Path(commands["first_pr_artifacts"])
        target_output = output.resolve() == (target / "target/unsafe-review").resolve()
        row("target-local-artifact-destination", target_output)
        if not target_output:
            raise RuntimeError("generated artifact destination escapes the task-owned target")
        env = os.environ.copy()
        env["PATH"] = str(binary.parent) + os.pathsep + env.get("PATH", "")
        row("generated-command-executable-selection", Path(shutil.which("unsafe-review", path=env["PATH"]) or "").resolve() == binary)
        shell = ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command"] if os.name == "nt" else ["sh", "-c"]
        result, elapsed = run([*shell, commands["doctor"]], cwd=caller, env=env)
        doctor_roots = [line[len("workspace root: "):] for line in result.stdout.decode().splitlines()
                        if line.startswith("workspace root: ")]
        row("generated-doctor-usable", result.returncode == 0
            and b"unsafe-review doctor" in result.stdout and len(doctor_roots) == 1
            and Path(doctor_roots[0]).resolve() == target.resolve(), result, elapsed)
        command = commands["first_pr"]
        result, elapsed = run([*shell, command], cwd=caller, env=env)
        first = json.loads((output / "cards.json").read_bytes())
        cards = first["cards"]
        changed_only = bool(cards) and all(c["site"]["file"] == "src/lib.rs" and c["site"]["owner"] == "changed_byte" for c in cards)
        row("generated-command-changed-seam-only", result.returncode == 0 and changed_only, result, elapsed)
        cards_path = output / "cards.json"
        cards_path.rename(scratch / "first-cards-before-repeat.json")
        result, elapsed = run([*shell, command], cwd=scratch, env=env)
        fresh_cards = cards_path.is_file()
        repeated = json.loads(cards_path.read_bytes()) if fresh_cards else {}
        row("repeat-from-third-cwd", result.returncode == 0 and fresh_cards
            and repeated.get("cards") == cards, result, elapsed)
        row("caller-and-third-cwd-no-output", all(not (root / name).exists() for root in (caller, scratch) for name in ("target", "policy", "badges")))
        row("owner-workflow-preserved", workflow.read_text() == "name: owner-managed\n")
        quiet = scratch / "quiet"
        repo(quiet)
        git(quiet, "update-ref", "refs/remotes/origin/main", "HEAD")
        (quiet / "src/lib.rs").write_text("pub fn changed_safe() -> u8 { 1 }\n")
        git(quiet, "add", "src/lib.rs")
        git(quiet, "commit", "-qm", "safe-only change")
        result, _ = run([binary, "init", "--root", quiet, "--format", "json"])
        quiet_proposal = json.loads(result.stdout)
        result, elapsed = run([*shell, quiet_proposal["commands"]["first_pr"]], cwd=caller, env=env)
        quiet_output = Path(quiet_proposal["commands"]["first_pr_artifacts"])
        quiet_cards = json.loads((quiet_output / "cards.json").read_bytes())
        row("safe-only-diff-no-sites-control", result.returncode == 0
            and quiet_cards["cards"] == [], result, elapsed)
    except Exception as error:
        failed = True
        receipt["stop_reason"] = str(error)[:500]
    failed = failed or any(r["result"] == "fail" for r in rows)
    receipt["result"] = "fail" if failed else "pass"
    receipt["qualification_status"] = ("qualified_for_exercised_cases" if not failed and args.provenance
                                       else "not_qualified")
    receipt["materialization_bytes"] = sum(p.stat().st_size for p in scratch.rglob("*") if p.is_file())
    receipt["cleanup"] = "Task-owned fixtures retained for review; user installation and PATH unchanged."
    path = scratch / "receipt.json"
    path.write_text(json.dumps(receipt, indent=2), encoding="utf-8")
    print(json.dumps({"result": receipt["result"], "rows": len(rows), "receipt": str(path),
                      "binary_sha256": receipt["binary_sha256"], "probe_sha256": receipt["probe_sha256"],
                      "source_binding": receipt["source_binding"],
                      "qualification_status": receipt["qualification_status"],
                      "row_results": [{"name": r["name"], "result": r["result"],
                                       "command_exit": r["command_exit"]} for r in rows]}))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
