use crate::api::DiscoveryOptions;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, WalkBuilder};
use std::path::{Path, PathBuf};

type DiscoveryProgressFn<'a> = &'a mut dyn FnMut(usize, &Path) -> Result<(), String>;

pub(crate) fn discover_rust_files(
    root: &Path,
    options: &DiscoveryOptions,
) -> Result<Vec<PathBuf>, String> {
    discover_rust_files_with_progress(root, options, None)
}

pub(crate) fn discover_rust_files_with_progress(
    root: &Path,
    options: &DiscoveryOptions,
    mut progress: Option<DiscoveryProgressFn<'_>>,
) -> Result<Vec<PathBuf>, String> {
    let matcher = DiscoveryMatcher::new(options)?;
    let mut out = Vec::new();
    let root_for_filter = root.to_path_buf();
    let large_repo_ignores = options.large_repo_ignores;
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(options.respect_gitignore)
        .git_ignore(options.respect_gitignore)
        .git_global(options.respect_gitignore)
        .git_exclude(options.respect_gitignore)
        .require_git(false)
        .parents(options.respect_gitignore)
        .filter_entry(move |entry| should_visit_entry(&root_for_filter, entry, large_repo_ignores));
    for entry in builder.build() {
        let entry = entry.map_err(|err| format!("walk {} failed: {err}", root.display()))?;
        let path = entry.path();
        if path == root {
            continue;
        }
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        if matcher.allows(&rel) {
            if let Some(progress) = progress.as_deref_mut() {
                progress(out.len() + 1, &rel)?;
            }
            out.push(rel);
        }
    }
    out.sort_by(|left, right| {
        rust_file_priority(left)
            .cmp(&rust_file_priority(right))
            .then(left.cmp(right))
    });
    if let Some(max_files) = options.max_files {
        out.truncate(max_files);
    }
    Ok(out)
}

struct DiscoveryMatcher {
    include: Option<GlobSet>,
    exclude: GlobSet,
}

impl DiscoveryMatcher {
    fn new(options: &DiscoveryOptions) -> Result<Self, String> {
        let include = if options.include.is_empty() {
            None
        } else {
            Some(build_glob_set("--include", &options.include)?)
        };
        let exclude = build_glob_set("--exclude", &options.exclude)?;
        Ok(Self { include, exclude })
    }

    fn allows(&self, path: &Path) -> bool {
        if self.exclude.is_match(path) {
            return false;
        }
        self.include
            .as_ref()
            .is_none_or(|include| include.is_match(path))
    }
}

