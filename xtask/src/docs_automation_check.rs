//! Docs-automation ledger validation (`check-docs-automation`).
//!
//! This module owns the checked-surface inventory and the boundary that keeps
//! external agent/tool state awareness-only. Path glob expansion remains in
//! `docs_automation_paths` so matching and ledger validation stay separate.

use crate::{
    docs_automation_paths, parse_toml_file, read_to_string, require_file, require_known,
    require_toml_string, required_table_string, spec_status, toml_array, toml_str_array,
    toml_table,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DOCS_AUTOMATION_LEDGER: &str = "policy/docs-automation.toml";
const DOCS_AUTOMATION_KINDS: &[&str] = &[
    "spec_status_dashboard",
    "operator_front_door",
    "agent_operating_contract",
    "lane_plan",
    "docs_map",
    "published_surface",
    "handoff_receipt",
];
const DOCS_AUTOMATION_MODES: &[&str] = &["check", "generate"];

pub(crate) fn check_docs_automation() -> Result<(), String> {
    let surfaces = check_docs_automation_impl()?;
    println!("check-docs-automation: ok ({surfaces} surfaces)");
    Ok(())
}

fn check_docs_automation_impl() -> Result<usize, String> {
    let value = parse_toml_file(Path::new(DOCS_AUTOMATION_LEDGER))?;
    require_toml_string(&value, "schema_version", DOCS_AUTOMATION_LEDGER)?;

    let scope = value
        .get("scope")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{DOCS_AUTOMATION_LEDGER} is missing table `scope`"))?;
    let owned_roots = require_scope_paths(scope, "owned_roots", true)?;
    let external_awareness_roots = require_scope_paths(scope, "external_awareness_only", false)?;
    check_docs_automation_scope_boundaries(&owned_roots, &external_awareness_roots)?;

    let surfaces = toml_array(&value, "generated_or_checked", DOCS_AUTOMATION_LEDGER)?;
    if surfaces.is_empty() {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} must list at least one generated_or_checked entry"
        ));
    }

    let mut ids = BTreeSet::new();
    for (idx, surface) in surfaces.iter().enumerate() {
        let table = toml_table(surface, DOCS_AUTOMATION_LEDGER, "generated_or_checked", idx)?;
        let id = required_table_string(
            table,
            "id",
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked",
            idx,
        )?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{DOCS_AUTOMATION_LEDGER} contains duplicate generated_or_checked id `{id}`"
            ));
        }

        let kind = required_table_string(
            table,
            "kind",
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked",
            idx,
        )?;
        let mode = required_table_string(
            table,
            "mode",
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked",
            idx,
        )?;
        require_known(
            kind,
            DOCS_AUTOMATION_KINDS,
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked.kind",
        )?;
        require_known(
            mode,
            DOCS_AUTOMATION_MODES,
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked.mode",
        )?;

        if let Some(sources) = table.get("sources") {
            for source in toml_str_array(sources, DOCS_AUTOMATION_LEDGER, "sources")? {
                require_existing_repo_path(source, DOCS_AUTOMATION_LEDGER, "sources")?;
                reject_docs_automation_external_path(
                    id,
                    "sources",
                    source,
                    &external_awareness_roots,
                )?;
            }
        }

        if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
            reject_docs_automation_external_path(id, "path", path, &external_awareness_roots)?;
        }
        if let Some(path_glob) = table.get("path_glob").and_then(toml::Value::as_str) {
            reject_docs_automation_external_path(
                id,
                "path_glob",
                path_glob,
                &external_awareness_roots,
            )?;
        }
        let paths = docs_automation_paths(table, idx)?;
        for path in &paths {
            let path = path.display().to_string();
            reject_docs_automation_external_path(id, "path", &path, &external_awareness_roots)?;
        }
        if kind == "spec_status_dashboard" {
            if !paths
                .iter()
                .any(|path| path == Path::new(spec_status::DASHBOARD))
            {
                return Err(format!(
                    "{DOCS_AUTOMATION_LEDGER} generated_or_checked `{id}` must point at {}",
                    spec_status::DASHBOARD
                ));
            }
            spec_status::check_dashboard_impl()?;
        }
        if let Some(required_text) = table.get("must_include") {
            let required_text =
                toml_str_array(required_text, DOCS_AUTOMATION_LEDGER, "must_include")?;
            require_docs_automation_text(id, &paths, &required_text)?;
        }
    }

    Ok(ids.len())
}

fn require_scope_paths(
    scope: &toml::map::Map<String, toml::Value>,
    key: &str,
    must_exist: bool,
) -> Result<Vec<String>, String> {
    let Some(values) = scope.get(key) else {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} scope is missing array `{key}`"
        ));
    };
    let values = toml_str_array(values, DOCS_AUTOMATION_LEDGER, key)?;
    if values.is_empty() {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} scope `{key}` must not be empty"
        ));
    }
    if must_exist {
        for value in &values {
            require_existing_repo_path(value, DOCS_AUTOMATION_LEDGER, key)?;
        }
    }
    Ok(values.into_iter().map(str::to_string).collect())
}

fn check_docs_automation_scope_boundaries(
    owned_roots: &[String],
    external_awareness_roots: &[String],
) -> Result<(), String> {
    for owned_root in owned_roots {
        if let Some(external_root) = external_awareness_roots
            .iter()
            .find(|root| repo_path_is_under_scope_root(owned_root, root))
        {
            return Err(format!(
                "{DOCS_AUTOMATION_LEDGER} scope owned_roots entry `{owned_root}` must not be under external_awareness_only root `{external_root}`"
            ));
        }
    }
    Ok(())
}

