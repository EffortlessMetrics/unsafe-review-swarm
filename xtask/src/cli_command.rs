//! Keep repository CLI children on Cargo's locked, source-checked dev path.

use std::path::Path;
use std::process::Command;

pub(crate) fn command() -> Command {
    command_in(std::env::current_dir().ok().as_deref())
}

fn command_in(directory: Option<&Path>) -> Command {
    let mut command = Command::new("cargo");
    command.args(["run", "--locked", "-p", "unsafe-review", "--"]);
    if let Some(directory) = directory {
        #[cfg(windows)]
        let ordinary = ordinary_workspace_directory(directory);
        #[cfg(windows)]
        let directory = ordinary.as_deref().unwrap_or(directory);
        command.current_dir(directory);
    }
    command
}

#[cfg(windows)]
fn ordinary_workspace_directory(directory: &Path) -> Option<std::path::PathBuf> {
    // xtask canonicalizes its root, which produces a Windows verbatim path.
    // Cargo can then load workspace members through both F:\... and \\?\F:\...
    // and invalidate otherwise warm units with PathToSourceChanged. Git gives
    // us the ordinary spelling without hand-stripping UNC/device prefixes.
    // Accept it only for the identical directory, preserving nested/no-Git
    // and unusual Windows layouts when we cannot establish that identity.
    let ordinary = crate::git_workspace_root(directory).ok().flatten()?;
    let canonical = std::fs::canonicalize(directory).ok()?;
    (std::fs::canonicalize(&ordinary).ok()? == canonical).then_some(ordinary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_locked_dev_cli_arguments() {
        let command = command_in(None);
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, ["run", "--locked", "-p", "unsafe-review", "--"]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_children_stay_warm_and_changed_source_rebuilds() -> Result<(), String> {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("unsafe-review-2173-{}-{nonce}", std::process::id()));
        let write = |path: &str, content: &str| -> Result<(), String> {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            fs::write(path, content).map_err(|err| err.to_string())
        };
        write(
            "Cargo.toml",
            "[workspace]\nmembers = ['facade', 'cli', 'core']\nresolver = '2'\n",
        )?;
        write(
            "facade/Cargo.toml",
            "[package]\nname = 'unsafe-review'\nversion = '0.0.0'\nedition = '2024'\n[dependencies]\nfixture-cli = { path = '../cli' }\nfixture-core = { path = '../core' }\n",
        )?;
        write(
            "facade/src/main.rs",
            "fn main() { println!(\"{}\", fixture_cli::value()); if std::env::args().nth(1).as_deref() == Some(\"advisory\") { std::process::exit(1); } }\n",
        )?;
        write(
            "cli/Cargo.toml",
            "[package]\nname = 'fixture-cli'\nversion = '0.0.0'\nedition = '2024'\n[dependencies]\nfixture-core = { path = '../core' }\n",
        )?;
        write(
            "cli/src/lib.rs",
            "pub fn value() -> u8 { fixture_core::value() }\n",
        )?;
        write(
            "cli/build.rs",
            "fn main() { println!(\"cargo:rerun-if-changed=build.rs\"); }\n",
        )?;
        write(
            "core/Cargo.toml",
            "[package]\nname = 'fixture-core'\nversion = '0.0.0'\nedition = '2024'\n",
        )?;
        write("core/src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        let canonical = fs::canonicalize(&root).map_err(|err| err.to_string())?;
        // A source archive/no-Git directory must keep its existing spelling.
        assert_eq!(
            command_in(Some(&canonical)).get_current_dir(),
            Some(canonical.as_path())
        );
        for (program, args) in [
            ("git", vec!["init", "--quiet"]),
            ("cargo", vec!["generate-lockfile", "--offline"]),
        ] {
            let output = Command::new(program)
                .args(args)
                .current_dir(&root)
                .output()
                .map_err(|err| err.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into_owned());
            }
        }
        let child = |directory: &Path| {
            let mut command = command_in(Some(directory));
            command
                .env("CARGO_TARGET_DIR", root.join("build"))
                .env("CARGO_BUILD_JOBS", "1")
                .env("CARGO_NET_OFFLINE", "true")
                .env("CARGO_LOG", "cargo::core::compiler::fingerprint=trace");
            command
        };
        // Cargo must not receive the verbatim spelling used by canonicalize:
        // real workspace graphs can alternate source paths under that cwd.
        assert_eq!(
            command_in(Some(&canonical)).get_current_dir(),
            Some(root.as_path())
        );
        let nested = fs::canonicalize(root.join("core")).map_err(|err| err.to_string())?;
        assert_eq!(
            command_in(Some(&nested)).get_current_dir(),
            Some(nested.as_path())
        );
        for attempt in 0..16 {
            let directory = if attempt % 2 == 0 { &root } else { &canonical };
            let output = child(directory).output().map_err(|err| err.to_string())?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !output.status.success() || output.stdout != b"1\n" && output.stdout != b"1\r\n" {
                return Err(format!("attempt {attempt}: {stderr}"));
            }
            if attempt > 0 && stderr.contains("Compiling ") {
                return Err(format!(
                    "warm attempt {attempt} rebuilt; fixture {} retained:\n{stderr}",
                    root.display()
                ));
            }
        }
        write("core/src/lib.rs", "pub fn value() -> u8 { 2 }\n")?;
        let changed = child(&canonical).output().map_err(|err| err.to_string())?;
        assert!(changed.status.success());
        assert_eq!(String::from_utf8_lossy(&changed.stdout).trim(), "2");
        assert!(String::from_utf8_lossy(&changed.stderr).contains("Compiling fixture-core"));

        // Exercise status(), used by the corpus exit-code helper, after capture.
        let log_path = root.join("status.stderr");
        let log = fs::File::create(&log_path).map_err(|err| err.to_string())?;
        let status = child(&root)
            .arg("advisory")
            .stdout(std::process::Stdio::null())
            .stderr(log)
            .status()
            .map_err(|err| err.to_string())?;
        assert_eq!(status.code(), Some(1));
        assert!(
            !fs::read_to_string(log_path)
                .map_err(|err| err.to_string())?
                .contains("Compiling ")
        );
        fs::remove_dir_all(&root).map_err(|err| err.to_string())?;
        Ok(())
    }
}
