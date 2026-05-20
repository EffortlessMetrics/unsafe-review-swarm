use serde_json::Value;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn check_artifact_formats_context_and_explain_work_end_to_end() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-e2e")?;

    let json = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let value = parse_json(&stdout_text(&json)?)?;
    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["scope"], "diff");
    assert_eq!(value["summary"]["cards"], 1);
    assert_eq!(value["cards"][0]["class"], "guard_missing");
    assert_eq!(value["cards"][0]["operation_family"], "raw_pointer_read");
    let card_id = json_str(&value["cards"][0]["id"], "cards[0].id")?;

    let root_relative_diff = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        os("change.diff"),
        os("--format"),
        os("json"),
    ])?;
    let root_relative_diff = parse_json(&stdout_text(&root_relative_diff)?)?;
    assert_eq!(root_relative_diff["summary"]["cards"], 1);

    let stdin_diff = fs::read_to_string(fixture.join("change.diff"))?;
    let piped_diff = run_success_with_stdin(
        [
            os("check"),
            os("--root"),
            fixture.as_os_str().to_os_string(),
            os("--diff"),
            os("-"),
            os("--format"),
            os("json"),
        ],
        &stdin_diff,
    )?;
    let piped_diff = parse_json(&stdout_text(&piped_diff)?)?;
    assert_eq!(piped_diff["summary"]["cards"], 1);

    let current_dir_out = temp.path().join("cards.json");
    let out_current_dir = run_success_in_dir(
        [
            os("check"),
            os("--root"),
            fixture.as_os_str().to_os_string(),
            os("--diff"),
            fixture.join("change.diff").into_os_string(),
            os("--format"),
            os("json"),
            os("--out"),
            os("cards.json"),
        ],
        temp.path(),
    )?;
    assert_eq!(stdout_text(&out_current_dir)?.trim(), "");
    assert_eq!(
        parse_json(&fs::read_to_string(&current_dir_out)?)?["summary"]["cards"],
        1
    );

    let summary_path = temp.path().join("nested").join("pr-summary.md");
    let summary = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("pr-summary"),
        os("--out"),
        summary_path.as_os_str().to_os_string(),
    ])?;
    assert_eq!(stdout_text(&summary)?.trim(), "");
    let summary_text = fs::read_to_string(&summary_path)?;
    assert!(summary_text.contains("# unsafe-review PR summary"));
    assert!(summary_text.contains("## Card table"));
    assert!(summary_text.contains("## Trust boundary"));

    let sarif_path = temp.path().join("cards.sarif");
    run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("sarif"),
        os("--out"),
        sarif_path.as_os_str().to_os_string(),
    ])?;
    let sarif = parse_json(&fs::read_to_string(&sarif_path)?)?;
    assert_eq!(sarif["version"], "2.1.0");
    assert!(sarif["runs"][0]["results"].is_array());

    let comment_plan = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("comment-plan"),
    ])?;
    let comment_plan = parse_json(&stdout_text(&comment_plan)?)?;
    assert_eq!(comment_plan["mode"], "plan_only");
    assert_eq!(comment_plan["comments"][0]["card_id"], card_id);
    assert!(
        comment_plan["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );

    let lsp_path = temp.path().join("lsp.json");
    run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("lsp"),
        os("--out"),
        lsp_path.as_os_str().to_os_string(),
    ])?;
    let lsp = parse_json(&fs::read_to_string(&lsp_path)?)?;
    assert_eq!(lsp["mode"], "read_only_projection");
    assert_eq!(lsp["status"]["state"], "actionable");
    assert_eq!(lsp["status"]["cards"], 1);
    assert_eq!(lsp["diagnostics"][0]["card_id"], card_id);
    assert_eq!(lsp["hovers"][0]["card_id"], card_id);
    assert_eq!(
        lsp["code_actions"][0]["command"],
        "unsafe-review.copyAgentPacket"
    );
    assert!(lsp["code_actions"].as_array().is_some_and(|actions| {
        actions
            .iter()
            .any(|action| action["command"] == "unsafe-review.openRelatedTest")
    }));

    let witness_plan_path = temp.path().join("witness-plan.md");
    run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("witness-plan"),
        os("--out"),
        witness_plan_path.as_os_str().to_os_string(),
    ])?;
    let witness_plan = fs::read_to_string(&witness_plan_path)?;
    assert!(witness_plan.contains("# unsafe-review witness plan"));
    assert!(witness_plan.contains("Route: `miri`"));
    assert!(witness_plan.contains("cargo +nightly miri test read_header"));
    assert!(witness_plan.contains("does not run Miri"));

    let context = run_success([
        os("context"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        OsString::from(card_id),
    ])?;
    let packet = parse_json(&stdout_text(&context)?)?;
    assert_eq!(packet["mode"], "bounded_repair_packet");
    assert_eq!(packet["source"], "review_card");
    assert_eq!(packet["policy"], "advisory");
    assert_eq!(packet["card_id"], card_id);
    assert_eq!(packet["card"]["id"], card_id);
    assert_eq!(packet["context"]["operation_family"], "raw_pointer_read");
    assert!(packet["witness_routes"].is_array());
    assert!(packet["do_not_do"].is_array());
    assert!(packet["stop_conditions"].is_array());

    let explain = run_success([
        os("explain"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        OsString::from(card_id),
    ])?;
    let explain = stdout_text(&explain)?;
    assert!(explain.contains("## Required safety conditions"));
    assert!(explain.contains("## Recommended witness routes"));
    assert!(explain.contains("## Trust boundary"));

    Ok(())
}

#[test]
fn check_reports_missing_diff_file_as_cli_failure() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("safe_code_no_cards");
    let missing_diff = fixture.join("missing.diff");

    let output = run_failure([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        missing_diff.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout_text(&output)?.trim(), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsafe-review: read diff"),
        "stderr should identify diff read failure: {stderr}"
    );
    assert!(
        stderr.contains("missing.diff"),
        "stderr should include the missing diff path: {stderr}"
    );

    Ok(())
}

#[test]
fn doctor_reports_first_install_signals_without_running_witnesses() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");

    let output = run_success([
        os("doctor"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
    ])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("unsafe-review doctor"));
    assert!(text.contains("workspace root:"));
    assert!(text.contains("git command:"));
    assert!(text.contains("git repository:"));
    assert!(text.contains("base ref origin/main:"));
    assert!(text.contains("Witness tool signals"));
    assert!(text.contains("miri:"));
    assert!(text.contains("cargo-careful:"));
    assert!(text.contains("sanitizers:"));
    assert!(text.contains("loom:"));
    assert!(text.contains("shuttle:"));
    assert!(text.contains("kani:"));
    assert!(text.contains("crux:"));
    assert!(text.contains("policy: advisory by default"));
    assert!(text.contains("witness execution: not run by doctor or by default"));
    assert!(text.contains("trust boundary: static review evidence"));

    Ok(())
}

