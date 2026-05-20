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
    assert!(summary_text.contains("- Scope: `diff`"));
    assert!(summary_text.contains("- Review cards: 1"));
    assert!(summary_text.contains("- Open actionable gaps: 1"));
    assert!(summary_text.contains("- Policy mode: `advisory`"));
    assert!(summary_text.contains("## Top card"));
    assert!(summary_text.contains(&format!("- ID: `{card_id}`")));
    assert!(summary_text.contains("- Class: `guard_missing`"));
    assert!(summary_text.contains("- Location: src/lib.rs:8"));
    assert!(summary_text.contains("- Operation: `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(summary_text.contains("Missing visible local guard for inferred safety obligations"));
    assert!(summary_text.contains("No witness receipt imported for this card"));
    assert!(summary_text.contains("- Primary route: `miri` because"));
    assert!(summary_text.contains("cargo +nightly miri test read_header"));
    assert!(summary_text.contains("- Next action: Add or expose the local guard"));
    assert!(summary_text.contains("## Card table"));
    assert!(summary_text.contains(
        "| ID | Class | Location | Operation | Missing evidence | Route | Next action |"
    ));
    assert!(summary_text.contains(&format!("| `{card_id}` | `guard_missing` | src/lib.rs:8 | `unsafe {{ ptr.cast::<Header>().read() }}`")));
    assert!(summary_text.contains("## Witness plan"));
    assert!(summary_text.contains(&format!("- `{card_id}`: `miri` because")));
    assert!(summary_text.contains("## Trust boundary"));
    assert!(summary_text.contains("not a proof of memory safety"));
    assert!(summary_text.contains("not UB-free status"));
    assert!(summary_text.contains("not a Miri result unless a witness receipt is attached"));
    assert!(!summary_text.contains("blocking policy"));
    assert!(!summary_text.contains("posted comment"));

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
    let sarif_result = &sarif["runs"][0]["results"][0];
    let sarif_properties = &sarif_result["properties"];
    assert_eq!(sarif_result["ruleId"], value["cards"][0]["class"]);
    assert_eq!(sarif_properties["cardId"], card_id);
    assert_eq!(sarif_properties["class"], value["cards"][0]["class"]);
    assert_eq!(
        sarif_properties["operationFamily"],
        value["cards"][0]["operation_family"]
    );
    assert_eq!(
        sarif_properties["operation"],
        value["cards"][0]["site"]["snippet"]
    );
    assert_eq!(sarif_properties["hazards"], value["cards"][0]["hazards"]);
    assert_eq!(
        sarif_properties["missingEvidence"],
        value["cards"][0]["missing"]
    );
    assert!(
        sarif_properties["witnessRoutes"]
            .as_array()
            .is_some_and(|routes| routes.iter().any(|route| {
                route
                    .as_str()
                    .unwrap_or("")
                    .contains("Miri is the strongest concrete-execution witness")
            }))
    );
    assert!(
        sarif_properties["nextAction"]
            .as_str()
            .unwrap_or("")
            .contains("raw_pointer_read` safety obligation")
    );
    assert!(
        sarif_properties["trustBoundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a Miri result")
    );
    assert!(
        sarif["runs"][0]["properties"]["trustBoundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
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
    assert_eq!(comment_plan["policy"], "advisory");
    let planned_comment = &comment_plan["comments"][0];
    assert_eq!(comment_plan["comments"][0]["card_id"], card_id);
    assert_eq!(planned_comment["class"], value["cards"][0]["class"]);
    assert_eq!(planned_comment["priority"], value["cards"][0]["priority"]);
    assert_eq!(
        planned_comment["confidence"],
        value["cards"][0]["confidence"]
    );
    assert_eq!(
        planned_comment["operation_family"],
        value["cards"][0]["operation_family"]
    );
    assert_eq!(planned_comment["path"], value["cards"][0]["site"]["file"]);
    assert_eq!(planned_comment["line"], value["cards"][0]["site"]["line"]);
    assert!(
        planned_comment["selection_reason"]
            .as_str()
            .unwrap_or("")
            .contains("actionable high-priority review card")
    );
    let planned_body = planned_comment["body"].as_str().unwrap_or("");
    assert!(planned_body.contains("`guard_missing` for `raw_pointer_read`"));
    assert!(planned_body.contains("Missing visible local guard for inferred safety obligations"));
    assert!(planned_body.contains("No witness receipt imported for this card"));
    assert!(planned_body.contains("Next action: Add or expose the local guard"));
    assert!(planned_body.contains("Witness route: `miri` because"));
    assert!(planned_body.contains("not memory-safety proof"));
    assert!(!planned_body.contains("posted"));
    assert!(!planned_body.contains("blocking"));
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
    assert_eq!(lsp["policy"], "advisory");
    assert_eq!(lsp["scope"], "diff");
    assert_eq!(lsp["status"]["state"], "actionable");
    assert_eq!(lsp["status"]["cards"], 1);
    assert_eq!(lsp["status"]["open_actionable_gaps"], 1);
    assert_eq!(lsp["status"]["high_priority_cards"], 1);
    assert!(
        lsp["status"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );
    let diagnostic = &lsp["diagnostics"][0];
    assert_eq!(diagnostic["card_id"], card_id);
    assert_eq!(diagnostic["path"], value["cards"][0]["site"]["file"]);
    assert_eq!(diagnostic["code"], value["cards"][0]["class"]);
    assert_eq!(
        diagnostic["operation_family"],
        value["cards"][0]["operation_family"]
    );
    assert_eq!(diagnostic["hazards"], value["cards"][0]["hazards"]);
    assert_eq!(diagnostic["missing_evidence"], value["cards"][0]["missing"]);
    assert!(
        diagnostic["message"]
            .as_str()
            .unwrap_or("")
            .contains("raw_pointer_read: Add or expose the local guard")
    );
    assert!(
        diagnostic["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a proof of memory safety")
    );
    let hover = &lsp["hovers"][0];
    assert_eq!(hover["card_id"], card_id);
    assert_eq!(hover["path"], value["cards"][0]["site"]["file"]);
    let hover_contents = hover["contents"].as_str().unwrap_or("");
    assert!(hover_contents.contains("unsafe-review `guard_missing` for `raw_pointer_read`"));
    assert!(hover_contents.contains("Required safety conditions"));
    assert!(hover_contents.contains("pointer is aligned for the accessed type"));
    assert!(hover_contents.contains("Missing visible local guard for inferred safety obligations"));
    assert!(hover_contents.contains("Witness route: `miri` because"));
    assert!(hover_contents.contains("not a Miri result"));
    assert!(
        hover["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );
    assert_eq!(
        lsp["code_actions"][0]["command"],
        "unsafe-review.copyAgentPacket"
    );
    assert!(lsp["code_actions"].as_array().is_some_and(|actions| {
        actions
            .iter()
            .any(|action| action["command"] == "unsafe-review.openRelatedTest")
    }));
    assert!(lsp["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["command"] == "unsafe-review.copyWitnessCommand"
                && action["arguments"][0]
                    .as_str()
                    .unwrap_or("")
                    .contains("cargo +nightly miri test read_header")
        })
    }));
    let lsp_text = serde_json::to_string(&lsp)?;
    assert!(!lsp_text.contains("\"edit\""));
    assert!(!lsp_text.contains("workspace/applyEdit"));
    assert!(
        lsp["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a Miri result")
    );

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
    assert!(witness_plan.contains("- Review cards: 1"));
    assert!(witness_plan.contains("- Open actionable gaps: 1"));
    assert!(witness_plan.contains("- Policy mode: `advisory`"));
    assert!(witness_plan.contains("## Routes"));
    assert!(witness_plan.contains(&format!("### `{card_id}`")));
    assert!(witness_plan.contains("- Class: `guard_missing`"));
    assert!(witness_plan.contains("- Location: src/lib.rs:8"));
    assert!(witness_plan.contains("- Operation: `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(witness_plan.contains("Missing visible local guard for inferred safety obligations"));
    assert!(witness_plan.contains("No witness receipt imported for this card"));
    assert!(witness_plan.contains("- Witness evidence: No imported witness receipt was found"));
    assert!(witness_plan.contains("Route: `miri`"));
    assert!(
        witness_plan.contains(
            "Miri is the strongest concrete-execution witness when the path is supported"
        )
    );
    assert!(witness_plan.contains("cargo +nightly miri test read_header"));
    assert!(witness_plan.contains("Route: `cargo-careful`"));
    assert!(
        witness_plan.contains("cargo-careful is a cheaper compatibility-oriented runtime check")
    );
    assert!(witness_plan.contains("cargo +nightly careful test read_header"));
    assert!(witness_plan.contains("## Trust boundary"));
    assert!(witness_plan.contains("does not run Miri"));
    assert!(
        witness_plan
            .contains("does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux")
    );
    assert!(witness_plan.contains("not a proof of memory safety"));
    assert!(witness_plan.contains("not UB-free status"));
    assert!(witness_plan.contains("not a Miri result unless a witness receipt is attached"));
    assert!(!witness_plan.contains("Miri passed"));
    assert!(!witness_plan.contains("site reached"));

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
    assert_eq!(packet["card"]["class"], value["cards"][0]["class"]);
    assert_eq!(packet["card"]["priority"], value["cards"][0]["priority"]);
    assert_eq!(
        packet["card"]["confidence"],
        value["cards"][0]["confidence"]
    );
    assert_eq!(
        packet["task"],
        "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
    );
    assert_eq!(packet["context"]["file"], value["cards"][0]["site"]["file"]);
    assert_eq!(packet["context"]["line"], value["cards"][0]["site"]["line"]);
    assert_eq!(
        packet["context"]["owner"],
        value["cards"][0]["site"]["owner"]
    );
    assert_eq!(
        packet["context"]["site_kind"],
        value["cards"][0]["site"]["kind"]
    );
    assert_eq!(packet["context"]["operation_family"], "raw_pointer_read");
    assert_eq!(
        packet["context"]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(
        packet["context"]["snippet"],
        value["cards"][0]["site"]["snippet"]
    );
    assert_eq!(packet["context"]["hazards"], value["cards"][0]["hazards"]);
    assert_eq!(
        packet["required_safety_conditions"],
        value["cards"][0]["obligations"]
    );
    assert_eq!(packet["missing"], value["cards"][0]["missing"]);
    assert_eq!(
        packet["allowed_repairs"][0],
        "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
    );
    assert_eq!(packet["repair_scope"], "this card only");
    assert_eq!(
        packet["verify_commands"],
        value["cards"][0]["verify_commands"]
    );
    assert!(
        packet["safety_contract"]["reach_limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not proof that the unsafe site executed")
    );
    assert!(
        packet["obligation_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence.iter().any(|item| {
                item["key"] == "alignment"
                    && item["discharge"]["state"] == "missing"
                    && item["contract"]["state"] == "present"
            }))
    );
    assert!(
        packet["missing_evidence"]
            .as_array()
            .is_some_and(|missing| missing.iter().any(|item| {
                item["message"] == "Missing visible local guard for inferred safety obligations"
            }))
    );
    assert!(packet["witness_routes"].as_array().is_some_and(|routes| {
        routes.iter().any(|route| {
            route["kind"] == "miri"
                && route["required"] == false
                && route["command"]
                    .as_str()
                    .unwrap_or("")
                    .contains("cargo +nightly miri test read_header")
        })
    }));
    assert!(packet["do_not_do"].as_array().is_some_and(|rules| {
        rules.iter().any(|rule| {
            rule.as_str()
                .unwrap_or("")
                .contains("do not change unrelated unsafe code")
        })
    }));
    assert!(packet["do_not_do"].as_array().is_some_and(|rules| {
        rules.iter().any(|rule| {
            rule.as_str()
                .unwrap_or("")
                .contains("do not claim Miri proof unless the witness command is run")
        })
    }));
    assert!(
        packet["stop_conditions"]
            .as_array()
            .is_some_and(|conditions| {
                conditions.iter().any(|condition| {
                    condition
                        .as_str()
                        .unwrap_or("")
                        .contains("ReviewCard identity still maps to the same unsafe seam")
                })
            })
    );
    assert!(
        packet["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a Miri result")
    );
    let packet_text = serde_json::to_string(&packet)?;
    assert!(!packet_text.contains("fix this unsafe code"));
    assert!(!packet_text.contains("automatic"));
    assert!(!packet_text.contains("\"edit\""));

    let explain = run_success([
        os("explain"),
        os("--root"),
        fixture.as_os_str().to_os_string(),
        OsString::from(card_id),
    ])?;
    let explain = stdout_text(&explain)?;
    assert!(explain.contains(&format!("# unsafe-review card `{card_id}`")));
    assert!(explain.contains("**Class:** `guard_missing`"));
    assert!(explain.contains("**Location:** src/lib.rs:8"));
    assert!(explain.contains("**Operation:** `unsafe { ptr.cast::<Header>().read() }`"));
    assert!(explain.contains("## Required safety conditions"));
    assert!(explain.contains("- pointer is live and dereferenceable for the accessed type"));
    assert!(explain.contains("- pointer is aligned for the accessed type"));
    assert!(explain.contains("## Hazards"));
    assert!(explain.contains("- `pointer_validity`"));
    assert!(explain.contains("- `alignment`"));
    assert!(explain.contains("## Evidence"));
    assert!(explain.contains("- Contract: Nearby `SAFETY:` comment was detected"));
    assert!(explain.contains(
        "- Discharge: Some inferred safety obligations are missing local guard evidence"
    ));
    assert!(explain.contains("- Reach: 1 related test file(s) mention owner `read_header`"));
    assert!(
        explain.contains(
            "- Reach note: static reach evidence only; it does not prove site execution."
        )
    );
    assert!(explain.contains("- Witness: No imported witness receipt was found"));
    assert!(explain.contains("## Obligation evidence"));
    assert!(explain.contains(
        "- `bounds`: contract `present`, guard `present`, reach `present`, witness `missing`"
    ));
    assert!(explain.contains(
        "- `alignment`: contract `present`, guard `missing`, reach `present`, witness `missing`"
    ));
    assert!(explain.contains("## Missing evidence"));
    assert!(explain.contains("- Missing visible local guard for inferred safety obligations"));
    assert!(explain.contains("- No witness receipt imported for this card"));
    assert!(explain.contains("## Recommended witness routes"));
    assert!(explain.contains("- `miri`: Pure-Rust UB-adjacent hazard"));
    assert!(explain.contains("cargo +nightly miri test read_header"));
    assert!(explain.contains(
        "- `cargo-careful`: cargo-careful is a cheaper compatibility-oriented runtime check"
    ));
    assert!(explain.contains("cargo +nightly careful test read_header"));
    assert!(explain.contains("## Next action"));
    assert!(explain.contains(
        "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
    ));
    assert!(explain.contains("## Trust boundary"));
    assert!(explain.contains("not a proof of memory safety"));
    assert!(explain.contains("not a Miri result unless a witness receipt is attached"));
    assert!(!explain.contains("Miri passed"));
    assert!(!explain.contains("site reached"));

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
    assert!(card["id"].as_str().unwrap_or("").starts_with("UR-"));
    assert_eq!(card["class"], "guard_missing");
    assert_eq!(card["priority"], "high");
    assert_eq!(card["confidence"], "medium");
    assert_eq!(card["site"]["file"], "src/lib.rs");
    assert_eq!(card["site"]["line"], 8);
    assert_eq!(card["site"]["owner"], "read_header");
    assert_eq!(card["site"]["kind"], "operation");
    assert_eq!(card["operation_family"], "raw_pointer_read");
    assert_eq!(card["hazards"][0], "pointer_validity");
    assert!(
        card["hazards"]
            .as_array()
            .is_some_and(|hazards| hazards.iter().any(|hazard| hazard == "alignment"))
    );
    assert!(card["obligations"].as_array().is_some_and(|obligations| {
        obligations.iter().any(|obligation| {
            obligation
                .as_str()
                .unwrap_or("")
                .contains("pointer is aligned for the accessed type")
        })
    }));
    assert!(
        card["obligation_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence.iter().any(|item| {
                item["key"] == "bounds" && item["discharge"]["state"] == "present"
            }))
    );
    assert!(
        card["obligation_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence.iter().any(|item| {
                item["key"] == "alignment" && item["discharge"]["state"] == "missing"
            }))
    );
    assert!(card["contract"].as_str().unwrap_or("").contains("SAFETY"));
    assert!(
        card["discharge"]
            .as_str()
            .unwrap_or("")
            .contains("missing local guard evidence")
    );
    assert!(
        card["reach"]
            .as_str()
            .unwrap_or("")
            .contains("related test file")
    );
    assert!(
        card["witness"]
            .as_str()
            .unwrap_or("")
            .contains("No imported witness receipt")
    );
    assert!(card["missing"].as_array().is_some_and(|missing| {
        missing.iter().any(|item| {
            item.as_str()
                .unwrap_or("")
                .contains("Missing visible local guard for inferred safety obligations")
        })
    }));
    assert!(card["verify_commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command
                .as_str()
                .unwrap_or("")
                .contains("cargo +nightly miri test read_header")
        })
    }));
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
    assert!(repo_markdown.contains("| Cards | Open gaps | Contract missing | Guard missing | Guarded unwitnessed | Requires Loom | Miri unsupported | Static unknown |"));
    assert!(repo_markdown.contains("| 1 | 1 | 0 | 1 | 0 | 0 | 0 | 0 |"));
    assert!(repo_markdown.contains("## Top classes"));
    assert!(repo_markdown.contains("| `guard_missing` | 1 |"));
    assert!(repo_markdown.contains("## Top operation families"));
    assert!(repo_markdown.contains("| `raw_pointer_read` | 1 |"));
    assert!(repo_markdown.contains("## Witness routes"));
    assert!(repo_markdown.contains("| `miri` | 1 |"));
    assert!(repo_markdown.contains("## Cards"));
    assert!(repo_markdown.contains("| ID | Class | Operation | Missing evidence | Route |"));
    assert!(repo_markdown.contains("| `UR-"));
    assert!(repo_markdown.contains("| `guard_missing` | `raw_pointer_read` |"));
    assert!(repo_markdown.contains("Missing visible local guard for inferred safety obligations"));
    assert!(repo_markdown.contains("No witness receipt imported for this card"));
    assert!(repo_markdown.contains("| `miri` |"));
    assert!(repo_markdown.contains("## Trust boundary"));
    assert!(repo_markdown.contains("not raw unsafe usage"));
    assert!(repo_markdown.contains("not UB-free status"));
    assert!(repo_markdown.contains("not a Miri result unless a witness receipt is attached"));

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
fn outcome_reports_receipt_movement_without_witness_execution_claim() -> Result<(), Box<dyn Error>>
{
    let temp = TempDir::new("unsafe-review-outcome-receipt-e2e")?;
    let before_path = temp.path().join("before.json");
    let after_path = temp.path().join("after.json");
    let card_id = "UR-receipt-movement-src-lib-rs-read-header-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    fs::write(
        &before_path,
        format!(
            r#"{{
  "schema_version": "0.1",
  "summary": {{
    "cards": 1,
    "open_actionable_gaps": 1
  }},
  "cards": [
    {{
      "id": "{card_id}",
      "class": "guard_missing",
      "priority": "high",
      "witness": "No imported witness receipt was found",
      "missing": [
        "Missing visible local guard for alignment",
        "No witness receipt imported for route `miri`"
      ]
    }}
  ]
}}
"#
        ),
    )?;
    fs::write(
        &after_path,
        format!(
            r#"{{
  "schema_version": "0.1",
  "summary": {{
    "cards": 1,
    "open_actionable_gaps": 1
  }},
  "cards": [
    {{
      "id": "{card_id}",
      "class": "guard_missing",
      "priority": "high",
      "witness": "Imported miri receipt with `ran` strength: saved fixture witness passed",
      "missing": [
        "Missing visible local guard for alignment"
      ]
    }}
  ]
}}
"#
        ),
    )?;

    let output = run_success([
        os("outcome"),
        os("--before"),
        before_path.as_os_str().to_os_string(),
        os("--after"),
        after_path.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let outcome = parse_json(&stdout_text(&output)?)?;
    assert_eq!(outcome["summary"]["improved"], 1);
    assert_eq!(outcome["summary"]["regressed"], 0);
    assert_eq!(outcome["cards"]["improved"][0]["card_id"], card_id);
    assert!(
        json_str(
            &outcome["cards"]["improved"][0]["reason"],
            "improved reason"
        )?
        .contains("witness receipt strength changed from `missing` to `ran`")
    );
    assert_eq!(outcome["cards"]["improved"][0]["after"]["witness"], "ran");
    assert!(
        outcome["cards"]["improved"][0]["after"]["missing"][0]
            .as_str()
            .unwrap_or("")
            .contains("alignment")
    );
    assert!(
        json_str(&outcome["trust_boundary"], "trust_boundary")?.contains("not witness execution")
    );
    assert!(outcome["limitations"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .unwrap_or("")
                .contains("does not rerun analysis or execute witness tools")
        })
    }));

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
    assert!(markdown.contains("witness receipt strength changed from `missing` to `ran`"));
    assert!(markdown.contains("1 missing / witness `ran`"));
    assert!(markdown.contains("does not rerun analysis or execute witness tools"));
    assert!(markdown.contains("not witness execution"));

    Ok(())
}

