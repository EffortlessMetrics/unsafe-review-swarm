//! The repository-owned structured test runner control plane.
//!
//! This module deliberately keeps the upstream runner behind a small, pinned
//! adapter. The runner's supported JUnit report is treated as hostile input;
//! only a bounded, closed-vocabulary projection is retained. The command's
//! process status remains the authoritative test result.

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const NEXTEST_VERSION: &str = "0.9.143";
const NEXTEST_RELEASE: &str =
    "https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.143";
const MAX_INPUT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 128;
const MAX_FIELD_BYTES: usize = 256;
const MAX_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_RUN_SECONDS: u64 = 1_800;
const PARSE_REASONS: &[&str] = &[
    "unexpected_root",
    "unknown_attribute",
    "malformed_tag",
    "unsupported_structure",
    "input_limit",
];

#[derive(Clone, Copy)]
struct ToolAsset {
    target: &'static str,
    archive_sha256: &'static str,
    executable: &'static str,
}

const ASSETS: &[ToolAsset] = &[
    ToolAsset {
        target: "x86_64-unknown-linux-gnu",
        archive_sha256: "66786b9abe23920d022a182d1416b1bbc8130dd4872a9553d76985a1708dcd1e",
        executable: "cargo-nextest",
    },
    ToolAsset {
        target: "x86_64-pc-windows-msvc",
        archive_sha256: "c42a1dbde532da06dc9b4a43d44fd0ce668b836c2ab7388410f10ff9834476a2",
        executable: "cargo-nextest.exe",
    },
];

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Diagnostic {
    package: String,
    test: String,
    status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParseOutcome {
    stream_status: &'static str,
    parse_reason: Option<&'static str>,
}

impl ParseOutcome {
    const fn ok() -> Self {
        Self {
            stream_status: "ok",
            parse_reason: None,
        }
    }

    const fn missing_report() -> Self {
        Self {
            stream_status: "missing_report",
            parse_reason: None,
        }
    }

    const fn malformed(reason: &'static str) -> Self {
        Self::status("malformed_report", reason)
    }

    const fn status(stream_status: &'static str, parse_reason: &'static str) -> Self {
        Self {
            stream_status,
            parse_reason: Some(parse_reason),
        }
    }
}

impl PartialEq<&str> for ParseOutcome {
    fn eq(&self, other: &&str) -> bool {
        self.stream_status == *other
    }
}

/// Run nextest plus doctests and retain only `test-diagnostics.json`.
pub(crate) fn run(root: &Path) -> Result<(), String> {
    let run_key = std::env::var("CORE_RUN_KEY").unwrap_or_else(|_| "local".to_string());
    validate_core_run_key(&run_key)?;
    let staging = if let Some(raw) = std::env::var_os("UNSAFE_REVIEW_CI_HANDOFF_DIR") {
        let handoff_key = std::env::var("CORE_RUN_KEY")
            .map_err(|_error| "private CI handoff requires CORE_RUN_KEY".to_string())?;
        let path = PathBuf::from(raw);
        if handoff_key.is_empty() || !path.to_string_lossy().contains(&handoff_key) {
            return Err("private CI handoff path is not keyed to CORE_RUN_KEY".to_string());
        }
        path
    } else {
        root.join("target").join("ci-core").join("structured")
    };
    create_private_directory(&staging)?;
    let output_path = staging.join("test-diagnostics.json");
    reject_symlink(&output_path)?;

    let runner = resolve_runner(root, &run_key)?;
    let (nextest_exit, records, parse_outcome) = run_nextest(&runner, root, &staging)?;
    let doctest_exit = run_doctests(root)?;
    let core_exit = if nextest_exit == 0 && doctest_exit == 0 {
        0
    } else if nextest_exit != 0 {
        nextest_exit
    } else {
        doctest_exit
    };
    write_diagnostics(
        &output_path,
        nextest_exit,
        doctest_exit,
        core_exit,
        parse_outcome.stream_status,
        parse_outcome.parse_reason,
        &records,
    )?;
    println!(
        "ci-test: nextest_exit={nextest_exit} doctest_exit={doctest_exit} stream_status={} parse_reason={}",
        parse_outcome.stream_status,
        parse_outcome.parse_reason.unwrap_or("none")
    );
    if core_exit == 0 {
        Ok(())
    } else {
        Err(format!(
            "structured test run failed with exit code {core_exit}"
        ))
    }
}

fn validate_core_run_key(run_key: &str) -> Result<(), String> {
    if run_key.is_empty()
        || matches!(run_key, "." | "..")
        || !run_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("CORE_RUN_KEY contained unsafe path characters".to_string());
    }
    Ok(())
}