#[test]
fn repo_inventory_and_badges_count_open_gaps_without_safety_claim() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-e2e")?;

    let repo = run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let repo = parse_json(&stdout_text(&repo)?)?;
    assert_eq!(repo["schema_version"], "0.1");
    assert_eq!(repo["tool"], "unsafe-review");
    assert_eq!(repo["scope"], "repo");
    assert_eq!(repo["mode"], "repo");
    assert_eq!(repo["policy"], "advisory");
    assert!(
        repo["root"]
            .as_str()
            .unwrap_or("")
            .ends_with("fixtures/raw_pointer_alignment")
    );
    let summary = &repo["summary"];
    for key in [
        "rust_files",
        "changed_rust_files",
        "unsafe_sites",
        "cards",
        "open_actionable_gaps",
        "contract_missing",
        "guard_missing",
        "guarded_unwitnessed",
        "unsafe_unreached",
        "requires_loom",
        "miri_unsupported",
        "static_unknown",
    ] {
        assert!(summary.get(key).is_some(), "repo summary missing `{key}`");
    }
    assert_eq!(summary["cards"], 1);
    assert_eq!(summary["open_actionable_gaps"], 1);
    assert_eq!(summary["guard_missing"], 1);
    let card = &repo["cards"][0];
    for key in [
        "id",
        "class",
        "priority",
        "confidence",
        "site",
        "operation_family",
        "hazards",
        "obligations",
        "obligation_evidence",
        "contract",
        "discharge",
        "reach",
        "witness",
        "missing",
        "verify_commands",
    ] {
        assert!(card.get(key).is_some(), "repo card missing `{key}`");
    }
    assert_eq!(card["operation_family"], "raw_pointer_read");
    assert!(
        repo["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );

    let badge_dir = temp.path().join("badges");
    let badges = run_success([
        os("badges"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--out"),
        badge_dir.as_os_str().to_os_string(),
    ])?;
    assert!(stdout_text(&badges)?.contains("wrote badges"));

    let main_badge = parse_json(&fs::read_to_string(badge_dir.join("unsafe-review.json"))?)?;
    assert_eq!(main_badge["label"], "unsafe-review");
    assert_eq!(main_badge["message"], "1 open gaps");
    assert_ne!(main_badge["message"], "safe");

    let plus_badge = parse_json(&fs::read_to_string(
        badge_dir.join("unsafe-review-plus.json"),
    )?)?;
    assert_eq!(plus_badge["label"], "unsafe-review+");
    assert_eq!(plus_badge["message"], "0 contract / 1 guard / 0 witness");
    assert_ne!(plus_badge["message"], "UB-free");

    let repo_markdown = run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let repo_markdown = stdout_text(&repo_markdown)?;
    assert!(repo_markdown.contains("# unsafe-review repo posture"));
    assert!(repo_markdown.contains("## Top classes"));
    assert!(repo_markdown.contains("| `guard_missing` | 1 |"));
    assert!(repo_markdown.contains("## Top operation families"));
    assert!(repo_markdown.contains("| `raw_pointer_read` | 1 |"));
    assert!(repo_markdown.contains("## Trust boundary"));
    assert!(repo_markdown.contains("not raw unsafe usage"));
    assert!(repo_markdown.contains("not UB-free status"));

    Ok(())
}

#[test]
fn repo_badges_follow_multicard_review_card_summary() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("attributed_unsafe_fn_no_duplicate");
    let temp = TempDir::new("unsafe-review-repo-multicard-e2e")?;

    let repo = run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let repo = parse_json(&stdout_text(&repo)?)?;
    assert_eq!(repo["summary"]["cards"], 2);
    assert_eq!(repo["summary"]["open_actionable_gaps"], 2);
    assert_eq!(repo["summary"]["contract_missing"], 2);
    assert_eq!(repo["summary"]["guard_missing"], 0);
    assert_eq!(repo["summary"]["guarded_unwitnessed"], 0);
    assert_eq!(repo["cards"][0]["class"], "contract_missing");
    assert_eq!(repo["cards"][1]["class"], "contract_missing");
    assert_eq!(repo["cards"][0]["operation_family"], "unknown");
    assert_eq!(repo["cards"][1]["operation_family"], "raw_pointer_write");

    let badge_dir = temp.path().join("badges");
    let badges = run_success([
        os("badges"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--out"),
        badge_dir.as_os_str().to_os_string(),
    ])?;
    assert!(stdout_text(&badges)?.contains("wrote badges"));

    let main_badge = parse_json(&fs::read_to_string(badge_dir.join("unsafe-review.json"))?)?;
    assert_eq!(main_badge["message"], "2 open gaps");
    assert_ne!(main_badge["message"], "safe");

    let plus_badge = parse_json(&fs::read_to_string(
        badge_dir.join("unsafe-review-plus.json"),
    )?)?;
    assert_eq!(plus_badge["message"], "2 contract / 0 guard / 0 witness");
    assert_ne!(plus_badge["message"], "UB-free");

    let repo_markdown = run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let repo_markdown = stdout_text(&repo_markdown)?;
    assert!(repo_markdown.contains("| 2 | 2 | 2 | 0 | 0 | 0 | 0 | 0 |"));
    assert!(repo_markdown.contains("| `contract_missing` | 2 |"));
    assert!(repo_markdown.contains("| `unknown` | 1 |"));
    assert!(repo_markdown.contains("| `raw_pointer_write` | 1 |"));
    assert!(repo_markdown.contains("not raw unsafe usage"));
    assert!(repo_markdown.contains("not UB-free status"));

    Ok(())
}

#[test]
fn safe_repo_human_output_stays_quiet() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("safe_code_no_cards");

    let output = run_success([os("repo"), os("--root"), fixture.as_os_str().to_os_string()])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("cards: 0, open gaps: 0"));
    assert!(text.contains("No unsafe-review cards found."));

    Ok(())
}

