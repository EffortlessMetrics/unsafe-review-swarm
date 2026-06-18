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
    assert_eq!(value["schema_version"], "0.2");
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

    // schema 0.2 provenance block assertions (instrument-truthfulness lane)
    assert!(
        value["tool_version"].is_string(),
        "tool_version must be present in schema 0.2 artifact"
    );
    assert!(
        value["provenance"].is_object(),
        "provenance block must be present in schema 0.2 artifact"
    );
    assert!(
        value["provenance"]["diff_sha256"]
            .as_str()
            .is_some_and(|s| s.len() == 64),
        "provenance.diff_sha256 must be a 64-char hex string when --diff <file> is used"
    );
    let generated_at = json_str(
        &value["provenance"]["generated_at"],
        "provenance.generated_at",
    )?;
    assert!(
        generated_at.ends_with('Z') && generated_at.contains('T'),
        "provenance.generated_at must be RFC3339 UTC: {generated_at}"
    );

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
    assert!(markdown.contains(
        "| ID | Class | Proof path | Operation | Hazard | Missing | Route | Next action |"
    ));
    assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(markdown.contains("Add or expose local guards"));

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
    assert!(summary_text.contains("- Diff scope: 1 file changed (1 Rust, 0 non-Rust)"));
    assert!(summary_text.contains("## Card table"));
    assert!(summary_text.contains("- Operation: `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(summary_text.contains("- Operation family: `raw_pointer_read`"));
    assert!(
        summary_text
            .contains("| ID | Class | Proof path | Location | Operation family | Operation |")
    );
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
    assert!(github_summary_text.contains("- Diff scope: 1 file changed (1 Rust, 0 non-Rust)"));
    assert!(github_summary_text.contains("## Top card"));
    assert!(github_summary_text.contains(&format!("- ID: `{card_id}`")));
    assert!(github_summary_text.contains(&format!("- Explain: `unsafe-review explain {card_id}`")));
    assert!(github_summary_text.contains(&format!(
        "- Agent context: `unsafe-review context {card_id} --json`"
    )));
    assert!(github_summary_text.contains("- Agent handoff: `ready_for_agent`"));
    assert!(github_summary_text.contains("bucket reasons: `guard_evidence_missing`"));
    assert!(github_summary_text.contains("readiness reasons: specific operation family"));
    assert!(github_summary_text.contains("## Open next"));
    assert!(github_summary_text.contains("Full reviewer cockpit: `pr-summary.md`"));
    assert!(github_summary_text.contains("not a site-execution claim"));
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
            .contains("Add or expose local guards")
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
                && action["payload"]["line"] == 16
                && action["payload"]["name"] == "reads_header"
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
        packet["confirmation_cue"]["build_this_first"]["kind"],
        "verify_command"
    );
    assert_eq!(
        packet["confirmation_cue"]["build_this_first"]["command"],
        "cargo +nightly miri test read_header"
    );
    assert!(
        packet["confirmation_cue"]["confirmation_step"]
            .as_str()
            .unwrap_or("")
            .contains("attach a matching receipt")
    );
    assert_eq!(
        packet["confirmation_cue"]["minimal_repro"]["kind"],
        "verify_command"
    );
    assert!(
        serde_json::to_string(&packet["confirmation_cue"]["minimal_repro"])?
            .contains("unsafe-review did not run this command")
    );
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
        "reads_header"
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
    assert!(
        explain.contains(
            "Add or expose local guards for these `raw_pointer_read` safety obligations:"
        )
    );
    assert!(explain.contains("## What would resolve this"));
    assert!(
        explain.contains(
            "- Add or expose local guards for these `raw_pointer_read` safety obligations:"
        )
    );
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
        mixed_diff_path.as_os_str().to_os_string(),
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

    let github_summary = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        mixed_diff_path.as_os_str().to_os_string(),
        os("--format"),
        os("github-summary"),
    ])?;
    let github_summary = stdout_text(&github_summary)?;
    assert!(github_summary.contains("- Diff scope: 3 files changed (1 Rust, 2 non-Rust)"));
    assert!(github_summary.contains("## unsafe-review advisory summary"));

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
    assert_eq!(packet["context"]["operation_family"], "unsafe_declaration");

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
    assert!(reasons.contains("operation family `unsafe_declaration`"));
    assert!(reasons.contains("no verify command"));
    assert!(
        packet["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );

    Ok(())
}

/// Pin: an unsafe fn declaration emits a contract_missing card in diff scope.
/// The card is advisory-only with operation_family = "unsafe_declaration".
#[test]
fn unsafe_declaration_family_unsafe_fn_emits_contract_missing_card_in_diff_scope()
-> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("unsafe_fn_unknown_family_no_card");

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
    assert_eq!(
        value["summary"]["cards"], 1,
        "unsafe declaration family must emit a card in diff scope"
    );
    assert_eq!(value["cards"][0]["class"], "contract_missing");
    assert_eq!(value["cards"][0]["operation_family"], "unsafe_declaration");
    assert_eq!(value["cards"][0]["site"]["kind"], "unsafe_fn");

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
        canonical["evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert_eq!(canonical["oracle_map"]["oracle_language"], "typescript");
    assert_eq!(
        canonical["oracle_map"]["oracle_path"],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        canonical["oracle_map"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not witness execution")
    );
    assert!(
        canonical["evidence"][1]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not analyzer-discovered")
    );
    write_textdecoder_stable_byte_seed_ledger(temp.path())?;

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
    assert_eq!(outcome["cards"]["new"][0]["after"]["evidence_count"], 3);
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["invariant"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["oracle_map"]["oracle_path"],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["oracle_map"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("memory-safety proof")
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["evidence"][1]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not memory-safety proof")
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["fix_options"][0]
            .as_str()
            .unwrap_or("")
            .contains("Copy SharedArrayBuffer-backed bytes")
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["test_targets"][0],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["do_not_touch"][0]
            .as_str()
            .unwrap_or("")
            .contains("Do not rewrite unrelated TextDecoder")
    );
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
    assert!(outcome_markdown.contains("route `new TextDecoder().decode"));
    assert!(outcome_markdown.contains("invariant &[u8] memory must not be concurrently mutated"));
    assert!(outcome_markdown.contains("first evidence `source_trace`"));
    assert!(outcome_markdown.contains("command `rg -n"));
    assert!(outcome_markdown.contains("limitation source trace only"));
    assert!(outcome_markdown.contains("first fix: Copy SharedArrayBuffer-backed bytes"));
    assert!(
        outcome_markdown
            .contains("first test: test/js/webcore/textdecoder-sharedarraybuffer.test.ts")
    );
    assert!(outcome_markdown.contains("first non-goal: Do not rewrite unrelated TextDecoder"));
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
        explain_packet["implementer_handoff"]["external_evidence"][1]["kind"],
        "runtime_witness"
    );
    assert_eq!(
        explain_packet["implementer_handoff"]["external_evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        explain_packet["implementer_handoff"]["external_evidence"][1]["limitation"]
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
        context_packet["evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert_eq!(context_packet["stable_byte_seed_source"]["included"], true);
    assert_eq!(
        context_packet["stable_byte_seed_source"]["matched_manual_candidates"],
        1
    );
    assert!(
        context_packet["stable_byte_seed_source"]["relationship"]
            .as_str()
            .unwrap_or("")
            .contains("manual candidate context packets")
    );
    assert_eq!(
        context_packet["stable_byte_seed"]["seed_id"],
        "bun-stable-byte-textdecoder-sab"
    );
    assert_eq!(context_packet["stable_byte_seed"]["owner_lane"], "rust2");
    assert_eq!(
        context_packet["stable_byte_seed"]["suggested_first_pr"],
        "TextDecoder shared-byte snapshot only"
    );
    assert_eq!(
        context_packet["stable_byte_seed"]["triage_labels"][1],
        "needs-miri-model"
    );
    assert_eq!(
        context_packet["stable_byte_seed"]["candidate_consistency"]["proof_mode_matches_manual_candidate"],
        true
    );
    assert!(
        context_packet["stable_byte_seed"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not Miri-clean status")
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
fn candidate_import_rejects_malformed_packet_and_preserves_source_manual_on_valid()
-> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-candidate-import-reject-e2e")?;

    // Invalid JSON is rejected with a parse error.
    let not_json = temp.path().join("not-json.json");
    fs::write(&not_json, b"not json")?;
    let bad_parse = run_failure([
        os("candidate"),
        os("import"),
        not_json.as_os_str().to_os_string(),
    ])?;
    let bad_parse_stderr = String::from_utf8_lossy(&bad_parse.stderr).to_string();
    assert!(
        bad_parse_stderr.contains("parse manual candidate"),
        "import should report parse error: {bad_parse_stderr}"
    );
    assert_eq!(
        bad_parse.status.code(),
        Some(2),
        "import should exit 2 on malformed input"
    );

    // Wrong schema_version is rejected naming the field.
    let wrong_schema = temp.path().join("wrong-schema.json");
    fs::write(
        &wrong_schema,
        br#"{
          "schema_version": "manual-candidate/v0",
          "id": "R4R2-S001",
          "title": "t",
          "location": {"file": "f.rs", "line": 1},
          "operation_family": "raw_pointer_read",
          "unsafe_operation": "op",
          "invariant": "inv",
          "safe_caller": "caller",
          "evidence": [],
          "trust_boundary": "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness"
        }"#,
    )?;
    let bad_schema = run_failure([
        os("candidate"),
        os("import"),
        wrong_schema.as_os_str().to_os_string(),
    ])?;
    let bad_schema_stderr = String::from_utf8_lossy(&bad_schema.stderr).to_string();
    assert!(
        bad_schema_stderr.contains("schema_version"),
        "import should name schema_version on version mismatch: {bad_schema_stderr}"
    );

    // A valid committed example is accepted with source = manual and
    // analyzer_discovered = false enforced by the importer.
    let out = temp.path().join("R4R2-S001.json");
    let import = run_success([
        os("candidate"),
        os("import"),
        manual_candidate_example_path().into_os_string(),
        os("--out"),
        out.as_os_str().to_os_string(),
    ])?;
    let import_stdout = stdout_text(&import)?;
    assert!(
        import_stdout.contains("source: manual"),
        "import stdout should confirm source=manual: {import_stdout}"
    );
    assert!(
        import_stdout.contains("manual_candidate: true"),
        "import stdout should confirm manual_candidate: {import_stdout}"
    );
    let canonical: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
    assert_eq!(
        canonical["source"], "manual",
        "canonical artifact must have source=manual"
    );
    assert_eq!(
        canonical["manual_candidate"], true,
        "canonical artifact must have manual_candidate=true"
    );
    assert_eq!(
        canonical["analyzer_discovered"], false,
        "canonical artifact must have analyzer_discovered=false"
    );

    Ok(())
}

#[test]
fn candidate_new_skeleton_is_schema_correct_but_fails_lint_on_todos() -> Result<(), Box<dyn Error>>
{
    let temp = TempDir::new("unsafe-review-candidate-new-e2e")?;
    let draft = temp.path().join("draft-candidate.json");

    let new = run_success([
        os("candidate"),
        os("new"),
        os("--class"),
        os("stable-byte-source-getter-reentry"),
        os("--out"),
        draft.as_os_str().to_os_string(),
    ])?;
    let new_stdout = stdout_text(&new)?;
    assert!(new_stdout.contains("wrote manual candidate skeleton"));
    assert!(new_stdout.contains("id: R4R2-S000-TODO"));
    assert!(new_stdout.contains("stable-byte class: stable-byte-source-getter-reentry"));
    assert!(new_stdout.contains("source: manual"));
    assert!(new_stdout.contains("not analyzer discovery"));

    let skeleton = parse_json(&fs::read_to_string(&draft)?)?;
    assert_eq!(skeleton["schema_version"], "manual-candidate/v1");
    assert_eq!(skeleton["source"], "manual");
    assert_eq!(skeleton["manual_candidate"], true);
    assert_eq!(skeleton["analyzer_discovered"], false);
    assert_eq!(
        skeleton["stable_byte"]["class"],
        "stable-byte-source-getter-reentry"
    );
    assert_eq!(
        skeleton["stable_byte"]["proof_required"],
        skeleton["proof_mode"]["kind"]
    );
    assert_eq!(
        skeleton["stable_byte"]["suggested_fix_boundary"],
        skeleton["fix_boundary"]
    );
    assert_eq!(
        skeleton["stable_byte"]["pr_aperture"],
        skeleton["pr_aperture"]
    );
    assert!(
        skeleton["invariant"]
            .as_str()
            .unwrap_or("")
            .contains("TODO")
    );

    // The skeleton passes the structural import validation; only the TODO
    // markers keep it from being a finished authoring packet.
    let imported = temp.path().join("imported-skeleton.json");
    run_success([
        os("candidate"),
        os("import"),
        draft.as_os_str().to_os_string(),
        os("--out"),
        imported.as_os_str().to_os_string(),
    ])?;

    let lint = run_failure([
        os("candidate"),
        os("lint"),
        draft.as_os_str().to_os_string(),
    ])?;
    assert_eq!(lint.status.code(), Some(2));
    let lint_stderr = String::from_utf8_lossy(&lint.stderr).to_string();
    assert!(
        lint_stderr.contains("candidate lint:"),
        "stderr should name candidate lint: {lint_stderr}"
    );
    assert!(
        lint_stderr.contains("todo: `title` still contains TODO placeholder text"),
        "stderr should flag the title TODO: {lint_stderr}"
    );
    assert!(
        lint_stderr.contains("todo: `invariant`"),
        "stderr should flag the invariant TODO: {lint_stderr}"
    );
    assert!(
        !lint_stderr.contains("schema: "),
        "skeleton should have no schema problems: {lint_stderr}"
    );

    Ok(())
}

#[test]
fn candidate_lint_accepts_all_committed_manual_candidate_examples() -> Result<(), Box<dyn Error>> {
    let dir = manual_candidate_examples_dir();
    let mut linted = 0usize;
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let lint = run_success([os("candidate"), os("lint"), path.as_os_str().to_os_string()])?;
        let stdout = stdout_text(&lint)?;
        assert!(
            stdout.contains("candidate lint: ok"),
            "{} should lint clean: {stdout}",
            path.display()
        );
        assert!(
            stdout.contains("not analyzer discovery"),
            "{} lint output should keep the advisory boundary: {stdout}",
            path.display()
        );
        linted += 1;
    }
    assert!(linted > 0, "no committed manual candidate examples found");

    Ok(())
}

#[test]
fn candidate_lint_reports_cross_field_and_todo_problems() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-candidate-lint-e2e")?;
    let example =
        manual_candidate_examples_dir().join("candidate7-sync-compression-getter-reentry.json");
    let dirty = fs::read_to_string(&example)?
        .replacen(
            "\"proof_required\": \"observable-red-green\"",
            "\"proof_required\": \"mutation-plus-miri\"",
            1,
        )
        .replacen(
            "\"invariant\": \"Sync compression must not read bytes",
            "\"invariant\": \"TODO: describe the invariant at risk",
            1,
        );
    let dirty_path = temp.path().join("dirty-candidate.json");
    fs::write(&dirty_path, dirty)?;

    let lint = run_failure([
        os("candidate"),
        os("lint"),
        dirty_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(lint.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&lint.stderr).to_string();
    assert!(
        stderr.contains("schema: ") && stderr.contains("stable_byte.proof_required"),
        "stderr should report the cross-field drift: {stderr}"
    );
    assert!(
        stderr.contains("todo: `invariant` still contains TODO placeholder text"),
        "stderr should report the TODO marker: {stderr}"
    );
    assert!(
        stderr.contains("nothing was imported or written"),
        "stderr should state lint imports nothing: {stderr}"
    );

    Ok(())
}

#[test]
fn manual_candidate_list_reports_imported_advisory_ledger() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-manual-candidate-list-e2e")?;
    let candidate_dir = temp.path().join(".unsafe-review/candidates");
    fs::create_dir_all(&candidate_dir)?;
    fs::write(
        candidate_dir.join("R4R2-S002.json"),
        mysql_manual_candidate_json(),
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
    assert_eq!(ledger["summary"]["external_evidence_refs"], 6);
    assert_eq!(
        ledger["summary"]["operation_families"]["raw_pointer_read"],
        1
    );
    assert_eq!(
        ledger["summary"]["operation_families"]["slice_from_raw_parts"],
        1
    );
    assert_eq!(ledger["summary"]["evidence_kinds"]["model"], 2);
    assert_eq!(ledger["summary"]["evidence_kinds"]["runtime_witness"], 2);
    assert_eq!(ledger["summary"]["evidence_kinds"]["source_trace"], 2);
    assert_eq!(ledger["summary"]["analyzer_discovered"], 0);
    assert_eq!(ledger["candidates"][0]["id"], "R4R2-S001");
    assert_eq!(ledger["candidates"][1]["id"], "R4R2-S002");
    assert_eq!(ledger["candidates"][0]["source"], "manual");
    assert_eq!(ledger["candidates"][0]["manual_candidate"], true);
    assert_eq!(ledger["candidates"][0]["analyzer_discovered"], false);
    assert_eq!(ledger["candidates"][1]["source"], "manual");
    assert_eq!(ledger["candidates"][1]["manual_candidate"], true);
    assert_eq!(ledger["candidates"][1]["analyzer_discovered"], false);
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
    assert_eq!(
        ledger["candidates"][0]["proof_mode"]["kind"],
        "mutation-plus-miri"
    );
    assert_eq!(
        ledger["candidates"][0]["proof_mode"]["system_bun_expected"],
        "nondiscriminating"
    );
    assert_eq!(
        ledger["candidates"][0]["fix_boundary"],
        "Snapshot shared/growable/resizable bytes before Rust receives &[u8]"
    );
    assert!(
        ledger["candidates"][0]["pr_aperture"]
            .as_str()
            .unwrap_or("")
            .contains("do not patch S3")
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["proof_mode"]["kind"],
        "mutation-plus-miri"
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["fix_boundary"],
        ledger["candidates"][0]["fix_boundary"]
    );
    assert_eq!(
        ledger["candidates"][0]["implementer_handoff"]["pr_aperture"],
        ledger["candidates"][0]["pr_aperture"]
    );
    assert!(
        ledger["candidates"][0]["implementer_handoff"]["stop_condition"]
            .as_str()
            .unwrap_or("")
            .contains("stop before source edits")
    );
    assert_eq!(
        ledger["candidates"][1]["location_text"],
        "src/sql_jsc/mysql/MySQLValue.rs:411"
    );
    assert_eq!(
        ledger["candidates"][1]["implementer_handoff"]["target"]["location_text"],
        "src/sql_jsc/mysql/MySQLValue.rs:411"
    );
    assert_eq!(
        ledger["candidates"][1]["implementer_handoff"]["route"]["safe_caller"],
        "Bun.SQL MySQL prepared statement binding a SharedArrayBuffer-backed Uint8Array as a BLOB parameter"
    );
    assert_eq!(
        ledger["candidates"][1]["implementer_handoff"]["route"]["unsafe_operation"],
        "JSC__JSValue__borrowBytesForOffThread -> core::slice::from_raw_parts -> Data::Temporary(RawSlice)"
    );
    assert_eq!(
        ledger["candidates"][1]["implementer_handoff"]["invariant_at_risk"],
        "MySQL packet construction must not borrow mutable or shared JS backing storage through a temporary raw slice"
    );
    assert!(
        ledger["candidates"][1]["fix_options"][0]
            .as_str()
            .unwrap_or("")
            .contains("stable BufferSource copy helper")
    );
    assert_eq!(
        ledger["candidates"][1]["test_targets"][0],
        "test/js/sql/sql-mysql-bind-blob-borrow.test.ts"
    );
    assert!(
        ledger["candidates"][1]["do_not_touch"][1]
            .as_str()
            .unwrap_or("")
            .contains("Postgres bytea parity")
    );
    assert!(
        ledger["candidates"][1]["implementer_handoff"]["fix_options"][1]
            .as_str()
            .unwrap_or("")
            .contains("owned or stable bytes")
    );
    assert_eq!(
        ledger["candidates"][1]["implementer_handoff"]["test_targets"][1],
        "bun target/unsafe-scout-mysql/mysql-blob-sab-matrix.js"
    );
    assert!(
        ledger["candidates"][1]["implementer_handoff"]["do_not_touch"][0]
            .as_str()
            .unwrap_or("")
            .contains("unrelated MySQL protocol packet")
    );
    assert_eq!(
        ledger["candidates"][1]["evidence"][0]["kind"],
        "source_trace"
    );
    assert_eq!(
        ledger["candidates"][1]["evidence"][1]["command"],
        "bun target/unsafe-scout-mysql/mysql-blob-sab-matrix.js"
    );
    assert!(
        ledger["candidates"][1]["evidence"][2]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("does not prove the Bun site executed under Miri")
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
    assert_eq!(
        ledger["reviewcard_artifact_applicability"]["cards.sarif"]["decision"],
        "reviewcard_only"
    );
    assert_eq!(
        ledger["reviewcard_artifact_applicability"]["comment-plan.json"]["applies_to_manual_candidates"],
        false
    );
    assert_eq!(
        ledger["reviewcard_artifact_applicability"]["policy-report.json"]["manual_candidate_markers_allowed"],
        false
    );
    assert_eq!(
        ledger["reviewcard_artifact_applicability"]["policy-report.json"]["decision"],
        "reviewcard_only"
    );
    assert_eq!(
        ledger["reviewcard_artifact_applicability"]["policy-report.md"]["manual_candidate_markers_allowed"],
        false
    );
    assert_eq!(
        ledger["reviewcard_artifact_applicability"]["policy-report.md"]["decision"],
        "reviewcard_only"
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
    assert!(
        markdown.contains("Operation families: `raw_pointer_read: 1, slice_from_raw_parts: 1`")
    );
    assert!(markdown.contains("Evidence kinds: `model: 2, runtime_witness: 2, source_trace: 2`"));
    assert!(markdown.contains("Analyzer-discovered: `0`"));
    assert!(markdown.contains("### `R4R2-S001`"));
    assert!(markdown.contains("Location: `src/runtime/webcore/TextDecoder.rs:237`"));
    assert!(markdown.contains("#### Implementer Handoff"));
    assert!(markdown.contains("Route: `new TextDecoder().decode"));
    assert!(markdown.contains("Invariant at risk: &[u8] memory must not be concurrently mutated"));
    assert!(markdown.contains("Evidence packet: `3` external reference(s)"));
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
    assert!(markdown.contains("### `R4R2-S002`"));
    assert!(markdown.contains("Location: `src/sql_jsc/mysql/MySQLValue.rs:411`"));
    assert!(markdown.contains("Route: `Bun.SQL MySQL prepared statement"));
    assert!(
        markdown.contains(
            "JSC__JSValue__borrowBytesForOffThread -> core::slice::from_raw_parts -> Data::Temporary(RawSlice)"
        )
    );
    assert!(markdown.contains("Evidence packet: `3` external reference(s)"));
    assert!(markdown.contains("Fix options:"));
    assert!(markdown.contains("owned or stable bytes before storing Data::Temporary"));
    assert!(markdown.contains("Test targets:"));
    assert!(markdown.contains("test/js/sql/sql-mysql-bind-blob-borrow.test.ts"));
    assert!(markdown.contains("Do not touch:"));
    assert!(markdown.contains("Postgres bytea parity"));
    assert!(markdown.contains("`source_trace` at `src/sql_jsc/mysql/MySQLValue.rs`"));
    assert!(markdown.contains(
        "Bun.SQL MySQL BLOB matrix covers SharedArrayBuffer-backed typed-array parameters"
    ));
    assert!(markdown.contains("Command: `bun target/unsafe-scout-mysql/mysql-blob-sab-matrix.js`"));
    assert!(markdown.contains("does not prove the Bun site executed under Miri"));

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
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["invariant"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["fix_options"][0]
            .as_str()
            .unwrap_or("")
            .contains("Copy SharedArrayBuffer-backed bytes")
    );
    assert_eq!(
        outcome["cards"]["new"][0]["after"]["test_targets"][0],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        outcome["cards"]["new"][0]["after"]["do_not_touch"][0]
            .as_str()
            .unwrap_or("")
            .contains("Do not rewrite unrelated TextDecoder")
    );
    assert_eq!(
        outcome["cards"]["new"][1]["after"]["safe_caller"],
        "Bun.SQL MySQL prepared statement binding a SharedArrayBuffer-backed Uint8Array as a BLOB parameter"
    );
    assert!(
        outcome["cards"]["new"][1]["after"]["invariant"]
            .as_str()
            .unwrap_or("")
            .contains("MySQL packet construction")
    );
    assert_eq!(
        outcome["cards"]["new"][1]["after"]["evidence"][1]["command"],
        "bun target/unsafe-scout-mysql/mysql-blob-sab-matrix.js"
    );
    assert!(
        outcome["cards"]["new"][1]["after"]["evidence"][2]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("does not prove the Bun site executed under Miri")
    );
    assert!(
        outcome["cards"]["new"][1]["after"]["fix_options"][0]
            .as_str()
            .unwrap_or("")
            .contains("stable BufferSource copy helper")
    );
    assert_eq!(
        outcome["cards"]["new"][1]["after"]["test_targets"][1],
        "bun target/unsafe-scout-mysql/mysql-blob-sab-matrix.js"
    );
    assert!(
        outcome["cards"]["new"][1]["after"]["do_not_touch"][1]
            .as_str()
            .unwrap_or("")
            .contains("Postgres bytea parity")
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
    assert_eq!(
        receipt["matched_manual_candidate"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        receipt["matched_manual_candidate"]["invariant"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert_eq!(
        receipt["matched_manual_candidate"]["oracle_map"]["oracle_language"],
        "typescript"
    );
    assert_eq!(
        receipt["matched_manual_candidate"]["oracle_map"]["oracle_path"],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        receipt["matched_manual_candidate"]["oracle_map"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("site-execution proof")
    );
    assert_eq!(
        receipt["matched_manual_candidate"]["evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        receipt["matched_manual_candidate"]["evidence"][1]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not memory-safety proof")
    );
    assert!(
        receipt["matched_manual_candidate"]["fix_options"][0]
            .as_str()
            .unwrap_or("")
            .contains("Copy SharedArrayBuffer-backed bytes")
    );
    assert_eq!(
        receipt["matched_manual_candidate"]["test_targets"][0],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        receipt["matched_manual_candidate"]["do_not_touch"][0]
            .as_str()
            .unwrap_or("")
            .contains("unrelated TextDecoder")
    );

    let audit_markdown = run_success([
        os("receipt"),
        os("audit"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--diff"),
        temp.path().join("change.diff").into_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let audit_markdown = stdout_text(&audit_markdown)?;
    assert!(audit_markdown.contains("manual_candidate, matched"));
    assert!(audit_markdown.contains("route: new TextDecoder().decode"));
    assert!(audit_markdown.contains("invariant: &[u8] memory must not be concurrently mutated"));
    assert!(audit_markdown.contains("oracle: `typescript`"));
    assert!(audit_markdown.contains("site-execution proof"));
    assert!(audit_markdown.contains("first fix: Copy SharedArrayBuffer-backed bytes"));
    assert!(
        audit_markdown
            .contains("first test: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`")
    );
    assert!(
        audit_markdown
            .contains("first do-not-touch: Do not rewrite unrelated TextDecoder encoding paths")
    );
    assert!(
        audit_markdown
            .contains("source trace only; does not prove the safe JS route reaches this site")
    );
    assert!(!audit_markdown.contains("imports_witness_evidence"));

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
        mysql_manual_candidate_json(),
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
    assert!(stdout.contains("Operation families: raw_pointer_read: 1, slice_from_raw_parts: 1"));
    assert!(stdout.contains("Evidence kinds: model: 2, runtime_witness: 2, source_trace: 2"));
    assert!(stdout.contains("First manual candidate: R4R2-S001"));
    assert!(stdout.contains("Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)"));
    assert!(
        stdout.contains("First test target: test/js/webcore/textdecoder-sharedarraybuffer.test.ts")
    );
    assert!(stdout.contains("Manual candidate queue preview: first 2 of 2 manual candidate(s)"));
    assert!(stdout.contains(
        "R4R2-S002 at src/sql_jsc/mysql/MySQLValue.rs:411 (slice_from_raw_parts) evidence refs: 3"
    ));
    assert!(stdout.contains("proof mode: mutation-plus-miri"));
    assert!(stdout.contains("unsafe-review explain --root"));
    assert!(stdout.contains("unsafe-review context --root"));
    assert!(stdout.contains("unsafe-review candidate witness-plan --root"));
    assert!(stdout.contains("Review-kit candidate queue: first 2 of 2 manual candidate(s)"));
    assert!(stdout.contains("Manual repair queue:"));
    assert!(stdout.contains("manual-repair-queue.json"));
    assert!(stdout.contains("Tokmd packet export:"));
    assert!(stdout.contains("tokmd-packets.json"));
    assert!(stdout.contains("formatting input only; tokmd was not run"));
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
    assert!(stdout.contains("Hypothesis: static `guard_missing` ReviewCard"));
    assert!(
        stdout.contains(
            "Build/run this first: Build/run `cargo +nightly miri test read_header` first"
        )
    );
    assert!(stdout.contains("Minimal repro cue:"));
    assert!(stdout.contains("Confirm ReviewCard `"));
    assert!(
        stdout
            .contains("Limitation: Minimal repro cue only; unsafe-review did not run this command")
    );
    assert!(
        stdout
            .contains("Confirmation step: build/run `cargo +nightly miri test read_header` first")
    );
    assert!(stdout.contains("Explain top card:"));
    assert!(stdout.contains("Agent packet:"));
    assert!(stdout.contains("Artifacts:"));
    assert!(stdout.contains("review-kit.json"));
    assert!(stdout.contains("unsafe-review-gate.json"));
    assert!(stdout.contains("cards.json"));
    assert!(stdout.contains("pr-summary.md"));
    assert!(stdout.contains("github-summary.md"));
    assert!(stdout.contains("cards.sarif"));
    assert!(stdout.contains("comment-plan.json"));
    assert!(stdout.contains("witness-plan.md"));
    assert!(stdout.contains("receipt-audit.md"));
    assert!(stdout.contains("receipt-audit.json"));
    assert!(stdout.contains("manual-candidates.json"));
    assert!(stdout.contains("manual-repair-queue.json"));
    assert!(stdout.contains("tokmd-packets.json"));
    assert!(stdout.contains("lsp.json"));
    assert!(stdout.contains("repair-queue.json"));
    assert!(stdout.contains("Trust boundary:"));
    assert!(stdout.contains("static unsafe contract review only"));
    assert!(stdout.contains("not memory-safety proof"));
    assert!(stdout.contains("not UB-free status"));
    assert!(stdout.contains("not Miri-clean status"));
    assert!(stdout.contains("not a site-execution claim"));
    assert!(stdout.contains("matching witness receipt"));
    assert!(stdout.contains("did not run witnesses"));
    assert!(stdout.contains("post comments"));
    assert!(stdout.contains("enforce blocking policy"));

    let cards = parse_json(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    assert_eq!(cards["schema_version"], "0.2");
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
            .contains("not memory-safety proof")
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
    assert_eq!(review_kit["summary"]["changed_files"], 1);
    assert_eq!(review_kit["summary"]["changed_rust_files"], 1);
    assert_eq!(review_kit["summary"]["changed_non_rust_files"], 0);
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
    assert_eq!(
        review_kit["handoff"]["review_cards"]["artifact"],
        "cards.json"
    );
    assert_eq!(
        review_kit["handoff"]["review_cards"]["repair_queue_artifact"],
        "repair-queue.json"
    );
    assert_eq!(review_kit["handoff"]["review_cards"]["review_cards"], 1);
    assert_eq!(review_kit["handoff"]["review_cards"]["card_queue_limit"], 5);
    assert_eq!(review_kit["handoff"]["review_cards"]["omitted_cards"], 0);
    let card_queue = json_array(
        &review_kit["handoff"]["review_cards"]["card_queue"],
        "review_kit.handoff.review_cards.card_queue",
    )?;
    assert_eq!(card_queue.len(), 1);
    assert_eq!(card_queue[0]["card_id"], card_id);
    assert_eq!(card_queue[0]["source"], "review_card");
    assert_eq!(card_queue[0]["class"], cards["cards"][0]["class"]);
    assert_eq!(card_queue[0]["priority"], cards["cards"][0]["priority"]);
    assert_eq!(card_queue[0]["confidence"], cards["cards"][0]["confidence"]);
    assert_eq!(card_queue[0]["path"], cards["cards"][0]["site"]["file"]);
    assert_eq!(card_queue[0]["line"], cards["cards"][0]["site"]["line"]);
    assert_eq!(
        card_queue[0]["operation_family"],
        cards["cards"][0]["operation_family"]
    );
    assert_eq!(card_queue[0]["operation"], cards["cards"][0]["operation"]);
    assert_eq!(
        card_queue[0]["missing_evidence"],
        cards["cards"][0]["missing"]
    );
    assert_eq!(
        card_queue[0]["next_action"],
        cards["cards"][0]["next_action"]
    );
    assert_eq!(
        card_queue[0]["verify_commands"],
        cards["cards"][0]["verify_commands"]
    );
    assert_eq!(
        card_queue[0]["witness_routes"],
        cards["cards"][0]["witness_routes"]
    );
    assert_eq!(card_queue[0]["witness_routes"][0]["kind"], "miri");
    assert_eq!(
        card_queue[0]["repair_queue_buckets"][0],
        "repairable_by_guard"
    );
    assert_eq!(
        card_queue[0]["repair_queue_buckets"][1],
        "requires_witness_receipt"
    );
    assert_eq!(
        card_queue[0]["repair_queue_bucket_reasons"][0],
        "guard_evidence_missing"
    );
    assert_eq!(
        card_queue[0]["repair_queue_bucket_reasons"][1],
        "witness_receipt_missing"
    );
    assert_eq!(card_queue[0]["agent_readiness"]["state"], "ready_for_agent");
    assert!(
        card_queue[0]["explain"]
            .as_str()
            .unwrap_or("")
            .contains(card_id)
    );
    assert!(
        card_queue[0]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains("--json")
    );
    assert!(
        review_kit["handoff"]["review_cards"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("repair-queue.json")
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["artifact"],
        "repair-queue.json"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["source"],
        "review_card"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["cards"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["unique_repair_queue_cards"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["agent_ready_cards"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["bucket_counts"]["repairable_by_guard"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["bucket_counts"]["requires_witness_receipt"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["review_card"]["bucket_counts"]["do_not_auto_repair"],
        0
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["artifact"],
        "manual-repair-queue.json"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["source"],
        "manual_candidate"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["manual_candidates"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["queued_candidates"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["bucket"],
        "manual_candidate_handoff"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["bucket_reason"],
        "manual_candidate_copy_only"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["agent_handoff_state"],
        "copy_ready"
    );
    assert_eq!(
        review_kit["handoff"]["repair_queues"]["manual_candidate"]["automatic"],
        false
    );
    assert!(
        review_kit["handoff"]["repair_queues"]["separation"]
            .as_str()
            .unwrap_or("")
            .contains("stay separate source ledgers")
    );
    assert!(
        review_kit["handoff"]["repair_queues"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("does not merge manual candidates into ReviewCard repair-queue.json")
    );
    assert!(
        review_kit["handoff"]["repair_queues"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("does not run agents")
    );
    assert!(
        review_kit["handoff"]["repair_queues"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("is not proof, repair success, or policy readiness")
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
        review_kit["handoff"]["manual_candidates"]["manual_repair_queue_artifact"],
        "manual-repair-queue.json"
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
        review_kit["handoff"]["manual_candidates"]["operation_families"]["raw_pointer_read"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["operation_families"]["slice_from_raw_parts"],
        1
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["evidence_kinds"]["model"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["evidence_kinds"]["runtime_witness"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["evidence_kinds"]["source_trace"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["proof_modes"]["mutation-plus-miri"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["stable_byte_source_classes"]["stable-byte-source-sab-race"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["ledger_states"]["handoff-ready"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_fix_options"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_test_targets"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_do_not_touch"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_oracle_map"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_proof_mode"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_fix_boundary"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["with_pr_aperture"],
        2
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["cards.sarif"]
            ["decision"],
        "reviewcard_only"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["comment-plan.json"]
            ["applies_to_manual_candidates"],
        false
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["policy-report.json"]
            ["manual_candidate_markers_allowed"],
        false
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["policy-report.json"]
            ["decision"],
        "reviewcard_only"
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["policy-report.md"]
            ["manual_candidate_markers_allowed"],
        false
    );
    assert_eq!(
        review_kit["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["policy-report.md"]
            ["decision"],
        "reviewcard_only"
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
    assert_eq!(candidate_queue[0]["evidence_refs"], 3);
    assert_eq!(
        candidate_queue[0]["implementer_handoff"]["route"]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        candidate_queue[0]["implementer_handoff"]["route"]["unsafe_operation"],
        "core::slice::from_raw_parts"
    );
    assert_eq!(
        candidate_queue[1]["location_text"],
        "src/sql_jsc/mysql/MySQLValue.rs:411"
    );
    assert_eq!(
        candidate_queue[1]["operation_family"],
        "slice_from_raw_parts"
    );
    assert_eq!(candidate_queue[1]["evidence_refs"], 3);
    assert!(
        candidate_queue[1]["implementer_handoff"]["fix_options"][0]
            .as_str()
            .unwrap_or("")
            .contains("stable BufferSource copy helper")
    );
    assert_eq!(
        candidate_queue[1]["implementer_handoff"]["test_targets"][2],
        "cargo +nightly miri test mysql_rawslice_shared_bytes_model"
    );
    assert!(
        candidate_queue[1]["implementer_handoff"]["do_not_touch"][2]
            .as_str()
            .unwrap_or("")
            .contains("manual/advisory marker")
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
        "unsafe-review-gate.json",
        "cards.json",
        "pr-summary.md",
        "github-summary.md",
        "cards.sarif",
        "comment-plan.json",
        "witness-plan.md",
        "receipt-audit.md",
        "receipt-audit.json",
        "policy-report.json",
        "policy-report.md",
        "manual-candidates.json",
        "manual-repair-queue.json",
        "tokmd-packets.json",
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
            "cards.json" => {
                assert_eq!(entry["schema_version"], "0.2")
            }
            "review-kit.json" | "comment-plan.json" | "lsp.json" | "repair-queue.json"
            | "policy-report.json" | "receipt-audit.json" => {
                assert_eq!(entry["schema_version"], "0.1")
            }
            "unsafe-review-gate.json" => {
                assert_eq!(entry["schema_version"], "unsafe-review-gate/v1")
            }
            "manual-candidates.json" => {
                assert_eq!(entry["schema_version"], "manual-candidates/v1")
            }
            "manual-repair-queue.json" => {
                assert_eq!(entry["schema_version"], "manual-repair-queue/v1")
            }
            "tokmd-packets.json" => {
                assert_eq!(entry["schema_version"], "tokmd-packets/v1")
            }
            "cards.sarif" => assert_eq!(entry["schema_version"], "2.1.0"),
            _ => assert!(entry["schema_version"].is_null()),
        }
    }
    // Verify the gate manifest file content.
    let gate_manifest = parse_json(&fs::read_to_string(
        out_dir.join("unsafe-review-gate.json"),
    )?)?;
    assert_eq!(gate_manifest["schema_version"], "unsafe-review-gate/v1");
    assert_eq!(gate_manifest["dialect"], "unsafe-review");
    assert_eq!(gate_manifest["status"], "advisory");
    assert_eq!(
        gate_manifest["trust_boundary"],
        "static unsafe-review coverage evidence; not proof, not a merge verdict"
    );
    assert_eq!(gate_manifest["artifacts"]["cards"], "cards.json");
    assert_eq!(
        gate_manifest["artifacts"]["comment_plan"],
        "comment-plan.json"
    );
    assert_eq!(
        gate_manifest["artifacts"]["repair_queue"],
        "repair-queue.json"
    );
    assert_eq!(
        gate_manifest["artifacts"]["receipt_audit"], "receipt-audit.json",
        "gate manifest receipt_audit pointer must resolve to the structured JSON artifact"
    );
    // receipt-audit.json must exist on disk and parse as JSON.
    let receipt_audit_json_path = out_dir.join("receipt-audit.json");
    assert!(
        receipt_audit_json_path.is_file(),
        "receipt-audit.json must be written alongside receipt-audit.md"
    );
    let receipt_audit_json = parse_json(&fs::read_to_string(&receipt_audit_json_path)?)?;
    assert_eq!(
        receipt_audit_json["schema_version"], "0.1",
        "receipt-audit.json must carry schema_version 0.1"
    );
    assert_eq!(
        receipt_audit_json["mode"], "receipt-audit",
        "receipt-audit.json must carry mode receipt-audit"
    );
    // receipt-audit.md must still exist for human consumers (ADDITIVE).
    assert!(
        out_dir.join("receipt-audit.md").is_file(),
        "receipt-audit.md must still be written for human consumers"
    );
    assert_eq!(
        gate_manifest["summary"]["new_gaps"],
        cards["summary"]["new_gaps"]
    );
    assert_eq!(
        gate_manifest["summary"]["worsened_gaps"],
        cards["summary"]["worsened_gaps"]
    );
    assert_eq!(
        gate_manifest["summary"]["resolved_gaps"],
        cards["summary"]["resolved_gaps"]
    );
    assert_eq!(
        gate_manifest["summary"]["inherited_gaps"],
        cards["summary"]["inherited_gaps"]
    );
    assert!(
        gate_manifest.get("generated_at").is_none(),
        "gate manifest must not contain volatile timestamp field"
    );
    assert!(
        gate_manifest.get("wall_seconds").is_none(),
        "gate manifest must not contain volatile wall_seconds field"
    );
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
    assert!(summary.contains("- Diff scope: 1 file changed (1 Rust, 0 non-Rust)"));
    assert!(summary.contains(&format!("- Top card: `{card_id}`")));
    assert!(summary.contains("- Missing/weak evidence:"));
    assert!(summary.contains("- Next reviewer action:"));
    assert!(summary.contains("- Witness route:"));
    assert!(summary.contains("- Receipt audit: `receipt-audit.md`"));
    assert!(summary.contains("no witness was run"));
    assert!(summary.contains(&format!("unsafe-review explain {card_id}")));
    assert!(summary.contains(&format!("unsafe-review context {card_id} --json")));
    assert!(summary.contains("## Trust boundary"));
    assert!(summary.contains("not a site-execution claim"));
    assert_manual_candidate_front_panel(&summary, "## Card table", 2, false);

    let github_summary = fs::read_to_string(out_dir.join("github-summary.md"))?;
    assert!(github_summary.contains("## unsafe-review advisory summary"));
    assert!(github_summary.contains("- Diff scope: 1 file changed (1 Rust, 0 non-Rust)"));
    assert!(github_summary.contains(&format!("- ID: `{card_id}`")));
    assert!(github_summary.contains(&format!("- Explain: `unsafe-review explain {card_id}`")));
    assert!(github_summary.contains(&format!(
        "- Agent context: `unsafe-review context {card_id} --json`"
    )));
    assert!(github_summary.contains("- Agent handoff: `ready_for_agent`"));
    assert!(github_summary.contains("bucket reasons: `guard_evidence_missing`"));
    assert!(github_summary.contains("readiness reasons: specific operation family"));
    assert!(github_summary.contains("## Open next"));
    assert!(github_summary.contains("Review kit manifest: `review-kit.json`"));
    assert!(github_summary.contains("Full reviewer cockpit: `pr-summary.md`"));
    assert!(github_summary.contains("Agent repair queue: `repair-queue.json`"));
    assert!(github_summary.contains("Receipt audit: `receipt-audit.md`"));
    assert!(github_summary.contains("Policy report: `policy-report.md`"));
    assert!(github_summary.contains("Manual candidate index: `manual-candidates.json`"));
    assert!(github_summary.contains("Tokmd packets: `tokmd-packets.json`; tokmd not run"));
    assert!(github_summary.contains("`comment-plan.json` is plan-only"));
    assert!(github_summary.contains("Full advisory bundle"));
    assert!(github_summary.contains("review-kit.json"));
    assert!(github_summary.contains("github-summary.md"));
    assert!(github_summary.contains("receipt-audit.md"));
    assert!(github_summary.contains("policy-report.json"));
    assert!(github_summary.contains("policy-report.md"));
    assert!(github_summary.contains("manual-candidates.json"));
    assert!(github_summary.contains("manual-repair-queue.json"));
    assert!(github_summary.contains("tokmd-packets.json"));
    assert!(github_summary.contains("not memory-safety proof"));
    assert!(github_summary.contains("not a site-execution claim"));
    assert!(github_summary.contains("unsafe-review did not run witnesses"));
    assert!(github_summary.contains("post comments"));
    assert!(github_summary.contains("edit source"));
    assert!(github_summary.contains("enforce blocking policy"));
    assert!(!github_summary.contains("# unsafe-review PR summary"));
    assert!(!github_summary.contains("## Card table"));
    assert_manual_candidate_front_panel(&github_summary, "## Open next", 1, true);

    let manual_candidates =
        parse_json(&fs::read_to_string(out_dir.join("manual-candidates.json"))?)?;
    assert_eq!(manual_candidates["schema_version"], "manual-candidates/v1");
    assert_eq!(manual_candidates["mode"], "manual_candidate_index");
    assert_eq!(manual_candidates["source"], "first_pr");
    assert_eq!(manual_candidates["summary"]["manual_candidates"], 2);
    assert_eq!(
        manual_candidates["summary"]["operation_families"]["raw_pointer_read"],
        1
    );
    assert_eq!(
        manual_candidates["summary"]["operation_families"]["slice_from_raw_parts"],
        1
    );
    assert_eq!(manual_candidates["summary"]["evidence_kinds"]["model"], 2);
    assert_eq!(
        manual_candidates["summary"]["evidence_kinds"]["runtime_witness"],
        2
    );
    assert_eq!(
        manual_candidates["summary"]["evidence_kinds"]["source_trace"],
        2
    );
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
        manual_candidates["candidates"][0]["evidence"][1]["command"],
        "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        manual_candidates["candidates"][0]["evidence"][1]["limitation"]
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
    assert_eq!(
        manual_candidates["candidates"][0]["proof_mode"]["kind"],
        "mutation-plus-miri"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["proof_mode"]["system_bun_expected"],
        "nondiscriminating"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["stable_byte"]["class"],
        "stable-byte-source-sab-race"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["stable_byte"]["proof_required"],
        manual_candidates["candidates"][0]["proof_mode"]["kind"]
    );
    assert_eq!(
        manual_candidates["candidates"][0]["stable_byte"]["ledger_state"],
        "handoff-ready"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["oracle_map"]["oracle_language"],
        "typescript"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["oracle_map"]["oracle_path"],
        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
    );
    assert!(
        manual_candidates["candidates"][0]["oracle_map"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not witness execution")
    );
    assert_eq!(
        manual_candidates["candidates"][0]["fix_boundary"],
        "Snapshot shared/growable/resizable bytes before Rust receives &[u8]"
    );
    assert!(
        manual_candidates["candidates"][0]["pr_aperture"]
            .as_str()
            .unwrap_or("")
            .contains("do not patch S3")
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["proof_mode"]["kind"],
        "mutation-plus-miri"
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["stable_byte"],
        manual_candidates["candidates"][0]["stable_byte"]
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["oracle_map"],
        manual_candidates["candidates"][0]["oracle_map"]
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["fix_boundary"],
        manual_candidates["candidates"][0]["fix_boundary"]
    );
    assert_eq!(
        manual_candidates["candidates"][0]["implementer_handoff"]["pr_aperture"],
        manual_candidates["candidates"][0]["pr_aperture"]
    );
    assert!(
        manual_candidates["candidates"][0]["implementer_handoff"]["stop_condition"]
            .as_str()
            .unwrap_or("")
            .contains("stop before source edits")
    );
    assert!(
        manual_candidates["candidates"][1]["fix_options"][2]
            .as_str()
            .unwrap_or("")
            .contains("local to MySQL bind-value conversion")
    );
    assert_eq!(
        manual_candidates["candidates"][1]["test_targets"][0],
        "test/js/sql/sql-mysql-bind-blob-borrow.test.ts"
    );
    assert!(
        manual_candidates["candidates"][1]["do_not_touch"][0]
            .as_str()
            .unwrap_or("")
            .contains("unrelated MySQL protocol packet")
    );
    assert!(
        manual_candidates["candidates"][1]["implementer_handoff"]["fix_options"][1]
            .as_str()
            .unwrap_or("")
            .contains("owned or stable bytes")
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
    assert_eq!(
        manual_candidates["reviewcard_artifact_applicability"]["cards.sarif"]["decision"],
        "reviewcard_only"
    );
    assert_eq!(
        manual_candidates["reviewcard_artifact_applicability"]["comment-plan.json"]["applies_to_manual_candidates"],
        false
    );
    assert_eq!(
        manual_candidates["reviewcard_artifact_applicability"]["policy-report.json"]["manual_candidate_markers_allowed"],
        false
    );
    assert_eq!(
        manual_candidates["reviewcard_artifact_applicability"]["policy-report.json"]["decision"],
        "reviewcard_only"
    );
    assert_eq!(
        manual_candidates["reviewcard_artifact_applicability"]["policy-report.md"]["manual_candidate_markers_allowed"],
        false
    );
    assert_eq!(
        manual_candidates["reviewcard_artifact_applicability"]["policy-report.md"]["decision"],
        "reviewcard_only"
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

    let manual_repair_queue = parse_json(&fs::read_to_string(
        out_dir.join("manual-repair-queue.json"),
    )?)?;
    assert_eq!(
        manual_repair_queue["schema_version"],
        "manual-repair-queue/v1"
    );
    assert_eq!(manual_repair_queue["mode"], "manual_candidate_repair_queue");
    assert_eq!(manual_repair_queue["source"], "manual_candidate");
    assert_eq!(manual_repair_queue["policy"], "advisory");
    assert_eq!(manual_repair_queue["summary"]["manual_candidates"], 2);
    assert_eq!(manual_repair_queue["summary"]["queued_candidates"], 2);
    assert_eq!(manual_repair_queue["summary"]["analyzer_discovered"], 0);
    assert_eq!(manual_repair_queue["summary"]["external_evidence_refs"], 6);
    assert_eq!(
        manual_repair_queue["summary"]["operation_families"]["raw_pointer_read"],
        1
    );
    assert_eq!(
        manual_repair_queue["summary"]["operation_families"]["slice_from_raw_parts"],
        1
    );
    assert_eq!(
        manual_repair_queue["summary"]["proof_modes"]["mutation-plus-miri"],
        2
    );
    assert_eq!(
        manual_repair_queue["summary"]["stable_byte_source_classes"]["stable-byte-source-sab-race"],
        2
    );
    assert_eq!(
        manual_repair_queue["summary"]["ledger_states"]["handoff-ready"],
        2
    );
    assert_eq!(manual_repair_queue["summary"]["with_fix_options"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_test_targets"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_do_not_touch"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_oracle_map"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_proof_mode"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_fix_boundary"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_pr_aperture"], 2);
    assert_eq!(manual_repair_queue["summary"]["with_stable_byte_seed"], 0);
    assert_eq!(
        manual_repair_queue["summary"]["stable_byte_seed_source"]["included"],
        false
    );
    assert!(
        manual_repair_queue["summary"]["stable_byte_seed_source"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("Root-local stable-byte seed ledger was absent")
    );
    assert_eq!(manual_repair_queue["queue"][0]["id"], "R4R2-S001");
    assert_eq!(manual_repair_queue["queue"][0]["source"], "manual");
    assert_eq!(manual_repair_queue["queue"][0]["manual_candidate"], true);
    assert_eq!(
        manual_repair_queue["queue"][0]["analyzer_discovered"],
        false
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["location_text"],
        "src/runtime/webcore/TextDecoder.rs:237"
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["safe_caller"],
        "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["invariant_at_risk"],
        "&[u8] memory must not be concurrently mutated"
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["proof_mode"],
        manual_candidates["candidates"][0]["proof_mode"]
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["stable_byte"],
        manual_candidates["candidates"][0]["stable_byte"]
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["oracle_map"],
        manual_candidates["candidates"][0]["oracle_map"]
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["fix_boundary"],
        manual_candidates["candidates"][0]["fix_boundary"]
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["pr_aperture"],
        manual_candidates["candidates"][0]["pr_aperture"]
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["implementer_handoff"],
        manual_candidates["candidates"][0]["implementer_handoff"]
    );
    assert!(
        manual_repair_queue["queue"][0]
            .get("stable_byte_seed")
            .is_none()
    );
    assert_eq!(
        manual_repair_queue["queue"][1]["id"],
        manual_candidates["candidates"][1]["id"]
    );
    assert_eq!(
        manual_repair_queue["queue"][1]["fix_options"],
        manual_candidates["candidates"][1]["fix_options"]
    );
    assert_eq!(
        manual_repair_queue["queue"][1]["test_targets"],
        manual_candidates["candidates"][1]["test_targets"]
    );
    assert_eq!(
        manual_repair_queue["queue"][1]["do_not_touch"],
        manual_candidates["candidates"][1]["do_not_touch"]
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["bucket"],
        "manual_candidate_handoff"
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["agent_handoff"]["state"],
        "copy_ready"
    );
    assert_eq!(
        manual_repair_queue["queue"][0]["agent_handoff"]["automatic"],
        false
    );
    assert!(
        manual_repair_queue["queue"][0]["agent_handoff"]["reasons"][1]
            .as_str()
            .unwrap_or("")
            .contains("separate from ReviewCard repair-queue.json")
    );
    assert!(
        manual_repair_queue["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not an automatic repair queue")
    );
    assert!(
        manual_repair_queue["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("did not run agents")
    );

    let tokmd_packets = parse_json(&fs::read_to_string(out_dir.join("tokmd-packets.json"))?)?;
    assert_eq!(tokmd_packets["schema_version"], "tokmd-packets/v1");
    assert_eq!(tokmd_packets["mode"], "tokmd_packet_bundle");
    assert_eq!(tokmd_packets["source"], "first_pr");
    assert_eq!(tokmd_packets["policy"], "advisory");
    assert_eq!(tokmd_packets["renderer"]["tokmd_run"], false);
    assert_eq!(
        tokmd_packets["renderer"]["available_presets"][0],
        "bun-ub-handoff"
    );
    assert_eq!(
        tokmd_packets["renderer"]["available_presets"][4],
        "bun-ub-next-pick"
    );
    assert!(
        tokmd_packets["renderer"]["presets_status"]
            .as_str()
            .unwrap_or("")
            .contains("did not render tokmd output")
    );
    assert_eq!(tokmd_packets["summary"]["manual_candidates"], 2);
    assert_eq!(tokmd_packets["summary"]["packets"], 2);
    assert_eq!(tokmd_packets["summary"]["analyzer_discovered"], 0);
    assert_eq!(tokmd_packets["summary"]["external_evidence_refs"], 6);
    assert_eq!(tokmd_packets["summary"]["with_proof_mode"], 2);
    assert_eq!(tokmd_packets["summary"]["with_fix_boundary"], 2);
    assert_eq!(tokmd_packets["summary"]["with_pr_aperture"], 2);
    assert_eq!(tokmd_packets["summary"]["with_oracle_map"], 2);
    assert_eq!(tokmd_packets["summary"]["with_stable_byte_source_class"], 2);
    assert_eq!(
        tokmd_packets["summary"]["operation_families"]["raw_pointer_read"],
        1
    );
    assert_eq!(
        tokmd_packets["inputs"]["manual-candidates.json"]["included"],
        true
    );
    assert_eq!(tokmd_packets["inputs"]["cards.json"]["included"], false);
    assert_eq!(
        tokmd_packets["inputs"]["comment-plan.json"]["included"],
        true
    );
    assert_eq!(
        tokmd_packets["inputs"]["comment-plan.json"]["summary"]["selected_count"],
        1
    );
    assert_eq!(
        tokmd_packets["inputs"]["comment-plan.json"]["summary"]["not_selected_count"],
        0
    );
    assert_eq!(
        tokmd_packets["inputs"]["comment-plan.json"]["summary"]["budget"],
        3
    );
    assert_eq!(
        tokmd_packets["inputs"]["comment-plan.json"]["summary"]["reason_code"],
        "bounded_reviewer_noise"
    );
    assert_eq!(
        tokmd_packets["inputs"]["comment-plan.json"]["selected_reason_codes"]["top_actionable_card"],
        1
    );
    assert!(
        tokmd_packets["inputs"]["comment-plan.json"]["relationship"]
            .as_str()
            .unwrap_or("")
            .contains("ReviewCard-only")
    );
    assert!(
        tokmd_packets["inputs"]["comment-plan.json"]["relationship"]
            .as_str()
            .unwrap_or("")
            .contains("manual candidates are not selected")
    );
    assert!(
        tokmd_packets["inputs"]["comment-plan.json"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("did not post comments")
    );
    assert!(
        tokmd_packets["inputs"]["stable-byte seed ledger"]["limitation"]
            .as_str()
            .unwrap_or("")
            .contains("packet-local stable_byte.ledger_state")
    );
    assert_eq!(tokmd_packets["packets"][0]["id"], "R4R2-S001");
    assert_eq!(tokmd_packets["packets"][0]["source"], "manual");
    assert_eq!(tokmd_packets["packets"][0]["manual_candidate"], true);
    assert_eq!(tokmd_packets["packets"][0]["analyzer_discovered"], false);
    assert_eq!(
        tokmd_packets["packets"][0]["packet_kind"],
        "manual_candidate"
    );
    assert_eq!(
        tokmd_packets["packets"][0]["tokmd_presets"][2],
        "bun-ub-ledger-note"
    );
    assert_eq!(
        tokmd_packets["packets"][0]["stable_byte_source_class"],
        manual_candidates["candidates"][0]["stable_byte"]["class"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["stable_byte"],
        manual_candidates["candidates"][0]["stable_byte"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["ledger_state"],
        manual_candidates["candidates"][0]["stable_byte"]["ledger_state"]
    );
    assert!(
        tokmd_packets["packets"][0]["ledger_state_limitation"]
            .as_str()
            .unwrap_or("")
            .contains("packet-local manual candidate metadata")
    );
    assert_eq!(
        tokmd_packets["packets"][0]["target"]["location_text"],
        manual_candidates["candidates"][0]["location_text"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["route"]["safe_caller"],
        manual_candidates["candidates"][0]["safe_caller"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["invariant_at_risk"],
        manual_candidates["candidates"][0]["invariant"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["proof_mode"],
        manual_candidates["candidates"][0]["proof_mode"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["oracle_map"],
        manual_candidates["candidates"][0]["oracle_map"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["fix_boundary"],
        manual_candidates["candidates"][0]["fix_boundary"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["pr_aperture"],
        manual_candidates["candidates"][0]["pr_aperture"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["implementer_handoff"],
        manual_candidates["candidates"][0]["implementer_handoff"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["manual_repair_queue_item"]["artifact"],
        "manual-repair-queue.json"
    );
    assert_eq!(
        tokmd_packets["packets"][0]["manual_repair_queue_item"]["id"],
        manual_repair_queue["queue"][0]["id"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["manual_repair_queue_item"]["bucket"],
        manual_repair_queue["queue"][0]["bucket"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["manual_repair_queue_item"]["bucket_reason"],
        manual_repair_queue["queue"][0]["bucket_reason"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["manual_repair_queue_item"]["agent_handoff"],
        manual_repair_queue["queue"][0]["agent_handoff"]
    );
    assert!(
        tokmd_packets["packets"][0]["manual_repair_queue_item"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not automatic repair")
    );
    assert_eq!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-handoff"]["stable_byte_family"],
        manual_candidates["candidates"][0]["stable_byte"]["class"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-handoff"]["safe_js_caller_route"],
        manual_candidates["candidates"][0]["safe_caller"]
    );
    assert!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-handoff"]["required_proof_action"]
            .as_str()
            .unwrap_or("")
            .contains("Miri/model")
    );
    assert_eq!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-handoff"]["stop_line"],
        format!(
            "stop at PR aperture: {}",
            manual_candidates["candidates"][0]["pr_aperture"]
                .as_str()
                .unwrap_or_default()
        )
    );
    assert_eq!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-pr-body"]["compatibility_oracle"],
        manual_candidates["candidates"][0]["oracle_map"]
    );
    assert!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-pr-body"]["claims_not_made"]
            .as_array()
            .is_some_and(|claims| claims
                .iter()
                .any(|claim| claim.as_str() == Some("not Miri-clean status")))
    );
    assert!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-review-map"]["comment_plan"]["relationship"]
            .as_str()
            .unwrap_or("")
            .contains("manual candidates are not selected")
    );
    assert_eq!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-review-map"]["repair_queue"]["bucket"],
        manual_repair_queue["queue"][0]["bucket"]
    );
    assert_eq!(
        tokmd_packets["packets"][0]["preset_inputs"]["bun-ub-next-pick"]["non_goals"],
        manual_candidates["candidates"][0]["do_not_touch"]
    );
    assert!(
        tokmd_packets["packets"][0]["preset_inputs"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("did not run tokmd")
    );
    assert_eq!(
        tokmd_packets["packets"][1]["external_evidence"][2]["kind"],
        manual_candidates["candidates"][1]["evidence"][2]["kind"]
    );
    assert!(
        tokmd_packets["packets"][1]["commands"]["context_json"]
            .as_str()
            .unwrap_or("")
            .contains("R4R2-S002")
    );
    let missing_inputs = json_array(
        &tokmd_packets["packets"][0]["missing_inputs"],
        "tokmd_packets.packets[0].missing_inputs",
    )?;
    assert!(
        missing_inputs
            .iter()
            .any(|input| input == "ReviewCard projection")
    );
    assert!(
        !missing_inputs
            .iter()
            .any(|input| input == "stable-byte ledger state"),
        "packet-local stable_byte.ledger_state should not be reported as missing"
    );
    assert!(
        tokmd_packets["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("did not run tokmd")
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
    assert_manual_candidate_witness_follow_up(&witness_plan);
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
            .contains("not a site-execution claim")
    );

    let repair_queue = parse_json(&fs::read_to_string(out_dir.join("repair-queue.json"))?)?;
    assert_eq!(repair_queue["mode"], "aggregate_repair_queue");
    assert_eq!(repair_queue["source"], "review_card");
    assert_eq!(repair_queue["policy"], "advisory");
    assert_eq!(repair_queue["summary"]["changed_files"], 1);
    assert_eq!(repair_queue["summary"]["changed_rust_files"], 1);
    assert_eq!(repair_queue["summary"]["changed_non_rust_files"], 0);
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
    let repair_queue_raw = fs::read_to_string(out_dir.join("repair-queue.json"))?;
    assert!(!repair_queue_raw.contains("R4R2-S001"));
    assert!(!repair_queue_raw.contains("manual_candidate"));

    let policy_report = parse_json(&fs::read_to_string(out_dir.join("policy-report.json"))?)?;
    assert_eq!(policy_report["schema_version"], "0.1");
    assert_eq!(policy_report["mode"], "policy-report");
    assert_eq!(policy_report["policy"], "advisory");
    assert_eq!(policy_report["summary"]["cards"], cards["summary"]["cards"]);
    assert_eq!(
        policy_report["summary"]["new_gaps"],
        cards["summary"]["open_actionable_gaps"]
    );
    assert_eq!(policy_report["cards"][0]["card_id"], card_id);
    assert!(
        policy_report["limitations"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|limitation| limitation
                .as_str()
                .unwrap_or("")
                .contains("Manual candidates are not policy-report inputs"))
    );
    assert!(
        policy_report["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("does not enforce blocking policy")
    );
    assert!(
        policy_report["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a site-execution claim")
    );
    assert!(
        policy_report["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("matching witness receipt")
    );
    let policy_report_raw = fs::read_to_string(out_dir.join("policy-report.json"))?;
    assert!(!policy_report_raw.contains("R4R2-S001"));
    assert!(!policy_report_raw.contains("manual_candidate"));

    let policy_report_markdown = fs::read_to_string(out_dir.join("policy-report.md"))?;
    assert!(policy_report_markdown.contains("# unsafe-review policy report"));
    assert!(policy_report_markdown.contains("Manual candidates are not policy-report inputs"));
    assert!(policy_report_markdown.contains("not Miri-clean status"));
    assert!(!policy_report_markdown.contains("R4R2-S001"));

    Ok(())
}

#[test]
fn first_pr_emits_usefulness_telemetry_artifact() -> Result<(), Box<dyn Error>> {
    // Verifies that first-pr emits usefulness-telemetry.json and that it
    // correctly projects from the same cards as cards.json (SPEC-0038).
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-usefulness-telemetry-e2e")?;
    let out_dir = temp.path().join("out");

    run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.as_os_str().to_os_string(),
    ])?;

    let telemetry_path = out_dir.join("usefulness-telemetry.json");
    assert!(
        telemetry_path.exists(),
        "usefulness-telemetry.json must be emitted"
    );

    let telemetry: serde_json::Value = parse_json(&fs::read_to_string(&telemetry_path)?)?;

    // Schema version
    assert_eq!(telemetry["schema_version"], "usefulness-telemetry/v1");

    // Trust boundary present and contains correct text
    let boundary = telemetry["trust_boundary"].as_str().unwrap_or("");
    assert!(!boundary.is_empty(), "trust_boundary must be present");
    assert!(
        boundary.contains("not calibrated"),
        "trust_boundary must say 'not calibrated'"
    );
    assert!(
        !boundary.to_ascii_lowercase().contains("precision"),
        "trust_boundary must not claim precision"
    );
    assert!(
        !boundary.to_ascii_lowercase().contains("recall"),
        "trust_boundary must not claim recall"
    );

    // Card inventory projects from cards.json
    let cards = parse_json(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    let expected_total = cards["summary"]["cards"].as_u64().unwrap_or(0);
    assert_eq!(
        telemetry["card_inventory"]["total_cards"]
            .as_u64()
            .unwrap_or(999),
        expected_total,
        "total_cards must match cards.json summary.cards"
    );

    // Agent readiness sums to total_cards
    let ready = telemetry["agent_readiness"]["ready"].as_u64().unwrap_or(0);
    let needs_human = telemetry["agent_readiness"]["needs_human"]
        .as_u64()
        .unwrap_or(0);
    let unsupported = telemetry["agent_readiness"]["unsupported"]
        .as_u64()
        .unwrap_or(0);
    assert_eq!(
        ready + needs_human + unsupported,
        expected_total,
        "agent_readiness histogram must sum to total_cards"
    );

    // Gate manifest pointer present
    let gate = parse_json(&fs::read_to_string(
        out_dir.join("unsafe-review-gate.json"),
    )?)?;
    assert_eq!(
        gate["artifacts"]["usefulness_telemetry"], "usefulness-telemetry.json",
        "gate manifest must point to usefulness-telemetry.json"
    );

    // --- Finding #2: scan_cost must be present and non-zero ---
    let scan_cost = &telemetry["scan_cost"];
    assert!(
        !scan_cost.is_null(),
        "scan_cost must be present in first-pr telemetry (injected by CLI)"
    );
    let elapsed_ms = scan_cost["elapsed_ms"].as_u64().unwrap_or(0);
    assert!(
        elapsed_ms > 0,
        "scan_cost.elapsed_ms must be > 0 for a real first-pr run; got {elapsed_ms}"
    );
    let output_bytes_total = scan_cost["output_bytes_total"].as_u64().unwrap_or(0);
    assert!(
        output_bytes_total > 0,
        "scan_cost.output_bytes_total must be > 0; got {output_bytes_total}"
    );

    // --- Finding #3: not_selected_class_histogram must be present ---
    let class_histogram = telemetry["comment_selection"]["not_selected_class_histogram"]
        .as_object()
        .ok_or("not_selected_class_histogram must be an object")?;
    // Every key must be in "reason_code/class" form
    for key in class_histogram.keys() {
        assert!(
            key.contains('/'),
            "not_selected_class_histogram key must contain '/'; got: {key}"
        );
    }
    // Sum must equal sum of reason histogram
    let reason_total: u64 = telemetry["comment_selection"]["not_selected_reason_histogram"]
        .as_object()
        .map(|m| m.values().filter_map(|v| v.as_u64()).sum())
        .unwrap_or(0);
    let class_total: u64 = class_histogram.values().filter_map(|v| v.as_u64()).sum();
    assert_eq!(
        reason_total, class_total,
        "not_selected_class_histogram sum ({class_total}) must equal reason_histogram sum ({reason_total})"
    );

    // --- Finding #4: unfulfilled_obligation_count must be present and >= 1 ---
    let unfulfilled = telemetry["unfulfilled_obligation_count"]
        .as_u64()
        .ok_or("unfulfilled_obligation_count must be a non-negative integer")?;
    assert!(
        unfulfilled >= 1,
        "raw_pointer_alignment has unmet obligations; unfulfilled_obligation_count must be >= 1, got {unfulfilled}"
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
            .contains("not memory-safety proof")
    );

    let review_kit = parse_json(&fs::read_to_string(out_dir.join("review-kit.json"))?)?;
    assert_eq!(review_kit["schema_version"], "0.1");
    assert_eq!(review_kit["mode"], "review_kit_manifest");
    assert_eq!(review_kit["summary"]["changed_files"], 1);
    assert_eq!(review_kit["summary"]["changed_rust_files"], 1);
    assert_eq!(review_kit["summary"]["changed_non_rust_files"], 0);
    assert_eq!(review_kit["summary"]["cards"], 0);
    assert_eq!(review_kit["summary"]["open_actionable_gaps"], 0);
    assert!(review_kit["top_card_id"].is_null());
    assert_eq!(review_kit["handoff"]["reviewer_summary"], "pr-summary.md");
    assert!(review_kit["handoff"]["top_card"].is_null());
    assert_eq!(review_kit["handoff"]["review_cards"]["review_cards"], 0);
    assert_eq!(review_kit["handoff"]["review_cards"]["card_queue_limit"], 5);
    assert_eq!(review_kit["handoff"]["review_cards"]["omitted_cards"], 0);
    assert!(
        json_array(
            &review_kit["handoff"]["review_cards"]["card_queue"],
            "review_kit.handoff.review_cards.card_queue",
        )?
        .is_empty()
    );
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
            .contains("not a site-execution claim")
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
            .contains("not a site-execution claim")
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
    assert!(text.contains("not a site-execution claim"));
    assert!(text.contains("matching witness receipt"));
    assert!(!text.contains("soundness proof"));
    assert!(!text.contains("All clear"));

    Ok(())
}

#[test]
fn help_lists_confirm_with_allow_heavy_boundary() -> Result<(), Box<dyn Error>> {
    let output = run_success([os("--help")])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("confirm <card-id> --dry-run|--allow-heavy"));
    assert!(
        text.contains("executes the routed witness command only with --allow-heavy; never default")
    );
    assert!(text.contains("--dry-run previews without executing"));

    Ok(())
}

#[test]
fn confirm_refuses_without_allow_heavy_and_points_at_dry_run() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");

    let output = run_failure([
        os("confirm"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"),
    ])?;

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout_text(&output)?.trim(), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("only with the explicit --allow-heavy opt-in"),
        "stderr should state the opt-in boundary: {stderr}"
    );
    assert!(
        stderr.contains("unsafe-review never executes witnesses by default"),
        "stderr should restate the default boundary: {stderr}"
    );
    assert!(
        stderr.contains("--dry-run to preview"),
        "stderr should point at --dry-run: {stderr}"
    );

    Ok(())
}

#[test]
fn confirm_dry_run_previews_routed_command_without_executing() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");

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
    let card_id = json_str(&value["cards"][0]["id"], "cards[0].id")?;

    let output = run_success([
        os("confirm"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--dry-run"),
        OsString::from(card_id),
    ])?;
    let text = stdout_text(&output)?;

    assert!(text.contains("unsafe-review confirm (dry run)"));
    assert!(text.contains(&format!("card: {card_id}")));
    assert!(text.contains("operation family: raw_pointer_read"));
    assert!(text.contains("route: miri"));
    assert!(text.contains("command: cargo +nightly miri test read_header"));
    assert!(text.contains("command provenance: analyzer-derived route"));
    assert!(text.contains("timeout: 600s"));
    assert!(text.contains("expected evidence: a `miri` witness receipt"));
    assert!(text.contains("dry run only; nothing was executed"));
    assert!(text.contains("unsafe-review never executes witnesses by default"));
    assert!(text.contains("trust boundary: static unsafe contract review only"));
    assert!(
        !fixture.join(".unsafe-review").join("receipts").exists(),
        "dry run must not write a receipt"
    );
    assert!(
        !fixture
            .join("target")
            .join("unsafe-review-confirm")
            .exists(),
        "dry run must not write an output log"
    );

    Ok(())
}

#[test]
fn confirm_allow_heavy_reports_spawn_failure_without_writing_a_receipt()
-> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");

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
    let card_id = json_str(&value["cards"][0]["id"], "cards[0].id")?;

    let output = run_failure([
        os("confirm"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--allow-heavy"),
        os("--author"),
        os("core/e2e"),
        os("--command"),
        os("unsafe-review-e2e-missing-witness-binary miri-test read_header"),
        OsString::from(card_id),
    ])?;

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to spawn `unsafe-review-e2e-missing-witness-binary`"),
        "stderr should report the spawn failure honestly: {stderr}"
    );
    assert!(
        stderr.contains("no receipt was written"),
        "stderr should confirm no receipt was fabricated: {stderr}"
    );
    assert!(
        !fixture.join(".unsafe-review").join("receipts").exists(),
        "spawn failure must not write a receipt"
    );

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
    assert!(text.contains("With --list-files, --format supports human, json, or markdown output"));
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
fn repo_scan_skips_nested_git_checkouts() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-nested-git-e2e")?;

    // Copy the target fixture to the temp root so we control its layout.
    copy_dir_all(&fixture, temp.path())?;

    // Create a nested directory that looks like a git checkout: it contains a
    // .git directory and a src/lib.rs with unsafe code that would produce cards.
    let vendor_clone = temp.path().join("vendor-clone");
    fs::create_dir_all(vendor_clone.join(".git"))?;
    fs::create_dir_all(vendor_clone.join("src"))?;
    fs::write(
        vendor_clone.join("src/lib.rs"),
        "// nested checkout — must not contribute cards\n\
         unsafe fn nested_raw_ptr(ptr: *const u8) -> u8 { *ptr }\n",
    )?;

    let output = run_success([
        os("repo"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let repo = parse_json(&stdout_text(&output)?)?;

    // The fixture itself has exactly 1 card; the nested checkout must not add more.
    assert_eq!(
        repo["summary"]["cards"], 1,
        "nested checkout must not inflate the card count: {repo}"
    );
    assert_eq!(
        repo["summary"]["open_actionable_gaps"], 1,
        "nested checkout must not inflate open-gap count: {repo}"
    );

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
fn check_bad_base_ref_emits_actionable_hint() -> Result<(), Box<dyn Error>> {
    // When --base names a ref that does not exist in the repository git returns
    // a non-zero exit code with an "unknown revision" message.  The CLI must
    // surface an actionable error (naming the bad ref and suggesting alternatives)
    // rather than dumping the raw git stderr without context.
    //
    // The fixture lives inside the repository worktree so HEAD is resolvable;
    // only the invented ref is unknown, which is what we want to exercise.
    let fixture = fixture_root("raw_pointer_alignment");

    let output = run_failure([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--base"),
        os("this-ref-does-not-exist-zzzz"),
    ])?;

    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code must be 2 for input error"
    );
    assert_eq!(
        stdout_text(&output)?.trim(),
        "",
        "stdout must be empty on ref error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("this-ref-does-not-exist-zzzz"),
        "stderr should name the bad ref: {stderr}"
    );
    assert!(
        stderr.contains("could not be resolved by git"),
        "stderr should describe the resolution failure: {stderr}"
    );
    assert!(
        stderr.contains("--base origin/main") || stderr.contains("--diff"),
        "stderr should suggest a valid alternative: {stderr}"
    );

    Ok(())
}

#[test]
fn check_reports_unparseable_diff_as_cli_failure() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-garbage-diff-e2e")?;

    // Copy the fixture into a temp dir so we can place the garbage diff alongside it.
    copy_dir_all(&fixture, temp.path())?;

    let garbage_diff = temp.path().join("garbage.diff");
    fs::write(&garbage_diff, "this is not a diff at all")?;

    let cards_out = temp.path().join("cards.json");

    let output = run_failure([
        os("check"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--diff"),
        garbage_diff.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        cards_out.as_os_str().to_os_string(),
    ])?;

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stdout_text(&output)?.trim(),
        "",
        "stdout should be empty on parse failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("garbage.diff"),
        "stderr should include the diff path: {stderr}"
    );
    assert!(
        stderr.contains("could not be parsed as a unified diff"),
        "stderr should describe the parse failure: {stderr}"
    );
    assert!(
        stderr.contains("no analysis was run"),
        "stderr should state no analysis was run: {stderr}"
    );
    assert!(
        !cards_out.exists(),
        "output file must not be created when diff is unparseable"
    );

    Ok(())
}

#[test]
fn check_empty_diff_is_complete_noop_not_whole_repo_scan() -> Result<(), Box<dyn Error>> {
    // A valid but empty diff (e.g. from `git diff` on a clean branch) must
    // produce a complete diff-scoped no-op run: scope=diff, 0 selected files,
    // 0 cards, no whole-repo cards.  This is distinct from a malformed diff
    // (which exits 2) and from a no-diff-supplied run (which keeps its own
    // existing behavior).
    //
    // Covers issue #1558 (instrument-truthfulness: valid empty diff = no-op).
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-empty-diff-e2e")?;

    // Copy the fixture so we can place an empty diff alongside it.
    copy_dir_all(&fixture, temp.path())?;

    let empty_diff = temp.path().join("empty.diff");
    fs::write(&empty_diff, "")?;

    let output = run_success([
        os("check"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--diff"),
        empty_diff.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;

    let value = parse_json(&stdout_text(&output)?)?;
    assert_eq!(value["scope"], "diff", "empty diff must produce scope=diff");
    assert_eq!(
        value["summary"]["cards"], 0,
        "empty diff must produce 0 cards, not whole-repo cards"
    );
    assert_eq!(
        value["summary"]["changed_files"], 0,
        "empty diff must report 0 changed_files"
    );
    assert_eq!(
        value["summary"]["changed_rust_files"], 0,
        "empty diff must report 0 changed_rust_files"
    );
    assert!(
        value["cards"].as_array().is_some_and(Vec::is_empty),
        "empty diff must produce an empty cards array"
    );
    assert!(
        value["trust_boundary"]
            .as_str()
            .is_some_and(|s| s.contains("not memory-safety proof")),
        "trust boundary must still be present on no-op output"
    );
    // The no-op output must not read as a safety pass.
    let rendered = serde_json::to_string(&value)?;
    assert!(
        !rendered.contains("all clear") && !rendered.contains("All clear"),
        "no-op output must not contain 'all clear'"
    );

    Ok(())
}

#[test]
fn check_empty_diff_no_new_debt_exits_0() -> Result<(), Box<dyn Error>> {
    // A valid empty diff with --policy no-new-debt must exit 0: no new unsafe
    // seams were added, so the no-new-debt policy is satisfied.
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-empty-diff-nnd-e2e")?;
    copy_dir_all(&fixture, temp.path())?;
    let empty_diff = temp.path().join("empty.diff");
    fs::write(&empty_diff, "")?;

    let output = run_success([
        os("check"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--diff"),
        empty_diff.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--policy"),
        os("no-new-debt"),
    ])?;

    let value = parse_json(&stdout_text(&output)?)?;
    assert_eq!(value["scope"], "diff");
    assert_eq!(value["summary"]["cards"], 0);
    assert_eq!(value["policy"], "no-new-debt");

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
fn repo_list_files_renders_json_and_markdown_scope_artifacts() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-repo-list-files-formats")?;
    write_e2e_file(temp.path(), "src/lib.rs")?;
    write_e2e_file(temp.path(), "packages/pkg/src/lib.rs")?;
    write_e2e_file(temp.path(), "packages/pkg/src/skip.rs")?;
    write_e2e_file(temp.path(), "ignored/lib.rs")?;
    fs::write(temp.path().join(".gitignore"), "ignored/\n")?;
    let json_out = temp.path().join("repo-files.json");

    let json_output = run_success([
        os("repo"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--include"),
        os("src/**/*.rs"),
        os("--include"),
        os("packages/**/*.rs"),
        os("--exclude"),
        os("packages/**/skip.rs"),
        os("--max-files"),
        os("2"),
        os("--format"),
        os("json"),
        os("--out"),
        json_out.as_os_str().to_os_string(),
        os("--list-files"),
    ])?;

    assert_eq!(stdout_text(&json_output)?.trim(), "");
    let list = parse_json(&fs::read_to_string(&json_out)?)?;
    assert_eq!(list["schema_version"], "repo-file-list/v1");
    assert_eq!(list["mode"], "repo_list_files");
    assert_eq!(list["root"], temp.path().display().to_string());
    assert_eq!(list["scan_scope"]["include"][0], "src/**/*.rs");
    assert_eq!(list["scan_scope"]["include"][1], "packages/**/*.rs");
    assert_eq!(list["scan_scope"]["exclude"][0], "packages/**/skip.rs");
    assert_eq!(list["scan_scope"]["respect_gitignore"], true);
    assert_eq!(list["scan_scope"]["large_repo_ignores"], true);
    assert_eq!(list["scan_scope"]["max_files"], 2);
    assert_eq!(list["summary"]["selected_rust_files"], 2);
    assert_eq!(list["summary"]["analysis_run"], false);
    assert_eq!(list["summary"]["reviewcards_created"], 0);
    assert_eq!(list["summary"]["witnesses_run"], false);
    assert_eq!(list["files"][0], "src/lib.rs");
    assert_eq!(list["files"][1], "packages/pkg/src/lib.rs");
    assert!(
        list["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("does not analyze files")
    );

    let markdown_output = run_success([
        os("repo"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--include"),
        os("ignored/**/*.rs"),
        os("--no-respect-gitignore"),
        os("--format"),
        os("markdown"),
        os("--list-files"),
    ])?;
    let markdown = stdout_text(&markdown_output)?;
    assert!(markdown.contains("# unsafe-review repo file list"));
    assert!(markdown.contains("- Analysis run: `false`"));
    assert!(markdown.contains("- ReviewCards created: `0`"));
    assert!(markdown.contains("- Respect gitignore: `false`"));
    assert!(markdown.contains("- `ignored/lib.rs`"));
    assert!(markdown.contains("not analyze files"));
    assert!(markdown.contains("UB-free/Miri-clean claims"));

    let unsupported = run_failure([
        os("repo"),
        os("--root"),
        temp.path().as_os_str().to_os_string(),
        os("--format"),
        os("sarif"),
        os("--list-files"),
    ])?;
    assert_eq!(unsupported.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&unsupported.stderr);
    assert!(
        stderr.contains("repo --list-files only supports human, json, or markdown output"),
        "stderr should explain list-files format limits: {stderr}"
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
    assert!(text.contains("not Miri-clean status"));
    assert!(text.contains("not a site-execution claim"));
    assert!(text.contains("matching witness receipt"));

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
    assert_eq!(repo["schema_version"], "0.2");
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
        "| ID | Class | Proof path | Location | Operation family | Operation | Missing evidence | Route | Next action |"
    ));
    assert!(repo_markdown.contains("src/lib.rs:8"));
    assert!(repo_markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(repo_markdown.contains("## Trust boundary"));
    assert!(repo_markdown.contains("Add or expose local guards"));
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
    assert_eq!(status["partial"], false);
    assert_eq!(status["stop_reason"], "none");
    assert_eq!(status["cap"], Value::Null);
    assert_eq!(status["files_discovered"], 1);
    assert_eq!(status["files_scanned"], 1);
    assert_eq!(status["files_remaining"], 0);
    assert_eq!(status["cards_found"], 1);
    assert_eq!(status["last_path"], "src/lib.rs");
    assert!(status["elapsed_ms"].as_u64().is_some());
    assert!(status["error"].is_null());
    assert!(status["partial_path"].is_null());
    assert_repo_status_operator(&status, "complete", false, "promoted final report")?;

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
    assert_eq!(status["partial"], true);
    // A report-write failure is an error, not a timeout — the shared
    // record_incomplete path must label it accurately.
    assert_eq!(status["stop_reason"], "error");
    assert_eq!(status["cap"], Value::Null);
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
    assert_repo_status_operator(&status, "failed", true, "fixing the error")?;

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
    assert_eq!(status["partial"], true);
    // An analysis read failure mid-scan is an error, not a timeout.
    assert_eq!(status["stop_reason"], "error");
    assert_eq!(status["cap"], Value::Null);
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
    assert_repo_status_operator(&status, "failed", true, "fixing the error")?;

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
    assert_eq!(status["partial"], true);
    assert_eq!(status["stop_reason"], "timeout");
    assert_eq!(status["cap"], Value::Null);
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
    assert_repo_status_operator(&status, "failed", true, "increasing timeout")?;

    Ok(())
}

/// Drift-lock: if per-file timing emission is dropped from the complete scan
/// path, `file_timings` will be null here and the assertions below go RED.
#[test]
fn repo_status_sidecar_includes_per_file_timings_for_small_scan() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-timings-e2e")?;
    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
    ])?;

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "complete");
    assert_eq!(status["completed"], true);

    // file_timings must be a non-null array for a small fixture scan.
    // Diagnostic only — not a proof, coverage claim, or performance guarantee.
    let timings = status["file_timings"].as_array().ok_or_else(|| {
        format!(
            "file_timings must be a JSON array; got: {}",
            status["file_timings"]
        )
    })?;
    assert!(
        !timings.is_empty(),
        "file_timings must be non-empty for a scan that scanned files"
    );
    // Each entry must have 'file' (string) and 'scan_ms' (number).
    for entry in timings {
        assert!(
            entry["file"].as_str().is_some(),
            "each file_timings entry must have a string 'file' field; got: {entry}"
        );
        assert!(
            entry["scan_ms"].as_u64().is_some(),
            "each file_timings entry must have a numeric 'scan_ms' field; got: {entry}"
        );
    }
    // The number of entries must match files_scanned.
    let files_scanned = status["files_scanned"]
        .as_u64()
        .ok_or_else(|| "files_scanned must be numeric".to_string())?;
    assert_eq!(
        timings.len() as u64,
        files_scanned,
        "file_timings entry count must equal files_scanned"
    );

    Ok(())
}

/// Drift-lock: if file_timings is emitted even for the incomplete/timeout path
/// (which would be a partial list without announcement), this test goes RED by
/// confirming the field is null in the timeout sidecar.
#[test]
fn repo_status_sidecar_file_timings_null_for_timeout_path() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-timings-timeout-e2e")?;
    let scan_root = temp.path().join("fixture");
    copy_dir_all(&fixture, &scan_root)?;
    fs::write(scan_root.join("src/z_safe.rs"), "pub fn safe() {}\n")?;
    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    // Run with a tight timeout so the scan stops mid-way.
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
    assert!(!output.status.success(), "timeout scan must exit non-zero");

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["stop_reason"], "timeout");
    // Timeout incomplete status must have file_timings: null — truncation
    // honesty: we never emit a partial list without announcement.
    assert!(
        status["file_timings"].is_null(),
        "file_timings must be null in timeout incomplete status; got: {}",
        status["file_timings"]
    );

    Ok(())
}

/// Drift-lock: a completed repo scan with `--out` must report `output_bytes > 0`
/// in the status sidecar, matching the on-disk size of the final report.
/// If the field is dropped or stays null for a completed scan, this goes RED.
#[test]
fn repo_status_sidecar_output_bytes_present_for_completed_scan() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-output-bytes-e2e")?;
    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
    ])?;

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(status["phase"], "complete");
    assert_eq!(status["completed"], true);

    // output_bytes must be a positive integer matching the final report size.
    // Diagnostic only — not a coverage claim, proof, UB-free, Miri-clean,
    // site-execution, or performance guarantee.
    let output_bytes = status["output_bytes"].as_u64().ok_or_else(|| {
        format!(
            "output_bytes must be a non-null number in a completed sidecar; got: {}",
            status["output_bytes"]
        )
    })?;
    assert!(
        output_bytes > 0,
        "output_bytes must be positive for a completed scan that wrote a report"
    );
    // The sidecar's output_bytes must match the actual file size on disk.
    let actual_size = fs::metadata(&report_path)?.len();
    assert_eq!(
        output_bytes, actual_size,
        "output_bytes in sidecar must match on-disk report file size"
    );

    Ok(())
}

/// Drift-lock: a timeout-incomplete scan must have `output_bytes: null` in its
/// status sidecar — no final report was written, so no byte count is available.
/// If output_bytes is accidentally populated for an incomplete scan, this goes RED.
#[test]
fn repo_status_sidecar_output_bytes_null_for_timeout_path() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-output-bytes-timeout-e2e")?;
    let scan_root = temp.path().join("fixture");
    copy_dir_all(&fixture, &scan_root)?;
    fs::write(scan_root.join("src/z_safe.rs"), "pub fn safe() {}\n")?;
    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    // Run with a tight timeout so the scan stops mid-way.
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
    assert!(!output.status.success(), "timeout scan must exit non-zero");

    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["stop_reason"], "timeout");
    // Timeout incomplete status must have output_bytes: null — no final
    // report was produced so no byte count is available.
    assert!(
        status["output_bytes"].is_null(),
        "output_bytes must be null in timeout incomplete status; got: {}",
        status["output_bytes"]
    );

    Ok(())
}

/// Drift-lock: a first-pr run must report output_bytes > 0 in its terminal
/// output, matching the total size of all artifacts written to --out-dir.
/// If the field is dropped or output_bytes is 0, this goes RED.
#[test]
fn first_pr_reports_output_bytes_in_terminal_output() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-first-pr-output-bytes-e2e")?;
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

    // The terminal summary must include the output bundle byte count.
    // Diagnostic only — not a coverage claim, proof, UB-free, Miri-clean,
    // site-execution, or performance guarantee.
    assert!(
        stdout.contains("- Output bundle:"),
        "stdout must contain '- Output bundle:' line; got:\n{stdout}"
    );
    assert!(
        stdout.contains(" bytes"),
        "stdout must contain ' bytes' in the output bundle line; got:\n{stdout}"
    );

    // Extract the byte count and verify it is > 0 and matches on-disk total.
    let bundle_line = stdout
        .lines()
        .find(|line| line.contains("- Output bundle:"))
        .ok_or("could not find '- Output bundle:' line")?;
    let bytes_str = bundle_line
        .split_whitespace()
        .find(|tok| tok.chars().all(|c| c.is_ascii_digit()))
        .ok_or_else(|| format!("could not extract byte count from line: {bundle_line}"))?;
    let reported_bytes: u64 = bytes_str
        .parse()
        .map_err(|err| format!("byte count parse failed: {err}"))?;
    assert!(
        reported_bytes > 0,
        "output_bytes must be positive for a run that wrote artifacts"
    );

    // Verify that the reported total matches the actual sum of artifact sizes.
    let mut actual_total: u64 = 0;
    for entry in fs::read_dir(&out_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            actual_total += entry.metadata()?.len();
        }
    }
    assert_eq!(
        reported_bytes, actual_total,
        "reported output_bytes must equal the sum of all artifact file sizes in --out-dir"
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
    assert_eq!(status["partial"], true);
    assert_eq!(status["stop_reason"], "terminated");
    assert_eq!(status["cap"], Value::Null);
    assert_eq!(status["signal"], "SIGTERM");
    assert!(
        status["error"]
            .as_str()
            .unwrap_or("")
            .contains("interrupted by SIGTERM")
    );
    assert!(status["partial_path"].is_null());
    assert_repo_status_operator(&status, "terminated", false, "rerun repo with --out")?;

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
    assert_eq!(status["partial"], true);
    assert_eq!(status["stop_reason"], "terminated");
    assert_eq!(status["cap"], Value::Null);
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
    assert_repo_status_operator(&status, "terminated", true, "restarting or narrowing")?;

    Ok(())
}

#[test]
fn repo_scan_start_stub_written_before_pipeline() -> Result<(), Box<dyn Error>> {
    // Verify that <out>.status.json is written immediately after RepoStatusReporter::new,
    // before analyze_with_discovery_and_repo_events fires its first event.
    // The debug exit hook (UNSAFE_REVIEW_INTERNAL_REPO_STUB_TEST_EXIT) causes the process
    // to exit(3) right after the stub is written, so we can observe the pre-pipeline state.
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-stub-e2e")?;
    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    let output = Command::new(env!("CARGO_BIN_EXE_unsafe-review"))
        .args([
            os("repo"),
            os("--root"),
            fixture.as_os_str().to_os_string(),
            os("--include"),
            os("src/lib.rs"),
            os("--format"),
            os("json"),
            os("--out"),
            report_path.as_os_str().to_os_string(),
        ])
        .env("UNSAFE_REVIEW_INTERNAL_REPO_STUB_TEST_EXIT", "1")
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(3),
        "expected stub-test exit code 3\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !report_path.exists(),
        "final report must not exist before the pipeline ran"
    );
    assert!(
        status_path.exists(),
        "status sidecar must exist immediately after reporter construction"
    );
    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(
        status["phase"], "discovering",
        "stub phase must be 'discovering'"
    );
    assert_eq!(
        status["stop_reason"], "none",
        "stub stop_reason must be 'none'"
    );
    assert_eq!(status["completed"], false, "stub must not be completed");
    assert_eq!(status["partial"], false, "stub must not be partial");
    assert_eq!(status["elapsed_ms"], 0u64, "stub elapsed_ms must be 0");
    assert_eq!(
        status["files_discovered"], 0u64,
        "stub files_discovered must be 0"
    );
    assert_eq!(
        status["files_scanned"], 0u64,
        "stub files_scanned must be 0"
    );
    assert_eq!(
        status["files_remaining"], 0u64,
        "stub files_remaining must be 0"
    );
    assert_eq!(status["cards_found"], 0u64, "stub cards_found must be 0");
    assert!(status["last_path"].is_null(), "stub last_path must be null");
    assert!(status["cap"].is_null(), "stub cap must be null");
    assert!(status["error"].is_null(), "stub error must be null");
    assert!(status["signal"].is_null(), "stub signal must be null");
    assert!(
        status["partial_path"].is_null(),
        "stub partial_path must be null"
    );
    // scan_scope must be populated with the root from the command
    assert_eq!(
        status["scan_scope"]["root"],
        fixture.display().to_string(),
        "stub scan_scope.root must be set"
    );
    assert_eq!(
        status["scan_scope"]["include"][0], "src/lib.rs",
        "stub scan_scope.include must be populated"
    );
    let operator = status
        .get("operator")
        .ok_or("stub status is missing operator block")?;
    assert_eq!(
        operator["state"], "in_progress",
        "stub operator state must be 'in_progress'"
    );
    assert!(
        operator["next_action"]
            .as_str()
            .unwrap_or("")
            .contains("scan_scope"),
        "stub operator next_action must mention scan_scope"
    );
    let boundary = operator["claim_boundary"].as_str().unwrap_or("");
    assert!(
        boundary.contains("Operational scan status only"),
        "stub operator claim_boundary must carry trust boundary: {boundary}"
    );

    Ok(())
}

/// SPEC-0034 parity: `unsafe-review repo` must emit `unsafe-review-gate.json`
/// alongside its `--out` report, matching the same envelope as `first-pr`.
///
/// The gate manifest is advisory posture only — `status` must be `"advisory"`,
/// `trust_boundary` must disclaim proof and merge verdict, and no volatile
/// timestamp or wall-time fields may appear.  The `artifacts.cards` pointer
/// must resolve to the basename of the `--out` file.
#[test]
fn repo_emits_gate_manifest_alongside_out_report() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-gate-manifest-e2e")?;
    let report_path = temp.path().join("repo.json");
    let gate_manifest_path = temp.path().join("unsafe-review-gate.json");

    run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--include"),
        os("src/lib.rs"),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
    ])?;

    // The main report must exist.
    assert!(
        report_path.exists(),
        "repo report must be written to --out path"
    );

    // The gate manifest must exist alongside the report.
    assert!(
        gate_manifest_path.exists(),
        "unsafe-review-gate.json must be written alongside the --out report (SPEC-0034 parity)"
    );

    let gate_manifest = parse_json(&fs::read_to_string(&gate_manifest_path)?)?;

    // Envelope fields must match the first-pr gate manifest contract.
    assert_eq!(
        gate_manifest["schema_version"], "unsafe-review-gate/v1",
        "schema_version must be 'unsafe-review-gate/v1'"
    );
    assert_eq!(
        gate_manifest["dialect"], "unsafe-review",
        "dialect must be 'unsafe-review'"
    );
    assert_eq!(
        gate_manifest["status"], "advisory",
        "status must be 'advisory' — not a merge verdict (SPEC-0034 trust boundary)"
    );
    assert_eq!(gate_manifest["tool"], "unsafe-review");
    assert!(
        gate_manifest["tool_version"]
            .as_str()
            .is_some_and(|v| !v.is_empty()),
        "tool_version must be a non-empty string"
    );

    // Trust boundary must disclaim proof and merge verdict.
    let boundary = gate_manifest["trust_boundary"]
        .as_str()
        .ok_or("trust_boundary must be a string")?;
    assert!(
        boundary.contains("not proof"),
        "trust_boundary must include 'not proof'; got: {boundary}"
    );
    assert!(
        boundary.contains("not a merge verdict"),
        "trust_boundary must include 'not a merge verdict'; got: {boundary}"
    );

    // Summary must be the SPEC-0030 movement block.
    let summary = &gate_manifest["summary"];
    assert!(
        summary["new_gaps"].is_number(),
        "summary.new_gaps must be a number"
    );
    assert!(
        summary["worsened_gaps"].is_number(),
        "summary.worsened_gaps must be a number"
    );
    assert!(
        summary["resolved_gaps"].is_number(),
        "summary.resolved_gaps must be a number"
    );
    assert!(
        summary["inherited_gaps"].is_number(),
        "summary.inherited_gaps must be a number"
    );

    // The cards artifact pointer must be the basename of the --out file.
    assert_eq!(
        gate_manifest["artifacts"]["cards"], "repo.json",
        "artifacts.cards must be the basename of the --out file"
    );

    // No volatile timestamp or wall-time fields.
    let obj = gate_manifest
        .as_object()
        .ok_or("gate manifest must be a JSON object")?;
    for volatile_key in ["generated_at", "timestamp", "wall_seconds", "elapsed_ms"] {
        assert!(
            !obj.contains_key(volatile_key),
            "gate manifest must not contain volatile field `{volatile_key}` (breaks determinism)"
        );
    }

    // The manifest must be valid JSON ending with a newline.
    let raw = fs::read_to_string(&gate_manifest_path)?;
    assert!(raw.ends_with('\n'), "gate manifest must end with a newline");

    Ok(())
}

/// SPEC-0034: gate manifest must NOT be emitted when `--out` is not supplied
/// (stdout-only repo runs have no artifact directory to put the manifest in).
#[test]
fn repo_does_not_emit_gate_manifest_for_stdout_run() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-repo-no-gate-manifest-e2e")?;
    let stray_manifest = temp.path().join("unsafe-review-gate.json");

    // Run repo to stdout only (no --out).
    run_success([
        os("repo"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--include"),
        os("src/lib.rs"),
        os("--format"),
        os("json"),
    ])?;

    // No manifest should appear in the temp dir (we did not set cwd to temp,
    // but this guards against accidental writes relative to cwd).
    assert!(
        !stray_manifest.exists(),
        "gate manifest must not be written for a stdout-only repo run"
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
    let trust_boundary = outcome["trust_boundary"].as_str().unwrap_or("");
    for phrase in [
        "not memory-safety proof",
        "not UB-free status",
        "not Miri-clean status",
        "not site-execution evidence",
        "not calibrated precision/recall",
        "not policy-ready status",
        "not witness execution",
    ] {
        assert!(
            trust_boundary.contains(phrase),
            "missing outcome trust boundary phrase: {phrase}"
        );
    }
    assert!(
        outcome["limitations"]
            .as_array()
            .is_some_and(|limitations| {
                limitations
                    .iter()
                    .any(|item| item.as_str().unwrap_or("").contains("Miri-clean status"))
            }),
        "limitations should be an array mentioning Miri-clean status"
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
    assert!(markdown.contains(
        "| Card | Class | Priority | Proof path | Operation family | Missing | Next action |"
    ));
    assert!(markdown.contains("guard_missing"));
    assert!(markdown.contains("high"));
    assert!(markdown.contains("| 2 |"));
    assert!(markdown.contains("| Status | Card | Reason | Before | After |"));
    assert!(markdown.contains("## Limitations"));
    assert!(markdown.contains("## Trust boundary"));
    assert!(markdown.contains("not Miri-clean status"));
    assert!(markdown.contains("not site-execution evidence"));
    assert!(markdown.contains("not calibrated precision/recall"));
    assert!(markdown.contains("not policy-ready status"));
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
fn receipt_import_sanitizer_runtime_failure_records_confirmed() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");
    let temp = TempDir::new("unsafe-review-sanitizer-runtime-failure-e2e")?;
    let receipt_path = temp.path().join("asan_runtime_failure.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("import-sanitizer"),
        os(card_id),
        os("--tool"),
        os("asan"),
        os("--allow-runtime"),
        os("--log"),
        fixture.join("asan.runtime_failure.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("ASAN_OPTIONS=abort_on_error=0 ./target/release/my-program"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["tool"], "asan");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(receipt["verdict"], "confirmed");
    let summary = receipt["summary"].as_str().unwrap_or("");
    assert!(
        summary.contains("sanitizer signal observed"),
        "expected 'sanitizer signal observed' in summary, got: {summary}"
    );
    let limitations = receipt["limitations"]
        .as_array()
        .ok_or("receipt limitations should be an array")?;
    assert!(
        limitations
            .iter()
            .any(|item| { item.as_str().unwrap_or("").contains("runtime witness mode") })
    );
    Ok(())
}

#[test]
fn receipt_import_sanitizer_runtime_clean_records_not_reproduced() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment_receipted");
    let temp = TempDir::new("unsafe-review-sanitizer-runtime-clean-e2e")?;
    let receipt_path = temp.path().join("asan_runtime_clean.json");
    let card_id =
        "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    let output = run_success([
        os("receipt"),
        os("import-sanitizer"),
        os(card_id),
        os("--tool"),
        os("asan"),
        os("--allow-runtime"),
        os("--log"),
        fixture.join("asan.runtime_clean.log").into_os_string(),
        os("--author"),
        os("core/fixtures"),
        os("--recorded-at"),
        os("2026-05-18T00:00:00Z"),
        os("--expires-at"),
        os("2026-08-18"),
        os("--command"),
        os("ASAN_OPTIONS=abort_on_error=0 ./target/release/my-program"),
        os("--limitation"),
        os("fixture only"),
        os("--out"),
        receipt_path.as_os_str().to_os_string(),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");
    let receipt = parse_json(&fs::read_to_string(receipt_path)?)?;
    assert_eq!(receipt["tool"], "asan");
    assert_eq!(receipt["strength"], "ran");
    assert_eq!(receipt["verdict"], "not_reproduced");
    let summary = receipt["summary"].as_str().unwrap_or("");
    assert!(
        summary.contains("no sanitizer signal"),
        "expected 'no sanitizer signal' in summary, got: {summary}"
    );
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
    // Exit-code taxonomy: policy violations exit 1, tool errors exit 2.
    assert_eq!(
        failing.status.code(),
        Some(1),
        "no-new-debt violation must exit 1 (policy), not 2 (tool error)"
    );
    let failing_json = parse_json(&stdout_text(&failing)?)?;
    assert_eq!(failing_json["policy"], "no-new-debt");
    assert_eq!(failing_json["summary"]["open_actionable_gaps"], 1);
    // SPEC-0030: no-new-debt fails on new/worsened gaps, not total open actionable count.
    let failing_stderr = String::from_utf8(failing.stderr.clone())?;
    assert!(
        failing_stderr.contains("no-new-debt policy: 1 new gap(s)"),
        "stderr should contain policy gap counts: {failing_stderr}"
    );
    // The policy category prefix must appear in stderr so wrappers can distinguish
    // policy violations from tool errors.
    assert!(
        failing_stderr.contains("policy:"),
        "stderr should carry the 'policy:' category prefix: {failing_stderr}"
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

// Brownfield / inherited-debt corpus case (Doc-5 adoption criterion):
// A repo with pre-existing unsafe gaps baselined before the PR shows the
// inherited shape: inherited_gaps > 0, new_gaps == 0, no-new-debt passes,
// and inherited cards are NOT selected for inline PR comments.
#[test]
fn brownfield_inherited_baseline_shows_no_new_debt_with_inherited_shape()
-> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_deref_brownfield_inherited");

    // The fixture ships with a committed baseline ledger that captures the
    // pre-existing raw_pointer_deref card.  Running without --policy should
    // exit 0 (advisory) and report inherited_gaps=1, new_gaps=0.
    let advisory = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    assert_eq!(advisory["summary"]["new_gaps"], 0, "no new gaps expected");
    assert_eq!(
        advisory["summary"]["inherited_gaps"], 1,
        "one inherited gap expected"
    );
    assert_eq!(advisory["summary"]["worsened_gaps"], 0);
    assert_eq!(advisory["summary"]["open_actionable_gaps"], 0);
    assert_eq!(advisory["cards"][0]["class"], "baseline_known");
    assert_eq!(
        advisory["cards"][0]["coverage"]["baseline_state"],
        "inherited"
    );

    // The comment-plan must NOT select the inherited card for inline PR
    // comments — it is not actionable and should be downgraded/summarised.
    let out_dir = TempDir::new("unsafe-review-brownfield-e2e")?;
    run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.path().as_os_str().to_os_string(),
    ])?;
    let comment_plan_path = out_dir.path().join("comment-plan.json");
    let comment_plan = parse_json(&fs::read_to_string(&comment_plan_path)?)?;
    assert_eq!(
        comment_plan["summary"]["selected_count"], 0,
        "inherited card must not be selected for inline PR comment"
    );
    assert_eq!(comment_plan["summary"]["not_selected_count"], 1);
    let not_selected = &comment_plan["not_selected"][0];
    assert_eq!(not_selected["class"], "baseline_known");
    assert_eq!(not_selected["actionability"], "not_actionable");

    // Usefulness telemetry must record inherited_cards=1.
    let telemetry_path = out_dir.path().join("usefulness-telemetry.json");
    let telemetry = parse_json(&fs::read_to_string(&telemetry_path)?)?;
    assert_eq!(
        telemetry["card_inventory"]["inherited_cards"], 1,
        "telemetry must record the inherited card shape"
    );
    assert_eq!(telemetry["card_inventory"]["new_cards"], 0);

    // no-new-debt policy must pass (exit 0) because the diff adds no new gap.
    let passing = run_success([
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
    let passing = parse_json(&stdout_text(&passing)?)?;
    assert_eq!(passing["policy"], "no-new-debt");
    assert_eq!(
        passing["summary"]["new_gaps"], 0,
        "no-new-debt must pass with new_gaps=0"
    );
    assert_eq!(passing["summary"]["inherited_gaps"], 1);

    Ok(())
}

// Resolved corpus case (Doc-5 adoption criterion, inverse of brownfield):
// A PR that adds a `# Safety` contract to a `pub unsafe fn` (unsafe retained)
// shows the resolved shape: resolved_gaps=1, new_gaps=0, worsened_gaps=0.
// The improvement is recorded in usefulness telemetry as resolved_cards=1.
//
// Trust boundary: "resolved" means the baseline-captured coverage gap is absent
// from the AFTER output because the caller obligations are now documented.  It
// does not prove the code is memory-safe, UB-free, or Miri-clean.
#[test]
fn resolved_corpus_case_shows_resolved_gap_and_no_new_debt() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_deref_resolved");

    // The fixture ships with a committed baseline ledger that captures the
    // pre-existing contract_missing card for `pub unsafe fn read_config`.
    // The PR diff adds a `# Safety` section (unsafe retained).  Running check
    // must report resolved_gaps=1, new_gaps=0, worsened_gaps=0,
    // open_actionable_gaps=0.
    let advisory = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    assert_eq!(
        advisory["summary"]["resolved_gaps"], 1,
        "one resolved gap expected: the baseline card is gone after the # Safety contract was added"
    );
    assert_eq!(
        advisory["summary"]["new_gaps"], 0,
        "no new gaps expected: the PR only adds a # Safety contract"
    );
    assert_eq!(advisory["summary"]["worsened_gaps"], 0);
    assert_eq!(advisory["summary"]["inherited_gaps"], 0);
    assert_eq!(advisory["summary"]["open_actionable_gaps"], 0);
    assert_eq!(
        advisory["cards"].as_array().map(|a| a.len()).unwrap_or(1),
        0
    );

    // The no-new-debt policy must pass (exit 0) because the diff adds no new
    // gap — it only resolves the existing baseline gap via a # Safety contract.
    let passing = run_success([
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
    let passing = parse_json(&stdout_text(&passing)?)?;
    assert_eq!(passing["policy"], "no-new-debt");
    assert_eq!(
        passing["summary"]["new_gaps"], 0,
        "no-new-debt must pass with new_gaps=0"
    );
    assert_eq!(passing["summary"]["resolved_gaps"], 1);

    // Usefulness telemetry must record resolved_cards=1 and new_cards=0.
    let out_dir = TempDir::new("unsafe-review-resolved-e2e")?;
    run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.path().as_os_str().to_os_string(),
    ])?;
    let telemetry_path = out_dir.path().join("usefulness-telemetry.json");
    let telemetry = parse_json(&fs::read_to_string(&telemetry_path)?)?;
    assert_eq!(
        telemetry["card_inventory"]["resolved_cards"], 1,
        "telemetry must record the resolved card shape"
    );
    assert_eq!(telemetry["card_inventory"]["new_cards"], 0);
    assert_eq!(telemetry["card_inventory"]["inherited_cards"], 0);

    // Gate manifest must also surface resolved_gaps=1.
    let gate_path = out_dir.path().join("unsafe-review-gate.json");
    let gate = parse_json(&fs::read_to_string(&gate_path)?)?;
    assert_eq!(gate["summary"]["resolved_gaps"], 1);
    assert_eq!(gate["summary"]["new_gaps"], 0);
    assert_eq!(gate["summary"]["worsened_gaps"], 0);

    Ok(())
}

// Exit-code taxonomy tests (issue #1518):
//   0 = ran to completion (clean or advisory findings)
//   1 = ran to completion: policy violation
//   2 = tool did not complete: usage / input / IO / internal error

#[test]
fn unknown_flag_exits_2() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let output = run_failure([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--this-flag-does-not-exist"),
    ])?;
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown flag must exit 2 (tool/usage error)"
    );
    Ok(())
}

#[test]
fn first_pr_unknown_flag_out_exits_2_without_writing_bundle() -> Result<(), Box<dyn Error>> {
    // Regression test for EffortlessMetrics/unsafe-review#531:
    // `first-pr --out <dir>` must exit 2 (usage/input error), name the unknown
    // flag in the diagnostic, and must NOT silently write a review bundle to the
    // default `target/unsafe-review` output directory.
    let source_fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-first-pr-unknown-flag-e2e")?;
    let fixture = temp.path().join("fixture");
    copy_dir_all(&source_fixture, &fixture)?;
    // The default out-dir would be inside the fixture copy; confirm it is absent.
    let default_out_dir = fixture.join("target").join("unsafe-review");

    let output = run_failure([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out"),
        temp.path().join("sensor-dir").as_os_str().to_os_string(),
    ])?;

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown `--out` flag on first-pr must exit 2 (usage/input error)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--out"),
        "diagnostic must name the unknown flag `--out`: {stderr}"
    );
    assert!(
        stderr.contains("--out-dir"),
        "diagnostic must suggest `--out-dir`: {stderr}"
    );
    assert!(
        !default_out_dir.exists(),
        "no review bundle must be written to default out-dir on a bad invocation: {default_out_dir:?}"
    );
    Ok(())
}

#[test]
fn first_pr_format_flag_exits_2_without_writing_bundle() -> Result<(), Box<dyn Error>> {
    // Generalizes EffortlessMetrics/unsafe-review#531 to --format/--json/--markdown:
    // these flags belong to `check`/`repo`. `first-pr` always writes a full
    // advisory artifact bundle to `--out-dir` and never honors a format flag.
    // Each form must exit 2 (usage/input error) and must NOT write a bundle.
    let source_fixture = fixture_root("raw_pointer_alignment");

    for (label, flag_args) in [
        ("--format json (space)", vec![os("--format"), os("json")]),
        ("--format=json (equals)", vec![os("--format=json")]),
        ("--json shorthand", vec![os("--json")]),
        ("--markdown shorthand", vec![os("--markdown")]),
    ] {
        let temp = TempDir::new("unsafe-review-first-pr-format-flag-e2e")?;
        let fixture = temp.path().join("fixture");
        copy_dir_all(&source_fixture, &fixture)?;
        let default_out_dir = fixture.join("target").join("unsafe-review");

        let mut cmd_args = vec![
            os("first-pr"),
            os("--root"),
            fixture.as_os_str().to_os_string(),
            os("--diff"),
            fixture.join("change.diff").into_os_string(),
        ];
        cmd_args.extend(flag_args);

        let output = run_failure(cmd_args)?;

        assert_eq!(
            output.status.code(),
            Some(2),
            "{label}: --format/--json/--markdown on first-pr must exit 2"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("check") || stderr.contains("repo"),
            "{label}: diagnostic must mention `check`/`repo` subcommands: {stderr}"
        );
        assert!(
            !default_out_dir.exists(),
            "{label}: no review bundle must be written on a bad invocation: {default_out_dir:?}"
        );
    }
    Ok(())
}

#[test]
fn first_pr_policy_flag_exits_2_without_writing_bundle() -> Result<(), Box<dyn Error>> {
    // Generalizes EffortlessMetrics/unsafe-review#531 to --policy:
    // this flag belongs to `check`/`repo`. `first-pr` is always advisory-only
    // and never honors a policy flag. Must exit 2 and must NOT write a bundle.
    let source_fixture = fixture_root("raw_pointer_alignment");

    for (label, flag_args) in [
        (
            "--policy no-new-debt (space)",
            vec![os("--policy"), os("no-new-debt")],
        ),
        (
            "--policy=no-new-debt (equals)",
            vec![os("--policy=no-new-debt")],
        ),
    ] {
        let temp = TempDir::new("unsafe-review-first-pr-policy-flag-e2e")?;
        let fixture = temp.path().join("fixture");
        copy_dir_all(&source_fixture, &fixture)?;
        let default_out_dir = fixture.join("target").join("unsafe-review");

        let mut cmd_args = vec![
            os("first-pr"),
            os("--root"),
            fixture.as_os_str().to_os_string(),
            os("--diff"),
            fixture.join("change.diff").into_os_string(),
        ];
        cmd_args.extend(flag_args);

        let output = run_failure(cmd_args)?;

        assert_eq!(
            output.status.code(),
            Some(2),
            "{label}: --policy on first-pr must exit 2"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("check") || stderr.contains("repo"),
            "{label}: diagnostic must mention `check`/`repo` subcommands: {stderr}"
        );
        assert!(
            !default_out_dir.exists(),
            "{label}: no review bundle must be written on a bad invocation: {default_out_dir:?}"
        );
    }
    Ok(())
}

#[test]
fn missing_diff_file_exits_2() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let missing = fixture.join("does-not-exist.diff");
    let output = run_failure([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        missing.as_os_str().to_os_string(),
    ])?;
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing diff file must exit 2 (tool/input error)"
    );
    Ok(())
}

#[test]
fn advisory_run_with_findings_exits_0() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    // Advisory policy (default) exits 0 even when cards are found.
    let output = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "advisory run with findings must exit 0"
    );
    let value = parse_json(&stdout_text(&output)?)?;
    // Confirm there actually are cards so the test is not vacuous.
    assert_eq!(
        value["summary"]["cards"], 1,
        "fixture should produce 1 card (test is checking exit 0 with findings)"
    );
    Ok(())
}

#[test]
fn baseline_init_out_override_never_writes_into_root() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-baseline-out-e2e")?;
    let copied = temp.path().join("fixture");
    copy_dir_all(&fixture, &copied)?;
    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir)?;
    let out_ledger = out_dir.join("consumer-baseline.toml");

    let output = run_success([
        os("baseline"),
        os("init"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--out"),
        out_ledger.as_os_str().to_os_string(),
    ])?;
    let stdout = stdout_text(&output)?;
    assert!(stdout.contains("baseline init: ok"));
    assert!(stdout.contains("consumer-baseline-snapshot.toml"));

    // Both authored files follow --out as siblings.
    assert!(out_ledger.is_file());
    assert!(out_dir.join("consumer-baseline-snapshot.toml").is_file());

    // The scanned root stays read-only: no ledger, snapshot, or policy directory
    // is created inside --root when --out points elsewhere.
    assert!(!copied.join("policy").exists());

    Ok(())
}

#[test]
fn baseline_init_stdout_lists_debt_scope() -> Result<(), Box<dyn Error>> {
    // The atomic_pointer_state_fetch_ops fixture has 3 actionable cards
    // (class: requires_loom). Verify that baseline init outputs a debt scope
    // listing with card ids and location information.
    let fixture = fixture_root("atomic_pointer_state_fetch_ops");
    let temp = TempDir::new("unsafe-review-baseline-debt-scope-e2e")?;
    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir)?;
    let out_ledger = out_dir.join("debt-scope-baseline.toml");

    let output = run_success([
        os("baseline"),
        os("init"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--out"),
        out_ledger.as_os_str().to_os_string(),
    ])?;
    let stdout = stdout_text(&output)?;

    // The debt scope heading must appear.
    assert!(
        stdout.contains("debt scope:"),
        "expected 'debt scope:' in stdout:\n{stdout}"
    );

    // At least one UR- card id must appear in the debt scope listing.
    assert!(
        stdout.contains("UR-"),
        "expected at least one UR- card id in stdout:\n{stdout}"
    );

    // Extract the debt scope block lines (lines beginning with "  UR-") and
    // verify none of them contain forbidden trust-boundary overclaims.
    let debt_scope_lines: Vec<&str> = stdout.lines().filter(|l| l.starts_with("  UR-")).collect();
    assert!(
        !debt_scope_lines.is_empty(),
        "expected at least one '  UR-' debt scope line in stdout:\n{stdout}"
    );
    for line in &debt_scope_lines {
        assert!(
            !line.contains("memory-safe")
                && !line.contains("UB-free")
                && !line.contains("Miri-clean")
                && !line.contains("site-execution"),
            "debt scope line must not contain forbidden trust-boundary claims: {line}"
        );
    }

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
    assert_eq!(report["limitations"].as_array().map(Vec::len), Some(5));
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
    assert!(
        json_str(&report["trust_boundary"], "trust_boundary")?
            .contains("not a site-execution claim")
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
    // SPEC-0030: reviewer front panel shows movement counts.
    assert!(markdown.contains("- Movement: 0 new gap(s), 0 worsened, 0 improved (evidence coverage improved; still advisory), 1 resolved, 1 inherited"));
    assert!(markdown.contains("- Current ledger-covered cards: 1 baseline-known, 0 suppressed"));
    assert!(markdown.contains("- Ledger cleanup: 1 resolved baseline entries"));
    assert!(markdown.contains("consider pruning or updating resolved baseline entries"));
    assert!(markdown.contains("advisory policy simulation only"));
    assert!(markdown.contains("## Classification explanations"));
    assert!(markdown.contains("Exact ReviewCard identity matched a baseline ledger entry"));
    assert!(markdown.contains("Next action"));
    assert!(markdown.contains("| Status | Baseline | Changed |"));
    assert!(markdown.contains("raw_pointer_read"));
    assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
    assert!(markdown.contains("Keep the baseline ledger"));
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

fn assert_manual_candidate_front_panel(
    text: &str,
    later_heading: &str,
    expected_queue_len: usize,
    compact: bool,
) {
    assert!(text.contains("## Manual candidates"));
    assert!(text.contains(
        "- Imported manual candidates: 2 (manual/advisory; not analyzer-discovered ReviewCards)"
    ));
    assert!(text.contains("- Operation families: `raw_pointer_read: 1, slice_from_raw_parts: 1`"));
    assert!(text.contains("- Evidence kinds: `model: 2, runtime_witness: 2, source_trace: 2`"));
    assert!(text.contains(
        "- First manual candidate: `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`)"
    ));
    if compact {
        assert!(text.contains(
            "- Stable-byte class: `stable-byte-source-sab-race`; proof `mutation-plus-miri`; ledger `handoff-ready`; route `SharedArrayBuffer-backed typed array decode` -> `src/runtime/webcore/TextDecoder.rs slice materialization`; hazard in sidecars"
        ));
        assert!(text.contains("- Evidence refs: 3; full route and evidence packet in sidecars."));
        assert!(!text.contains("- Safe caller route:"));
        assert!(!text.contains("- Invariant at risk:"));
        assert!(!text.contains("- External evidence refs:"));
    } else {
        assert!(text.contains(
            "- Safe caller route: new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
        ));
        assert!(
            text.contains("- Invariant at risk: &[u8] memory must not be concurrently mutated")
        );
        assert!(text.contains("- External evidence refs: 3"));
        assert!(text.contains(
            "- Stable-byte class: `stable-byte-source-sab-race` (observable: `no`; proof required: `mutation-plus-miri`; ledger state: `handoff-ready`)"
        ));
        assert!(text.contains(
            "- Stable-byte route: source `SharedArrayBuffer-backed typed array decode` -> sink `src/runtime/webcore/TextDecoder.rs slice materialization`"
        ));
        assert!(text.contains(
            "- Stable-byte hazard: Rust slice materialization can treat shared JS bytes as stable while JS can mutate the backing storage concurrently"
        ));
    }
    if compact {
        assert!(text.contains(
            "- Proof mode: `mutation-plus-miri` (system Bun expected: `nondiscriminating`; mutation required: `true`; Miri/model required: `true`)"
        ));
        assert!(text.contains(
            "- Oracle map: `src/runtime/webcore/TextDecoder.rs::decode` -> `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(text.contains("`shared-byte-mutation-model`; limitation in sidecars"));
    } else {
        assert!(text.contains(
            "- Proof mode: `mutation-plus-miri` (system Bun expected: `nondiscriminating`; mutation required: `true`; Miri/model required: `true`)"
        ));
        assert!(text.contains(
            "- Oracle map: Rust seam `src/runtime/webcore/TextDecoder.rs::decode` -> `typescript` oracle `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(
            text.contains("not witness execution, site-execution proof, or memory-safety proof")
        );
    }
    if compact {
        assert!(text.contains(
            "- Fix boundary: Snapshot shared/growable/resizable bytes before Rust receives &[u8]"
        ));
        assert!(text.contains(
            "- PR aperture: TextDecoder shared-byte snapshot only; do not patch S3, fs, writev, or unrelated encodings"
        ));
        assert!(text.contains("- Stop line: keep the PR inside this aperture."));
        assert!(
            text.contains("- Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)")
        );
        assert!(text.contains(
            "- First fix option: Copy SharedArrayBuffer-backed bytes into stable owned storage before creating a Rust slice"
        ));
        assert!(text.contains(
            "- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(text.contains(
            "- First do-not-touch note: Do not rewrite unrelated TextDecoder encoding paths"
        ));
    } else {
        assert!(text.contains(
            "- Fix boundary: Snapshot shared/growable/resizable bytes before Rust receives &[u8]"
        ));
        assert!(text.contains(
            "- PR aperture: TextDecoder shared-byte snapshot only; do not patch S3, fs, writev, or unrelated encodings"
        ));
        assert!(text.contains("- Stop line: keep the PR inside this aperture"));
        assert!(
            text.contains("- Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)")
        );
        assert!(text.contains(
            "- First fix option: Copy SharedArrayBuffer-backed bytes into stable owned storage before creating a Rust slice"
        ));
        assert!(text.contains(
            "- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(text.contains(
            "- First do-not-touch note: Do not rewrite unrelated TextDecoder encoding paths"
        ));
    }
    if compact {
        assert!(!text.contains("- Manual candidate queue preview:"));
    } else {
        assert!(text.contains(&format!(
            "- Manual candidate queue preview: first {expected_queue_len} of 2 manual candidate(s)"
        )));
        assert!(text.contains(
            "`R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`); evidence refs: 3; proof mode: `mutation-plus-miri`"
        ));
        if expected_queue_len >= 2 {
            assert!(text.contains(
                "`R4R2-S002` at `src/sql_jsc/mysql/MySQLValue.rs:411` (`slice_from_raw_parts`); evidence refs: 3; proof mode: `mutation-plus-miri`"
            ));
        } else {
            assert!(!text.contains(
                "`R4R2-S002` at `src/sql_jsc/mysql/MySQLValue.rs:411` (`slice_from_raw_parts`); evidence refs: 3; proof mode: `mutation-plus-miri`"
            ));
        }
    }
    if !compact {
        assert!(text.contains("unsafe-review explain --root"));
    }
    assert!(text.contains("unsafe-review context --root"));
    assert!(text.contains("unsafe-review candidate witness-plan --root"));
    assert!(text.contains("manual-candidates.json"));
    assert!(text.contains("manual-repair-queue.json"));
    assert!(text.contains("separate from ReviewCard `repair-queue.json`"));
    assert!(text.contains("no agent was run"));
    assert!(text.contains("ReviewCard-only outputs"));
    assert!(text.contains("did not discover"));
    assert!(text.contains("did not run witnesses"));
    assert!(text.contains("edit source"));
    assert!(text.contains("policy inputs"));
    let manual_index = text.find("## Manual candidates");
    let later_index = text.find(later_heading);
    assert!(
        manual_index.is_some(),
        "manual candidate section should exist"
    );
    assert!(
        later_index.is_some(),
        "later front-door heading should exist"
    );
    assert!(manual_index < later_index);
}

fn assert_manual_candidate_witness_follow_up(text: &str) {
    assert!(text.contains("## Manual candidate witness follow-up"));
    assert!(text.contains(
        "- Imported manual candidates: 2 (manual/advisory; not analyzer-discovered ReviewCards)"
    ));
    assert!(text.contains("- Operation families: `raw_pointer_read: 1, slice_from_raw_parts: 1`"));
    assert!(text.contains("- Evidence kinds: `model: 2, runtime_witness: 2, source_trace: 2`"));
    assert!(text.contains(
        "- First manual candidate: `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`)"
    ));
    assert!(text.contains(
        "- Safe caller route: new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
    ));
    assert!(text.contains("- Invariant at risk: &[u8] memory must not be concurrently mutated"));
    assert!(text.contains("- External evidence refs: 3"));
    assert!(text.contains(
        "- Stable-byte class: `stable-byte-source-sab-race` (observable: `no`; proof required: `mutation-plus-miri`; ledger state: `handoff-ready`)"
    ));
    assert!(text.contains(
        "- Stable-byte route: source `SharedArrayBuffer-backed typed array decode` -> sink `src/runtime/webcore/TextDecoder.rs slice materialization`"
    ));
    assert!(text.contains(
        "- Stable-byte hazard: Rust slice materialization can treat shared JS bytes as stable while JS can mutate the backing storage concurrently"
    ));
    assert!(text.contains(
        "- Proof mode: `mutation-plus-miri` (system Bun expected: `nondiscriminating`; mutation required: `true`; Miri/model required: `true`)"
    ));
    assert!(text.contains(
        "- Oracle map: Rust seam `src/runtime/webcore/TextDecoder.rs::decode` -> `typescript` oracle `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
    ));
    assert!(text.contains("not witness execution, site-execution proof, or memory-safety proof"));
    assert!(text.contains(
        "- Fix boundary: Snapshot shared/growable/resizable bytes before Rust receives &[u8]"
    ));
    assert!(text.contains(
        "- PR aperture: TextDecoder shared-byte snapshot only; do not patch S3, fs, writev, or unrelated encodings"
    ));
    assert!(text.contains("- Stop line: keep the PR inside this aperture"));
    assert!(text.contains("- Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)"));
    assert!(text.contains(
        "- First fix option: Copy SharedArrayBuffer-backed bytes into stable owned storage before creating a Rust slice"
    ));
    assert!(
        text.contains(
            "- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        )
    );
    assert!(text.contains(
        "- First do-not-touch note: Do not rewrite unrelated TextDecoder encoding paths"
    ));
    assert!(text.contains("- Manual candidate queue preview: first 2 of 2 manual candidate(s)"));
    assert!(text.contains(
        "`R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`); evidence refs: 3; proof mode: `mutation-plus-miri`"
    ));
    assert!(text.contains(
        "`R4R2-S002` at `src/sql_jsc/mysql/MySQLValue.rs:411` (`slice_from_raw_parts`); evidence refs: 3; proof mode: `mutation-plus-miri`"
    ));
    assert!(text.contains("unsafe-review candidate witness-plan --root"));
    assert!(text.contains("unsafe-review context --root"));
    assert!(text.contains("manual-candidates.json"));
    assert!(text.contains("ReviewCard-only witness route groups"));
    assert!(text.contains("do not import ReviewCard witness evidence"));
    assert!(text.contains("copy-only manual follow-up"));
    assert!(text.contains("did not discover these candidates"));
    assert!(text.contains("did not run witnesses"));
    assert!(text.contains("did not edit source"));
    assert!(text.contains("policy inputs"));
    let witness_index = text.find("## Manual candidate witness follow-up");
    let trust_index = text.find("## Trust boundary");
    assert!(
        witness_index.is_some(),
        "manual candidate witness section should exist"
    );
    assert!(trust_index.is_some(), "trust boundary section should exist");
    assert!(witness_index < trust_index);
}

#[test]
fn repo_max_cards_cap_emits_partial_status_sidecar() -> Result<(), Box<dyn Error>> {
    // Build a temp crate with >=2 unsafe sites so max-cards=1 stops early.
    let temp = TempDir::new("unsafe-review-repo-maxcards-e2e")?;
    let scan_root = temp.path().join("fixture");
    fs::create_dir_all(scan_root.join("src"))?;
    fs::write(
        scan_root.join("Cargo.toml"),
        "[package]\nname = \"maxcards-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    // Two unsafe files — cap of 1 guarantees the scan stops after the first card.
    fs::write(
        scan_root.join("src/lib.rs"),
        "pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;
    fs::write(
        scan_root.join("src/beta.rs"),
        "pub unsafe fn beta(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;

    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");
    let partial_path = temp.path().join("repo.json.partial");

    // A capped scan is NOT an error — exit code must be success.
    let output = run_success([
        os("repo"),
        os("--root"),
        scan_root.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
        os("--out"),
        report_path.as_os_str().to_os_string(),
        os("--max-cards"),
        os("1"),
    ])?;

    assert_eq!(stdout_text(&output)?.trim(), "");

    // The final report is written (a capped scan still promotes the capped result).
    assert!(
        report_path.exists(),
        "capped repo scan should write a final report"
    );
    // The partial file is promoted away after success.
    assert!(
        !partial_path.exists(),
        "capped scan should promote the partial file and remove it"
    );

    // Status sidecar must exist and carry the partial/cap markers.
    assert!(
        status_path.exists(),
        "capped repo scan should write a status sidecar"
    );
    let status = parse_json(&fs::read_to_string(&status_path)?)?;
    assert_eq!(status["schema_version"], "repo-scan-status/v1");
    assert_eq!(
        status["phase"], "complete",
        "capped scan phase should still be 'complete'"
    );
    assert_eq!(
        status["completed"], false,
        "capped scan completed must be false"
    );
    assert_eq!(
        status["partial"], true,
        "capped scan must mark partial=true"
    );
    assert_eq!(
        status["stop_reason"], "max_cards",
        "capped scan must carry stop_reason=max_cards"
    );
    assert_eq!(
        status["cap"], 1,
        "capped scan must record the configured cap"
    );
    assert_eq!(
        status["cards_found"], 1,
        "capped scan cards_found must equal the emitted card count"
    );
    assert!(status["error"].is_null(), "capped scan must not set error");
    assert!(
        status["signal"].is_null(),
        "capped scan must not set signal"
    );
    assert!(
        status["partial_path"].is_null(),
        "capped scan must not set partial_path (the final report is the capped artifact)"
    );

    // Operator block: state=capped, next_action mentions include/exclude or max-cards.
    let operator = status
        .get("operator")
        .ok_or("repo status missing operator")?;
    assert_eq!(operator["state"], "capped");
    assert_eq!(
        operator["downstream_consumable"], true,
        "a capped scan is downstream-consumable (it produced a valid bounded report)"
    );
    assert_eq!(operator["partial_report_available"], false);
    let next_action = operator["next_action"].as_str().unwrap_or("");
    assert!(
        next_action.contains("include/exclude") || next_action.contains("max-cards"),
        "operator next_action should guide narrowing scope or raising cap: {next_action}"
    );
    let limitation = operator["partial_report_limitation"].as_str().unwrap_or("");
    // Bug B fix: capped scans use card-level wording (all files scanned,
    // card list truncated) instead of the old file-level snapshot wording.
    assert!(
        limitation.contains("All files scanned"),
        "operator limitation must say all files were scanned (card-level wording): {limitation}"
    );
    assert!(
        limitation.contains("card list truncated") || limitation.contains("--max-cards"),
        "operator limitation must describe card-list truncation: {limitation}"
    );
    let boundary = operator["claim_boundary"].as_str().unwrap_or("");
    assert!(
        boundary.contains("not complete repo posture"),
        "operator claim_boundary must carry partial posture wording: {boundary}"
    );

    Ok(())
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

fn assert_repo_status_operator(
    status: &Value,
    state: &str,
    partial_report_available: bool,
    next_action_contains: &str,
) -> Result<(), Box<dyn Error>> {
    let operator = status
        .get("operator")
        .ok_or("repo status is missing operator diagnostics")?;
    assert_eq!(operator["state"], state);
    // downstream_consumable is true only for complete and capped scans; all other
    // states (in_progress, failed, terminated) are not safe to consume.
    let expected_consumable = state == "complete" || state == "capped";
    assert_eq!(
        operator["downstream_consumable"], expected_consumable,
        "operator downstream_consumable should be {expected_consumable} for state={state}"
    );
    assert_eq!(
        operator["partial_report_available"],
        partial_report_available
    );
    let limitation = operator["partial_report_limitation"].as_str().unwrap_or("");
    assert!(
        !limitation.is_empty(),
        "operator should explain partial report limitations"
    );
    if partial_report_available {
        assert!(
            limitation.contains("Completed-file snapshot only"),
            "operator limitation should describe partial snapshot scope: {limitation}"
        );
    } else {
        assert!(
            limitation.contains("No partial report")
                || limitation.contains("No completed-file partial report"),
            "operator limitation should explain absence of partial report: {limitation}"
        );
    }
    let next_action = operator["next_action"].as_str().unwrap_or("");
    assert!(
        next_action.contains(next_action_contains),
        "operator next_action `{next_action}` should include `{next_action_contains}`"
    );
    assert!(
        next_action.contains("scan_scope"),
        "operator next_action should point back to replayable scan_scope: {next_action}"
    );
    let boundary = operator["claim_boundary"].as_str().unwrap_or("");
    for expected in [
        "Operational scan status only",
        "not complete repo posture",
        "witness execution",
        "proof",
        "UB-free status",
        "Miri-clean status",
        "site-execution proof",
        "policy gating",
    ] {
        assert!(
            boundary.contains(expected),
            "operator claim_boundary `{boundary}` should include `{expected}`"
        );
    }
    Ok(())
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn manual_candidate_example_path() -> PathBuf {
    manual_candidate_examples_dir().join("textdecoder-sab.json")
}

fn manual_candidate_examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/manual-candidates")
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

fn write_textdecoder_stable_byte_seed_ledger(root: &Path) -> Result<(), Box<dyn Error>> {
    let docs_dir = root.join("docs/dogfood");
    fs::create_dir_all(&docs_dir)?;
    fs::write(
        docs_dir.join("stable-byte-follow-up-seeds.md"),
        r#"# Bun stable-byte follow-up seed index

## Seeds

| Seed ID | Ledger state | Candidate family | Surface | Manual candidate | Safe JS caller | Rust/native sink | Proof mode | Suggested first PR | Owner lane | Triage labels |
|---|---|---|---|---|---|---|---|---|---|---|
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `.unsafe-review/candidates/R4R2-S001.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
"#,
    )?;
    Ok(())
}

fn mysql_manual_candidate_json() -> &'static str {
    include_str!("../../../docs/examples/manual-candidates/mysql-blob-sab.json")
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

#[test]
fn candidate_list_nonexistent_root_exits_2() -> Result<(), Box<dyn Error>> {
    let output = run_failure([
        os("candidate"),
        os("list"),
        os("--root"),
        os("/nonexistent/path/that/does/not/exist"),
    ])?;

    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code must be 2 for nonexistent root"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is not a directory"),
        "stderr should mention 'is not a directory': {stderr}"
    );
    Ok(())
}

#[test]
fn candidate_list_valid_root_no_candidates_exits_0() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");

    let output = run_success([
        os("candidate"),
        os("list"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
    ])?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "valid root with no candidates subdir must still exit 0"
    );
    Ok(())
}

#[test]
fn receipt_validate_nonexistent_root_exits_2() -> Result<(), Box<dyn Error>> {
    let output = run_failure([
        os("receipt"),
        os("validate"),
        os("--root"),
        os("/nonexistent/path/that/does/not/exist"),
    ])?;

    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code must be 2 for nonexistent root"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is not a directory"),
        "stderr should mention 'is not a directory': {stderr}"
    );
    Ok(())
}

#[test]
fn receipt_validate_valid_root_no_receipts_exits_0() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");

    let output = run_success([
        os("receipt"),
        os("validate"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
    ])?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "valid root with no receipts subdir must still exit 0"
    );
    Ok(())
}

// Coverage-improved movement case (SPEC-0030 symmetric to worsened):
// A PR that adds a `# Safety` contract to a private (non-unsafe) fn owning
// a raw pointer deref — whose contract slot was `missing` in the baseline
// snapshot — shows improved_gaps=1, worsened_gaps=0, resolved_gaps=0.
//
// The card is still `baseline_known`, still advisory, still open.  This is NOT
// a resolution, NOT safety proof, NOT UB-free, NOT Miri-clean.  The signal is
// "evidence coverage improved for a retained baseline site."
#[test]
fn coverage_improved_baseline_shows_improved_gap_shape() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_deref_coverage_improved");

    // The fixture ships with a baseline ledger (card is baseline_known) and a
    // baseline snapshot recording contract_coverage="missing".  The current
    // source has a `# Safety` doc → contract_coverage is now "present".
    let advisory = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    assert_eq!(
        advisory["summary"]["improved_gaps"], 1,
        "improved_gaps must be 1: contract slot advanced from missing to present"
    );
    assert_eq!(
        advisory["summary"]["worsened_gaps"], 0,
        "worsened_gaps must be 0: no slot regressed"
    );
    assert_eq!(
        advisory["summary"]["resolved_gaps"], 0,
        "resolved_gaps must be 0: card is still present (site not removed)"
    );
    assert_eq!(
        advisory["summary"]["new_gaps"], 0,
        "new_gaps must be 0: site is baseline_known"
    );
    assert_eq!(
        advisory["summary"]["inherited_gaps"], 1,
        "inherited_gaps must be 1: baseline_known card still open"
    );
    assert_eq!(
        advisory["summary"]["open_actionable_gaps"], 0,
        "open_actionable_gaps must be 0: baseline_known is not actionable"
    );
    // Card must still be baseline_known — NOT resolved.
    assert_eq!(advisory["cards"][0]["class"], "baseline_known");

    // Usefulness telemetry must record improved_cards=1.
    let out_dir = TempDir::new("unsafe-review-coverage-improved-e2e")?;
    run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.path().as_os_str().to_os_string(),
    ])?;
    let telemetry_path = out_dir.path().join("usefulness-telemetry.json");
    let telemetry = parse_json(&fs::read_to_string(&telemetry_path)?)?;
    assert_eq!(
        telemetry["card_inventory"]["improved_cards"], 1,
        "telemetry must record improved_cards=1"
    );
    assert_eq!(telemetry["card_inventory"]["worsened_cards"], 0);
    assert_eq!(telemetry["card_inventory"]["new_cards"], 0);
    assert_eq!(telemetry["card_inventory"]["resolved_cards"], 0);

    // PR summary must surface the positive movement signal.
    let pr_summary = fs::read_to_string(out_dir.path().join("pr-summary.md"))?;
    assert!(
        pr_summary.contains("1 improved"),
        "pr-summary must surface improved_gaps=1 as positive movement; got: {}",
        &pr_summary[..pr_summary.len().min(600)]
    );
    assert!(
        pr_summary.contains("still advisory"),
        "pr-summary improved wording must say 'still advisory' (trust boundary); got: {}",
        &pr_summary[..pr_summary.len().min(600)]
    );

    // Policy report must also surface improved_gaps=1 in the summary table.
    let policy_report = fs::read_to_string(out_dir.path().join("policy-report.md"))?;
    assert!(
        policy_report.contains("1 improved"),
        "policy-report must surface improved_gaps=1; got: {}",
        &policy_report[..policy_report.len().min(800)]
    );

    // no-new-debt must still pass (exit 0): an improved card is still
    // baseline_known; no new gap was added.
    let passing = run_success([
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
    let passing = parse_json(&stdout_text(&passing)?)?;
    assert_eq!(passing["policy"], "no-new-debt");
    assert_eq!(
        passing["summary"]["new_gaps"], 0,
        "no-new-debt must pass: improved card does not count as a new gap"
    );
    assert_eq!(passing["summary"]["improved_gaps"], 1);

    Ok(())
}

// SPEC-0030: per-card coverage.baseline_state/outcome_movement must agree with
// summary.worsened_gaps.  This fixture ships a baseline ledger + snapshot recording
// contract_coverage="present"; the current source has no # Safety doc →
// contract_coverage is now "missing".  The card must report baseline_state="worsened"
// and outcome_movement="regressed", and the summary must count worsened_gaps=1.
//
// The card is still `baseline_known`, still advisory, still open.  This is NOT
// a resolution, NOT safety proof, NOT UB-free, NOT Miri-clean.  The signal is
// "evidence coverage worsened for a retained baseline site."
#[test]
fn coverage_worsened_baseline_shows_worsened_gap_shape() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_deref_coverage_worsened");

    // The fixture ships with a baseline ledger (card is baseline_known) and a
    // baseline snapshot recording contract_coverage="present".  The current
    // source removed the `# Safety` doc → contract_coverage is now "missing".
    let advisory = run_success([
        os("check"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let advisory = parse_json(&stdout_text(&advisory)?)?;
    assert_eq!(
        advisory["summary"]["worsened_gaps"], 1,
        "worsened_gaps must be 1: contract slot regressed from present to missing"
    );
    assert_eq!(
        advisory["summary"]["improved_gaps"], 0,
        "improved_gaps must be 0: no slot advanced"
    );
    assert_eq!(
        advisory["summary"]["resolved_gaps"], 0,
        "resolved_gaps must be 0: card is still present (site not removed)"
    );
    assert_eq!(
        advisory["summary"]["new_gaps"], 0,
        "new_gaps must be 0: site is baseline_known"
    );
    assert_eq!(
        advisory["summary"]["inherited_gaps"], 1,
        "inherited_gaps must be 1: baseline_known card still open"
    );
    assert_eq!(
        advisory["summary"]["open_actionable_gaps"], 0,
        "open_actionable_gaps must be 0: baseline_known is not actionable"
    );
    // Card must still be baseline_known — NOT a new gap, NOT resolved.
    assert_eq!(advisory["cards"][0]["class"], "baseline_known");

    // SPEC-0030 single-truth: per-card coverage must agree with summary.
    // baseline_state must be "worsened" (contract slot regressed present→missing).
    assert_eq!(
        advisory["cards"][0]["coverage"]["baseline_state"], "worsened",
        "per-card baseline_state must be 'worsened' to agree with summary.worsened_gaps=1"
    );
    assert_eq!(
        advisory["cards"][0]["coverage"]["outcome_movement"], "regressed",
        "per-card outcome_movement must be 'regressed' when contract slot worsened"
    );

    // Usefulness telemetry must record worsened_cards=1.
    let out_dir = TempDir::new("unsafe-review-coverage-worsened-e2e")?;
    run_success([
        os("first-pr"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        os("--diff"),
        fixture.join("change.diff").into_os_string(),
        os("--out-dir"),
        out_dir.path().as_os_str().to_os_string(),
    ])?;
    let telemetry_path = out_dir.path().join("usefulness-telemetry.json");
    let telemetry = parse_json(&fs::read_to_string(&telemetry_path)?)?;
    assert_eq!(
        telemetry["card_inventory"]["worsened_cards"], 1,
        "telemetry must record worsened_cards=1"
    );
    assert_eq!(telemetry["card_inventory"]["improved_cards"], 0);
    assert_eq!(telemetry["card_inventory"]["new_cards"], 0);
    assert_eq!(telemetry["card_inventory"]["resolved_cards"], 0);

    // PR summary must surface the negative movement signal.
    let pr_summary = fs::read_to_string(out_dir.path().join("pr-summary.md"))?;
    assert!(
        pr_summary.contains("1 worsened"),
        "pr-summary must surface worsened_gaps=1 as negative movement; got: {}",
        &pr_summary[..pr_summary.len().min(600)]
    );

    // no-new-debt exits 1 for worsened gaps (same as new gaps): a worsened
    // card means evidence coverage regressed on a baseline site, which is a
    // policy signal worth surfacing as an advisory failure.
    let violation = run_failure([
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
    let violation_json = parse_json(&stdout_text(&violation)?)?;
    assert_eq!(violation_json["policy"], "no-new-debt");
    assert_eq!(
        violation_json["summary"]["new_gaps"], 0,
        "new_gaps must be 0: worsened card is not a new gap"
    );
    assert_eq!(violation_json["summary"]["worsened_gaps"], 1);

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

// SPEC-0030 §diff-scope: on a diff-scoped run, baseline cards whose file was NOT in the
// candidate set must be counted as `inherited`, not `resolved`.
//
// This test builds a two-file repo, captures a full baseline (both files), then runs a
// diff-scoped check where only `src/a.rs` is in the diff.  The `src/b.rs` card must NOT
// appear as `resolved_gaps=1` — it was never scanned, so the PR cannot claim it was fixed.
//
// Expected shape: resolved_gaps=0, inherited_gaps=4 (2 per file, 2 files), new_gaps=0.
#[test]
fn diff_scoped_run_does_not_count_unscanned_baseline_cards_as_resolved()
-> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-scope-guard-e2e")?;
    let root = temp.path().join("two-file-repo");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"two-file-repo\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    // Two files each with a raw pointer deref — both will generate cards on a full scan.
    fs::write(
        root.join("src/a.rs"),
        "pub unsafe fn op_a(ptr: *const u32) -> u32 {\n    // Safety: caller guarantees ptr is valid\n    unsafe { *ptr }\n}\n",
    )?;
    fs::write(
        root.join("src/b.rs"),
        "pub unsafe fn op_b(ptr: *const u64) -> u64 {\n    // Safety: caller guarantees ptr is valid\n    unsafe { *ptr }\n}\n",
    )?;

    // Capture a full-repo baseline so both cards are tracked.
    let out_dir = TempDir::new("unsafe-review-scope-guard-baseline")?;
    let out_ledger = out_dir.path().join("baseline.toml");
    run_success([
        os("baseline"),
        os("init"),
        os("--root"),
        root.as_os_str().to_os_string(),
        os("--out"),
        out_ledger.as_os_str().to_os_string(),
    ])?;

    // Copy the generated baseline into the repo's policy directory.
    let policy_dir = root.join("policy");
    fs::create_dir_all(&policy_dir)?;
    fs::copy(&out_ledger, policy_dir.join("unsafe-review-baseline.toml"))?;

    // A diff that touches only src/a.rs (adding an innocuous comment — no unsafe change,
    // so the file is in scope but the card stays BaselineKnown, not new).
    let diff_text = "\
diff --git a/src/a.rs b/src/a.rs\n\
--- a/src/a.rs\n\
+++ b/src/a.rs\n\
@@ -1,4 +1,5 @@\n\
+// reviewed in this PR\n\
 pub unsafe fn op_a(ptr: *const u32) -> u32 {\n\
     // Safety: caller guarantees ptr is valid\n\
     unsafe { *ptr }\n\
 }\n";

    // Diff-scoped check: only src/a.rs is a candidate file.
    let out = run_success_with_stdin(
        [
            os("check"),
            os("--root"),
            root.as_os_str().to_os_string(),
            os("--diff"),
            os("-"),
            os("--format"),
            os("json"),
        ],
        diff_text,
    )?;
    let value = parse_json(&stdout_text(&out)?)?;

    // SPEC-0030 scope guard: the b.rs card was not scanned and must NOT be resolved.
    assert_eq!(
        value["summary"]["resolved_gaps"], 0,
        "b.rs card must not be counted as resolved — its file was out of diff scope; got: {}",
        value["summary"]
    );
    // Each file has 2 cards (one `unsafe_fn` context + one `raw_pointer_deref` operation).
    // All 4 baseline IDs are inherited:
    //   - 2 from a.rs: appeared as BaselineKnown in the scan (file was in scope, still present)
    //   - 2 from b.rs: out-of-scope (file not in diff); counted as inherited, NOT resolved
    assert_eq!(
        value["summary"]["inherited_gaps"], 4,
        "all 4 baseline cards must be inherited (2 from a.rs scan + 2 from b.rs out-of-scope); got: {}",
        value["summary"]
    );
    assert_eq!(
        value["summary"]["new_gaps"], 0,
        "no new gaps expected: the diff only adds a comment to an already-baselined file; got: {}",
        value["summary"]
    );

    Ok(())
}