/// Validate the exact sanitized handoff immediately before artifact upload.
pub(crate) fn validate_diagnostics(path: &Path) -> Result<(), String> {
    let bytes = bounded_read(path)?;
    if bytes
        .iter()
        .any(|byte| *byte < 0x20 && *byte != b'\n' && *byte != b'\r' && *byte != b'\t')
    {
        return Err("test diagnostics contained control bytes".to_string());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse test diagnostics: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "test diagnostics root must be an object".to_string())?;
    let expected = [
        "schema_version",
        "runner",
        "runner_version",
        "nextest_exit",
        "doctest_exit",
        "core_exit",
        "stream_status",
        "parse_reason",
        "records",
    ];
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("test diagnostics schema fields drifted".to_string());
    }
    if object
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some("1.0")
        || object.get("runner").and_then(serde_json::Value::as_str) != Some("cargo-nextest")
        || object
            .get("runner_version")
            .and_then(serde_json::Value::as_str)
            != Some(NEXTEST_VERSION)
    {
        return Err("test diagnostics runner identity is not pinned".to_string());
    }
    let nextest_exit = required_exit(object, "nextest_exit")?;
    let doctest_exit = required_exit(object, "doctest_exit")?;
    let core_exit = required_exit(object, "core_exit")?;
    let expected_core = if nextest_exit == 0 && doctest_exit == 0 {
        0
    } else if nextest_exit != 0 {
        nextest_exit
    } else {
        doctest_exit
    };
    if core_exit != expected_core {
        return Err("test diagnostics core_exit does not match component exits".to_string());
    }
    let stream_status = object
        .get("stream_status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "test diagnostics stream_status is not a string".to_string())?;
    if !matches!(
        stream_status,
        "ok" | "missing_report"
            | "malformed_report"
            | "malformed_record"
            | "unexpected_field"
            | "hostile_record"
            | "truncated_input"
            | "record_limit"
            | "overlong_record"
    ) {
        return Err("test diagnostics stream_status is outside the closed vocabulary".to_string());
    }
    let parse_reason = object
        .get("parse_reason")
        .ok_or_else(|| "test diagnostics parse_reason is missing".to_string())?;
    let parse_reason = match parse_reason {
        serde_json::Value::Null => None,
        serde_json::Value::String(reason) if PARSE_REASONS.contains(&reason.as_str()) => {
            Some(reason.as_str())
        }
        _ => {
            return Err(
                "test diagnostics parse_reason is outside the closed vocabulary".to_string(),
            );
        }
    };
    let requires_reason = !matches!(stream_status, "ok" | "missing_report");
    if requires_reason != parse_reason.is_some() {
        return Err("test diagnostics parse_reason does not match stream_status".to_string());
    }
    let records = object
        .get("records")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "test diagnostics records is not an array".to_string())?;
    if records.len() > MAX_RECORDS {
        return Err("test diagnostics record count exceeded bound".to_string());
    }
    let mut previous: Option<(String, String)> = None;
    for record in records {
        let record = record
            .as_object()
            .ok_or_else(|| "test diagnostic record is not an object".to_string())?;
        if record.len() != 3
            || !record.contains_key("package")
            || !record.contains_key("test")
            || !record.contains_key("status")
        {
            return Err("test diagnostic record schema drifted".to_string());
        }
        let package = bounded_json_string(record.get("package"))?;
        let test = bounded_json_string(record.get("test"))?;
        if record.get("status").and_then(serde_json::Value::as_str) != Some("failed") {
            return Err("test diagnostic status is outside the closed vocabulary".to_string());
        }
        if contains_hostile(&package) || contains_hostile(&test) {
            return Err("test diagnostic contained hostile identity".to_string());
        }
        let current = (package, test);
        if previous.as_ref().is_some_and(|value| value >= &current) {
            return Err(
                "test diagnostic records are not strictly sorted and deduplicated".to_string(),
            );
        }
        previous = Some(current);
    }
    println!("ci-test-validate: ok ({})", path.display());
    Ok(())
}

fn required_exit(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<i32, String> {
    let value = object
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("test diagnostics {key} is not an integer"))?;
    i32::try_from(value).map_err(|_error| format!("test diagnostics {key} is out of range"))
}

fn bounded_json_string(value: Option<&serde_json::Value>) -> Result<String, String> {
    let value = value
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "test diagnostic identity is not a string".to_string())?;
    if value.is_empty() || value.len() > MAX_FIELD_BYTES {
        return Err("test diagnostic identity exceeded bound".to_string());
    }
    Ok(value.to_string())
}

fn create_private_directory(path: &Path) -> Result<(), String> {
    reject_symlink_ancestors(path)?;
    fs::create_dir_all(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    reject_symlink(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("protect {}: {error}", path.display()))?;
    }
    Ok(())
}

fn reject_symlink_ancestors(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing symlinked ancestor: {}",
                    ancestor.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect {}: {error}", ancestor.display())),
        }
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("inspect {}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() {
        Err(format!("refusing symlinked path: {}", path.display()))
    } else {
        Ok(())
    }
}

fn current_asset() -> Result<ToolAsset, String> {
    let target = match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        _ => {
            return Err(format!(
                "cargo-nextest {NEXTEST_VERSION} has no pinned asset for {}-{}",
                std::env::consts::ARCH,
                std::env::consts::OS
            ));
        }
    };
    ASSETS
        .iter()
        .copied()
        .find(|asset| asset.target == target)
        .ok_or_else(|| format!("missing pinned cargo-nextest asset for {target}"))
}

fn resolve_runner(root: &Path, run_key: &str) -> Result<PathBuf, String> {
    let asset = current_asset()?;
    let tool_dir = root
        .join("target")
        .join("ci-tools")
        .join(run_key)
        .join(format!("cargo-nextest-{NEXTEST_VERSION}-{}", asset.target));
    create_private_directory(&tool_dir)?;
    let executable = tool_dir.join(asset.executable);
    let archive_name = format!("cargo-nextest-{NEXTEST_VERSION}-{}.tar.gz", asset.target);
    let archive = tool_dir.join(&archive_name);
    let url = format!("{NEXTEST_RELEASE}/{archive_name}");
    let verified_cache = tool_dir.join("cargo-nextest.verified.tar.gz");
    let mut redownloaded = false;
    loop {
        let verification = if archive.exists() {
            verify_sha256(&archive, asset.archive_sha256)
        } else {
            download(&url, &archive).and_then(|_| verify_sha256(&archive, asset.archive_sha256))
        };
        match verification {
            Ok(()) => break,
            Err(_error) if !redownloaded => {
                let _ = fs::remove_file(&archive);
                let _ = fs::remove_file(&verified_cache);
                redownloaded = true;
                continue;
            }
            Err(error) => return Err(error),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&archive, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("protect {}: {error}", archive.display()))?;
    }
    // Copy the verified bytes to an immutable, private handoff before
    // extraction. This closes the hash-then-use window if the cache changes
    // between verification and the tar process opening its input.
    let verified_archive = verified_archive(&archive, &tool_dir, asset.archive_sha256)?;
    unpack(&verified_archive, &tool_dir)?;
    reject_symlink(&executable)?;
    if !fs::metadata(&executable)
        .map_err(|error| format!("inspect {}: {error}", executable.display()))?
        .is_file()
    {
        return Err(format!(
            "pinned runner archive did not produce {}",
            executable.display()
        ));
    }
    verify_version(&executable)?;
    Ok(executable)
}

fn verified_archive(archive: &Path, directory: &Path, expected: &str) -> Result<PathBuf, String> {
    verify_sha256(archive, expected)?;
    let verified = directory.join("cargo-nextest.verified.tar.gz");
    if verified.exists() {
        if let Ok(metadata) = fs::metadata(&verified) {
            let mut permissions = metadata.permissions();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                permissions.set_mode(0o600);
            }
            #[cfg(windows)]
            #[allow(
                clippy::permissions_set_readonly_false,
                reason = "Windows cache files may retain the read-only bit"
            )]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(&verified, permissions);
        }
        let _ = fs::remove_file(&verified);
    }
    let mut source = File::open(archive)
        .map_err(|error| format!("open verified source {}: {error}", archive.display()))?;
    let mut destination = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&verified)
        .map_err(|error| format!("create immutable archive {}: {error}", verified.display()))?;
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|error| format!("read verified source: {error}"))?;
        if count == 0 {
            break;
        }
        copied = copied.saturating_add(count as u64);
        if copied > MAX_ARCHIVE_BYTES {
            return Err("verified cargo-nextest asset exceeded size bounds".to_string());
        }
        destination
            .write_all(&buffer[..count])
            .map_err(|error| format!("write immutable archive: {error}"))?;
    }
    destination
        .sync_all()
        .map_err(|error| format!("sync immutable archive: {error}"))?;
    drop(destination);
    verify_sha256(&verified, expected)?;
    let mut permissions = fs::metadata(&verified)
        .map_err(|error| format!("inspect immutable archive: {error}"))?
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&verified, permissions)
        .map_err(|error| format!("seal immutable archive: {error}"))?;
    Ok(verified)
}