#[test]
fn outcome_compares_existing_json_snapshots_without_safety_claim() -> Result<(), Box<dyn Error>> {
    let before_fixture = fixture_root("safe_code_no_cards");
    let after_fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-outcome-e2e")?;
    let before_path = temp.path().join("before.json");
    let after_path = temp.path().join("after.json");
    let outcome_path = temp.path().join("outcome.json");

    run_success([
        os("check"),
        os("--root"),
        before_fixture.as_os_str().to_os_string(),
        os("--diff"),
        before_fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        before_path.as_os_str().to_os_string(),
    ])?;
    run_success([
        os("check"),
        os("--root"),
        after_fixture.as_os_str().to_os_string(),
        os("--diff"),
        after_fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        after_path.as_os_str().to_os_string(),
    ])?;

    let output = run_success([
        os("outcome"),
        os("--before"),
        before_path.as_os_str().to_os_string(),
        os("--after"),
        after_path.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        outcome_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let outcome = parse_json(&fs::read_to_string(&outcome_path)?)?;
    assert_eq!(outcome["schema_version"], "0.1");
    assert_eq!(outcome["mode"], "outcome");
    assert_eq!(outcome["before"]["cards"], 0);
    assert_eq!(outcome["after"]["cards"], 1);
    assert_eq!(outcome["summary"]["new"], 1);
    assert_eq!(outcome["summary"]["resolved"], 0);
    assert!(outcome["cards"]["new"][0]["card_id"].is_string());
    assert!(
        outcome["cards"]["new"][0]["reason"]
            .as_str()
            .unwrap_or("")
            .contains("after snapshot")
    );
    assert!(
        outcome["before_id"]
            .as_str()
            .unwrap_or("")
            .starts_with("snapshot-")
    );
    assert!(
        outcome["after_id"]
            .as_str()
            .unwrap_or("")
            .starts_with("snapshot-")
    );
    assert!(outcome["limitations"].is_array());
    assert!(
        outcome["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not memory-safety proof")
    );

    let markdown = run_success([
        os("outcome"),
        os("--before"),
        before_path.as_os_str().to_os_string(),
        os("--after"),
        after_path.as_os_str().to_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let markdown = stdout_text(&markdown)?;
    assert!(markdown.contains("# unsafe-review outcome"));
    assert!(markdown.contains("| Status | Card | Reason | Before | After |"));
    assert!(markdown.contains("## Limitations"));
    assert!(markdown.contains("## Trust boundary"));
    assert!(markdown.contains("| 1 | 0 | 0 | 0 | 0 |"));

    Ok(())
}

#[test]
fn check_json_imports_witness_receipts_without_hiding_guard_gaps() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");

    let json = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let value = parse_json(&stdout_text(&json)?)?;
    let card = &value["cards"][0];

    assert_eq!(value["summary"]["cards"], 1);
    assert_eq!(card["class"], "guard_missing");
    assert!(
        card["witness"]
            .as_str()
            .unwrap_or("")
            .contains("Imported miri receipt")
    );
    assert!(
        card["witness"]
            .as_str()
            .unwrap_or("")
            .contains("expires_at: 2026-08-18")
    );
    let missing = card["missing"]
        .as_array()
        .ok_or("card missing field should be an array")?;
    assert!(missing.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("Missing visible local guard")
    }));
    assert!(!missing.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("No witness receipt imported")
    }));
    let obligations = card["obligation_evidence"]
        .as_array()
        .ok_or("obligation_evidence should be an array")?;
    assert!(
        obligations
            .iter()
            .all(|evidence| evidence["witness"]["present"] == true)
    );
    assert!(obligations.iter().any(|evidence| {
        evidence["key"] == "alignment" && evidence["discharge"]["present"] == false
    }));

    Ok(())
}