#[test]
fn outcome_reports_receipt_regression_without_policy_claim() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-outcome-receipt-regression-e2e")?;
    let before_path = temp.path().join("before.json");
    let after_path = temp.path().join("after.json");
    let card_id = "UR-receipt-regression-src-lib-rs-read-header-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    fs::write(
        &before_path,
        format!(
            r#"{{
  "schema_version": "0.1",
  "summary": {{
    "cards": 1,
    "open_actionable_gaps": 0
  }},
  "cards": [
    {{
      "id": "{card_id}",
      "class": "guarded_and_witnessed",
      "priority": "low",
      "witness": "Imported miri receipt with `site_reached` strength: saved fixture witness reached the targeted seam",
      "missing": []
    }}
  ]
}}
"#
        ),
    )?;
    fs::write(
        &after_path,
        format!(
            r#"{{
  "schema_version": "0.1",
  "summary": {{
    "cards": 1,
    "open_actionable_gaps": 0
  }},
  "cards": [
    {{
      "id": "{card_id}",
      "class": "guarded_and_witnessed",
      "priority": "low",
      "witness": "Imported miri receipt with `configured` strength: receipt metadata was configured but no saved run is attached",
      "missing": []
    }}
  ]
}}
"#
        ),
    )?;

    let output = run_success([
        os("outcome"),
        os("--before"),
        before_path.as_os_str().to_os_string(),
        os("--after"),
        after_path.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let outcome = parse_json(&stdout_text(&output)?)?;
    assert_eq!(outcome["summary"]["improved"], 0);
    assert_eq!(outcome["summary"]["regressed"], 1);
    assert_eq!(outcome["cards"]["regressed"][0]["card_id"], card_id);
    assert!(
        json_str(
            &outcome["cards"]["regressed"][0]["reason"],
            "regressed reason"
        )?
        .contains("witness receipt strength changed from `site_reached` to `configured`")
    );
    assert_eq!(
        outcome["cards"]["regressed"][0]["before"]["witness"],
        "site_reached"
    );
    assert_eq!(
        outcome["cards"]["regressed"][0]["after"]["witness"],
        "configured"
    );
    assert!(
        json_str(&outcome["trust_boundary"], "trust_boundary")?.contains("not witness execution")
    );
    assert!(outcome["limitations"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .unwrap_or("")
                .contains("does not make policy or blocking decisions")
        })
    }));

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
    assert!(
        markdown.contains("witness receipt strength changed from `site_reached` to `configured`")
    );
    assert!(markdown.contains("0 missing / witness `configured`"));
    assert!(markdown.contains("does not make policy or blocking decisions"));
    assert!(markdown.contains("not witness execution"));

    Ok(())
}