fn download(url: &str, destination: &Path) -> Result<(), String> {
    if !url.starts_with("https://github.com/nextest-rs/nextest/releases/download/") {
        return Err("refusing non-pinned HTTPS cargo-nextest URL".to_string());
    }
    reject_symlink(destination)?;
    let temp = destination.with_extension("download");
    let _ = fs::remove_file(&temp);
    let status = if cfg!(windows) {
        Command::new("curl.exe")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--connect-timeout",
                "30",
                "--max-time",
                "300",
                "--output",
            ])
            .arg(&temp)
            .arg(url)
            .status()
    } else {
        Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--connect-timeout",
                "30",
                "--max-time",
                "300",
                "--output",
            ])
            .arg(&temp)
            .arg(url)
            .status()
    }
    .map_err(|error| format!("download pinned cargo-nextest asset: {error}"))?;
    if !status.success() {
        return Err(format!(
            "download of pinned cargo-nextest asset failed: {status}"
        ));
    }
    reject_symlink(&temp)?;
    let size = fs::metadata(&temp)
        .map_err(|error| format!("inspect downloaded cargo-nextest asset: {error}"))?
        .len();
    if size == 0 || size > MAX_ARCHIVE_BYTES {
        return Err("downloaded cargo-nextest asset exceeded size bounds".to_string());
    }
    fs::rename(&temp, destination).map_err(|error| {
        format!(
            "install downloaded asset {}: {error}",
            destination.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("protect {}: {error}", destination.display()))?;
    }
    Ok(())
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    reject_symlink(path)?;
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("hash {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let digest = format!("{:x}", hasher.finalize());
    if digest != expected {
        return Err(format!("sha256 mismatch for {}", path.display()));
    }
    Ok(())
}

fn unpack(archive: &Path, directory: &Path) -> Result<(), String> {
    let listing = Command::new("tar")
        .args(["-tzf"])
        .arg(archive)
        .output()
        .map_err(|error| format!("list pinned cargo-nextest archive: {error}"))?;
    if !listing.status.success() {
        return Err("pinned cargo-nextest archive listing failed".to_string());
    }
    let names = String::from_utf8_lossy(&listing.stdout);
    if names.lines().any(|name| {
        let name = name.trim();
        name.starts_with('/') || name.starts_with('\\') || name.split('/').any(|part| part == "..")
    }) {
        return Err("pinned cargo-nextest archive contained an unsafe path".to_string());
    }
    let strip_components = if names.lines().any(|name| {
        let name = name.trim();
        !name.is_empty() && !name.ends_with('/') && !name.contains('/')
    }) {
        0
    } else {
        1
    };
    for name in ["cargo-nextest", "cargo-nextest.exe"] {
        let existing = directory.join(name);
        if let Ok(metadata) = fs::metadata(&existing) {
            let mut permissions = metadata.permissions();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                permissions.set_mode(0o700);
            }
            #[cfg(windows)]
            #[allow(
                clippy::permissions_set_readonly_false,
                reason = "Windows cache files may retain the read-only bit"
            )]
            permissions.set_readonly(false);
            fs::set_permissions(&existing, permissions)
                .map_err(|error| format!("unlock cached runner {}: {error}", existing.display()))?;
            fs::remove_file(&existing)
                .map_err(|error| format!("remove cached runner {}: {error}", existing.display()))?;
        }
    }
    let mut extraction = Command::new("tar");
    extraction.args(["-xzf"]).arg(archive);
    if strip_components != 0 {
        extraction.args(["--strip-components", "1"]);
    }
    let status = extraction
        .args(["-C"])
        .arg(directory)
        .status()
        .map_err(|error| format!("unpack pinned cargo-nextest archive: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "unpack pinned cargo-nextest archive failed: {status}"
        ))
    }
}

fn verify_version(path: &Path) -> Result<(), String> {
    let output = Command::new(path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("verify pinned cargo-nextest version: {error}"))?;
    let version = String::from_utf8_lossy(&output.stdout);
    let expected = format!("cargo-nextest {NEXTEST_VERSION}");
    let version_matches = version.lines().any(|line| {
        let line = line.trim();
        line.strip_prefix(&expected).is_some_and(|suffix| {
            suffix.is_empty() || suffix.starts_with(' ') || suffix.starts_with('(')
        })
    });
    if !output.status.success() || !version_matches {
        return Err(format!(
            "cached cargo-nextest is not version {NEXTEST_VERSION}"
        ));
    }
    Ok(())
}

fn run_nextest(
    runner: &Path,
    root: &Path,
    handoff: &Path,
) -> Result<(i32, BTreeSet<Diagnostic>, ParseOutcome), String> {
    reject_symlink(runner)?;
    let junit = handoff.join("nextest-junit.xml");
    let config = handoff.join("nextest.toml");
    let _ = fs::remove_file(&junit);
    let _ = fs::remove_file(&config);
    let junit_for_config = junit.to_string_lossy().replace('\\', "/");
    let config_text = format!(
        "[profile.ci.junit]\npath = \"{junit_for_config}\"\nstore-success-output = false\nstore-failure-output = false\nreport-skipped = \"none\"\n"
    );
    fs::write(&config, config_text)
        .map_err(|error| format!("write private nextest config: {error}"))?;
    reject_symlink(&config)?;
    let mut command = runner_command(runner);
    let mut child = command
        .current_dir(root)
        .args([
            "nextest",
            "run",
            "--profile",
            "ci",
            "--config-file",
            config.to_string_lossy().as_ref(),
            "--workspace",
            "--all-targets",
            "--locked",
            "--no-fail-fast",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("start pinned cargo-nextest: {error}"))?;
    let exit = wait_bounded(&mut child)?;
    let mut records = BTreeSet::new();
    let status = if junit.exists() && !junit.is_symlink() {
        let locked_report = handoff.join("nextest-junit.locked.xml");
        let _ = fs::remove_file(&locked_report);
        fs::rename(&junit, &locked_report)
            .map_err(|error| format!("lock private nextest report: {error}"))?;
        reject_symlink(&locked_report)?;
        let status = match bounded_read(&locked_report) {
            Ok(bytes) => parse_junit(&bytes, &mut records),
            Err(_) => ParseOutcome::status("truncated_input", "input_limit"),
        };
        let _ = fs::remove_file(&locked_report);
        status
    } else {
        ParseOutcome::missing_report()
    };
    let _ = fs::remove_file(&junit);
    let _ = fs::remove_file(&config);
    Ok((exit, records, status))
}

fn runner_command(runner: &Path) -> Command {
    if runner.extension().and_then(|value| value.to_str()) == Some("ps1") {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]);
        command.arg(runner);
        command
    } else if runner.extension().and_then(|value| value.to_str()) == Some("sh") {
        let mut command = Command::new("sh");
        command.arg(runner);
        command
    } else {
        Command::new(runner)
    }
}

