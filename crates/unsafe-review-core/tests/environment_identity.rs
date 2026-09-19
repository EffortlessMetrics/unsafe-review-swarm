//! Analysis-environment discovery against disposable manifests
//! (issue #2318 PR1).
//!
//! No test builds, executes, or fetches anything: discovery reads manifests,
//! `rust-toolchain.toml`, and (optionally) a `rustc -vV` probe only.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unsafe_review_core::{
    CargoTargetKind, EnvDiscoverOptions, EnvironmentLimitationKind, EnvironmentSource,
    FeatureSelection, ToolchainIdentity, discover_environment, render_environment_human,
    render_environment_json,
};

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn write(dir: &Path, rel: &str, content: &str) -> Result<(), String> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create parent dirs failed: {err}"))?;
    }
    fs::write(&path, content).map_err(|err| format!("write fixture file failed: {err}"))?;
    Ok(())
}

/// Express an absolute fixture dir as a relative path from the process
/// working directory (`..` segments when the fixture is outside the cwd).
/// Keeps the relative-start regression test hermetic: no fixture is written
/// into the repository and no working directory is changed.
fn cwd_relative(abs: &Path) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| format!("current dir failed: {err}"))?;
    if let Ok(rel) = abs.strip_prefix(&cwd) {
        return Ok(rel.to_path_buf());
    }
    let mut cwd_rest: Vec<_> = cwd.components().collect();
    let mut abs_rest: Vec<_> = abs.components().collect();
    while !cwd_rest.is_empty() && !abs_rest.is_empty() && cwd_rest[0] == abs_rest[0] {
        cwd_rest.remove(0);
        abs_rest.remove(0);
    }
    let mut rel = PathBuf::new();
    for _ in &cwd_rest {
        rel.push("..");
    }
    for component in abs_rest {
        rel.push(component);
    }
    Ok(rel)
}

fn options_no_probe(features: FeatureSelection) -> EnvDiscoverOptions {
    EnvDiscoverOptions {
        probe_toolchain: false,
        expand_members: true,
        features,
    }
}

const PACKAGE_MANIFEST: &str = r#"[package]
name = "demo-crate"
version = "0.1.0"
edition = "2021"

[features]
default = ["checked"]
checked = []
fast = []

[lib]
name = "demo_crate"
"#;

/// A single-crate fixture with features, a lib target, and a toolchain file.
fn fixture_crate(prefix: &str) -> Result<PathBuf, String> {
    let dir = unique_dir(prefix);
    write(&dir, "Cargo.toml", PACKAGE_MANIFEST)?;
    write(&dir, "src/lib.rs", "pub fn demo() {}\n")?;
    write(
        &dir,
        "rust-toolchain.toml",
        "[toolchain]\nchannel = \"1.90.0\"\n",
    )?;
    Ok(dir)
}

#[test]
fn discovers_package_features_targets_and_toolchain_channel() -> Result<(), String> {
    let dir = fixture_crate("env-basic")?;
    let env = discover_environment(
        &dir,
        EnvironmentSource::RepositoryDefaults,
        &options_no_probe(FeatureSelection::DefaultFeatures),
    )?;
    if env.source != EnvironmentSource::RepositoryDefaults {
        return Err(format!("source must round-trip, got {:?}", env.source));
    }
    if env.packages.len() != 1 {
        return Err(format!("expected one package, got {:?}", env.packages));
    }
    if env.packages[0].name != "demo-crate" || env.packages[0].version != "0.1.0" {
        return Err(format!("package identity wrong: {:?}", env.packages[0]));
    }
    for feature in ["checked", "fast", "default"] {
        if !env.known_features.iter().any(|known| known == feature) {
            return Err(format!(
                "known features must include `{feature}`: {:?}",
                env.known_features
            ));
        }
    }
    if env.default_features != vec!["checked".to_string()] {
        return Err(format!(
            "default features wrong: {:?}",
            env.default_features
        ));
    }
    if !env.targets.iter().any(|target| target.name == "demo_crate") {
        return Err(format!("lib target must be discovered: {:?}", env.targets));
    }
    match &env.toolchain {
        ToolchainIdentity::Known { channel, .. } if channel.as_deref() == Some("1.90.0") => {}
        other => {
            return Err(format!(
                "toolchain channel must come from rust-toolchain.toml: {other:?}"
            ));
        }
    }
    if env.features != FeatureSelection::DefaultFeatures {
        return Err(format!("selection must round-trip: {:?}", env.features));
    }
    // Unknown inputs stay visible: no metadata, build-script cfgs, cfg eval.
    for kind in [
        EnvironmentLimitationKind::NoCargoMetadata,
        EnvironmentLimitationKind::BuildScriptCfgsUnknown,
        EnvironmentLimitationKind::CfgEvaluationDeferred,
    ] {
        if !env
            .unknown_inputs
            .iter()
            .any(|limitation| limitation.kind == kind)
        {
            return Err(format!(
                "unknown inputs must include {kind:?}: {:?}",
                env.unknown_inputs
            ));
        }
    }
    if !env.digest.starts_with("environment-sha256:") {
        return Err(format!("digest must carry the scheme: {}", env.digest));
    }
    Ok(())
}

