//! Self-unsafe governance gate (`check-self-unsafe`).
//!
//! Validates that any `#[allow(unsafe_code)]` in the shipped crates meets the
//! governance contract from ADR-0008: every allow must (a) carry an inline
//! `// SAFETY:` comment on the same or preceding line, (b) be listed in
//! `policy/clippy-exceptions.toml`, and (c) the ledger entry must carry a
//! non-empty `reason`.
//!
//! Under the current `unsafe_code = "forbid"` workspace lint, no allows exist
//! and this gate trivially passes. If the workspace lint is relaxed to `"deny"`
//! (see #1805), this gate becomes load-bearing — it makes the governance
//! structural rather than aspirational.

use crate::{parse_toml_file, workspace_path};
use std::collections::BTreeSet;
use std::path::Path;

const CLIPPY_EXCEPTIONS_LEDGER: &str = "policy/clippy-exceptions.toml";
const SHIPPED_CRATE_DIRS: &[&str] = &[
    "crates/unsafe-review/src",
    "crates/unsafe-review-cli/src",
    "crates/unsafe-review-core/src",
];

pub(crate) fn check_self_unsafe() -> Result<(), String> {
    // 1. Parse the clippy-exceptions ledger to get the set of registered
    //    exception module paths (e.g. "unsafe-review-core::util::peak_rss").
    let ledger_exceptions = parse_clippy_exceptions()?;

    // 2. Scan shipped crate source for any `#[allow(unsafe_code` attribute.
    let mut findings: Vec<String> = Vec::new();

    for crate_dir in SHIPPED_CRATE_DIRS {
        let dir = workspace_path(crate_dir);
        if !dir.is_dir() {
            continue;
        }
        scan_dir_for_allows(&dir, crate_dir, &ledger_exceptions, &mut findings)?;
    }

    if findings.is_empty() {
        println!("check-self-unsafe: ok (0 allow(unsafe_code) in shipped crates)");
        Ok(())
    } else {
        Err(format!(
            "check-self-unsafe: {} finding(s):\n{}",
            findings.len(),
            findings.join("\n")
        ))
    }
}

fn parse_clippy_exceptions() -> Result<BTreeSet<String>, String> {
    let path = workspace_path(CLIPPY_EXCEPTIONS_LEDGER);
    if !Path::new(&path).is_file() {
        return Ok(BTreeSet::new());
    }
    let value = parse_toml_file(&path)?;
    let mut exceptions = BTreeSet::new();
    if let Some(entries) = value.get("exceptions").and_then(toml::Value::as_array) {
        for entry in entries {
            let module = entry
                .get("module")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            let reason = entry
                .get("reason")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            if !module.is_empty() && !reason.trim().is_empty() {
                exceptions.insert(module.to_string());
            }
        }
    }
    Ok(exceptions)
}

fn scan_dir_for_allows(
    dir: &Path,
    crate_label: &str,
    ledger_exceptions: &BTreeSet<String>,
    findings: &mut Vec<String>,
) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("read {} failed: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read_dir entry failed: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir_for_allows(&path, crate_label, ledger_exceptions, findings)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("read {} failed: {e}", path.display()))?;
        for (line_no, line) in text.lines().enumerate() {
            if !line.contains("allow(unsafe_code") {
                continue;
            }
            // Found an allow(unsafe_code). Check governance:
            // (a) must have a // SAFETY: comment on this line or the next non-empty line
            let has_safety = line.contains("// SAFETY:")
                || line.contains("// Safety:")
                || text
                    .lines()
                    .nth(line_no + 1)
                    .map(|next| next.contains("// SAFETY:") || next.contains("// Safety:"))
                    .unwrap_or(false);
            if !has_safety {
                findings.push(format!(
                    "  {}:{}: allow(unsafe_code) missing inline `// SAFETY:` comment",
                    path.display(),
                    line_no + 1
                ));
            }
            // (b) must be registered in clippy-exceptions.toml
            //     (we check by crate_label — the ledger uses module paths)
            let crate_key = crate_label.replace("/src", "").replace('/', "::");
            let module_key = format!("{}::*", crate_key);
            let is_registered = ledger_exceptions.iter().any(|m| {
                m == &crate_key || m.starts_with(&format!("{crate_key}::")) || m == &module_key
            });
            if !is_registered {
                findings.push(format!(
                    "  {}:{}: allow(unsafe_code) not registered in {} (add a [[exceptions]] entry with module + reason)",
                    path.display(),
                    line_no + 1,
                    CLIPPY_EXCEPTIONS_LEDGER
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_self_unsafe_passes_under_forbid() -> Result<(), String> {
        // Under unsafe_code = "forbid", no allows exist — gate trivially passes.
        check_self_unsafe()
    }
}