fn wait_bounded(child: &mut std::process::Child) -> Result<i32, String> {
    let deadline = Instant::now() + Duration::from_secs(MAX_RUN_SECONDS);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("wait for structured test runner: {error}"))?
        {
            return Ok(status.code().unwrap_or(1));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(124);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    reject_symlink(path)?;
    let metadata =
        fs::metadata(path).map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "structured report exceeded {}-byte bound",
            MAX_INPUT_BYTES
        ));
    }
    let capacity = metadata.len().min(MAX_INPUT_BYTES) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(format!(
            "structured report exceeded {}-byte bound",
            MAX_INPUT_BYTES
        ));
    }
    Ok(bytes)
}

fn parse_junit(bytes: &[u8], records: &mut BTreeSet<Diagnostic>) -> ParseOutcome {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return ParseOutcome::malformed("malformed_tag");
    };
    let Ok(text) = strip_xml_comments(text) else {
        return ParseOutcome::malformed("unsupported_structure");
    };
    let Ok(text) = strip_cdata_sections(&text) else {
        return ParseOutcome::malformed("unsupported_structure");
    };
    if text.contains("<!DOCTYPE") {
        return ParseOutcome::malformed("unsupported_structure");
    }
    if validate_junit_envelope(&text).is_err() {
        return ParseOutcome::malformed("unexpected_root");
    }
    if validate_root_opening(&text).is_err() {
        return ParseOutcome::malformed("unexpected_root");
    }
    let mut cursor = 0_usize;
    let mut count = 0_usize;
    while let Some(relative) = text[cursor..].find("<testcase") {
        let start = cursor + relative;
        let Some(tag_end_relative) = text[start..].find('>') else {
            return ParseOutcome::malformed("malformed_tag");
        };
        let tag_end = start + tag_end_relative;
        let tag = &text[start..=tag_end];
        let self_closing = tag.trim_end().ends_with("/>");
        let (body, next_cursor) = if self_closing {
            ("", tag_end + 1)
        } else {
            let Some(close_relative) = text[tag_end + 1..].find("</testcase>") else {
                return ParseOutcome::malformed("malformed_tag");
            };
            let close = tag_end + 1 + close_relative;
            (&text[tag_end + 1..close], close + "</testcase>".len())
        };
        if body.contains("<testcase") || validate_testcase_body(body).is_err() {
            return ParseOutcome::status("malformed_record", "unsupported_structure");
        }
        let attrs = match parse_xml_attributes(tag) {
            Ok(attrs) => attrs,
            Err(status) => return ParseOutcome::status(status, parser_reason(status)),
        };
        let Some(package) = attrs.get("classname") else {
            return ParseOutcome::status("malformed_record", "malformed_tag");
        };
        let Some(test) = attrs.get("name") else {
            return ParseOutcome::status("malformed_record", "malformed_tag");
        };
        let status = if body.contains("<failure")
            || body.contains("<error")
            || body.contains("<rerunFailure")
            || body.contains("<flakyFailure")
            || body.contains("<flakyError")
        {
            "failed"
        } else if body.contains("<skipped") {
            "skipped"
        } else {
            "passed"
        };
        if status == "failed" {
            if count >= MAX_RECORDS {
                return ParseOutcome::status("record_limit", "input_limit");
            }
            records.insert(Diagnostic {
                package: package.clone(),
                test: test.clone(),
                status: status.to_string(),
            });
            count += 1;
        }
        cursor = next_cursor;
    }
    ParseOutcome::ok()
}