#[test]
fn receipt_template_writes_valid_receipt_json_without_running_witnesses()
-> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-receipt-template-e2e")?;
    let receipt_path = temp.path().join("miri.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("template"),
        os(card_id),
        os("--tool"),
        os("miri"),
        os("--strength"),
        os("ran"),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--summary"),
        os("focused witness passed"),
        os("--command"),
        os("cargo +nightly miri test read_header"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "miri");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(receipt["author"], "core/fixtures");
    assert_eq!(receipt["recorded_at"], "2026-05-18T00:00:00Z");
    assert_eq!(receipt["expires_at"], "2026-08-18");
    assert_eq!(receipt["summary"], "focused witness passed");
    assert_eq!(receipt["command"], "cargo +nightly miri test read_header");
    assert_eq!(receipt["limitations"][0], "fixture only");

    Ok(())
}

#[test]
fn receipt_validate_counts_importable_receipts() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");

    let output = run_success([
        os("receipt"),
        os("validate"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "witness receipts: 1 valid");
    Ok(())
}

#[test]
fn receipt_audit_reports_matching_saved_receipts_without_running_witnesses()
-> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");
    let temp = TempDir::new("unsafe-review-receipt-audit-e2e")?;
    let audit_path = temp.path().join("receipt-audit.md");

    let json = run_success([
        os("receipt"),
        os("audit"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let value = parse_json(&stdout_text(&json)?)?;

    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["mode"], "receipt-audit");
    assert_eq!(value["policy"], "advisory");
    assert_eq!(value["summary"]["receipts"], 1);
    assert_eq!(value["summary"]["matched"], 1);
    assert_eq!(value["summary"]["unmatched"], 0);
    assert_eq!(value["summary"]["duplicate"], 0);
    assert!(
        value["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("does not execute witnesses")
    );
    let receipt = &value["receipts"][0];
    assert_eq!(receipt["receipt_tool"], "miri");
    assert!(
        receipt["statuses"]
            .as_array()
            .ok_or("statuses should be an array")?
            .iter()
            .any(|status| status == "matched")
    );
    assert_eq!(receipt["matched_card"]["class"], "guard_missing");

    let markdown = run_success([
        os("receipt"),
        os("audit"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("markdown"),
        os("--out"),
        audit_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&markdown)?.trim(), "");
    let markdown = fs::read_to_string(audit_path)?;
    assert!(markdown.contains("# unsafe-review receipt audit"));
    assert!(markdown.contains("Duplicate"));
    assert!(markdown.contains("does not execute witnesses"));
    assert!(markdown.contains("| 1 | 1 | 0 | 0 | 0 |"));
    Ok(())
}

#[test]
fn receipt_import_miri_writes_receipt_from_saved_success_log() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");
    let temp = TempDir::new("unsafe-review-miri-receipt-e2e")?;
    let receipt_path = temp.path().join("miri.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("import-miri"),
        os(card_id),
        os("--log"),
        fixture.join("miri.success.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("cargo +nightly miri test read_header"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "miri");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(
        receipt["summary"],
        "saved Miri output reported `test result: ok`"
    );
    assert_eq!(receipt["command"], "cargo +nightly miri test read_header");
    let limitations = receipt["limitations"]
        .as_array()
        .ok_or("receipt limitations should be an array")?;
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("unsafe-review did not run Miri")
    }));
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("site reach is not claimed")
    }));
    assert!(limitations.iter().any(|item| item == "fixture only"));

    Ok(())
}