fn build_glob_set(flag: &str, patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).map_err(|err| format!("invalid {flag} glob `{pattern}`: {err}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|err| format!("invalid {flag} glob set: {err}"))
}

fn should_visit_entry(root: &Path, entry: &DirEntry, large_repo_ignores: bool) -> bool {
    let path = entry.path();
    if path == root {
        return true;
    }
    if !entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir())
    {
        return true;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    if is_default_skipped_dir(name) {
        return false;
    }
    if large_repo_ignores && is_large_repo_skipped_dir(name) {
        return false;
    }
    // Skip nested git checkouts (contain a `.git` directory) and gitfile
    // worktrees (contain a `.git` file).  The scan root is excluded above so
    // the root's own `.git` sibling is never checked here.
    let dot_git = path.join(".git");
    if dot_git.is_dir() || dot_git.is_file() {
        return false;
    }
    true
}

fn is_default_skipped_dir(name: &str) -> bool {
    matches!(name, ".git" | ".github" | "target" | "node_modules")
        || name.starts_with(".unsafe-review")
}

fn is_large_repo_skipped_dir(name: &str) -> bool {
    matches!(name, "vendor" | "build" | "dist" | "generated")
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

        let files = discover_rust_files(&root, &DiscoveryOptions::default())?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files.first(), Some(&PathBuf::from("src/lib.rs")));
        Ok(())
    }

    #[test]
    fn discovery_applies_include_exclude_and_max_files() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-workspace-filters")?;
        write_file(&root, "src/a.rs")?;
        write_file(&root, "src/b.rs")?;
        write_file(&root, "packages/pkg/src/lib.rs")?;
        write_file(&root, "crates/other/src/lib.rs")?;

        let files = discover_rust_files(
            &root,
            &DiscoveryOptions {
                include: vec!["src/**/*.rs".to_string(), "packages/**/*.rs".to_string()],
                exclude: vec!["src/b.rs".to_string()],
                max_files: Some(2),
                ..DiscoveryOptions::default()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(
            files,
            vec![
                PathBuf::from("src/a.rs"),
                PathBuf::from("packages/pkg/src/lib.rs")
            ]
        );
        Ok(())
    }

    #[test]
    fn discovery_skips_large_repo_default_directories() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-workspace-skips")?;
        write_file(&root, "src/lib.rs")?;
        for path in [
            "target/debug/build.rs",
            ".git/hooks/hook.rs",
            ".github/workflows/action.rs",
            ".unsafe-review/cache.rs",
            ".unsafe-review-spec/spec.rs",
            "node_modules/pkg/lib.rs",
            "vendor/pkg/lib.rs",
            "build/out/lib.rs",
            "dist/pkg/lib.rs",
            "crates/pkg/generated/lib.rs",
        ] {
            write_file(&root, path)?;
        }

        let files = discover_rust_files(&root, &DiscoveryOptions::repo_defaults())?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files, vec![PathBuf::from("src/lib.rs")]);
        Ok(())
    }

    #[test]
    fn discovery_respects_gitignore_by_default() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-workspace-gitignore")?;
        write_file(&root, "src/lib.rs")?;
        write_file(&root, "ignored/lib.rs")?;
        fs::write(root.join(".gitignore"), "ignored/\n")
            .map_err(|err| format!("write gitignore failed: {err}"))?;

        let files = discover_rust_files(&root, &DiscoveryOptions::repo_defaults())?;
        let unignored = discover_rust_files(
            &root,
            &DiscoveryOptions {
                respect_gitignore: false,
                ..DiscoveryOptions::repo_defaults()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files, vec![PathBuf::from("src/lib.rs")]);
        assert_eq!(
            unignored,
            vec![PathBuf::from("src/lib.rs"), PathBuf::from("ignored/lib.rs")]
        );
        Ok(())
    }

    #[test]
    fn discovery_skips_nested_git_checkout_directory() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-workspace-nested-git-dir")?;
        write_file(&root, "src/lib.rs")?;
        // Nested checkout: a subdirectory that contains a .git directory.
        let nested = root.join("vendor-clone");
        fs::create_dir_all(nested.join(".git"))
            .map_err(|err| format!("create nested .git dir failed: {err}"))?;
        write_file(&root, "vendor-clone/src/lib.rs")?;

        let files = discover_rust_files(
            &root,
            &DiscoveryOptions {
                respect_gitignore: false,
                ..DiscoveryOptions::default()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files, vec![PathBuf::from("src/lib.rs")]);
        Ok(())
    }

    #[test]
    fn discovery_skips_nested_gitfile_worktree() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-workspace-nested-gitfile")?;
        write_file(&root, "src/lib.rs")?;
        // Gitfile worktree: a subdirectory that contains a .git FILE.
        let nested = root.join("linked-worktree");
        fs::create_dir_all(&nested)
            .map_err(|err| format!("create nested worktree dir failed: {err}"))?;
        fs::write(
            nested.join(".git"),
            "gitdir: ../somewhere/.git/worktrees/linked-worktree\n",
        )
        .map_err(|err| format!("write .git file failed: {err}"))?;
        write_file(&root, "linked-worktree/src/lib.rs")?;

        let files = discover_rust_files(
            &root,
            &DiscoveryOptions {
                respect_gitignore: false,
                ..DiscoveryOptions::default()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files, vec![PathBuf::from("src/lib.rs")]);
        Ok(())
    }

    #[test]
    fn discovery_skips_nested_git_even_without_gitignore() -> Result<(), String> {
        // Both nested-checkout and gitfile-worktree skips must be independent of
        // gitignore handling (respect_gitignore: false).
        let root = unique_temp_dir("unsafe-review-workspace-nested-git-no-gitignore")?;
        write_file(&root, "src/lib.rs")?;

        let nested_checkout = root.join("checkout-clone");
        fs::create_dir_all(nested_checkout.join(".git"))
            .map_err(|err| format!("create checkout .git dir failed: {err}"))?;
        write_file(&root, "checkout-clone/src/unsafe.rs")?;

        let gitfile_worktree = root.join("worktree-link");
        fs::create_dir_all(&gitfile_worktree)
            .map_err(|err| format!("create worktree dir failed: {err}"))?;
        fs::write(
            gitfile_worktree.join(".git"),
            "gitdir: ../main-repo/.git/worktrees/worktree-link\n",
        )
        .map_err(|err| format!("write gitfile failed: {err}"))?;
        write_file(&root, "worktree-link/src/unsafe.rs")?;

        let files = discover_rust_files(
            &root,
            &DiscoveryOptions {
                respect_gitignore: false,
                ..DiscoveryOptions::default()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(files, vec![PathBuf::from("src/lib.rs")]);
        Ok(())
    }

    #[test]
    fn discovery_does_not_skip_plain_subdirectory_without_dot_git() -> Result<(), String> {
        // A normal subdirectory without a .git entry must still be discovered.
        let root = unique_temp_dir("unsafe-review-workspace-plain-subdir")?;
        write_file(&root, "src/lib.rs")?;
        write_file(&root, "subdir/src/lib.rs")?;

        let files = discover_rust_files(
            &root,
            &DiscoveryOptions {
                respect_gitignore: false,
                ..DiscoveryOptions::default()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(
            files,
            vec![
                PathBuf::from("src/lib.rs"),
                PathBuf::from("subdir/src/lib.rs"),
            ]
        );
        Ok(())
    }

    fn write_file(root: &Path, rel: &str) -> Result<(), String> {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("create parent failed: {err}"))?;
        }
        fs::write(&path, "unsafe fn fixture_data() {}\n")
            .map_err(|err| format!("write {} failed: {err}", path.display()))
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
