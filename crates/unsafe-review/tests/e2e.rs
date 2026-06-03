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
    assert_eq!(value["summary"]["changed_files"], 1);
    assert_eq!(value["summary"]["changed_non_rust_files"], 0);
    assert_eq!(value["summary"]["cards"], 1);
    assert_eq!(value["cards"][0]["class"], "guard_missing");
    assert_eq!(
        value["cards"][0]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(value["cards"][0]["operation_family"], "raw_pointer_read");
    let card_id = json_str(&value["cards"][0]["id"], "cards[0].id")?;

    let human = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
    ])?;
    let human = stdout_text(&human)?;
    assert!(human.contains("operation: unsafe { ptr.cast::<Header>().read() }"));
    assert!(human.contains("operation_family: raw_pointer_read"));
    assert!(human.contains("next: Add or expose"));

    let markdown = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let markdown = stdout_text(&markdown)?;
    assert!(
        markdown.contains("| ID | Class | Operation | Hazard | Missing | Route | Next action |")
    );
    assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(markdown.contains("Add or expose the local guard"));

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
    assert!(summary_text.contains("- Operation: `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(summary_text.contains("- Operation family: `raw_pointer_read`"));
    assert!(summary_text.contains("| ID | Class | Location | Operation family | Operation |"));
    assert!(summary_text.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(summary_text.contains("| `raw_pointer_read` |"));
    assert!(summary_text.contains("## Trust boundary"));

    let github_summary_path = temp.path().join("nested").join("github-summary.md");
    let github_summary = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("github-summary"),
        os("--out"),
        github_summary_path.as_os_str().to_os_string(),
    ])?;
    assert_eq!(stdout_text(&github_summary)?.trim(), "");
    let github_summary_text = fs::read_to_string(&github_summary_path)?;
    assert!(github_summary_text.contains("## unsafe-review advisory summary"));
    assert!(github_summary_text.contains("## Top card"));
    assert!(github_summary_text.contains(&format!("- ID: `{card_id}`")));
    assert!(github_summary_text.contains(&format!("- Explain: `unsafe-review explain {card_id}`")));
    assert!(github_summary_text.contains(&format!(
        "- Agent context: `unsafe-review context {card_id} --json`"
    )));
    assert!(github_summary_text.contains("## Open next"));
    assert!(github_summary_text.contains("Full reviewer cockpit: `pr-summary.md`"));
    assert!(github_summary_text.contains("not site-execution proof"));
    assert!(github_summary_text.contains("unsafe-review did not run witnesses"));
    assert!(github_summary_text.contains("post comments"));
    assert!(github_summary_text.contains("edit source"));
    assert!(github_summary_text.contains("enforce blocking policy"));
    assert!(!github_summary_text.contains("# unsafe-review PR summary"));
    assert!(!github_summary_text.contains("## Card table"));
    assert!(!github_summary_text.contains("## Witness plan"));

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
    assert_eq!(
        sarif["runs"][0]["results"][0]["properties"]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["properties"]["witnessRouteDetails"][0]["kind"],
        "miri"
    );
    assert!(
        sarif["runs"][0]["results"][0]["properties"]["verifyCommands"][0]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );

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
    assert_eq!(
        comment_plan["comments"][0]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(
        comment_plan["comments"][0]["witness_routes"][0]["kind"],
        "miri"
    );
    assert!(
        comment_plan["comments"][0]["verify_commands"][0]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );
    assert!(
        comment_plan["comments"][0]["body"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe { ptr.cast::<Header>().read() }")
    );
    assert!(
        comment_plan["comments"][0]["body"]
            .as_str()
            .unwrap_or("")
            .contains("Verify command: `cargo +nightly miri test read_header`")
    );
    assert!(
        comment_plan["comments"][0]["body"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review did not post this comment")
    );
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
    assert_eq!(
        lsp["diagnostics"][0]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(
        lsp["diagnostics"][0]["required_safety_conditions"][0]["key"],
        "pointer-live"
    );
    assert!(
        lsp["diagnostics"][0]["required_safety_conditions"][0]["description"]
            .as_str()
            .unwrap_or("")
            .contains("pointer is live")
    );
    assert_eq!(
        lsp["diagnostics"][0]["evidence_summary"]["contract"]["state"],
        "present"
    );
    assert_eq!(
        lsp["diagnostics"][0]["evidence_summary"]["discharge"]["state"],
        "missing"
    );
    assert!(
        lsp["diagnostics"][0]["evidence_summary"]["reach_limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not proof")
    );
    assert!(
        lsp["diagnostics"][0]["obligation_evidence"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item["key"] == "alignment"
                        && item["discharge"]["state"] == "missing"
                        && item["witness"]["state"] == "missing"
                })
            })
    );
    assert!(
        lsp["diagnostics"][0]["next_action"]
            .as_str()
            .unwrap_or("")
            .contains("Add or expose the local guard")
    );
    assert_eq!(lsp["diagnostics"][0]["witness_routes"][0]["kind"], "miri");
    assert!(
        lsp["diagnostics"][0]["verify_commands"][0]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );
    assert_eq!(lsp["hovers"][0]["card_id"], card_id);
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Operation: `unsafe { ptr.cast::<Header>().read() }`")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Evidence found")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Contract [present]")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Guard/discharge [missing]")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Missing visible local guard")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("What would resolve this")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("What would not resolve this")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );
    assert!(
        lsp["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("does not prove the unsafe site executed")
    );
    assert_eq!(
        lsp["code_actions"][0]["command"],
        "unsafe-review.copyAgentPacket"
    );
    assert_eq!(
        lsp["code_actions"][0]["payload"]["kind"],
        "unsafe-review.agent_packet"
    );
    assert_eq!(
        lsp["code_actions"][0]["payload"]["card_id"]
            .as_str()
            .unwrap_or(""),
        card_id
    );
    assert!(lsp["code_actions"][0]["arguments"].is_array());
    assert!(lsp["code_actions"].as_array().is_some_and(|actions| {
        actions
            .iter()
            .any(|action| action["command"] == "unsafe-review.openRelatedTest")
    }));
    assert!(lsp["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["command"] == "unsafe-review.openRelatedTest"
                && action["payload"]["kind"] == "unsafe-review.related_test"
                && action["payload"]["card_id"].as_str() == Some(card_id)
                && action["payload"]["file"] == "src/lib.rs"
                && action["payload"]["line"] == 3
                && action["payload"]["name"] == "read_header"
        })
    }));
    assert!(lsp["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["command"] == "unsafe-review.copyWitnessCommand"
                && action["title"] == "Copy witness command (does not run)"
                && action["payload"]["kind"] == "unsafe-review.witness_command"
                && action["payload"]["card_id"].as_str() == Some(card_id)
                && action["payload"]["command"]
                    .as_str()
                    .unwrap_or("")
                    .contains("cargo +nightly miri test read_header")
        })
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
    assert!(witness_plan.contains("## Route groups"));
    assert!(witness_plan.contains("### Miri / cargo-careful"));
    assert!(witness_plan.contains("Operation: `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(witness_plan.contains("Route: `miri`"));
    assert!(witness_plan.contains("Next action: Add or expose"));
    assert!(witness_plan.contains("Verify command"));
    assert!(witness_plan.contains("cargo +nightly miri test read_header"));
    assert!(witness_plan.contains("What it can show"));
    assert!(witness_plan.contains("What it cannot prove"));
    assert!(witness_plan.contains("unsafe-review receipt import-miri"));
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
    assert_eq!(
        packet["context"]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(packet["context"]["operation_family"], "raw_pointer_read");
    assert_eq!(
        packet["source_context"]["unsafe_site"]["file"],
        "src/lib.rs"
    );
    assert_eq!(
        packet["source_context"]["unsafe_site"]["snippet"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert!(
        packet["source_context"]["nearby_safety_contract"]["summary"]
            .as_str()
            .unwrap_or("")
            .contains("SAFETY")
    );
    assert_eq!(
        packet["source_context"]["nearby_guard_evidence"][0]["key"],
        "bounds"
    );
    assert_eq!(
        packet["source_context"]["related_tests"][0]["name"],
        "read_header"
    );
    assert!(packet["witness_routes"].is_array());
    assert_eq!(packet["agent_readiness"]["ready"], true);
    assert_eq!(packet["agent_readiness"]["state"], "ready_for_agent");
    let allowed_repairs = serde_json::to_string(&packet["allowed_repairs"])?;
    assert!(allowed_repairs.contains("alignment guard"));
    assert!(allowed_repairs.contains("witness receipt"));
    let allowed_repairs_lower = allowed_repairs.to_ascii_lowercase();
    assert!(!allowed_repairs_lower.contains("suppress"));
    assert!(!allowed_repairs_lower.contains("suppression"));
    let repair_queue = serde_json::to_string(&packet["repair_queue"])?;
    assert!(repair_queue.contains("repairable_by_guard"));
    assert!(repair_queue.contains("requires_witness_receipt"));
    assert!(!repair_queue.contains("requires_human_review"));
    assert!(
        packet["verify_commands"][0]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );
    assert!(packet["do_not_do"].is_array());
    assert!(serde_json::to_string(&packet["do_not_do"])?.contains("do not suppress this card"));
    assert!(
        serde_json::to_string(&packet["do_not_do"])?
            .contains("do not change unrelated unsafe code")
    );
    assert!(serde_json::to_string(&packet["do_not_do"])?.contains("ran witnesses"));
    assert!(packet["stop_conditions"].is_array());
    assert!(serde_json::to_string(&packet["stop_conditions"])?.contains("same unsafe seam"));

    let explain = run_success([
        os("explain"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        OsString::from(card_id),
    ])?;
    let explain = stdout_text(&explain)?;
    assert!(explain.contains("## Why this card exists"));
    assert!(explain.contains("## Required safety conditions"));
    assert!(explain.contains("## Evidence found"));
    assert!(explain.contains("## Evidence missing"));
    assert!(explain.contains("**Operation:** `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(explain.contains("**Operation family:** `raw_pointer_read`"));
    assert!(explain.contains("cargo +nightly miri test read_header"));
    assert!(explain.contains(
        "- `cargo-careful`: cargo-careful is a cheaper compatibility-oriented runtime check"
    ));
    assert!(explain.contains("cargo +nightly careful test read_header"));
    assert!(explain.contains(
        "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
    ));
    assert!(explain.contains("## What would resolve this"));
    assert!(explain.contains(
        "- Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
    ));
    assert!(explain.contains("Then attach a matching witness receipt only after running"));
    assert!(explain.contains("## What would not resolve this"));
    assert!(
        explain.contains("A `SAFETY:` comment alone does not discharge missing guard evidence.")
    );
    assert!(
        explain.contains("A related test mention is not proof that this unsafe site executed.")
    );
    assert!(explain.contains("Do not claim witness proof unless a matching receipt exists."));
    assert!(explain.contains("## Witness route"));
    assert!(explain.contains("## Trust boundary"));
    assert!(explain.contains("not UB-free status"));

    Ok(())
}

#[test]
fn check_json_summary_reports_mixed_language_diff_scope() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-mixed-diff-e2e")?;
    let mixed_diff_path = temp.path().join("mixed.diff");
    let mut mixed_diff = fs::read_to_string(fixture.join("change.diff"))?;
    mixed_diff.push_str(
        r#"diff --git a/src/js/buffer.ts b/src/js/buffer.ts
--- a/src/js/buffer.ts
+++ b/src/js/buffer.ts
@@ -1,0 +1,1 @@
+export const changed = true;
diff --git a/src/binding.cpp b/src/binding.cpp
--- a/src/binding.cpp
+++ b/src/binding.cpp
@@ -1,0 +1,1 @@
+void changed() {}
"#,
    );
    fs::write(&mixed_diff_path, mixed_diff)?;

    let output = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        mixed_diff_path.into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let value = parse_json(&stdout_text(&output)?)?;
    let summary = &value["summary"];

    assert_eq!(summary["changed_files"], 3);
    assert_eq!(summary["changed_rust_files"], 1);
    assert_eq!(summary["changed_non_rust_files"], 2);
    assert_eq!(summary["cards"], 1);
    assert_eq!(value["cards"][0]["site"]["file"], "src/lib.rs");

    Ok(())
}

#[test]
fn context_packet_queues_contract_gaps_for_public_safety_docs() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("public_unsafe_fn_missing_safety");

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
    assert_eq!(value["summary"]["cards"], 1);
    assert_eq!(value["cards"][0]["class"], "contract_missing");
    let card_id = json_str(&value["cards"][0]["id"], "cards[0].id")?;

    let context = run_success([
        os("context"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        OsString::from(card_id),
    ])?;
    let packet = parse_json(&stdout_text(&context)?)?;
    assert_eq!(packet["mode"], "bounded_repair_packet");
    assert_eq!(packet["source"], "review_card");
    assert_eq!(packet["card_id"], card_id);
    assert_eq!(packet["card"]["class"], "contract_missing");
    assert_eq!(packet["context"]["operation_family"], "unknown");

    let allowed_repairs = serde_json::to_string(&packet["allowed_repairs"])?;
    assert!(allowed_repairs.contains("safety contract"));
    let repair_queue = serde_json::to_string(&packet["repair_queue"])?;
    assert!(repair_queue.contains("repairable_by_safety_docs"));
    assert!(repair_queue.contains("repairable_by_test"));
    assert!(repair_queue.contains("requires_witness_receipt"));
    assert!(repair_queue.contains("requires_human_review"));
    assert!(repair_queue.contains("do_not_auto_repair"));
    assert_eq!(packet["agent_readiness"]["ready"], false);
    assert_eq!(packet["agent_readiness"]["state"], "requires_human_review");
    let reasons = serde_json::to_string(&packet["agent_readiness"]["reasons"])?;
    assert!(reasons.contains("operation family `unknown`"));
    assert!(reasons.contains("no verify command"));
    assert!(
        packet["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );

    Ok(())
}

#[test]
fn manual_candidate_import_explain_context_and_witness_plan_preserve_manual_marker()
-> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-manual-candidate-e2e")?;
    let input = manual_candidate_example_path();
    let out = temp.path().join(".unsafe-review/candidates/R4R2-S001.json");
    fs::create_dir_all(out.parent().ok_or("candidate output missing parent")?)?;

    let imported = run_success([
        os("candidate"),
        os("import"),
        input.into_os_string(),
        os("--out"),
        out.as_os_str().to_os_string(),
    ])?;
    let imported_stdout = stdout_text(&imported)?;
    assert!(imported_stdout.contains("wrote manual candidate"));
    assert!(imported_stdout.contains("source: manual"));

    let canonical = parse_json(&fs::read_to_string(&out)?)?;
    assert_eq!(canonical["schema_version"], "manual-candidate/v1");
    assert_eq!(canonical["source"], "manual");
    assert_eq!(canonical["manual_candidate"], true);
    assert_eq!(canonical["analyzer_discovered"], false);
    assert_eq!(canonical["id"], "R4R2-S001");
    assert_eq!(
        canonical["evidence"][0]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        canonical["evidence"][0]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not analyzer-discovered")
    );

    let empty_snapshot = temp.path().join("empty-snapshot.json");
    fs::write(&empty_snapshot, empty_review_card_snapshot_json())?;
    let outcome = run_success([
        os("outcome"),
        os("--before"),
        empty_snapshot.as_os_str().to_os_string(),
        os("--after"),
        out.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let outcome = parse_json(&stdout_text(&outcome)?)?;
    assert_eq!(outcome["after"]["schema_version"], "manual-candidate/v1");
    assert_eq!(outcome["after"]["source"], "manual");
    assert_eq!(outcome["summary"]["new"], 1);
    assert_eq!(outcome["cards"]["new"][0]["card_id"], "R4R2-S001");
    assert_eq!(outcome["cards"]["new"][0]["after"]["source"], "manual");
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["manual_candidate"],
        true
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["analyzer_discovered"],
        false
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["operation_family"],
        "raw_pointer_read"
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["operation"],
        "core::slice::from_raw_parts"
    );
    assert_eq!(outcome["cards"]["new"][0]["after"]["evidence_count"], 2);
    assert!(
        outcome["cards"]["new"][0]["reason"]
            .as_str()
            .unwrap_or("")
            .contains("new manual candidate")
    );
    let limitations = outcome["limitations"]
        .as_array()
        .ok_or("outcome limitations missing")?;
    assert!(limitations.iter().any(|limitation| {
        limitation
            .as_str()
            .unwrap_or("")
            .contains("manual candidate JSON artifacts")
    }));

    let outcome_markdown = run_success([
        os("outcome"),
        os("--before"),
        empty_snapshot.as_os_str().to_os_string(),
        os("--after"),
        out.as_os_str().to_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let outcome_markdown = stdout_text(&outcome_markdown)?;
    assert!(outcome_markdown.contains("new manual candidate"));
    assert!(outcome_markdown.contains("source `manual`"));
    assert!(outcome_markdown.contains("manual_candidate `true`"));
    assert!(outcome_markdown.contains("analyzer-discovered `false`"));
    assert!(outcome_markdown.contains("not analyzer-discovered"));

    let explain = run_success([
        os("explain"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("R4R2-S001"),
    ])?;
    let explain = stdout_text(&explain)?;
    assert!(explain.contains("unsafe-review manual candidate"));
    assert!(explain.contains("Source: `manual`"));
    assert!(explain.contains("Analyzer-discovered: `false`"));
    assert!(explain.contains("## Implementer handoff"));
    assert!(explain.contains("Inspect: `src/runtime/webcore/TextDecoder.rs:237`"));
    assert!(explain.contains("Stop line: stop before source edits"));
    assert!(explain.contains("not analyzer-discovered"));

    let explain_json = run_success([
        os("explain"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--json"),
        os("R4R2-S001"),
    ])?;
    let explain_packet = parse_json(&stdout_text(&explain_json)?)?;
    assert_eq!(explain_packet["source"], "manual");
    assert_eq!(explain_packet["manual_candidate"], true);
    assert_eq!(explain_packet["analyzer_discovered"], false);
    assert_eq!(
        explain_packet["implementer_handoff"]["target"]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        explain_packet["implementer_handoff"]["route"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        explain_packet["implementer_handoff"]["invariant_at_risk"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert_eq!(
        explain_packet["implementer_handoff"]["external_evidence"][0]["kind"],
        "runtime_witness"
    );
    assert_eq!(
        explain_packet["implementer_handoff"]["external_evidence"][0]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        explain_packet["implementer_handoff"]["external_evidence"][0]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("runtime route evidence only")
    );
    assert!(
        explain_packet["implementer_handoff"]["stop_condition"]
            .as_str()
            .unwrap_or("")
            .contains("stop before source edits")
    );

    let context = run_success([
        os("context"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("R4R2-S001"),
    ])?;
    let context_packet = parse_json(&stdout_text(&context)?)?;
    assert_eq!(context_packet["source"], "manual");
    assert_eq!(context_packet["manual_candidate"], true);
    assert_eq!(
        context_packet["implementer_handoff"]["route"]["unsafe_operation"],
        "core::slice::from_raw_parts"
    );
    assert!(
        serde_json::to_string(&context_packet["implementer_handoff"]["non_goals"])?
            .contains("do not treat this as analyzer-discovered")
    );
    assert_eq!(
        context_packet["evidence"][0]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );

    let witness_plan = run_success([
        os("candidate"),
        os("witness-plan"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("R4R2-S001"),
    ])?;
    let witness_plan = stdout_text(&witness_plan)?;
    assert!(witness_plan.contains("manual candidate witness plan"));
    assert!(witness_plan.contains("## Implementer handoff"));
    assert!(witness_plan.contains("Route: `new TextDecoder().decode"));
    assert!(witness_plan.contains("command `bun test"));
    assert!(witness_plan.contains("limitation: runtime route evidence only"));
    assert!(witness_plan.contains("does not run witnesses"));
    assert!(witness_plan.contains("unsafe-review receipt template R4R2-S001"));
    assert!(witness_plan.contains("not analyzer-discovered"));

    Ok(())
}

#[test]
fn manual_candidate_list_reports_imported_advisory_ledger() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-manual-candidate-list-e2e")?;
    let candidate_dir = temp.path().join(".unsafe-review/candidates");
    fs::create_dir_all(&candidate_dir)?;
    fs::write(
        candidate_dir.join("R4R2-S002.json"),
        manual_candidate_json().replace("\"id\": \"R4R2-S001\"", "\"id\": \"R4R2-S002\""),
    )?;
    fs::write(
        candidate_dir.join("R4R2-S001.json"),
        manual_candidate_json(),
    )?;

    let json = run_success([
        os("candidate"),
        os("list"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let ledger = parse_json(&stdout_text(&json)?)?;

    assert_eq!(ledger["schema_version"], "manual-candidates/v1");
    assert_eq!(ledger["mode"], "manual_candidate_index");
    assert_eq!(ledger["source"], "candidate_list");
    assert_eq!(ledger["root"], temp.path().display().to_string());
    assert_eq!(ledger["summary"]["manual_candidates"], 2);
    assert_eq!(ledger["summary"]["external_evidence_refs"], 4);
    assert_eq!(ledger["summary"]["analyzer_discovered"], 0);
    assert_eq!(ledger["candidates"][0]["id"], "R4R2-S001");
    assert_eq!(ledger["candidates"][1]["id"], "R4R2-S002");
    assert_eq!(ledger["candidates"][0]["source"], "manual");
    assert_eq!(ledger["candidates"][0]["manual_candidate"], true);
    assert_eq!(ledger["candidates"][0]["analyzer_discovered"], false);
    assert_eq!(
        ledger["candidates"][0]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["target"]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["route"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["route"]["unsafe_operation"],
        "core::slice::from_raw_parts"
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["invariant_at_risk"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert!(
        ledger["candidates"][0]["implementer_handoff"]["stop_condition"]
            .as_str()
            .unwrap_or("")
            .contains("stop before source edits")
    );
    assert!(
        ledger["candidates"][0]["explain_command"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review explain --root")
    );
    assert!(
        ledger["candidates"][0]["context_json_command"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review context --root")
    );
    assert!(
        ledger["candidates"][0]["witness_plan_command"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review candidate witness-plan --root")
    );
    assert!(
        ledger["reviewcard_artifact_relationship"]["cards.json"]
            .as_str()
            .unwrap_or("")
            .contains("ReviewCard-only analyzer output")
    );
    assert!(
        ledger["reviewcard_artifact_relationship"]["repair-queue.json"]
            .as_str()
            .unwrap_or("")
            .contains("not automatic repair tasks")
    );
    assert!(
        ledger["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not analyzer-discovered ReviewCards")
    );
    assert!(
        ledger["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review did not run witnesses")
    );

    let markdown = run_success([
        os("candidate"),
        os("list"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
    ])?;
    let markdown = stdout_text(&markdown)?;
    assert!(markdown.contains("# unsafe-review manual candidate list"));
    assert!(markdown.contains("Manual candidates: `2`"));
    assert!(markdown.contains("Analyzer-discovered: `0`"));
    assert!(markdown.contains("### `R4R2-S001`"));
    assert!(markdown.contains("Location: `src/runtime/webcore/TextDecoder.rs:237`"));
    assert!(markdown.contains("#### Implementer Handoff"));
    assert!(markdown.contains("Route: `new TextDecoder().decode"));
    assert!(markdown.contains("Invariant at risk: &[u8] memory must not be concurrently mutated"));
    assert!(markdown.contains("Evidence packet: `2` external reference(s)"));
    assert!(
        markdown.contains(
            "`runtime_witness` at `target/unsafe-scout/textdecoder-shared-race-route.out`"
        )
    );
    assert!(
        markdown.contains("Bun TextDecoder route reaches shared backing bytes through safe JS")
    );
    assert!(
        markdown
            .contains("Command: `bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts`")
    );
    assert!(markdown.contains(
        "Limitation: runtime route evidence only; not memory-safety proof and not analyzer-discovered"
    ));
    assert!(
        markdown.contains("`model` at `target/unsafe-scout/miri-textdecoder-shared-slice.out`")
    );
    assert!(markdown.contains("Next steps:"));
    assert!(markdown.contains("confirm the file:line and safe caller route before editing"));
    assert!(markdown.contains("Non-goals:"));
    assert!(markdown.contains("do not treat this as analyzer-discovered"));
    assert!(
        markdown
            .contains("do not claim proof, UB-free status, Miri-clean status, or site execution")
    );
    assert!(markdown.contains("do not broaden the task to unrelated unsafe sites"));
    assert!(markdown.contains("Stop line: stop before source edits"));
    assert!(markdown.contains("Context: `unsafe-review context --root"));
    assert!(markdown.contains("ReviewCard-only repair queue"));
    assert!(markdown.contains("not analyzer-discovered ReviewCards"));
    assert!(markdown.contains("did not run witnesses"));

    let out = temp.path().join("manual-candidates-list.json");
    let wrote = run_success([
        os("candidate"),
        os("list"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        out.as_os_str().to_os_string(),
    ])?;
    assert_eq!(stdout_text(&wrote)?.trim(), "");
    assert_eq!(
        parse_json(&fs::read_to_string(&out)?)?["summary"]["manual_candidates"],
        2
    );

    let empty_snapshot = temp.path().join("empty-snapshot.json");
    fs::write(&empty_snapshot, empty_review_card_snapshot_json())?;
    let outcome = run_success([
        os("outcome"),
        os("--before"),
        empty_snapshot.as_os_str().to_os_string(),
        os("--after"),
        out.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let outcome = parse_json(&stdout_text(&outcome)?)?;
    assert_eq!(outcome["after"]["schema_version"], "manual-candidates/v1");
    assert_eq!(outcome["after"]["source"], "candidate_list");
    assert_eq!(outcome["after"]["cards"], 2);
    assert_eq!(outcome["summary"]["new"], 2);
    assert_eq!(outcome["cards"]["new"][0]["card_id"], "R4R2-S001");
    assert_eq!(outcome["cards"]["new"][1]["card_id"], "R4R2-S002");
    assert_eq!(outcome["cards"]["new"][0]["after"]["source"], "manual");
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["manual_candidate"],
        true
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["analyzer_discovered"],
        false
    );
    assert!(
        outcome["cards"]["new"][0]["reason"]
            .as_str()
            .unwrap_or("")
            .contains("new manual candidate")
    );

    Ok(())
}

#[test]
fn manual_candidate_receipts_audit_as_manual_advisory_targets() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-manual-candidate-receipt-e2e")?;
    copy_dir_all(&fixture, temp.path())?;
    let input = temp.path().join("candidate.json");
    let candidate_out = temp.path().join(".unsafe-review/candidates/R4R2-S001.json");
    fs::create_dir_all(
        candidate_out
            .parent()
            .ok_or("candidate output missing parent")?,
    )?;
    fs::write(&input, manual_candidate_json())?;

    run_success([
        os("candidate"),
        os("import"),
        input.as_os_str().to_os_string(),
        os("--out"),
        candidate_out.as_os_str().to_os_string(),
    ])?;

    let receipt_out = temp.path().join(".unsafe-review/receipts/R4R2-S001.json");
    fs::create_dir_all(
        receipt_out
            .parent()
            .ok_or("receipt output missing parent")?,
    )?;
    run_success([
        os("receipt"),
        os("template"),
        os("R4R2-S001"),
        os("--tool"),
        os("human-deep-review"),
        os("--strength"),
        os("test_targeted"),
        os("--author"),
        os("unsafe-scout"),
        os("--recorded-at"),
        os("2026-05-31T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--summary"),
        os("manual route reviewed with external witness packet"),
        os("--command"),
        os("manual review R4R2-S001"),
        os("--out"),
        receipt_out.as_os_str().to_os_string(),
    ])?;

    let validate = run_success([
        os("receipt"),
        os("validate"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
    ])?;
    assert!(stdout_text(&validate)?.contains("witness receipts: 1 valid"));

    let audit = run_success([
        os("receipt"),
        os("audit"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--diff"),
        temp.path().join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let audit = parse_json(&stdout_text(&audit)?)?;
    assert_eq!(audit["summary"]["receipts"], 1);
    assert_eq!(audit["summary"]["matched"], 1);
    assert_eq!(audit["summary"]["wrong_identity"], 0);
    let receipt = &audit["receipts"][0];
    let statuses = serde_json::to_string(&receipt["statuses"])?;
    assert!(statuses.contains("manual_candidate"));
    assert!(statuses.contains("matched"));
    assert!(!statuses.contains("imports_witness_evidence"));
    assert!(receipt["matched_card"].is_null());
    assert_eq!(receipt["matched_manual_candidate"]["id"], "R4R2-S001");
    assert_eq!(receipt["matched_manual_candidate"]["source"], "manual");
    assert_eq!(
        receipt["matched_manual_candidate"]["manual_candidate"],
        true
    );
    assert_eq!(
        receipt["matched_manual_candidate"]["analyzer_discovered"],
        false
    );
    assert!(
        receipt["matched_manual_candidate"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not analyzer-discovered")
    );

    Ok(())
}

#[test]
fn first_pr_writes_standard_advisory_review_bundle() -> Result<(), Box<dyn Error>> {
    let source_fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-first-pr-e2e")?;
    let fixture = temp.path().join("fixture");
    copy_dir_all(&source_fixture, &fixture)?;
    let candidate_dir = fixture.join(".unsafe-review").join("candidates");
    fs::create_dir_all(&candidate_dir)?;
    fs::write(
        candidate_dir.join("R4R2-S001.json"),
        manual_candidate_json(),
    )?;
    fs::write(
        candidate_dir.join("R4R2-S002.json"),
        manual_candidate_json().replace("\"id\": \"R4R2-S001\"", "\"id\": \"R4R2-S002\""),
    )?;
    let out_dir = temp.path().join("unsafe-review");

    let output = run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.as_os_str().to_os_string(),
    ])?;
    let stdout = stdout_text(&output)?;

    assert!(stdout.contains("unsafe-review first-pr"));
    assert!(stdout.contains("unsafe-review wrote an advisory PR bundle."));
    assert!(stdout.contains("- Artifact directory:"));
    assert!(stdout.contains("- Review cards: 1"));
    assert!(stdout.contains("- Open actionable gaps: 1"));
    assert!(stdout.contains("Open:"));
    assert!(stdout.contains("pr-summary.md"));
    assert!(stdout.contains("Agent repair queue:"));
    assert!(stdout.contains("repair-queue.json"));
    assert!(stdout.contains("copy-only; unsafe-review did not run an agent"));
    assert!(stdout.contains("Manual candidates:"));
    assert!(stdout.contains("manual-candidates.json"));
    assert!(stdout.contains("Count: 2"));
    assert!(stdout.contains("First manual candidate: R4R2-S001"));
    assert!(stdout.contains("unsafe-review explain --root"));
    assert!(stdout.contains("unsafe-review context --root"));
    assert!(stdout.contains("unsafe-review candidate witness-plan --root"));
    assert!(stdout.contains("Review-kit candidate queue: first 2 of 2 manual candidate(s)"));
    assert!(stdout.contains("manual candidates are advisory manual targets"));
    assert!(stdout.contains("not analyzer-discovered"));
    assert!(stdout.contains("not policy inputs"));
    assert!(stdout.contains("Audit saved receipts:"));
    assert!(stdout.contains("unsafe-review receipt audit --root"));
    assert!(stdout.contains("--diff"));
    assert!(stdout.contains("--format markdown"));
    assert!(stdout.contains("saved receipt metadata only; unsafe-review did not run a witness"));
    assert!(stdout.contains("Top card:"));
    assert!(stdout.contains("`raw_pointer_read`"));
    assert!(stdout.contains("Class: `guard_missing`"));
    assert!(stdout.contains("Route: `miri`"));
    assert!(stdout.contains("Explain top card:"));
    assert!(stdout.contains("Agent packet:"));
    assert!(stdout.contains("Artifacts:"));
    assert!(stdout.contains("review-kit.json"));
    assert!(stdout.contains("cards.json"));
    assert!(stdout.contains("pr-summary.md"));
    assert!(stdout.contains("github-summary.md"));
    assert!(stdout.contains("cards.sarif"));
    assert!(stdout.contains("comment-plan.json"));
    assert!(stdout.contains("witness-plan.md"));
    assert!(stdout.contains("receipt-audit.md"));
    assert!(stdout.contains("manual-candidates.json"));
    assert!(stdout.contains("lsp.json"));
    assert!(stdout.contains("repair-queue.json"));
    assert!(stdout.contains("Trust boundary:"));
    assert!(stdout.contains("static unsafe contract review only"));
    assert!(stdout.contains("not memory-safety proof"));
    assert!(stdout.contains("not UB-free status"));
    assert!(stdout.contains("not Miri-clean status"));
    assert!(stdout.contains("did not run witnesses"));
    assert!(stdout.contains("post comments"));
    assert!(stdout.contains("enforce blocking policy"));

    let cards = parse_json(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    assert_eq!(cards["schema_version"], "0.1");
    assert_eq!(cards["scope"], "diff");
    assert_eq!(cards["policy"], "advisory");
    assert_eq!(cards["summary"]["cards"], 1);
    assert_eq!(cards["cards"][0]["class"], "guard_missing");
    assert_eq!(cards["cards"][0]["operation_family"], "raw_pointer_read");
    assert!(!serde_json::to_string(&cards)?.contains("R4R2-S001"));
    assert!(
        cards["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a proof of memory safety")
    );
    let card_id = json_str(&cards["cards"][0]["id"], "cards[0].id")?;
    assert!(stdout.contains("unsafe-review explain --root"));
    assert!(stdout.contains("unsafe-review context --root"));
    assert!(stdout.contains(" --json"));
    assert!(stdout.contains(card_id));

    let review_kit = parse_json(&fs::read_to_string(out_dir.join("review-kit.json"))?)?;
    assert_eq!(review_kit["schema_version"], "0.1");
    assert_eq!(review_kit["tool"], "unsafe-review");
    assert_eq!(review_kit["mode"], "review_kit_manifest");
    assert_eq!(review_kit["source"], "first_pr");
    assert_eq!(review_kit["policy"], "advisory");
    assert_eq!(review_kit["scope"], "diff");
    assert_eq!(review_kit["summary"]["cards"], 1);
    assert_eq!(review_kit["summary"]["open_actionable_gaps"], 1);
    assert_eq!(review_kit["top_card_id"], card_id);
    assert_eq!(review_kit["handoff"]["reviewer_summary"], "pr-summary.md");
    assert_eq!(review_kit["handoff"]["top_card"]["card_id"], card_id);
    assert!(
        review_kit["handoff"]["top_card"]["explain"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review explain --root")
    );
    assert!(
        review_kit["handoff"]["top_card"]["explain"]
            .as_str()
            .unwrap_or("")
            .contains(card_id)
    );
    assert!(
        review_kit["handoff"]["top_card"]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review context --root")
    );
    assert!(
        review_kit["handoff"]["top_card"]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains(card_id)
    );
    assert!(
        review_kit["handoff"]["top_card"]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains("--json")
    );
    assert!(
        review_kit["handoff"]["receipt_audit_markdown"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review receipt audit --root")
    );
    assert!(
        review_kit["handoff"]["receipt_audit_markdown"]
            .as_str()
            .unwrap_or("")
            .contains("--format markdown")
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["artifact"],
        "manual-candidates.json"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["manual_candidates"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["analyzer_discovered"],
        0
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["id"],
        "R4R2-S001"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["source"],
        "manual"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["manual_candidate"],
        true
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["analyzer_discovered"],
        false
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["implementer_handoff"]["target"]
            ["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["implementer_handoff"]["route"]
            ["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["implementer_handoff"]["invariant_at_risk"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["implementer_handoff"]
            ["stop_condition"]
            .as_str()
            .unwrap_or("")
            .contains("stop before source edits")
    );
    assert!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["explain"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review explain --root")
    );
    assert!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review context --root")
    );
    assert!(
        review_kit["handoff"]["manual_candidates"]["first_candidate"]["witness_plan"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review candidate witness-plan --root")
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["candidate_queue_limit"],
        5
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["omitted_candidates"],
        0
    );
    let candidate_queue = json_array(
        &review_kit["handoff"]["manual_candidates"]["candidate_queue"],
        "review_kit.handoff.manual_candidates.candidate_queue",
    )?;
    assert_eq!(candidate_queue.len(), 2);
    assert_eq!(candidate_queue[0]["id"], "R4R2-S001");
    assert_eq!(candidate_queue[1]["id"], "R4R2-S002");
    assert_eq!(candidate_queue[0]["source"], "manual");
    assert_eq!(candidate_queue[0]["manual_candidate"], true);
    assert_eq!(candidate_queue[0]["analyzer_discovered"], false);
    assert_eq!(
        candidate_queue[0]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(candidate_queue[0]["operation_family"], "raw_pointer_read");
    assert_eq!(candidate_queue[0]["evidence_refs"], 2);
    assert_eq!(
        candidate_queue[0]["implementer_handoff"]["route"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        candidate_queue[0]["implementer_handoff"]["route"]["unsafe_operation"],
        "core::slice::from_raw_parts"
    );
    assert!(
        candidate_queue[0]["explain"]
            .as_str()
            .unwrap_or("")
            .contains("R4R2-S001")
    );
    assert!(
        candidate_queue[1]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains("R4R2-S002")
    );
    assert!(
        candidate_queue[1]["witness_plan"]
            .as_str()
            .unwrap_or("")
            .contains("R4R2-S002")
    );
    assert!(
        review_kit["handoff"]["manual_candidates"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not analyzer-discovered ReviewCards")
    );
    assert!(
        review_kit["handoff"]["manual_candidates"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("do not import ReviewCard witness evidence")
    );
    assert!(
        review_kit["handoff"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("did not run witnesses")
    );
    let artifacts = json_array(&review_kit["artifacts"], "review_kit.artifacts")?;
    for expected in [
        "review-kit.json",
        "cards.json",
        "pr-summary.md",
        "github-summary.md",
        "cards.sarif",
        "comment-plan.json",
        "witness-plan.md",
        "receipt-audit.md",
        "manual-candidates.json",
        "lsp.json",
        "repair-queue.json",
    ] {
        let Some(entry) = artifacts
            .iter()
            .find(|artifact| artifact["path"] == expected)
        else {
            return Err(format!("review-kit.json is missing artifact entry `{expected}`").into());
        };
        assert!(
            out_dir.join(expected).is_file(),
            "review-kit.json listed missing artifact `{expected}`"
        );
        if expected.ends_with(".json") {
            assert_eq!(entry["format"], "json");
        } else if expected.ends_with(".md") {
            assert_eq!(entry["format"], "markdown");
        } else if expected.ends_with(".sarif") {
            assert_eq!(entry["format"], "sarif");
        }
        match expected {
            "review-kit.json" | "cards.json" | "comment-plan.json" | "lsp.json"
            | "repair-queue.json" => assert_eq!(entry["schema_version"], "0.1"),
            "manual-candidates.json" => {
                assert_eq!(entry["schema_version"], "manual-candidates/v1")
            }
            "cards.sarif" => assert_eq!(entry["schema_version"], "2.1.0"),
            _ => assert!(entry["schema_version"].is_null()),
        }
    }
    assert!(
        review_kit["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("does not reclassify ReviewCards")
    );
    assert!(
        review_kit["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("did not run witnesses")
    );

    let summary = fs::read_to_string(out_dir.join("pr-summary.md"))?;
    assert!(summary.contains("# unsafe-review PR summary"));
    assert!(summary.contains("## Reviewer cockpit"));
    assert!(summary.contains(&format!("- Top card: `{card_id}`")));
    assert!(summary.contains("- Missing/weak evidence:"));
    assert!(summary.contains("- Next reviewer action:"));
    assert!(summary.contains("- Witness route:"));
    assert!(summary.contains("- Receipt audit: `receipt-audit.md`"));
    assert!(summary.contains("no witness was run"));
    assert!(summary.contains(&format!("unsafe-review explain {card_id}")));
    assert!(summary.contains(&format!("unsafe-review context {card_id} --json")));
    assert!(summary.contains("## Trust boundary"));
    assert!(summary.contains("not a Miri result unless a witness receipt is attached"));

    let github_summary = fs::read_to_string(out_dir.join("github-summary.md"))?;
    assert!(github_summary.contains("## unsafe-review advisory summary"));
    assert!(github_summary.contains(&format!("- ID: `{card_id}`")));
    assert!(github_summary.contains(&format!("- Explain: `unsafe-review explain {card_id}`")));
    assert!(github_summary.contains(&format!(
        "- Agent context: `unsafe-review context {card_id} --json`"
    )));
    assert!(github_summary.contains("## Open next"));
    assert!(github_summary.contains("Review kit manifest: `review-kit.json`"));
    assert!(github_summary.contains("Full reviewer cockpit: `pr-summary.md`"));
    assert!(github_summary.contains("Agent repair queue: `repair-queue.json`"));
    assert!(github_summary.contains("Receipt audit: `receipt-audit.md`"));
    assert!(github_summary.contains("Manual candidate index: `manual-candidates.json`"));
    assert!(github_summary.contains("`comment-plan.json` is plan-only"));
    assert!(github_summary.contains("Full advisory bundle"));
    assert!(github_summary.contains("review-kit.json"));
    assert!(github_summary.contains("github-summary.md"));
    assert!(github_summary.contains("receipt-audit.md"));
    assert!(github_summary.contains("manual-candidates.json"));
    assert!(github_summary.contains("not memory-safety proof"));
    assert!(github_summary.contains("not site-execution proof"));
    assert!(github_summary.contains("unsafe-review did not run witnesses"));
    assert!(github_summary.contains("post comments"));
    assert!(github_summary.contains("edit source"));
    assert!(github_summary.contains("enforce blocking policy"));
    assert!(!github_summary.contains("# unsafe-review PR summary"));
    assert!(!github_summary.contains("## Card table"));

    let manual_candidates =
        parse_json(&fs::read_to_string(out_dir.join("manual-candidates.json"))?)?;
    assert_eq!(manual_candidates["schema_version"], "manual-candidates/v1");
    assert_eq!(manual_candidates["mode"], "manual_candidate_index");
    assert_eq!(manual_candidates["source"], "first_pr");
    assert_eq!(manual_candidates["summary"]["manual_candidates"], 2);
    assert_eq!(manual_candidates["summary"]["analyzer_discovered"], 0);
    assert_eq!(manual_candidates["candidates"][0]["id"], "R4R2-S001");
    assert_eq!(manual_candidates["candidates"][1]["id"], "R4R2-S002");
    assert_eq!(manual_candidates["candidates"][0]["source"], "manual");
    assert_eq!(manual_candidates["candidates"][0]["manual_candidate"], true);
    assert_eq!(
        manual_candidates["candidates"][0]["analyzer_discovered"],
        false
    );
    assert_eq!(
        manual_candidates["candidates"][0]["operation_family"],
        "raw_pointer_read"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["evidence"][0]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        manual_candidates["candidates"][0]["evidence"][0]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("runtime route evidence only")
    );
    assert!(
        manual_candidates["candidates"][0]["explain_command"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review explain --root")
    );
    assert!(
        manual_candidates["candidates"][0]["context_command"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review context --root")
    );
    assert!(
        manual_candidates["candidates"][0]["witness_plan_command"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe-review candidate witness-plan --root")
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["target"]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["route"]["unsafe_operation"],
        "core::slice::from_raw_parts"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["invariant_at_risk"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert!(
        manual_candidates["candidates"][0]["implementer_handoff"]["stop_condition"]
            .as_str()
            .unwrap_or("")
            .contains("stop before source edits")
    );
    assert!(
        manual_candidates["reviewcard_artifact_relationship"]["cards.json"]
            .as_str()
            .unwrap_or("")
            .contains("ReviewCard-only")
    );
    assert!(
        manual_candidates["reviewcard_artifact_relationship"]["comment-plan.json"]
            .as_str()
            .unwrap_or("")
            .contains("not selected")
    );
    assert!(
        manual_candidates["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not analyzer-discovered")
    );
    assert!(
        manual_candidates["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not policy gating")
    );

    let receipt_audit = fs::read_to_string(out_dir.join("receipt-audit.md"))?;
    assert!(receipt_audit.contains("# unsafe-review receipt audit"));
    assert!(receipt_audit.contains("Static audit of saved receipt metadata"));
    assert!(receipt_audit.contains("## Reviewer front panel"));
    assert!(receipt_audit.contains("No receipt files found."));
    assert!(receipt_audit.contains("does not execute witnesses"));
    assert!(receipt_audit.contains("does not independently prove site reach"));
    assert!(receipt_audit.contains("matched witness receipts improve witness evidence only"));

    let sarif = parse_json(&fs::read_to_string(out_dir.join("cards.sarif"))?)?;
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["runs"][0]["results"][0]["properties"]["cardId"],
        card_id
    );
    assert!(
        sarif["runs"][0]["properties"]["trustBoundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );

    let comment_plan = parse_json(&fs::read_to_string(out_dir.join("comment-plan.json"))?)?;
    assert_eq!(comment_plan["mode"], "plan_only");
    assert_eq!(comment_plan["policy"], "advisory");
    assert_eq!(comment_plan["comments"][0]["card_id"], card_id);
    assert!(
        comment_plan["comments"][0]["body"]
            .as_str()
            .unwrap_or("")
            .contains("not memory-safety proof")
    );

    let witness_plan = fs::read_to_string(out_dir.join("witness-plan.md"))?;
    assert!(witness_plan.contains("# unsafe-review witness plan"));
    assert!(witness_plan.contains("### Miri / cargo-careful"));
    assert!(witness_plan.contains(&format!("#### `{card_id}`")));
    assert!(witness_plan.contains("does not run Miri"));
    assert!(witness_plan.contains("Receipt hint"));
    assert!(witness_plan.contains("unsafe-review receipt import-miri"));
    assert!(
        witness_plan
            .contains("does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux")
    );
    assert!(!witness_plan.contains("Miri passed"));
    assert!(!witness_plan.contains("site reached"));

    let lsp = parse_json(&fs::read_to_string(out_dir.join("lsp.json"))?)?;
    assert_eq!(lsp["mode"], "read_only_projection");
    assert_eq!(lsp["policy"], "advisory");
    assert_eq!(lsp["diagnostics"][0]["card_id"], card_id);
    assert_eq!(lsp["hovers"][0]["card_id"], card_id);
    assert_eq!(lsp["code_actions"][0]["card_id"], card_id);
    assert!(
        lsp["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a Miri result")
    );

    let repair_queue = parse_json(&fs::read_to_string(out_dir.join("repair-queue.json"))?)?;
    assert_eq!(repair_queue["mode"], "aggregate_repair_queue");
    assert_eq!(repair_queue["source"], "review_card");
    assert_eq!(repair_queue["policy"], "advisory");
    assert_eq!(repair_queue["summary"]["cards"], 1);
    assert_eq!(
        repair_queue["buckets"]["repairable_by_guard"][0]["card_id"],
        card_id
    );
    assert_eq!(
        repair_queue["buckets"]["requires_witness_receipt"][0]["card_id"],
        card_id
    );
    assert_eq!(
        repair_queue["buckets"]["repairable_by_guard"][0]["context_command"],
        format!("unsafe-review context {card_id} --json")
    );
    assert!(
        repair_queue["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not an automatic repair queue")
    );

    Ok(())
}

#[test]
fn first_pr_clean_output_stays_advisory_not_all_clear() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("safe_code_no_cards");
    let temp = TempDir::new("unsafe-review-first-pr-clean-e2e")?;
    let out_dir = temp.path().join("unsafe-review");

    let output = run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.as_os_str().to_os_string(),
    ])?;
    let stdout = stdout_text(&output)?;

    assert!(stdout.contains("unsafe-review first-pr"));
    assert!(stdout.contains("unsafe-review wrote an advisory PR bundle."));
    assert!(stdout.contains("- Artifact directory:"));
    assert!(stdout.contains("- Review cards: 0"));
    assert!(stdout.contains("- Open actionable gaps: 0"));
    assert!(stdout.contains("Open:"));
    assert!(stdout.contains("pr-summary.md"));
    assert!(stdout.contains("Agent repair queue:"));
    assert!(stdout.contains("repair-queue.json"));
    assert!(stdout.contains("copy-only; unsafe-review did not run an agent"));
    assert!(stdout.contains("Audit saved receipts:"));
    assert!(stdout.contains("unsafe-review receipt audit --root"));
    assert!(stdout.contains("--diff"));
    assert!(stdout.contains("--format markdown"));
    assert!(stdout.contains("saved receipt metadata only; unsafe-review did not run a witness"));
    assert!(stdout.contains("github-summary.md"));
    assert!(stdout.contains("No changed unsafe-review gaps were found."));
    assert!(stdout.contains("This does not prove the repo safe"));
    assert!(stdout.contains("UB-free"));
    assert!(stdout.contains("Miri-clean"));
    assert!(stdout.contains("unsafe site executed"));
    assert!(!stdout.contains("All clear"));

    let cards = parse_json(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    assert_eq!(cards["summary"]["cards"], 0);
    assert_eq!(cards["summary"]["open_actionable_gaps"], 0);
    assert!(
        cards["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a proof of memory safety")
    );

    let review_kit = parse_json(&fs::read_to_string(out_dir.join("review-kit.json"))?)?;
    assert_eq!(review_kit["schema_version"], "0.1");
    assert_eq!(review_kit["mode"], "review_kit_manifest");
    assert_eq!(review_kit["summary"]["cards"], 0);
    assert_eq!(review_kit["summary"]["open_actionable_gaps"], 0);
    assert!(review_kit["top_card_id"].is_null());
    assert_eq!(review_kit["handoff"]["reviewer_summary"], "pr-summary.md");
    assert!(review_kit["handoff"]["top_card"].is_null());
    assert!(
        review_kit["handoff"]["receipt_audit_markdown"]
            .as_str()
            .unwrap_or("")
            .contains("--format markdown")
    );
    assert!(
        review_kit["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not site-execution proof")
    );

    let summary = fs::read_to_string(out_dir.join("pr-summary.md"))?;
    assert!(summary.contains("No changed unsafe-review gaps were found."));
    assert!(summary.contains("This does not prove the repo safe"));
    assert!(summary.contains("unsafe site executed"));
    assert!(summary.contains("- Receipt audit: `receipt-audit.md`"));
    assert!(summary.contains("no witness was run"));
    assert!(!summary.contains("All clear"));

    let github_summary = fs::read_to_string(out_dir.join("github-summary.md"))?;
    assert!(github_summary.contains("## unsafe-review advisory summary"));
    assert!(github_summary.contains("No changed unsafe-review gaps were found."));
    assert!(github_summary.contains("This does not prove the repo safe"));
    assert!(github_summary.contains("unsafe site executed"));
    assert!(github_summary.contains("## Open next"));
    assert!(github_summary.contains("Review kit manifest: `review-kit.json`"));
    assert!(github_summary.contains("Full reviewer cockpit: `pr-summary.md`"));
    assert!(github_summary.contains("Agent repair queue: `repair-queue.json`"));
    assert!(github_summary.contains("Receipt audit: `receipt-audit.md`"));
    assert!(github_summary.contains("Full advisory bundle"));
    assert!(github_summary.contains("review-kit.json"));
    assert!(!github_summary.contains("All clear"));

    let receipt_audit = fs::read_to_string(out_dir.join("receipt-audit.md"))?;
    assert!(receipt_audit.contains("# unsafe-review receipt audit"));
    assert!(receipt_audit.contains("No receipt files found."));
    assert!(receipt_audit.contains("does not execute witnesses"));
    assert!(receipt_audit.contains("does not independently prove site reach"));

    let witness_plan = fs::read_to_string(out_dir.join("witness-plan.md"))?;
    assert!(witness_plan.contains("No changed unsafe-review gaps were found."));
    assert!(witness_plan.contains("No witness routes are recommended"));
    assert!(witness_plan.contains("not UB-free status"));
    assert!(!witness_plan.contains("Miri passed"));
    assert!(!witness_plan.contains("site reached"));

    let comment_plan = parse_json(&fs::read_to_string(out_dir.join("comment-plan.json"))?)?;
    assert_eq!(
        comment_plan["no_changed_gaps"]["message"],
        "No changed unsafe-review gaps were found."
    );
    assert!(
        comment_plan["no_changed_gaps"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("unsafe site executed")
    );

    let lsp = parse_json(&fs::read_to_string(out_dir.join("lsp.json"))?)?;
    assert_eq!(lsp["mode"], "read_only_projection");
    assert_eq!(lsp["diagnostics"].as_array().map_or(1, Vec::len), 0);
    assert_eq!(lsp["hovers"].as_array().map_or(1, Vec::len), 0);
    assert!(
        lsp["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a Miri result")
    );

    let repair_queue = parse_json(&fs::read_to_string(out_dir.join("repair-queue.json"))?)?;
    assert_eq!(repair_queue["mode"], "aggregate_repair_queue");
    assert_eq!(repair_queue["summary"]["cards"], 0);
    assert_eq!(
        repair_queue["buckets"]["repairable_by_guard"]
            .as_array()
            .map_or(1, Vec::len),
        0
    );
    assert!(
        repair_queue["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not an automatic repair queue")
    );

    Ok(())
}

#[test]
fn first_pr_comment_plan_explains_not_selected_cards() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("ffi_sanitizer_route");
    let temp = TempDir::new("unsafe-review-first-pr-comment-not-selected-e2e")?;
    let out_dir = temp.path().join("unsafe-review");

    run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.as_os_str().to_os_string(),
    ])?;

    let cards = parse_json(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    let card_id = json_str(&cards["cards"][0]["id"], "cards[0].id")?;

    let comment_plan = parse_json(&fs::read_to_string(out_dir.join("comment-plan.json"))?)?;
    assert_eq!(comment_plan["comments"].as_array().map_or(1, Vec::len), 0);
    assert_eq!(
        comment_plan["not_selected"].as_array().map_or(0, Vec::len),
        1
    );
    assert_eq!(comment_plan["not_selected"][0]["card_id"], card_id);
    assert_eq!(comment_plan["not_selected"][0]["changed_line"], true);
    assert_eq!(comment_plan["not_selected"][0]["class"], "miri_unsupported");
    assert_eq!(
        comment_plan["not_selected"][0]["operation"],
        cards["cards"][0]["operation"]
    );
    assert_eq!(comment_plan["not_selected"][0]["operation_family"], "ffi");
    assert_eq!(
        comment_plan["not_selected"][0]["next_action"],
        cards["cards"][0]["next_action"]
    );
    assert_eq!(
        comment_plan["not_selected"][0]["actionability"],
        "specific_witness_missing"
    );
    assert_eq!(
        comment_plan["not_selected"][0]["reason"],
        "priority/confidence below inline comment threshold"
    );

    Ok(())
}

#[test]
fn help_reports_first_run_trust_boundary_without_overclaims() -> Result<(), Box<dyn Error>> {
    let output = run_success([os("--help")])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("unsafe-review: cheap unsafe contract review for Rust"));
    assert!(text.contains("Trust boundary: static unsafe contract review only"));
    assert!(text.contains("not memory-safety proof"));
    assert!(text.contains("not UB-free status"));
    assert!(text.contains("not Miri-clean status"));
    assert!(!text.contains("soundness proof"));
    assert!(!text.contains("All clear"));

    Ok(())
}

#[test]
fn repo_help_reports_repo_specific_scale_guidance() -> Result<(), Box<dyn Error>> {
    let output = run_success([os("repo"), os("--help")])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("unsafe-review repo: advisory unsafe contract review"));
    assert!(text.contains("What repo scans today:"));
    assert!(text.contains("Repo mode scans the selected Rust files"));
    assert!(text.contains("--base and --diff are accepted"));
    assert!(text.contains("--include <glob>"));
    assert!(text.contains("--exclude <glob>"));
    assert!(text.contains("--list-files prints selected Rust files"));
    assert!(text.contains("--progress prints scan-status heartbeats"));
    assert!(text.contains("--timeout-seconds <N>"));
    assert!(text.contains("--max-files <N>"));
    assert!(text.contains("<out>.partial"));
    assert!(text.contains("<out>.status.json"));
    assert!(text.contains("incomplete status is kept"));
    assert!(text.contains("Unix SIGTERM/SIGINT"));
    assert!(text.contains("phase=terminated"));
    assert!(text.contains("Trust boundary:"));
    assert!(!text.contains("unsafe-review: cheap unsafe contract review for Rust"));
    assert!(!text.contains("status artifacts are not implemented yet"));

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
fn repo_list_files_honors_selection_controls() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-repo-list-files")?;
    write_e2e_file(temp.path(), "src/lib.rs")?;
    write_e2e_file(temp.path(), "packages/pkg/src/lib.rs")?;
    write_e2e_file(temp.path(), "packages/pkg/src/skip.rs")?;
    write_e2e_file(temp.path(), "vendor/pkg/lib.rs")?;
    write_e2e_file(temp.path(), "build/out/lib.rs")?;
    write_e2e_file(temp.path(), "crates/pkg/generated/lib.rs")?;
    write_e2e_file(temp.path(), "ignored/lib.rs")?;
    fs::write(temp.path().join(".gitignore"), "ignored/\n")?;

    let output = run_success([
        os("repo"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--include"),
        os("src/**/*.rs"),
        os("--include"),
        os("packages/**/*.rs"),
        os("--exclude"),
        os("packages/**/skip.rs"),
        os("--list-files"),
        os("--max-files"),
        os("2"),
    ])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("unsafe-review repo file list"));
    assert!(text.contains("files: 2"));
    assert!(text.contains("src/lib.rs"));
    assert!(text.contains("packages/pkg/src/lib.rs"));
    assert!(!text.contains("skip.rs"));
    assert!(!text.contains("vendor/pkg/lib.rs"));
    assert!(!text.contains("build/out/lib.rs"));
    assert!(!text.contains("generated/lib.rs"));
    assert!(!text.contains("ignored/lib.rs"));

    let ignored = run_success([
        os("repo"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--include"),
        os("ignored/**/*.rs"),
        os("--list-files"),
        os("--no-respect-gitignore"),
    ])?;
    assert!(stdout_text(&ignored)?.contains("ignored/lib.rs"));

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
    assert!(text.contains("cargo metadata:"));
    assert!(text.contains("artifact dir"));
    assert!(text.contains("target"));
    assert!(text.contains("unsafe-review"));
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
    assert!(text.contains("trust boundary: static unsafe contract review only"));
    assert!(text.contains("not memory-safety proof"));
    assert!(text.contains("not UB-free status"));

    Ok(())
}

#[test]
fn support_reports_current_posture_without_overclaims() -> Result<(), Box<dyn Error>> {
    let output = run_success([os("support")])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("unsafe-review support"));
    assert!(text.contains("ReviewCards: experimental"));
    assert!(text.contains("first-pr bundle: advisory"));
    assert!(text.contains("receipts: saved-output template/import/audit only"));
    assert!(text.contains("policy report: advisory"));
    assert!(text.contains("comment posting: not default"));
    assert!(text.contains("source edits: not supported"));
    assert!(text.contains("witness execution: not default"));
    assert!(text.contains("blocking policy: not default"));
    assert!(text.contains("live LSP: deferred"));
    assert!(text.contains("saved lsp.json"));
    assert!(text.contains("static unsafe contract review only"));
    assert!(text.contains("not memory-safety proof"));
    assert!(text.contains("not UB-free status"));
    assert!(text.contains("not Miri-clean status"));
    assert!(text.contains("not a site-execution claim"));
    assert!(text.contains("docs/status/SUPPORT_SUMMARY.md"));
    assert!(!text.contains("safe to use"));
    assert!(!text.contains("Miri passed"));
    assert!(!text.contains("All clear"));

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
        "changed_files",
        "changed_rust_files",
        "changed_non_rust_files",
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
    let stdout = stdout_text(&badges)?;
    assert!(stdout.contains("wrote:"));
    assert!(stdout.contains("unsafe-review.json"));
    assert!(stdout.contains("git add"));
    assert!(stdout.contains("OWNER/REPO/BRANCH"));
    assert!(stdout.contains("not safety, UB-free, or Miri-clean status"));

    let main_badge = parse_json(&fs::read_to_string(badge_dir.join("unsafe-review.json"))?)?;
    assert_eq!(main_badge["schemaVersion"], 1);
    assert_eq!(main_badge["label"], "unsafe-review");
    assert_eq!(main_badge["message"], "1");
    assert_public_badge_payload(&main_badge)?;
    assert_ne!(main_badge["message"], "safe");

    let plus_badge = parse_json(&fs::read_to_string(
        badge_dir.join("unsafe-review-plus.json"),
    )?)?;
    assert_eq!(plus_badge["schemaVersion"], 1);
    assert_eq!(plus_badge["label"], "unsafe-review+");
    assert_eq!(plus_badge["message"], "1");
    assert_public_badge_payload(&plus_badge)?;
    let evidence_quality_component_count =
        json_usize(&summary["contract_missing"], "contract_missing")?
            + json_usize(&summary["guard_missing"], "guard_missing")?
            + json_usize(&summary["guarded_unwitnessed"], "guarded_unwitnessed")?;
    let plus_count = plus_badge["message"]
        .as_str()
        .ok_or("plus badge message missing")?
        .parse::<usize>()
        .map_err(|err| format!("plus badge message parse failed: {err}"))?;
    assert_eq!(plus_count, evidence_quality_component_count);
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
    assert!(repo_markdown.contains(
        "| ID | Class | Location | Operation family | Operation | Missing evidence | Route | Next action |"
    ));
    assert!(repo_markdown.contains("src/lib.rs:8"));
    assert!(repo_markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(repo_markdown.contains("## Trust boundary"));
    assert!(repo_markdown.contains("Add or expose the local guard"));
    assert!(repo_markdown.contains("not raw unsafe usage"));
    assert!(repo_markdown.contains("not UB-free status"));

    Ok(())
}

#[test]
fn repo_progress_writes_status_sidecar_for_out_reports() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-status-e2e")?;
    let report_path = temp.path().join("repo.json");
    let partial_path = temp.path().join("repo.json.partial");
    let status_path = temp.path().join("repo.json.status.json");

    let output = run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--include"),
        os("src/lib.rs"),
        os("--exclude"),
        os("src/generated.rs"),
        os("--max-files"),
        os("5"),
        os("--no-respect-gitignore"),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
        os("--progress"),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsafe-review repo: phase=complete"),
        "stderr should include a final progress heartbeat: {stderr}"
    );
    assert!(
        stderr.contains("files_remaining=0"),
        "stderr should include remaining file count: {stderr}"
    );
    let report = parse_json(&fs::read_to_string(&report_path)?)?;
    assert_eq!(report["scope"], "repo");
    assert!(
        !partial_path.exists(),
        "successful repo output should promote and remove the partial report"
    );
    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "complete");
    assert_eq!(status["scan_scope"]["root"], fixture.display().to_string());
    assert_eq!(status["scan_scope"]["include"][0], "src/lib.rs");
    assert_eq!(status["scan_scope"]["exclude"][0], "src/generated.rs");
    assert_eq!(status["scan_scope"]["respect_gitignore"], false);
    assert_eq!(status["scan_scope"]["large_repo_ignores"], true);
    assert_eq!(status["scan_scope"]["max_files"], 5);
    assert_eq!(status["completed"], true);
    assert_eq!(status["files_discovered"], 1);
    assert_eq!(status["files_scanned"], 1);
    assert_eq!(status["files_remaining"], 0);
    assert_eq!(status["cards_found"], 1);
    assert_eq!(status["last_path"], "src/lib.rs");
    assert!(status["elapsed_ms"].as_u64().is_some());
    assert!(status["error"].is_null());
    assert!(status["partial_path"].is_null());

    Ok(())
}

#[test]
fn repo_output_failure_keeps_partial_and_marks_status_incomplete() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-partial-e2e")?;
    let report_path = temp.path().join("repo.json");
    let partial_path = temp.path().join("repo.json.partial");
    let status_path = temp.path().join("repo.json.status.json");
    fs::create_dir(&report_path)?;

    let output = run_failure([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
    ])?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rename partial repo report"),
        "stderr should explain the failed promotion: {stderr}"
    );
    assert!(
        stderr.contains("incomplete repo status written to"),
        "stderr should point to the incomplete status sidecar: {stderr}"
    );
    assert!(
        stderr.contains("partial repo report kept at"),
        "stderr should point to the retained partial report: {stderr}"
    );
    assert!(partial_path.exists(), "partial report should be retained");
    let partial = parse_json(&fs::read_to_string(&partial_path)?)?;
    assert_eq!(partial["scope"], "repo");
    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "failed");
    assert_default_repo_status_scope(&status, &fixture, 0)?;
    assert_eq!(status["completed"], false);
    assert_eq!(status["files_discovered"], 1);
    assert_eq!(status["files_scanned"], 1);
    assert_eq!(status["cards_found"], 1);
    assert!(
        status["error"]
            .as_str()
            .unwrap_or("")
            .contains("rename partial repo report")
    );
    assert!(
        status["partial_path"]
            .as_str()
            .unwrap_or("")
            .ends_with("repo.json.partial")
    );

    Ok(())
}

#[test]
fn repo_analysis_failure_keeps_completed_file_partial_snapshot() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-analysis-error-partial-e2e")?;
    let scan_root = temp.path().join("fixture");
    copy_dir_all(&fixture, &scan_root)?;
    fs::write(scan_root.join("src/z_bad.rs"), [0xff])?;
    let report_path = temp.path().join("repo.json");
    let partial_path = temp.path().join("repo.json.partial");
    let status_path = temp.path().join("repo.json.status.json");

    let output = run_failure([
        os("repo"),
        os("--root"),
        scan_root.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
    ])?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("read") && stderr.contains("z_bad.rs"),
        "stderr should explain the analysis read failure: {stderr}"
    );
    assert!(
        stderr.contains("incomplete repo status written to"),
        "stderr should point to the incomplete status sidecar: {stderr}"
    );
    assert!(
        stderr.contains("partial repo report kept at"),
        "stderr should point to the retained partial report: {stderr}"
    );
    assert!(
        !report_path.exists(),
        "final report should not look successful"
    );
    assert!(
        partial_path.exists(),
        "analysis error after a completed file should retain a partial report"
    );
    let partial = parse_json(&fs::read_to_string(&partial_path)?)?;
    assert_eq!(partial["scope"], "repo");
    assert_eq!(partial["summary"]["rust_files"], 2);
    assert_eq!(partial["summary"]["cards"], 1);
    assert_eq!(partial["cards"][0]["site"]["file"], "src/lib.rs");

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "failed");
    assert_default_repo_status_scope(&status, &scan_root, 1)?;
    assert_eq!(status["completed"], false);
    assert_eq!(status["files_discovered"], 2);
    assert_eq!(status["files_scanned"], 1);
    assert_eq!(status["cards_found"], 1);
    assert!(status["error"].as_str().unwrap_or("").contains("z_bad.rs"));
    assert!(
        status["partial_path"]
            .as_str()
            .unwrap_or("")
            .ends_with("repo.json.partial")
    );

    Ok(())
}

#[test]
fn repo_timeout_keeps_completed_file_partial_snapshot() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-timeout-partial-e2e")?;
    let scan_root = temp.path().join("fixture");
    copy_dir_all(&fixture, &scan_root)?;
    fs::write(scan_root.join("src/z_safe.rs"), "pub fn safe() {}\n")?;
    let report_path = temp.path().join("repo.json");
    let partial_path = temp.path().join("repo.json.partial");
    let status_path = temp.path().join("repo.json.status.json");

    let output = Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
        .args([
            os("repo"),
            os("--root"),
            scan_root.as_os_str().to_os_string(),
            os("--format"),
            os("json"),
            os("--out"),
            report_path.as_os_str().to_os_string(),
            os("--timeout-seconds"),
            os("1"),
        ])
        .env(
            "UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_AFTER_SCANNED",
            "1",
        )
        .env(
            "UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_AFTER_SCAN_MS",
            "1100",
        )
        .output()?;
    if output.status.success() {
        return Err(format!(
            "expected timeout command to fail\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    assert_eq!(stdout_text(&output)?.trim(), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("repo scan timed out after 1s"),
        "stderr should explain the timeout: {stderr}"
    );
    assert!(
        stderr.contains("incomplete repo status written to"),
        "stderr should point to the incomplete status sidecar: {stderr}"
    );
    assert!(
        stderr.contains("partial repo report kept at"),
        "stderr should point to the retained partial report: {stderr}"
    );
    assert!(
        !report_path.exists(),
        "timed-out report should not look successful"
    );
    assert!(
        partial_path.exists(),
        "timeout after a completed file should retain a partial report"
    );
    let partial = parse_json(&fs::read_to_string(&partial_path)?)?;
    assert_eq!(partial["scope"], "repo");
    assert_eq!(partial["summary"]["rust_files"], 2);
    assert_eq!(partial["summary"]["cards"], 1);
    assert_eq!(partial["cards"][0]["site"]["file"], "src/lib.rs");

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "failed");
    assert_default_repo_status_scope(&status, &scan_root, 1)?;
    assert_eq!(status["completed"], false);
    assert_eq!(status["files_discovered"], 2);
    assert_eq!(status["files_scanned"], 1);
    assert_eq!(status["cards_found"], 1);
    assert_eq!(status["signal"], Value::Null);
    assert!(
        status["error"]
            .as_str()
            .unwrap_or("")
            .contains("repo scan timed out after 1s")
    );
    assert!(
        status["partial_path"]
            .as_str()
            .unwrap_or("")
            .ends_with("repo.json.partial")
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn repo_sigterm_writes_interrupted_status_sidecar() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-sigterm-e2e")?;
    let report_path = temp.path().join("repo.json");
    let partial_path = temp.path().join("repo.json.partial");
    let status_path = temp.path().join("repo.json.status.json");

    let child = Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
        .args([
            os("repo"),
            os("--root"),
            fixture.as_os_str().to_os_string(),
            os("--format"),
            os("json"),
            os("--out"),
            report_path.as_os_str().to_os_string(),
        ])
        .env("UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_MS", "5000")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    std::thread::sleep(std::time::Duration::from_millis(500));
    let kill_status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()?;
    assert!(kill_status.success(), "kill -TERM should succeed");

    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(143));
    assert_eq!(stdout_text(&output)?.trim(), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("interrupted by SIGTERM"),
        "stderr should explain SIGTERM interruption: {stderr}"
    );
    assert!(
        stderr.contains("incomplete repo status written to"),
        "stderr should point to the status sidecar: {stderr}"
    );
    assert!(
        !report_path.exists(),
        "final report should not look successful"
    );
    assert!(
        !partial_path.exists(),
        "SIGTERM before rendering should not invent a partial report"
    );

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "terminated");
    assert_default_repo_status_scope(&status, &fixture, 0)?;
    assert_eq!(status["completed"], false);
    assert_eq!(status["signal"], "SIGTERM");
    assert!(
        status["error"]
            .as_str()
            .unwrap_or("")
            .contains("interrupted by SIGTERM")
    );
    assert!(status["partial_path"].is_null());

    Ok(())
}

#[cfg(unix)]
#[test]
fn repo_sigterm_keeps_completed_file_partial_report() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-sigterm-partial-e2e")?;
    let scan_root = temp.path().join("fixture");
    copy_dir_all(&fixture, &scan_root)?;
    fs::write(scan_root.join("src/z_safe.rs"), "pub fn safe() {}\n")?;
    let report_path = temp.path().join("repo.json");
    let partial_path = temp.path().join("repo.json.partial");
    let status_path = temp.path().join("repo.json.status.json");

    let child = Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
        .args([
            os("repo"),
            os("--root"),
            scan_root.as_os_str().to_os_string(),
            os("--format"),
            os("json"),
            os("--out"),
            report_path.as_os_str().to_os_string(),
        ])
        .env(
            "UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_AFTER_SCANNED",
            "1",
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    std::thread::sleep(std::time::Duration::from_millis(500));
    let kill_status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()?;
    assert!(kill_status.success(), "kill -TERM should succeed");

    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(143));
    assert_eq!(stdout_text(&output)?.trim(), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("interrupted by SIGTERM"),
        "stderr should explain SIGTERM interruption: {stderr}"
    );
    assert!(
        stderr.contains("incomplete repo status written to"),
        "stderr should point to the status sidecar: {stderr}"
    );
    assert!(
        stderr.contains("partial repo report kept at"),
        "stderr should point to the retained partial report: {stderr}"
    );
    assert!(
        !report_path.exists(),
        "final report should not look successful"
    );
    assert!(
        partial_path.exists(),
        "SIGTERM after a completed file should retain a partial report"
    );
    let partial = parse_json(&fs::read_to_string(&partial_path)?)?;
    assert_eq!(partial["scope"], "repo");
    assert_eq!(partial["summary"]["rust_files"], 2);
    assert_eq!(partial["summary"]["cards"], 1);
    assert_eq!(partial["cards"][0]["site"]["file"], "src/lib.rs");

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "terminated");
    assert_default_repo_status_scope(&status, &scan_root, 1)?;
    assert_eq!(status["completed"], false);
    assert_eq!(status["signal"], "SIGTERM");
    assert_eq!(status["files_discovered"], 2);
    assert_eq!(status["files_scanned"], 1);
    assert_eq!(status["cards_found"], 1);
    assert!(
        status["partial_path"]
            .as_str()
            .unwrap_or("")
            .ends_with("repo.json.partial")
    );

    Ok(())
}

#[test]
fn safe_repo_human_output_stays_quiet() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("safe_code_no_cards");

    let output = run_success([os("repo"), os("--root"), fixture.as_os_str().to_os_string()])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("cards: 0, open gaps: 0"));
    assert!(text.contains("No changed unsafe-review gaps were found."));
    assert!(text.contains("This does not prove the repo safe"));
    assert!(!text.contains("All clear"));

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
    assert_eq!(outcome["reviewer_delta"]["new_cards"], 1);
    assert_eq!(outcome["reviewer_delta"]["resolved_cards"], 0);
    assert_eq!(
        outcome["reviewer_delta"]["top_remaining_gaps"][0]["card_id"],
        outcome["cards"]["new"][0]["card_id"]
    );
    assert_eq!(
        outcome["reviewer_delta"]["top_remaining_gaps"][0]["class"],
        "guard_missing"
    );
    assert_eq!(
        outcome["reviewer_delta"]["top_remaining_gaps"][0]["priority"],
        "high"
    );
    assert_eq!(
        outcome["reviewer_delta"]["top_remaining_gaps"][0]["operation_family"],
        "raw_pointer_read"
    );
    assert_eq!(
        outcome["reviewer_delta"]["top_remaining_gaps"][0]["missing_count"],
        2
    );
    assert!(
        outcome["reviewer_delta"]["top_remaining_gaps"][0]["next_action"]
            .as_str()
            .unwrap_or("")
            .contains("Add or expose")
    );
    assert!(outcome["cards"]["new"][0]["card_id"].is_string());
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["operation_family"],
        "raw_pointer_read"
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["next_action"]
            .as_str()
            .unwrap_or("")
            .contains("Add or expose")
    );
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
    assert!(markdown.contains("## Reviewer delta"));
    assert!(markdown.contains("- New cards: 1"));
    assert!(markdown.contains("- Receipt movement: 0 improved, 0 regressed"));
    assert!(markdown.contains("## Movement reasons"));
    assert!(markdown.contains("- `new`"));
    assert!(markdown.contains("new card: appears in the after snapshot"));
    assert!(markdown.contains("Top remaining gaps"));
    assert!(
        markdown.contains("| Card | Class | Priority | Operation family | Missing | Next action |")
    );
    assert!(markdown.contains("guard_missing"));
    assert!(markdown.contains("high"));
    assert!(markdown.contains("| 2 |"));
    assert!(markdown.contains("| Status | Card | Reason | Before | After |"));
    assert!(markdown.contains("## Limitations"));
    assert!(markdown.contains("## Trust boundary"));
    assert!(markdown.contains("| 1 | 0 | 0 | 0 | 0 |"));
    assert!(markdown.contains("raw_pointer_read"));
    assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(markdown.contains("Add or expose"));

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
    assert_eq!(receipt["command_hash"], "3e163b0bce29ff2e");
    assert_eq!(receipt["limitations"][0], "fixture only");

    Ok(())
}

#[test]
fn receipt_template_writes_external_integration_reach_receipt() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-external-reach-template-e2e")?;
    let receipt_path = temp.path().join("external-reach.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("template"),
        os(card_id),
        os("--tool"),
        os("external-integration-test"),
        os("--strength"),
        os("site_reached"),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-06-02T00:00:00Z"),
        os("--expires-at"),
        os("2026-09-02"),
        os("--summary"),
        os("TS integration suite reaches the unsafe seam"),
        os("--command"),
        os("bun test test/js/sab-copy-to-unshared.test.ts"),
        os("--limitation"),
        os("external integration reach only; unsafe-review did not run the command"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["schema_version"], "0.1");
    assert_eq!(receipt["card_id"], card_id);
    assert_eq!(receipt["tool"], "external-integration-test");
    assert_eq!(receipt["strength"], "site_reached");
    assert_eq!(receipt["author"], "core/fixtures");
    assert_eq!(receipt["recorded_at"], "2026-06-02T00:00:00Z");
    assert_eq!(receipt["expires_at"], "2026-09-02");
    assert_eq!(
        receipt["summary"],
        "TS integration suite reaches the unsafe seam"
    );
    assert_eq!(
        receipt["command"],
        "bun test test/js/sab-copy-to-unshared.test.ts"
    );
    assert!(json_str(&receipt["command_hash"], "command_hash")?.len() >= 16);
    assert_eq!(
        receipt["limitations"][0],
        "external integration reach only; unsafe-review did not run the command"
    );

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
    assert!(
        value["limitations"]
            .as_array()
            .ok_or("receipt audit limitations should be an array")?
            .iter()
            .any(|limitation| limitation
                .as_str()
                .unwrap_or("")
                .contains("does not execute Miri"))
    );
    assert!(
        value["limitations"]
            .as_array()
            .ok_or("receipt audit limitations should be an array")?
            .iter()
            .any(|limitation| limitation
                .as_str()
                .unwrap_or("")
                .contains("do not erase missing contracts"))
    );
    let receipt = &value["receipts"][0];
    assert_eq!(receipt["receipt_tool"], "miri");
    assert_eq!(receipt["summary"], "focused fixture witness passed");
    assert_eq!(receipt["author"], "core/fixtures");
    assert_eq!(receipt["recorded_at"], "2026-05-18T00:00:00Z");
    assert_eq!(receipt["expires_at"], "2026-08-18");
    assert_eq!(receipt["command_hash"], "3e163b0bce29ff2e");
    assert_eq!(receipt["limitations"][0], "fixture only");
    assert!(
        receipt["statuses"]
            .as_array()
            .ok_or("statuses should be an array")?
            .iter()
            .any(|status| status == "matched")
    );
    assert!(
        receipt["statuses"]
            .as_array()
            .ok_or("statuses should be an array")?
            .iter()
            .any(|status| status == "imports_witness_evidence")
    );
    assert_eq!(receipt["matched_card"]["class"], "guard_missing");
    assert_eq!(
        receipt["matched_card"]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(
        receipt["matched_card"]["operation_family"],
        "raw_pointer_read"
    );
    assert_eq!(receipt["matched_card"]["missing_count"], 2);
    assert!(
        receipt["matched_card"]["next_action"]
            .as_str()
            .unwrap_or("")
            .contains("Add or expose")
    );

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
    assert!(markdown.contains("## Reviewer front panel"));
    assert!(markdown.contains("- Matched receipt metadata: 1"));
    assert!(markdown.contains("- Receipts imported as current witness evidence: 1"));
    assert!(markdown.contains("- Receipts without a current card match: 0 unmatched, 0 stale"));
    assert!(markdown.contains("- Problem flags: none"));
    assert!(markdown.contains("keep matching receipt metadata attached to the review record"));
    assert!(markdown.contains("do not erase missing contracts"));
    assert!(markdown.contains("Duplicate"));
    assert!(markdown.contains("Matched target"));
    assert!(markdown.contains("Summary"));
    assert!(markdown.contains("focused fixture witness passed"));
    assert!(markdown.contains("imports_witness_evidence, matched"));
    assert!(markdown.contains("core/fixtures"));
    assert!(markdown.contains("2026-05-18T00:00:00Z"));
    assert!(markdown.contains("2026-08-18"));
    assert!(markdown.contains("3e163b0bce29ff2e"));
    assert!(markdown.contains("fixture only"));
    assert!(markdown.contains("raw_pointer_read"));
    assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(markdown.contains("Add or expose"));
    assert!(markdown.contains("## Limitations"));
    assert!(markdown.contains("does not execute Miri"));
    assert!(markdown.contains("do not erase missing contracts"));
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
    assert_eq!(receipt["command_hash"], "3e163b0bce29ff2e");
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
    assert_eq!(receipt["command_hash"], "efd39ded576cc7d5");
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
    assert_eq!(receipt["command_hash"], "a81457494b1bfe76");
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
    assert_eq!(receipt["command_hash"], "4ce9d7c8eeb19a30");
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
    assert_eq!(receipt["command_hash"], "c17a7978dc51122e");
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
    assert_eq!(report["schema_version"], "0.1");
    assert_eq!(report["limitations"].as_array().map(Vec::len), Some(4));
    assert!(
        json_str(
            &report["classification_explanations"]["new_gap"],
            "classification_explanations.new_gap"
        )?
        .contains("baseline ledger or active suppression ledger")
    );
    assert_eq!(report["summary"]["new_gaps"], 1);
    assert_eq!(report["summary"]["baseline_known"], 0);
    assert_eq!(report["summary"]["unmatched_baseline"], 0);
    assert_eq!(report["summary"]["invalid_ledger_entries"], 0);
    assert_eq!(
        report["cards"][0]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(report["cards"][0]["operation_family"], "raw_pointer_read");
    assert!(
        json_str(&report["cards"][0]["next_action"], "cards[0].next_action")?
            .contains("Add or expose")
    );
    assert!(
        json_str(
            &report["cards"][0]["policy_reason"],
            "cards[0].policy_reason"
        )?
        .contains("was not found in the baseline ledger")
    );
    assert!(report["unmatched_baseline"].as_array().is_some());
    assert!(report["invalid_ledger_entries"].as_array().is_some());
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
    assert_eq!(baselined["summary"]["resolved_baseline"], 1);
    assert_eq!(baselined["summary"]["unmatched_baseline"], 1);
    assert_eq!(baselined["summary"]["invalid_ledger_entries"], 0);
    assert_eq!(
        baselined["resolved_baseline"][0]["evidence"],
        "resolved fixture card"
    );
    assert_eq!(
        baselined["unmatched_baseline"][0]["card_id"],
        baselined["resolved_baseline"][0]["card_id"]
    );

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
    assert!(markdown.contains("## Reviewer front panel"));
    assert!(markdown.contains("- New unbaselined gaps: 0"));
    assert!(markdown.contains("- Current ledger-covered cards: 1 baseline-known, 0 suppressed"));
    assert!(markdown.contains("- Ledger cleanup: 1 resolved baseline entries"));
    assert!(markdown.contains("consider pruning or updating resolved baseline entries"));
    assert!(markdown.contains("advisory policy simulation only"));
    assert!(markdown.contains("## Classification explanations"));
    assert!(markdown.contains("Exact ReviewCard identity matched a baseline ledger entry"));
    assert!(markdown.contains("Next action"));
    assert!(markdown.contains("| Status | Reason | Card | Class |"));
    assert!(markdown.contains("raw_pointer_read"));
    assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(markdown.contains("Known baseline card"));
    assert!(markdown.contains("| Card | Owner | Review after | Expires | Reason | Evidence |"));
    assert!(markdown.contains("resolved fixture card"));
    assert!(markdown.contains("## Limitations"));
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

fn assert_public_badge_payload(value: &Value) -> Result<(), Box<dyn Error>> {
    let object = value
        .as_object()
        .ok_or("badge endpoint payload should be a JSON object")?;
    for key in object.keys() {
        if !["schemaVersion", "label", "message", "color"].contains(&key.as_str()) {
            return Err(format!("badge endpoint contains non-Shields field `{key}`").into());
        }
    }
    for internal in [
        "contract_version",
        "kind",
        "scope",
        "basis",
        "status",
        "counts",
    ] {
        assert!(
            value.get(internal).is_none(),
            "public badge JSON must not contain internal field `{internal}`"
        );
    }
    Ok(())
}

fn json_usize(value: &Value, field: &str) -> Result<usize, Box<dyn Error>> {
    Ok(value
        .as_u64()
        .ok_or_else(|| format!("{field} must be an unsigned count"))?
        .try_into()
        .map_err(|_err| format!("{field} does not fit in usize"))?)
}

fn json_str<'a>(value: &'a Value, path: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .as_str()
        .ok_or_else(|| format!("{path} should be a string").into())
}

fn json_array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, Box<dyn Error>> {
    value
        .as_array()
        .ok_or_else(|| format!("{path} should be an array").into())
}

fn assert_default_repo_status_scope(
    status: &Value,
    root: &Path,
    files_remaining: u64,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(status["scan_scope"]["root"], root.display().to_string());
    assert_eq!(
        json_array(&status["scan_scope"]["include"], "scan_scope.include")?.len(),
        0
    );
    assert_eq!(
        json_array(&status["scan_scope"]["exclude"], "scan_scope.exclude")?.len(),
        0
    );
    assert_eq!(status["scan_scope"]["respect_gitignore"], true);
    assert_eq!(status["scan_scope"]["large_repo_ignores"], true);
    assert!(status["scan_scope"]["max_files"].is_null());
    assert_eq!(status["files_remaining"], files_remaining);
    Ok(())
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn manual_candidate_example_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/manual-candidates/textdecoder-sab.json")
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

[[entries]]
card_id = "UR-resolved-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
owner = "core/policy"
reason = "resolved e2e baseline"
evidence = "resolved fixture card"
review_after = "2026-08-01"
"#
        ),
    )?;
    Ok(())
}

fn write_e2e_file(root: &Path, rel: &str) -> Result<(), Box<dyn Error>> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, "unsafe fn fixture_data() {}\n")?;
    Ok(())
}

fn manual_candidate_json() -> &'static str {
    include_str!("../../../docs/examples/manual-candidates/textdecoder-sab.json")
}

fn empty_review_card_snapshot_json() -> &'static str {
    r#"{
  "schema_version": "0.1",
  "summary": {
    "cards": 0,
    "open_actionable_gaps": 0
  },
  "cards": []
}"#
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