#[test]
fn receipt_import_careful_writes_receipt_from_saved_success_log() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");
    let temp = TempDir::new("unsafe-review-careful-receipt-e2e")?;
    let receipt_path = temp.path().join("careful.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("import-careful"),
        os(card_id),
        os("--log"),
        fixture.join("careful.success.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("cargo +nightly careful test read_header"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "cargo-careful");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(
        receipt["summary"],
        "saved cargo-careful output reported `test result: ok`"
    );
    assert_eq!(
        receipt["command"],
        "cargo +nightly careful test read_header"
    );
    let limitations = receipt["limitations"]
        .as_array()
        .ok_or("receipt limitations should be an array")?;
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("unsafe-review did not run cargo-careful")
    }));
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("site reach is not claimed")
    }));
    assert!(limitations.iter().any(|item| item == "fixture only"));

    Ok(())
}

#[test]
fn receipt_import_sanitizer_writes_receipt_from_saved_success_log() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");
    let temp = TempDir::new("unsafe-review-sanitizer-receipt-e2e")?;
    let receipt_path = temp.path().join("asan.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("import-sanitizer"),
        os(card_id),
        os("--tool"),
        os("asan"),
        os("--log"),
        fixture.join("asan.success.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "asan");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(
        receipt["summary"],
        "saved asan output reported `test result: ok`"
    );
    assert_eq!(
        receipt["command"],
        "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header"
    );
    let limitations = receipt["limitations"]
        .as_array()
        .ok_or("receipt limitations should be an array")?;
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("unsafe-review did not run a sanitizer")
    }));
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("site reach is not claimed")
    }));
    assert!(limitations.iter().any(|item| item == "fixture only"));

    Ok(())
}

