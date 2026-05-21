use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn discover_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort_by(|left, right| {
        rust_file_priority(left)
            .cmp(&rust_file_priority(right))
            .then(left.cmp(right))
    });
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if path.is_dir() {
            if matches!(
                name.as_ref(),
                ".git"
                    | "target"
                    | ".unsafe-review"
                    | ".unsafe-review-spec"
                    | ".github"
                    | "node_modules"
            ) {
                continue;
            }
            visit(root, &path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

fn rust_file_priority(path: &Path) -> u8 {
    let mut components = path.components();
    let first = components
        .next()
        .and_then(|component| component.as_os_str().to_str())
        .unwrap_or_default();
    match first {
        "src" => 0,
        "tests" => 1,
        "benches" => 2,
        "examples" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn discovery_prioritizes_cargo_source_roots_before_miscellaneous_rust_files()
    -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-workspace-order")?;
        fs::create_dir_all(root.join("benchmarks/haystacks/code"))
            .map_err(|err| format!("create benchmark dirs failed: {err}"))?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("benchmarks/haystacks/code/rust-library.rs"),
            "unsafe fn fixture_data() {}\n",
        )
        .map_err(|err| format!("write benchmark file failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "unsafe fn source_root() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;

        let files = discover_rust_files(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files.first(), Some(&PathBuf::from("src/lib.rs")));
        Ok(())
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