fn parser_reason(status: &'static str) -> &'static str {
    match status {
        "unexpected_field" => "unknown_attribute",
        "hostile_record" => "unsupported_structure",
        "truncated_input" | "record_limit" | "overlong_record" => "input_limit",
        _ => "malformed_tag",
    }
}

fn strip_cdata_sections(text: &str) -> Result<String, ()> {
    let mut sanitized = text.to_string();
    let mut cursor = 0_usize;
    while let Some(relative) = sanitized[cursor..].find("<![CDATA[") {
        let start = cursor + relative;
        let Some(end_relative) = sanitized[start + 9..].find("]]>") else {
            return Err(());
        };
        let end = start + 9 + end_relative;
        sanitized.replace_range(start..end + 3, &" ".repeat(end + 3 - start));
        cursor = start;
    }
    if sanitized[cursor..].contains("]]>") {
        return Err(());
    }
    Ok(sanitized)
}

fn strip_xml_comments(text: &str) -> Result<String, ()> {
    let mut sanitized = text.to_string();
    let mut cursor = 0_usize;
    while let Some(relative) = sanitized[cursor..].find("<!--") {
        let start = cursor + relative;
        let Some(end_relative) = sanitized[start + 4..].find("-->") else {
            return Err(());
        };
        let end = start + 4 + end_relative;
        let comment = &sanitized[start + 4..end];
        if comment.contains("<!--") || comment.contains("--") {
            return Err(());
        }
        sanitized.replace_range(start..end + 3, &" ".repeat(end + 3 - start));
        cursor = start;
    }
    if sanitized[cursor..].contains("-->") {
        return Err(());
    }
    Ok(sanitized)
}

fn validate_junit_envelope(text: &str) -> Result<(), ()> {
    let mut start = text.trim_start();
    if start.starts_with("<?xml") {
        let end = start.find("?>").ok_or(())? + 2;
        start = start[end..].trim_start();
    }
    let root = if start.starts_with("<testsuites") {
        "testsuites"
    } else if start.starts_with("<testsuite") {
        "testsuite"
    } else {
        return Err(());
    };
    let close = format!("</{root}>");
    let end = start.rfind(&close).ok_or(())? + close.len();
    if !start[end..].trim().is_empty() {
        return Err(());
    }
    Ok(())
}

fn validate_root_opening(text: &str) -> Result<(), ()> {
    let mut start = text.trim_start();
    if start.starts_with("<?xml") {
        let end = start.find("?>").ok_or(())? + 2;
        start = start[end..].trim_start();
    }
    let end = start.find('>').ok_or(())?;
    let tag = &start[..=end];
    let root = if tag.starts_with("<testsuites") {
        "testsuites"
    } else if tag.starts_with("<testsuite") {
        "testsuite"
    } else {
        return Err(());
    };
    if tag.ends_with("/>") {
        return Err(());
    }
    let allowed = [
        "name",
        "tests",
        "failures",
        "errors",
        "skipped",
        "time",
        "timestamp",
        "hostname",
        "id",
        "uuid",
        "package",
    ];
    parse_attributes(tag, root, &allowed)
        .map(|_| ())
        .map_err(|_error| ())
}

fn validate_testcase_body(body: &str) -> Result<(), ()> {
    let mut rest = body;
    let mut open = Vec::new();
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let end = rest.find('>').ok_or(())?;
        let tag = rest[..=end].trim();
        let closing = tag.starts_with("</");
        let self_closing = tag.ends_with("/>");
        let name = tag
            .trim_start_matches('<')
            .trim_start_matches('/')
            .trim_end_matches('>')
            .trim_end_matches('/')
            .split(|character: char| character.is_whitespace())
            .next()
            .ok_or(())?;
        let name = name.trim();
        let allowed_attributes = match name {
            "failure" | "error" | "skipped" => &["message", "type"][..],
            "rerunFailure" | "flakyFailure" | "flakyError" => {
                &["message", "type", "time", "timestamp"][..]
            }
            "system-out" | "system-err" => &[][..],
            _ => return Err(()),
        };
        if !closing
            && open.last().is_some_and(|parent| {
                !matches!(
                    (*parent, name),
                    (
                        "rerunFailure" | "flakyFailure" | "flakyError",
                        "system-out" | "system-err"
                    )
                )
            })
        {
            return Err(());
        }
        if !closing && parse_attributes(tag, name, allowed_attributes).is_err() {
            return Err(());
        }
        if closing && tag != format!("</{name}>") {
            return Err(());
        }
        if closing {
            if open.pop() != Some(name) {
                return Err(());
            }
        } else if self_closing {
            if open.last().is_some_and(|parent| {
                !matches!(
                    (*parent, name),
                    (
                        "rerunFailure" | "flakyFailure" | "flakyError",
                        "system-out" | "system-err"
                    )
                )
            }) {
                return Err(());
            }
        } else {
            open.push(name);
        }
        rest = &rest[end + 1..];
    }
    if !open.is_empty() {
        return Err(());
    }
    Ok(())
}

fn parse_xml_attributes(
    tag: &str,
) -> Result<std::collections::BTreeMap<String, String>, &'static str> {
    parse_attributes(
        tag,
        "testcase",
        &["name", "classname", "time", "timestamp", "file", "line"],
    )
}

fn parse_attributes(
    tag: &str,
    expected_name: &str,
    allowed: &[&str],
) -> Result<std::collections::BTreeMap<String, String>, &'static str> {
    let mut attrs = std::collections::BTreeMap::new();
    let mut rest = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/')
        .trim();
    let (name, remaining) = match rest.split_once(char::is_whitespace) {
        Some((name, remaining)) => (name, remaining),
        None if rest == expected_name => return Ok(attrs),
        None => return Err("malformed_record"),
    };
    if name != expected_name {
        return Err("malformed_record");
    }
    rest = remaining.trim();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else {
            return Err("unexpected_field");
        };
        let key = rest[..eq].trim();
        if !allowed.contains(&key) {
            return Err("unexpected_field");
        }
        let quoted = rest[eq + 1..].trim_start();
        if !quoted.starts_with('"') {
            return Err("malformed_record");
        }
        let Some(end) = quoted[1..].find('"').map(|offset| offset + 1) else {
            return Err("malformed_record");
        };
        let value = xml_unescape(&quoted[1..end])?;
        if value.is_empty() || value.len() > MAX_FIELD_BYTES || contains_hostile(&value) {
            return Err("hostile_record");
        }
        if attrs.insert(key.to_string(), value).is_some() {
            return Err("unexpected_field");
        }
        rest = quoted[end + 1..].trim();
    }
    Ok(attrs)
}

fn xml_unescape(value: &str) -> Result<String, &'static str> {
    let mut out = value.to_string();
    for (escaped, plain) in [
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
    ] {
        out = out.replace(escaped, plain);
    }
    if out.chars().any(|character| character.is_control()) {
        Err("hostile_record")
    } else {
        Ok(out)
    }
}

fn contains_hostile(value: &str) -> bool {
    value.chars().any(|character| character.is_control())
        || value.contains("-----BEGIN")
        || value.contains("ghp_")
        || value.contains("github_pat_")
        || value.contains("AKIA")
        || value.contains("eyJhbGci")
}