#[test]
fn outcome_rejects_invalid_saved_snapshots_at_cli_boundary() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-outcome-invalid-snapshot-e2e")?;
    let before_path = temp.path().join("before.json");
    let after_path = temp.path().join("after.json");
    let duplicate_path = temp.path().join("duplicate.json");
    let count_mismatch_path = temp.path().join("count-mismatch.json");

    fs::write(
        &before_path,
        r#"{
  "schema_version": "0.1",
  "summary": {
    "cards": 0,
    "open_actionable_gaps": 0
  },
  "cards": []
}
"#,
    )?;
    fs::write(
        &after_path,
        r#"{
  "schema_version": "0.1",
  "summary": {
    "cards": 0,
    "open_actionable_gaps": 0
  },
  "cards": []
}
"#,
    )?;
    fs::write(
        &duplicate_path,
        r#"{
  "schema_version": "0.1",
  "summary": {
    "cards": 2,
    "open_actionable_gaps": 2
  },
  "cards": [
    {
      "id": "UR-duplicate-card-c1",
      "class": "guard_missing",
      "priority": "high",
      "witness": "No imported witness receipt was found",
      "missing": ["Missing visible local guard for alignment"]
    },
    {
      "id": "UR-duplicate-card-c1",
      "class": "contract_missing",
      "priority": "high",
      "witness": "No imported witness receipt was found",
      "missing": ["Missing # Safety contract"]
    }
  ]
}
"#,
    )?;
    fs::write(
        &count_mismatch_path,
        r#"{
  "schema_version": "0.1",
  "summary": {
    "cards": 2,
    "open_actionable_gaps": 1
  },
  "cards": [
    {
      "id": "UR-count-mismatch-card-c1",
      "class": "guard_missing",
      "priority": "high",
      "witness": "No imported witness receipt was found",
      "missing": ["Missing visible local guard for alignment"]
    }
  ]
}
"#,
    )?;

    let duplicate = run_failure([
        os("outcome"),
        os("--before"),
        before_path.as_os_str().to_os_string(),
        os("--after"),
        duplicate_path.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    assert_eq!(stdout_text(&duplicate)?.trim(), "");
    let stderr = String::from_utf8_lossy(&duplicate.stderr);
    assert!(stderr.contains("unsafe-review:"), "stderr: {stderr}");
    assert!(
        stderr.contains("after snapshot contains duplicate card id"),
        "stderr: {stderr}"
    );

    let mismatch = run_failure([
        os("outcome"),
        os("--before"),
        count_mismatch_path.as_os_str().to_os_string(),
        os("--after"),
        after_path.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])?;
    assert_eq!(stdout_text(&mismatch)?.trim(), "");
    let stderr = String::from_utf8_lossy(&mismatch.stderr);
    assert!(stderr.contains("unsafe-review:"), "stderr: {stderr}");
    assert!(
        stderr.contains("before snapshot summary card count 2 does not match 1 card object(s)"),
        "stderr: {stderr}"
    );

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
fn receipt_audit_reports_problem_statuses_without_running_witnesses() -> Result<(), Box<dyn Error>>
{
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-receipt-audit-problems-e2e")?;
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
    let stale_id =
        "UR-stale-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";

    write_receipt_file(
        &copied,
        "matched.json",
        card_id,
        "miri",
        "ran",
        "2026-05-18T00:00:00Z",
        "2026-08-18",
    )?;
    write_receipt_file(
        &copied,
        "wrong-tool.json",
        card_id,
        "loom",
        "ran",
        "2026-05-18T00:00:00Z",
        "2026-08-18",
    )?;
    write_receipt_file(
        &copied,
        "weak.json",
        card_id,
        "miri",
        "configured",
        "2026-05-18T00:00:00Z",
        "2026-08-18",
    )?;
    write_receipt_file(
        &copied,
        "expired.json",
        card_id,
        "miri",
        "ran",
        "2026-05-01T00:00:00Z",
        "2026-05-19",
    )?;
    write_receipt_file(
        &copied,
        "stale.json",
        stale_id,
        "miri",
        "ran",
        "2026-05-18T00:00:00Z",
        "2026-08-18",
    )?;
    write_receipt_file(
        &copied,
        "wrong-identity.json",
        "not-counted",
        "miri",
        "ran",
        "2026-05-18T00:00:00Z",
        "2026-08-18",
    )?;
    write_receipt_file(
        &copied,
        "invalid-strength.json",
        card_id,
        "miri",
        "almost",
        "2026-05-18T00:00:00Z",
        "2026-08-18",
    )?;

    let audit = run_success([
        os("receipt"),
        os("audit"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("json"),
    ])?;
    let audit = parse_json(&stdout_text(&audit)?)?;

    assert_eq!(audit["summary"]["receipts"], 7);
    assert_eq!(audit["summary"]["matched"], 5);
    assert_eq!(audit["summary"]["unmatched"], 1);
    assert_eq!(audit["summary"]["expired"], 1);
    assert_eq!(audit["summary"]["stale"], 1);
    assert_eq!(audit["summary"]["wrong_identity"], 1);
    assert_eq!(audit["summary"]["wrong_tool"], 1);
    assert_eq!(audit["summary"]["weaker_than_required"], 1);
    assert_eq!(audit["summary"]["duplicate"], 5);
    assert_eq!(audit["summary"]["invalid"], 2);
    assert!(
        json_str(&audit["trust_boundary"], "trust_boundary")?
            .contains("does not execute witnesses")
    );
    for status in [
        "matched",
        "unmatched",
        "expired",
        "stale",
        "wrong_identity",
        "wrong_tool",
        "weaker_than_required",
        "duplicate",
        "invalid",
    ] {
        assert!(
            audit["receipts"]
                .as_array()
                .is_some_and(|receipts| receipts.iter().any(|receipt| receipt["statuses"]
                    .as_array()
                    .is_some_and(|statuses| statuses.iter().any(|item| item == status)))),
            "missing receipt status {status}"
        );
    }

    let markdown = run_success([
        os("receipt"),
        os("audit"),
        os("--root"),
        copied.as_os_str().to_os_string(),
        os("--diff"),
        copied.join("change.diff").into_os_string(),
        os("--format"),
        os("markdown"),
    ])?;
    let markdown = stdout_text(&markdown)?;
    assert!(markdown.contains("| 7 | 5 | 1 | 1 | 1 | 1 | 1 | 1 | 5 | 2 |"));
    assert!(markdown.contains("wrong_tool"));
    assert!(markdown.contains("weaker_than_required"));
    assert!(markdown.contains("does not execute witnesses"));

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

#[test]
fn policy_report_reports_resolved_baseline_and_expired_suppression() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-policy-report-ledgers-e2e")?;
    let copied = temp.path().join("fixture");
    copy_dir_all(&fixture, &copied)?;
    let policy = copied.join("policy");
    fs::create_dir_all(&policy)?;
    let resolved_id =
        "UR-resolved-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
    let expired_id =
        "UR-expired-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
    fs::write(
        policy.join("unsafe-review-baseline.toml"),
        format!(
            r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{resolved_id}"
owner = "core/policy"
reason = "resolved fixture debt"
evidence = "e2e policy report"
review_after = "2026-08-01"
"#
        ),
    )?;
    fs::write(
        policy.join("unsafe-review-suppressions.toml"),
        format!(
            r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{expired_id}"
owner = "core/policy"
reason = "expired false-positive review"
evidence = "e2e policy report"
expires = "2026-01-01"
"#
        ),
    )?;

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
    assert_eq!(report["summary"]["new_gaps"], 1);
    assert_eq!(report["summary"]["resolved_baseline"], 1);
    assert_eq!(report["summary"]["expired_suppressions"], 1);
    assert_eq!(report["resolved_baseline"][0]["card_id"], resolved_id);
    assert_eq!(report["expired_suppressions"][0]["card_id"], expired_id);
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
    assert!(markdown.contains("## Resolved baseline entries"));
    assert!(markdown.contains("## Expired suppression entries"));
    assert!(markdown.contains(resolved_id));
    assert!(markdown.contains(expired_id));
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

fn write_receipt_file(
    root: &Path,
    name: &str,
    card_id: &str,
    tool: &str,
    strength: &str,
    recorded_at: &str,
    expires_at: &str,
) -> Result<(), Box<dyn Error>> {
    let receipts = root.join(".unsafe-review").join("receipts");
    fs::create_dir_all(&receipts)?;
    fs::write(
        receipts.join(name),
        format!(
            r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "{tool}",
  "strength": "{strength}",
  "author": "core/e2e",
  "recorded_at": "{recorded_at}",
  "expires_at": "{expires_at}",
  "summary": "e2e receipt audit status fixture",
  "command": "saved witness command",
  "limitations": [
    "fixture only"
  ]
}}
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