#[test]
fn receipt_import_concurrency_writes_receipt_from_saved_success_log() -> Result<(), Box<dyn Error>>
{
    let fixture = fixture_root("unsafe_impl_send");
    let temp = TempDir::new("unsafe-review-concurrency-receipt-e2e")?;
    let receipt_path = temp.path().join("loom.json");
    let card_id = "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1";

    let output = run_success([
        os("receipt"),
        os("import-concurrency"),
        os(card_id),
        os("--tool"),
        os("loom"),
        os("--log"),
        fixture.join("loom.success.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("cargo test shared_cell_loom -- --nocapture"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "loom");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(
        receipt["summary"],
        "saved loom output reported `test result: ok`"
    );
    assert_eq!(
        receipt["command"],
        "cargo test shared_cell_loom -- --nocapture"
    );
    let limitations = receipt["limitations"]
        .as_array()
        .ok_or("receipt limitations should be an array")?;
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("unsafe-review did not run a concurrency witness")
    }));
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("site reach is not claimed")
    }));
    assert!(limitations.iter().any(|item| item == "fixture only"));

    Ok(())
}

#[test]
fn receipt_import_proof_writes_receipt_from_saved_success_log() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("transmute_invalid_value");
    let temp = TempDir::new("unsafe-review-proof-receipt-e2e")?;
    let receipt_path = temp.path().join("kani.json");
    let card_id = "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1";

    let output = run_success([
        os("receipt"),
        os("import-proof"),
        os(card_id),
        os("--tool"),
        os("kani"),
        os("--log"),
        fixture.join("kani.success.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("cargo kani --harness byte_to_bool_harness"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "kani");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(
        receipt["summary"],
        "saved kani proof output reported verification success"
    );
    assert_eq!(
        receipt["command"],
        "cargo kani --harness byte_to_bool_harness"
    );
    let limitations = receipt["limitations"]
        .as_array()
        .ok_or("receipt limitations should be an array")?;
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("unsafe-review did not run a proof tool")
    }));
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("site reach is not claimed")
    }));
    assert!(limitations.iter().any(|item| {
        item.as_str()
            .unwrap_or("")
            .contains("recorded harness/output")
    }));
    assert!(limitations.iter().any(|item| item == "fixture only"));

    Ok(())
}