#[test]
fn feature_selections_are_distinguishable_by_digest() -> Result<(), String> {
    let dir = fixture_crate("env-features")?;
    let discover = |features| {
        discover_environment(
            &dir,
            EnvironmentSource::ExplicitCli,
            &options_no_probe(features),
        )
    };
    let default = discover(FeatureSelection::DefaultFeatures)?;
    let no_default = discover(FeatureSelection::NoDefaultFeatures)?;
    let explicit = discover(FeatureSelection::Explicit(vec!["fast".to_string()]))?;
    let all = discover(FeatureSelection::AllFeatures)?;
    let digests = [
        default.digest.clone(),
        no_default.digest.clone(),
        explicit.digest.clone(),
        all.digest.clone(),
    ];
    let mut sorted = digests.to_vec();
    sorted.sort();
    sorted.dedup();
    if sorted.len() != 4 {
        return Err(format!(
            "all four selections must digest distinctly: {digests:?}"
        ));
    }
    // Same selection rediscovers to the same digest.
    let again = discover(FeatureSelection::Explicit(vec!["fast".to_string()]))?;
    if again.digest != explicit.digest {
        return Err("identical selections must digest identically".to_string());
    }
    Ok(())
}

#[test]
fn missing_manifest_fails_closed() -> Result<(), String> {
    let dir = unique_dir("env-nomanifest");
    fs::create_dir_all(&dir).map_err(|err| format!("create plain dir failed: {err}"))?;
    match discover_environment(
        &dir,
        EnvironmentSource::RepositoryDefaults,
        &EnvDiscoverOptions::default(),
    ) {
        Err(err) if err.contains("no Cargo.toml") => Ok(()),
        Err(err) => Err(format!("missing-manifest error must name the cause: {err}")),
        Ok(_) => Err("discovery without a manifest must fail".to_string()),
    }
}

#[test]
fn disabled_probe_yields_unknown_toolchain_not_failure() -> Result<(), String> {
    let dir = unique_dir("env-notoolchain");
    write(
        &dir,
        "Cargo.toml",
        "[package]\nname = \"bare\"\nversion = \"0.0.0\"\n",
    )?;
    let env = discover_environment(
        &dir,
        EnvironmentSource::RepositoryDefaults,
        &options_no_probe(FeatureSelection::DefaultFeatures),
    )?;
    match &env.toolchain {
        ToolchainIdentity::Unknown { .. } => {}
        known => {
            return Err(format!(
                "disabled probe must yield unknown toolchain: {known:?}"
            ));
        }
    }
    if !env
        .unknown_inputs
        .iter()
        .any(|limitation| limitation.kind == EnvironmentLimitationKind::ToolchainUnknown)
    {
        return Err("unknown toolchain must be listed as an unknown input".to_string());
    }
    Ok(())
}