fn reject_docs_automation_external_path(
    id: &str,
    field: &str,
    path: &str,
    external_awareness_roots: &[String],
) -> Result<(), String> {
    if let Some(external_root) = external_awareness_roots
        .iter()
        .find(|root| repo_path_is_under_scope_root(path, root))
    {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} generated_or_checked `{id}` {field} `{path}` must not be under external_awareness_only root `{external_root}`"
        ));
    }
    Ok(())
}

fn repo_path_is_under_scope_root(path: &str, root: &str) -> bool {
    let path = normalize_repo_scope_path(path);
    let root = normalize_repo_scope_path(root);
    path == root || path.starts_with(&format!("{root}/"))
}

fn normalize_repo_scope_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn docs_automation_paths(
    table: &toml::map::Map<String, toml::Value>,
    idx: usize,
) -> Result<Vec<PathBuf>, String> {
    let path = table.get("path").and_then(toml::Value::as_str);
    let path_glob = table.get("path_glob").and_then(toml::Value::as_str);
    match (path, path_glob) {
        (Some(path), None) => {
            require_file(path)?;
            Ok(vec![PathBuf::from(path)])
        }
        (None, Some(path_glob)) => docs_automation_glob_paths(path_glob),
        (Some(_), Some(_)) => Err(format!(
            "{DOCS_AUTOMATION_LEDGER} generated_or_checked[{idx}] must not set both path and path_glob"
        )),
        (None, None) => Err(format!(
            "{DOCS_AUTOMATION_LEDGER} generated_or_checked[{idx}] must set path or path_glob"
        )),
    }
}

fn docs_automation_glob_paths(path_glob: &str) -> Result<Vec<PathBuf>, String> {
    let pattern_path = Path::new(path_glob);
    let file_pattern = pattern_path.file_name().and_then(|value| value.to_str());
    if file_pattern.is_some_and(|pattern| !pattern.contains('*')) {
        require_file(path_glob)?;
        return Ok(vec![PathBuf::from(path_glob)]);
    }

    let paths = docs_automation_paths::collect_paths(path_glob, DOCS_AUTOMATION_LEDGER)?;
    if paths.is_empty() {
        Err(format!(
            "{DOCS_AUTOMATION_LEDGER} path_glob `{path_glob}` did not match any files"
        ))
    } else {
        Ok(paths)
    }
}

fn require_docs_automation_text(
    id: &str,
    paths: &[PathBuf],
    required_text: &[&str],
) -> Result<(), String> {
    let mut documents = Vec::new();
    for path in paths {
        documents.push((path, read_to_string(path)?));
    }
    for needle in required_text {
        if !documents.iter().any(|(_, text)| text.contains(needle)) {
            let paths = paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "{DOCS_AUTOMATION_LEDGER} generated_or_checked `{id}` requires text `{needle}` in one of: {paths}"
            ));
        }
    }
    Ok(())
}

fn require_existing_repo_path(path: &str, ledger: &str, field: &str) -> Result<(), String> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(format!("{ledger} {field} path does not exist: {path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        check_docs_automation_scope_boundaries, reject_docs_automation_external_path,
        repo_path_is_under_scope_root,
    };
    use crate::docs_automation_paths;

    #[test]
    fn docs_automation_glob_matches_publication_receipts() {
        assert!(docs_automation_paths::wildcard_match(
            "*publication*.md",
            "2026-05-21-release-0.2.0-publication.md",
        ));
        assert!(!docs_automation_paths::wildcard_match(
            "*publication*.md",
            "2026-05-21-source-promotion-0.2-sync.md",
        ));
    }

    #[test]
    fn docs_automation_scope_detects_external_agent_state_roots() {
        assert!(repo_path_is_under_scope_root(
            ".codex/agent-state.md",
            ".codex"
        ));
        assert!(repo_path_is_under_scope_root(
            ".jules\\goals\\README.md",
            ".jules"
        ));
        assert!(repo_path_is_under_scope_root(
            ".\\.codex\\AGENTS.md",
            ".codex"
        ));
        assert!(repo_path_is_under_scope_root(
            ".codex/agent-state.md",
            ".codex\\"
        ));
        assert!(!repo_path_is_under_scope_root(
            "docs/contributing/spec-rails.md",
            ".codex"
        ));
    }

    #[test]
    fn docs_automation_rejects_owned_external_state_root() -> Result<(), String> {
        let owned_roots = vec!["docs".to_string(), ".codex".to_string()];
        let external_roots = vec![".codex".to_string()];

        let Err(err) = check_docs_automation_scope_boundaries(&owned_roots, &external_roots) else {
            return Err("external state root in owned_roots should fail".to_string());
        };

        assert!(err.contains("owned_roots"));
        assert!(err.contains("external_awareness_only"));
        assert!(err.contains(".codex"));
        Ok(())
    }

    #[test]
    fn docs_automation_rejects_checked_external_state_path() -> Result<(), String> {
        let external_roots = vec![".codex".to_string()];

        let Err(err) = reject_docs_automation_external_path(
            "agent-operating-contract",
            "path",
            ".codex/AGENTS.md",
            &external_roots,
        ) else {
            return Err("external state path should fail".to_string());
        };

        assert!(err.contains("agent-operating-contract"));
        assert!(err.contains("external_awareness_only"));
        assert!(err.contains(".codex/AGENTS.md"));
        Ok(())
    }
}