#[test]
fn no_new_debt_policy_fails_only_for_unbaselined_actionable_gaps() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let failing = run_failure([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
        os("--policy"),
        os("no-new-debt"),
    ])?;
    let failing_json = parse_json(&stdout_text(&failing)?)?;
    assert_eq!(failing_json["policy"], "no-new-debt");
    assert_eq!(failing_json["summary"]["open_actionable_gaps"], 1);
    assert!(
        String::from_utf8(failing.stderr)?
            .contains("no-new-debt policy found 1 open actionable gap(s)")
    );

    let temp = TempDir::new("unsafe-review-no-new-debt-e2e")?;
    let copied = temp.path().join("fixture");
    copy_dir_all(&fixture, &copied)?;
    let advisory = run_success([
        os("check"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    let card_id = json_str(&advisory["cards"][0]["id"], "cards[0].id")?;
    write_baseline(&copied, card_id)?;

    let passing = run_success([
        os("check"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
        os("--policy"),
        os("no-new-debt"),
    ])?;
    let passing = parse_json(&stdout_text(&passing)?)?;
    assert_eq!(passing["policy"], "no-new-debt");
    assert_eq!(passing["summary"]["open_actionable_gaps"], 0);
    assert_eq!(passing["cards"][0]["class"], "baseline_known");

    Ok(())
}

#[test]
fn suppression_policy_suppresses_only_exact_review_card_identity() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-suppression-e2e")?;
    let copied = temp.path().join("fixture");
    copy_dir_all(&fixture, &copied)?;

    let advisory = run_success([
        os("check"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    let card_id = json_str(&advisory["cards"][0]["id"], "cards[0].id")?;
    write_suppression(&copied, card_id)?;

    let passing = run_success([
        os("check"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
        os("--policy"),
        os("no-new-debt"),
    ])?;
    let passing = parse_json(&stdout_text(&passing)?)?;
    assert_eq!(passing["policy"], "no-new-debt");
    assert_eq!(passing["summary"]["open_actionable_gaps"], 0);
    assert_eq!(passing["summary"]["guard_missing"], 0);
    assert_eq!(passing["cards"][0]["id"], card_id);
    assert_eq!(passing["cards"][0]["class"], "suppressed");
    assert_eq!(passing["cards"][0]["priority"], "low");

    let report = run_success([
        os("policy"),
        os("report"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let report = parse_json(&stdout_text(&report)?)?;
    assert_eq!(report["summary"]["new_gaps"], 0);
    assert_eq!(report["summary"]["suppressed"], 1);
    assert_eq!(report["cards"][0]["card_id"], card_id);
    assert_eq!(report["cards"][0]["policy_status"], "suppressed");
    assert!(
        json_str(&report["trust_boundary"], "trust_boundary")?
            .contains("does not enforce blocking policy")
    );

    let markdown = run_success([
        os("policy"),
        os("report"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let markdown = stdout_text(&markdown)?;
    assert!(markdown.contains("| 1 | 0 | 0 | 1 | 0 | 0 |"));
    assert!(markdown.contains("`suppressed`"));
    assert!(markdown.contains("## Trust boundary"));

    Ok(())
}

#[test]
fn policy_report_is_advisory_and_counts_baseline_state() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let report = run_success([
        os("policy"),
        os("report"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let report = parse_json(&stdout_text(&report)?)?;
    assert_eq!(report["mode"], "policy-report");
    assert_eq!(report["policy"], "advisory");
    assert_eq!(report["summary"]["new_gaps"], 1);
    assert_eq!(report["summary"]["baseline_known"], 0);
    assert!(
        json_str(&report["trust_boundary"], "trust_boundary")?
            .contains("does not enforce blocking policy")
    );

    let temp = TempDir::new("unsafe-review-policy-report-e2e")?;
    let copied = temp.path().join("fixture");
    copy_dir_all(&fixture, &copied)?;
    let advisory = run_success([
        os("check"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    let card_id = json_str(&advisory["cards"][0]["id"], "cards[0].id")?;
    write_baseline(&copied, card_id)?;

    let baselined = run_success([
        os("policy"),
        os("report"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let baselined = parse_json(&stdout_text(&baselined)?)?;
    assert_eq!(baselined["summary"]["new_gaps"], 0);
    assert_eq!(baselined["summary"]["baseline_known"], 1);

    let markdown_path = temp.path().join("policy-report.md");
    let markdown = run_success([
        os("policy"),
        os("report"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("markdown"),
        os("--out"),
        markdown_path.as_os_str().to_os_string(),
    ])?;
    assert_eq!(stdout_text(&markdown)?.trim(), "");
    let markdown = fs::read_to_string(markdown_path)?;
    assert!(markdown.contains("# unsafe-review policy report"));
    assert!(markdown.contains("## Trust boundary"));

    Ok(())
}

fn run_success<I, S>(args: I) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    checked_output(Command::new(env!("CARGO_BIN_EXE_unsafe-review")).args(args))
}

fn run_failure<I, S>(args: I) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
        .args(args)
        .output()?;
    if output.status.success() {
        return Err(format!(
            "expected command to fail\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn run_success_in_dir<I, S>(args: I, current_dir: &Path) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    checked_output(
        Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
            .current_dir(current_dir)
            .args(args),
    )
}

fn run_success_with_stdin<I, S>(args: I, stdin: &str) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let Some(mut child_stdin) = child.stdin.take() else {
        return Err("failed to open child stdin".into());
    };
    child_stdin.write_all(stdin.as_bytes())?;
    drop(child_stdin);
    checked_completed_output(child.wait_with_output()?)
}

fn checked_output(command: &mut Command) -> Result<Output, Box<dyn Error>> {
    checked_completed_output(command.output()?)
}

fn checked_completed_output(output: Output) -> Result<Output, Box<dyn Error>> {
    if output.status.success() {
        return Ok(output);
    }
    Err(format!(
        "command failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn stdout_text(output: &Output) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(output.stdout.clone())?)
}

fn parse_json(text: &str) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(text)?)
}

fn json_str<'a>(value: &'a Value, path: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .as_str()
        .ok_or_else(|| format!("{path} should be a string").into())
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn os(value: &str) -> OsString {
    OsString::from(value)
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn write_baseline(root: &Path, card_id: &str) -> Result<(), Box<dyn Error>> {
    let policy = root.join("policy");
    fs::create_dir_all(&policy)?;
    fs::write(
        policy.join("unsafe-review-baseline.toml"),
        format!(
            r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{card_id}"
owner = "core/policy"
reason = "e2e no-new-debt baseline"
evidence = "fixture card"
review_after = "2026-08-01"
"#
        ),
    )?;
    Ok(())
}

fn write_suppression(root: &Path, card_id: &str) -> Result<(), Box<dyn Error>> {
    let policy = root.join("policy");
    fs::create_dir_all(&policy)?;
    fs::write(
        policy.join("unsafe-review-suppressions.toml"),
        format!(
            r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{card_id}"
owner = "core/policy"
reason = "e2e exact suppression"
evidence = "fixture card"
expires = "2026-08-01"
"#
        ),
    )?;
    Ok(())
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