#[test]
fn workspace_members_expand_and_missing_members_are_limitations() -> Result<(), String> {
    let dir = unique_dir("env-workspace");
    write(
        &dir,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\", \"missing-dir\"]\n",
    )?;
    write(
        &dir,
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.2.0\"\n",
    )?;
    // crates/beta matches the glob but has no manifest: unresolved, not silent.
    fs::create_dir_all(dir.join("crates/beta"))
        .map_err(|err| format!("create beta dir failed: {err}"))?;

    let env = discover_environment(
        &dir,
        EnvironmentSource::RepositoryDefaults,
        &options_no_probe(FeatureSelection::DefaultFeatures),
    )?;
    if !env.packages.iter().any(|package| package.name == "alpha") {
        return Err(format!("member package must expand: {:?}", env.packages));
    }
    if !env
        .unknown_inputs
        .iter()
        .any(|limitation| limitation.kind == EnvironmentLimitationKind::MemberUnresolved)
    {
        return Err(format!(
            "unresolvable member entries must be limitations: {:?}",
            env.unknown_inputs
        ));
    }
    Ok(())
}

#[test]
fn relative_start_discovers_workspace_members() -> Result<(), String> {
    let dir = unique_dir("env-relative");
    write(
        &dir,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    )?;
    write(
        &dir,
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    )?;
    write(&dir, "crates/alpha/src/lib.rs", "pub fn a() {}\n")?;
    // A relative start (the CLI default `.`) must not collapse the
    // workspace-root walk into an empty path with zero members.
    let relative = cwd_relative(&dir)?;
    let env = discover_environment(
        &relative,
        EnvironmentSource::RepositoryDefaults,
        &options_no_probe(FeatureSelection::DefaultFeatures),
    )?;
    if !env.packages.iter().any(|package| package.name == "alpha") {
        return Err(format!(
            "relative start must expand members: {:?}",
            env.packages
        ));
    }
    if !env
        .targets
        .iter()
        .any(|target| target.name == "alpha" && target.kind == CargoTargetKind::Lib)
    {
        return Err(format!(
            "relative start must discover member targets: {:?}",
            env.targets
        ));
    }
    Ok(())
}

#[test]
fn workspace_member_targets_are_discovered() -> Result<(), String> {
    let dir = unique_dir("env-member-targets");
    write(
        &dir,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    )?;
    write(
        &dir,
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    )?;
    write(&dir, "crates/alpha/src/lib.rs", "pub fn a() {}\n")?;
    write(
        &dir,
        "crates/beta/Cargo.toml",
        "[package]\nname = \"beta-bin\"\nversion = \"0.1.0\"\n",
    )?;
    write(&dir, "crates/beta/src/main.rs", "fn main() {}\n")?;
    let env = discover_environment(
        &dir,
        EnvironmentSource::RepositoryDefaults,
        &options_no_probe(FeatureSelection::DefaultFeatures),
    )?;
    // The virtual root contributes no targets itself; members do.
    if !env
        .targets
        .iter()
        .any(|target| target.name == "alpha" && target.kind == CargoTargetKind::Lib)
    {
        return Err(format!(
            "member lib target must be discovered: {:?}",
            env.targets
        ));
    }
    if !env
        .targets
        .iter()
        .any(|target| target.name == "beta-bin" && target.kind == CargoTargetKind::Bin)
    {
        return Err(format!(
            "member bin target must be discovered: {:?}",
            env.targets
        ));
    }
    Ok(())
}

#[test]
fn projections_name_envelope_selection_unknowns_and_digest() -> Result<(), String> {
    let dir = fixture_crate("env-render")?;
    let env = discover_environment(
        &dir,
        EnvironmentSource::ExplicitCli,
        &options_no_probe(FeatureSelection::Explicit(vec!["fast".to_string()])),
    )?;
    let human = render_environment_human(&env);
    for needle in [
        "environment-source: explicit_cli",
        "demo-crate 0.1.0",
        "features: explicit",
        "fast",
        "unknown-inputs: ",
        "digest: environment-sha256:",
    ] {
        if !human.contains(needle) {
            return Err(format!("human summary must contain `{needle}`:\n{human}"));
        }
    }
    let json = render_environment_json(&env)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|err| format!("json must parse: {err}"))?;
    if parsed["source"] != "explicit_cli" {
        return Err(format!("json source must round-trip: {json}"));
    }
    if parsed["features"]["explicit"] != serde_json::json!(["fast"]) {
        return Err(format!(
            "json features must carry the explicit list: {json}"
        ));
    }
    Ok(())
}