fn run_doctests(root: &Path) -> Result<i32, String> {
    let mut child = Command::new("cargo")
        .current_dir(root)
        .args(["test", "--workspace", "--doc", "--locked"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("start doctest parity step: {error}"))?;
    wait_bounded(&mut child)
}

fn write_diagnostics(
    path: &Path,
    nextest_exit: i32,
    doctest_exit: i32,
    core_exit: i32,
    stream_status: &str,
    parse_reason: Option<&str>,
    records: &BTreeSet<Diagnostic>,
) -> Result<(), String> {
    reject_symlink(path)?;
    let mut output = serde_json::Map::new();
    output.insert("schema_version".to_string(), serde_json::json!("1.0"));
    output.insert("runner".to_string(), serde_json::json!("cargo-nextest"));
    output.insert(
        "runner_version".to_string(),
        serde_json::json!(NEXTEST_VERSION),
    );
    output.insert("nextest_exit".to_string(), serde_json::json!(nextest_exit));
    output.insert("doctest_exit".to_string(), serde_json::json!(doctest_exit));
    output.insert("core_exit".to_string(), serde_json::json!(core_exit));
    output.insert(
        "stream_status".to_string(),
        serde_json::json!(stream_status),
    );
    output.insert(
        "parse_reason".to_string(),
        parse_reason.map_or(serde_json::Value::Null, |reason| {
            serde_json::Value::String(reason.to_string())
        }),
    );
    let mut record_values: Vec<serde_json::Value> = records
        .iter()
        .take(MAX_RECORDS)
        .map(|record| {
            serde_json::json!({
                "package": record.package,
                "test": record.test,
                "status": record.status,
            })
        })
        .collect();
    let bytes = loop {
        output.insert(
            "records".to_string(),
            serde_json::Value::Array(record_values.clone()),
        );
        let bytes = serde_json::to_vec(&serde_json::Value::Object(output.clone()))
            .map_err(|error| format!("serialize test diagnostics: {error}"))?;
        if bytes.len() < MAX_OUTPUT_BYTES {
            break bytes;
        }
        if record_values.pop().is_none() {
            return Err("test diagnostics exceeded 16-KiB bound".to_string());
        }
        output.insert(
            "stream_status".to_string(),
            serde_json::json!("truncated_input"),
        );
        output.insert("parse_reason".to_string(), serde_json::json!("input_limit"));
    };
    if bytes
        .iter()
        .any(|byte| *byte < 0x20 && *byte != b'\n' && *byte != b'\r' && *byte != b'\t')
    {
        return Err("test diagnostics contained control bytes".to_string());
    }
    let temp = path.with_extension("tmp");
    let _ = fs::remove_file(&temp);
    reject_symlink(&temp)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|error| format!("create {}: {error}", temp.display()))?;
        file.write_all(&bytes)
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| format!("write {}: {error}", temp.display()))?;
        file.sync_all()
            .map_err(|error| format!("sync {}: {error}", temp.display()))?;
        drop(file);
        fs::rename(&temp, path).map_err(|error| format!("publish {}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("unsafe-review-ci-test-{name}-{nanos}"));
        fs::create_dir_all(&path).map_err(|error| format!("create temp fixture: {error}"))?;
        Ok(path)
    }

    fn write_fixture_runner(dir: &Path) -> Result<PathBuf, String> {
        write_fixture_runner_mode(dir, "failed")
    }

    fn write_fixture_runner_mode(dir: &Path, mode: &str) -> Result<PathBuf, String> {
        if cfg!(windows) {
            let path = dir.join("fixture-nextest.ps1");
            let report_path = dir
                .join("handoff-run-1")
                .join("nextest-junit.xml")
                .to_string_lossy()
                .replace('\\', "/");
            let script = format!(
                r#"$configPath = Get-ChildItem -Path (Get-Location) -Recurse -Filter 'nextest.toml' | Select-Object -First 1 -ExpandProperty FullName
$debugPath = Join-Path (Get-Location) 'fixture-debug.txt'
Set-Content -LiteralPath $debugPath -Value ('configPath=' + $configPath)
if ([string]::IsNullOrEmpty($configPath)) {{ exit 2 }}
$configText = Get-Content -Raw -LiteralPath $configPath -ErrorAction Stop
$reportPath = '{report_path}'
$expectedPath = $reportPath.Replace('/', '\').ToLowerInvariant()
$configNormalized = $configText.Replace('/', '\').ToLowerInvariant()
Add-Content -LiteralPath $debugPath -Value ('reportPath=' + $reportPath + "`nexpectedPath=" + $expectedPath + "`nconfigNormalized=" + $configNormalized)
if (-not $configNormalized.Contains($expectedPath)) {{ exit 2 }}
if ('{mode}' -eq 'failed') {{
  Set-Content -LiteralPath $reportPath -NoNewline -Value '<testsuites><testsuite name="fixture"><testcase classname="fixture::tests" name="fixture_failure"><failure /></testcase></testsuite></testsuites>'
  exit 101
}}
if ('{mode}' -eq 'malformed') {{
  Set-Content -LiteralPath $reportPath -NoNewline -Value '<testsuite><testcase classname="fixture::tests" name="broken"><failure></testsuite>'
  exit 101
}}
if ('{mode}' -eq 'oversize') {{
  Set-Content -LiteralPath $reportPath -NoNewline -Value ('x' * 2097153)
  exit 101
}}
exit 0
"#
            );
            fs::write(&path, script)
                .map_err(|error| format!("write PowerShell fixture: {error}"))?;
            Ok(path)
        } else {
            let path = dir.join("fixture-nextest.sh");
            let script = format!(
                r#"#!/bin/sh
config=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--config-file" ]; then config="$2"; shift; fi
  shift
done
report=$(sed -n 's/^path = "\(.*\)"$/\1/p' "$config")
case "{mode}" in
  failed) printf '%s' '<testsuites><testsuite name="fixture"><testcase classname="fixture::tests" name="fixture_failure"><failure /></testcase></testsuite></testsuites>' > "$report"; exit 101 ;;
  malformed) printf '%s' '<testsuite><testcase classname="fixture::tests" name="broken"><failure></testsuite>' > "$report"; exit 101 ;;
  oversize) head -c 2097153 /dev/zero | tr '\0' x > "$report"; exit 101 ;;
  *) exit 0 ;;
esac
"#
            );
            fs::write(&path, script).map_err(|error| format!("write shell fixture: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .map_err(|error| format!("protect shell fixture: {error}"))?;
            }
            Ok(path)
        }
    }

    #[test]
    fn exact_assets_are_pinned() -> Result<(), String> {
        if NEXTEST_VERSION != "0.9.143" || ASSETS.len() != 2 {
            return Err("cargo-nextest pin drifted".to_string());
        }
        for asset in ASSETS {
            if asset.archive_sha256.len() != 64
                || !asset
                    .archive_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!("invalid hash for {}", asset.target));
            }
        }
        Ok(())
    }

    #[test]
    fn core_run_key_rejects_dot_path_segments() -> Result<(), String> {
        for run_key in [".", ".."] {
            if validate_core_run_key(run_key).is_ok() {
                return Err(format!("CORE_RUN_KEY value {run_key:?} was accepted"));
            }
        }
        Ok(())
    }

    #[test]
    fn shipped_wrapper_executes_fixture_runner_into_private_handoff() -> Result<(), String> {
        let dir = temp_dir("runner-execution")?;
        let handoff = dir.join("handoff-run-1");
        fs::create_dir_all(&handoff).map_err(|error| format!("create handoff: {error}"))?;
        let runner = write_fixture_runner(&dir)?;
        let (exit, records, status) = run_nextest(&runner, &dir, &handoff)?;
        if exit != 101 || status != "ok" || records.len() != 1 {
            return Err(format!(
                "fixture runner proof mismatch: exit={exit} status={} parse_reason={} records={}",
                status.stream_status,
                status.parse_reason.unwrap_or("none"),
                records.len()
            ));
        }
        if handoff.join("nextest-junit.xml").exists()
            || handoff.join("nextest.toml").exists()
            || handoff.join("nextest-junit.locked.xml").exists()
        {
            return Err("raw JUnit/config handoff residue was retained".to_string());
        }
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn shipped_wrapper_reports_missing_and_malformed_reports() -> Result<(), String> {
        for (mode, expected) in [
            ("missing", "missing_report"),
            ("malformed", "malformed_report"),
        ] {
            let dir = temp_dir(mode)?;
            let handoff = dir.join("handoff-run-1");
            fs::create_dir_all(&handoff).map_err(|error| format!("create handoff: {error}"))?;
            let runner = write_fixture_runner_mode(&dir, mode)?;
            let (exit, records, status) = run_nextest(&runner, &dir, &handoff)?;
            if status != expected || !records.is_empty() || (mode == "missing" && exit != 0) {
                return Err(format!(
                    "negative fixture mismatch: mode={mode} exit={exit} status={} parse_reason={} records={}",
                    status.stream_status,
                    status.parse_reason.unwrap_or("none"),
                    records.len()
                ));
            }
            let _ = fs::remove_dir_all(&dir);
        }
        Ok(())
    }

    #[test]
    fn shipped_wrapper_rejects_oversize_report() -> Result<(), String> {
        let dir = temp_dir("oversize")?;
        let handoff = dir.join("handoff-run-1");
        fs::create_dir_all(&handoff).map_err(|error| format!("create handoff: {error}"))?;
        let runner = write_fixture_runner_mode(&dir, "oversize")?;
        let (exit, records, outcome) = run_nextest(&runner, &dir, &handoff)?;
        if exit != 101
            || !records.is_empty()
            || outcome.stream_status != "truncated_input"
            || outcome.parse_reason != Some("input_limit")
        {
            return Err(format!(
                "oversize JUnit envelope mismatch: exit={exit} status={} parse_reason={} records={}",
                outcome.stream_status,
                outcome.parse_reason.unwrap_or("none"),
                records.len()
            ));
        }
        let diagnostics = dir.join("test-diagnostics.json");
        write_diagnostics(
            &diagnostics,
            exit,
            0,
            exit,
            outcome.stream_status,
            outcome.parse_reason,
            &records,
        )?;
        validate_diagnostics(&diagnostics)?;
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn shipped_wrapper_rejects_symlink_report() -> Result<(), String> {
        let dir = temp_dir("symlink")?;
        let handoff = dir.join("handoff-run-1");
        fs::create_dir_all(&handoff).map_err(|error| format!("create handoff: {error}"))?;
        let config = handoff.join("nextest.toml");
        let runner = dir.join("fixture-nextest.sh");
        let script = r#"#!/bin/sh
config=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--config-file" ]; then config="$2"; shift; fi
  shift
done
report=$(sed -n 's/^path = "\(.*\)"$/\1/p' "$config")
rm -f "$report"
ln -s /dev/null "$report"
exit 101
"#;
        fs::write(&runner, script).map_err(|error| format!("write symlink fixture: {error}"))?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("protect symlink fixture: {error}"))?;
        let (_, records, status) = run_nextest(&runner, &dir, &handoff)?;
        if status != "missing_report" || !records.is_empty() || config.exists() {
            return Err("symlink JUnit report was not rejected and cleaned".to_string());
        }
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn junit_parser_deduplicates_and_rejects_hostile_values() -> Result<(), String> {
        let mut records = BTreeSet::new();
        let junit = br#"<?xml version="1.0"?><testsuite><testcase classname="demo" name="fails"><failure/></testcase><testcase classname="demo" name="fails"><failure/></testcase></testsuite>"#;
        let status = parse_junit(junit, &mut records);
        if status != "ok" {
            return Err(format!(
                "valid JUnit fixture was rejected: status={} parse_reason={}",
                status.stream_status,
                status.parse_reason.unwrap_or("none")
            ));
        }
        if records.len() != 1 {
            return Err(format!(
                "expected one deduplicated record, got {}",
                records.len()
            ));
        }
        let hostile = br#"<testsuite><testcase classname="demo" name="ghp_secret"><failure/></testcase></testsuite>"#;
        if parse_junit(hostile, &mut records) != "hostile_record" {
            return Err("hostile record was not rejected".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_and_unexpected_junit_fields_fail_closed() -> Result<(), String> {
        let mut records = BTreeSet::new();
        let malformed =
            br#"<testsuite><testcase classname="demo" name="fails"><failure></testsuite>"#;
        if parse_junit(malformed, &mut records) != "malformed_report" {
            return Err("malformed JUnit was not rejected".to_string());
        }
        let unexpected = br#"<testsuite><testcase classname="demo" name="fails" secret="token"><failure/></testcase></testsuite>"#;
        if parse_junit(unexpected, &mut records) != "unexpected_field" {
            return Err("unexpected JUnit field was not rejected".to_string());
        }
        let mismatched =
            br#"<testsuite><testcase classname="demo" name="fails"><failure></error></testcase></testsuite>"#;
        if parse_junit(mismatched, &mut records) != "malformed_record" {
            return Err("mismatched JUnit body tags were not rejected".to_string());
        }
        let attributed = br#"<testsuite><testcase classname="demo" name="fails"><failure secret="raw"/></testcase></testsuite>"#;
        if parse_junit(attributed, &mut records) != "malformed_record" {
            return Err("unexpected JUnit body attributes were not rejected".to_string());
        }
        let nextest_failure = br#"<testsuites tests="1" failures="1"><testsuite name="demo" tests="1" failures="1"><testcase classname="demo::tests" name="fails" time="0.01"><failure message="assertion failed" type="panic">details</failure></testcase></testsuite></testsuites>"#;
        records.clear();
        if parse_junit(nextest_failure, &mut records) != "ok" || records.len() != 1 {
            return Err("nextest failure metadata fixture was rejected".to_string());
        }
        // cargo-nextest may emit retry/flaky failure elements plus diagnostic
        // streams under one testcase. Accept their bounded attributes and
        // structure, but project only the failed identity, never their body.
        let nextest_nested_failures = br#"<testsuite name="demo"><testcase classname="demo::tests" name="rerun-fails"><rerunFailure message="retry" type="panic" timestamp="2026-08-22T03:00:00Z" time="0.01"><system-out>nested retry output</system-out>raw retry output</rerunFailure><flakyFailure message="flaky" type="panic"><system-err>nested flaky output</system-err>raw flaky output</flakyFailure><flakyError message="error" type="panic">raw flaky error</flakyError><system-out>raw stdout</system-out><system-err>raw stderr</system-err></testcase></testsuite>"#;
        records.clear();
        if parse_junit(nextest_nested_failures, &mut records) != "ok" || records.len() != 1 {
            return Err("nextest nested failure fixture was rejected".to_string());
        }
        let projected = records.iter().next().ok_or_else(|| {
            "nextest nested failure fixture did not retain its identity".to_string()
        })?;
        if projected.package != "demo::tests"
            || projected.test != "rerun-fails"
            || projected.status != "failed"
        {
            return Err("nextest nested failure projection drifted".to_string());
        }
        let uuid_root = br#"<testsuite name="demo" uuid="6f6f2b1e-1eb2-4f5c-8b0e-1e0d6ebf5a10"><testcase classname="demo::tests" name="uuid-fails"><failure/></testcase></testsuite>"#;
        records.clear();
        if parse_junit(uuid_root, &mut records) != "ok" || records.len() != 1 {
            return Err("UUID-bearing JUnit root fixture was rejected".to_string());
        }
        let cdata_failure = br#"<testsuite><testcase classname="demo::tests" name="cdata-fails"><failure><![CDATA[assertion details]]></failure></testcase></testsuite>"#;
        records.clear();
        if parse_junit(cdata_failure, &mut records) != "ok" || records.len() != 1 {
            return Err("CDATA-wrapped failure fixture was rejected".to_string());
        }
        let self_closing_pass =
            br#"<testsuite><testcase classname="demo::tests" name="passes" time="0"/></testsuite>"#;
        records.clear();
        if parse_junit(self_closing_pass, &mut records) != "ok" || !records.is_empty() {
            return Err("self-closing passing testcase was not accepted".to_string());
        }
        let mut long_valid = String::from("<testsuite>\n");
        for index in 0..=2048 {
            long_valid.push_str(&format!(
                "<testcase classname=\"demo::tests\" name=\"pass-{index}\"/>\n"
            ));
        }
        long_valid.push_str("</testsuite>");
        records.clear();
        if parse_junit(long_valid.as_bytes(), &mut records) != "ok" || !records.is_empty() {
            return Err("valid long self-closing JUnit fixture was rejected".to_string());
        }
        let commented = br#"<testsuite><!-- <testcase classname="demo" name="commented"><failure/></testcase> --></testsuite>"#;
        if parse_junit(commented, &mut records) != "ok" || !records.is_empty() {
            return Err("commented JUnit testcase was counted".to_string());
        }
        let unknown_root =
            br#"<testsuite secret="raw"><testcase classname="demo" name="fails"/></testsuite>"#;
        if parse_junit(unknown_root, &mut records) != "malformed_report" {
            return Err("unknown JUnit root attribute was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn diagnostics_are_bounded_and_regular() -> Result<(), String> {
        let dir = temp_dir("bounded")?;
        let path = dir.join("test-diagnostics.json");
        let mut records = BTreeSet::new();
        for index in 0..MAX_RECORDS {
            records.insert(Diagnostic {
                package: format!("package-{index}"),
                test: format!("test-{index}"),
                status: "failed".to_string(),
            });
        }
        write_diagnostics(&path, 101, 0, 101, "ok", None, &records)?;
        reject_symlink(&path)?;
        let bytes = fs::read(&path).map_err(|error| format!("read fixture: {error}"))?;
        if bytes.len() > MAX_OUTPUT_BYTES {
            return Err(format!("diagnostics exceeded bound: {}", bytes.len()));
        }
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn oversized_diagnostics_truncate_records_with_closed_status() -> Result<(), String> {
        let dir = temp_dir("truncated")?;
        let path = dir.join("test-diagnostics.json");
        let mut records = BTreeSet::new();
        for index in 0..MAX_RECORDS {
            records.insert(Diagnostic {
                package: format!("package-{index}-{}", "p".repeat(MAX_FIELD_BYTES - 16)),
                test: format!("test-{index}-{}", "t".repeat(MAX_FIELD_BYTES - 12)),
                status: "failed".to_string(),
            });
        }
        write_diagnostics(&path, 101, 0, 101, "ok", None, &records)?;
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(&path).map_err(|error| format!("read fixture: {error}"))?,
        )
        .map_err(|error| format!("parse fixture: {error}"))?;
        if value["core_exit"] != 101
            || value["stream_status"] != "truncated_input"
            || value["parse_reason"] != "input_limit"
            || value["records"]
                .as_array()
                .is_none_or(|items| items.len() >= MAX_RECORDS)
        {
            return Err(
                "oversized diagnostics were not truncated with a closed status".to_string(),
            );
        }
        validate_diagnostics(&path)?;
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn shipped_junit_projection_round_trips_through_schema_validator() -> Result<(), String> {
        let dir = temp_dir("junit-round-trip")?;
        let path = dir.join("test-diagnostics.json");
        let junit = br#"<?xml version="1.0"?><testsuites><testsuite name="demo"><testcase classname="demo::tests" name="fails" time="0.01"><failure/></testcase><testcase classname="demo::tests" name="passes" time="0.01"/></testsuite></testsuites>"#;
        let mut records = BTreeSet::new();
        let status = parse_junit(junit, &mut records);
        if status != "ok" || records.len() != 1 {
            return Err(format!(
                "shipped JUnit fixture did not yield one failed identity: status={} parse_reason={} records={}",
                status.stream_status,
                status.parse_reason.unwrap_or("none"),
                records.len()
            ));
        }
        write_diagnostics(&path, 101, 0, 101, "ok", None, &records)?;
        validate_diagnostics(&path)?;
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn schema_validator_rejects_unknown_fields_and_authority_drift() -> Result<(), String> {
        let dir = temp_dir("schema-negative")?;
        let path = dir.join("test-diagnostics.json");
        fs::write(
            &path,
            br#"{"schema_version":"1.0","runner":"cargo-nextest","runner_version":"0.9.143","nextest_exit":101,"doctest_exit":0,"core_exit":0,"stream_status":"ok","records":[],"unexpected":"value"}"#,
        )
        .map_err(|error| format!("write negative fixture: {error}"))?;
        if validate_diagnostics(&path).is_ok() {
            return Err("unknown schema field was accepted".to_string());
        }
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn schema_validator_accepts_closed_stream_statuses() -> Result<(), String> {
        let dir = temp_dir("stream-statuses")?;
        for status in [
            "ok",
            "missing_report",
            "malformed_report",
            "malformed_record",
            "unexpected_field",
            "hostile_record",
            "truncated_input",
            "record_limit",
            "overlong_record",
        ] {
            let path = dir.join(format!("{status}.json"));
            let records = BTreeSet::new();
            let parse_reason = match status {
                "ok" | "missing_report" => None,
                _ => Some(parser_reason(status)),
            };
            write_diagnostics(&path, 101, 0, 101, status, parse_reason, &records)?;
            validate_diagnostics(&path)?;
        }
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
