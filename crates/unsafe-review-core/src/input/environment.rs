//! Canonical analysis-environment identity (issue #2318).
//!
//! An [`AnalysisEnvironment`] names the configuration envelope a result was
//! computed under: where the facts came from, which workspace and packages
//! were selected, which features, which target triple, which toolchain, and
//! which inputs remain unknown. The same source bytes under different
//! features or targets can expose different unsafe code, so a result that
//! does not name its envelope is not reproducible.
//!
//! PR1 discovers facts read-only: manifests, `rust-toolchain.toml`, and an
//! optional `rustc -vV` probe. It never builds the target, executes build
//! scripts or proc macros, fetches dependencies, or touches the network.
//! Cargo metadata is deliberately not consulted (that would resolve and may
//! fetch dependencies); member expansion reads member manifests directly and
//! records the absence of resolved dependency facts as an unknown input.
//! cfg applicability evaluation belongs to a later slice: `cfgs` starts
//! empty and custom cfgs are reported as unknown, never as inactive.

use crate::sha256_hex_of;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the environment facts came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentSource {
    ExplicitCli,
    RepositoryDefaults,
    EditorHost,
    CiMatrix,
    Imported,
    Unknown,
}

impl EnvironmentSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExplicitCli => "explicit_cli",
            Self::RepositoryDefaults => "repository_defaults",
            Self::EditorHost => "editor_host",
            Self::CiMatrix => "ci_matrix",
            Self::Imported => "imported",
            Self::Unknown => "unknown",
        }
    }
}

/// One Cargo package observed from its manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
    /// Manifest path relative to the workspace root.
    pub manifest: PathBuf,
}

/// One Cargo target observed from the root package manifest and `src/` layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoTargetIdentity {
    pub name: String,
    pub kind: CargoTargetKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoTargetKind {
    Lib,
    Bin,
}

impl CargoTargetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lib => "lib",
            Self::Bin => "bin",
        }
    }
}

/// Which features the analysis envelope selects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureSelection {
    /// Cargo defaults (`default = [...]` applies).
    DefaultFeatures,
    /// `--no-default-features`.
    NoDefaultFeatures,
    /// Explicit `--features` list.
    Explicit(Vec<String>),
    /// `--all-features`.
    AllFeatures,
}

impl FeatureSelection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DefaultFeatures => "default_features",
            Self::NoDefaultFeatures => "no_default_features",
            Self::Explicit(_) => "explicit",
            Self::AllFeatures => "all_features",
        }
    }
}

/// Toolchain facts, or the reason they are unknown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainIdentity {
    /// From `rust-toolchain.toml` and/or a `rustc -vV` probe.
    Known {
        channel: Option<String>,
        version: Option<String>,
        commit: Option<String>,
    },
    Unknown {
        reason: String,
    },
}

/// Why an environment input remains unknown. Unknown never becomes inactive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentLimitationKind {
    /// No manifest was found: discovery did not happen.
    NoManifest,
    /// Full member/dependency facts need Cargo metadata, which PR1 never runs.
    NoCargoMetadata,
    /// Custom cfgs produced only by build scripts remain unknown.
    BuildScriptCfgsUnknown,
    /// cfg applicability evaluation belongs to a later slice.
    CfgEvaluationDeferred,
    /// No toolchain file and no (or a failed) `rustc` probe.
    ToolchainUnknown,
    /// A workspace member pattern matched nothing, or a member lacks a manifest.
    MemberUnresolved,
    /// No explicit target triple was selected; the host triple is observed only.
    TripleUnselected,
}

impl EnvironmentLimitationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoManifest => "no_manifest",
            Self::NoCargoMetadata => "no_cargo_metadata",
            Self::BuildScriptCfgsUnknown => "build_script_cfgs_unknown",
            Self::CfgEvaluationDeferred => "cfg_evaluation_deferred",
            Self::ToolchainUnknown => "toolchain_unknown",
            Self::MemberUnresolved => "member_unresolved",
            Self::TripleUnselected => "triple_unselected",
        }
    }
}

/// One machine-readable unknown input with human detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentLimitation {
    pub kind: EnvironmentLimitationKind,
    pub detail: String,
}

/// One canonical analysis-environment identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisEnvironment {
    pub source: EnvironmentSource,
    /// Workspace root as given (never canonicalized into portable output).
    /// The digest deliberately excludes it: a location is not configuration.
    pub workspace_root: PathBuf,
    pub packages: Vec<PackageIdentity>,
    pub targets: Vec<CargoTargetIdentity>,
    pub features: FeatureSelection,
    /// Feature names known from the root manifest `[features]` table.
    pub known_features: Vec<String>,
    /// The `default = [...]` list from the root manifest, when present.
    pub default_features: Vec<String>,
    /// Observed host triple from the `rustc` probe, when probed successfully.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_triple: Option<String>,
    /// Explicitly selected target triple (CLI only; PR1 default `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_triple: Option<String>,
    /// Known cfg atoms. PR1 discovers none; evaluation is a later slice.
    pub cfgs: BTreeSet<String>,
    pub toolchain: ToolchainIdentity,
    /// Explicitly selected profile (CLI only; PR1 default `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub unknown_inputs: Vec<EnvironmentLimitation>,
    /// Content digest over the canonical encoding below. A feature, target,
    /// or toolchain change alters it, so dependent tasks, receipts, and
    /// cached facts can treat it as an invalidation key.
    pub digest: String,
}

impl AnalysisEnvironment {
    /// Canonical encoding for the digest. Deliberately checkout-independent:
    /// the workspace root is a location, not configuration, so it is
    /// excluded, and absolute paths inside unknown-input details are
    /// relativized to `$WORKSPACE`. Two checkouts of the same configuration
    /// therefore digest identically. Sections carry markers so list
    /// boundaries are unambiguous.
    fn canonical_encoding(&self) -> String {
        let root = self.workspace_root.display().to_string();
        let relativize = |text: &str| text.replace(root.as_str(), "$WORKSPACE");
        let mut out = String::new();
        out.push_str("source:\n");
        out.push_str(self.source.as_str());
        out.push('\n');
        let mut packages: Vec<String> = self
            .packages
            .iter()
            .map(|package| {
                format!(
                    "{}:{}:{}",
                    package.name,
                    package.version,
                    package.manifest.display()
                )
            })
            .collect();
        packages.sort();
        out.push_str("packages:\n");
        for entry in packages {
            out.push_str(&entry);
            out.push('\n');
        }
        let mut targets: Vec<String> = self
            .targets
            .iter()
            .map(|target| format!("{}:{}", target.kind.as_str(), target.name))
            .collect();
        targets.sort();
        out.push_str("targets:\n");
        for entry in targets {
            out.push_str(&entry);
            out.push('\n');
        }
        out.push_str("features:\n");
        out.push_str(self.features.as_str());
        out.push('\n');
        if let FeatureSelection::Explicit(selected) = &self.features {
            let mut selected = selected.clone();
            selected.sort();
            for feature in selected {
                out.push_str(&feature);
                out.push('\n');
            }
        }
        let mut known = self.known_features.clone();
        known.sort();
        out.push_str("known-features:\n");
        for feature in known {
            out.push_str(&feature);
            out.push('\n');
        }
        let mut default = self.default_features.clone();
        default.sort();
        out.push_str("default-features:\n");
        for feature in default {
            out.push_str(&feature);
            out.push('\n');
        }
        out.push_str("triples:\n");
        out.push_str(self.host_triple.as_deref().unwrap_or("-"));
        out.push('\n');
        out.push_str(self.selected_triple.as_deref().unwrap_or("-"));
        out.push('\n');
        out.push_str("cfgs:\n");
        for cfg in &self.cfgs {
            out.push_str(cfg);
            out.push('\n');
        }
        out.push_str("toolchain:\n");
        match &self.toolchain {
            ToolchainIdentity::Known {
                channel,
                version,
                commit,
            } => {
                out.push_str("known\n");
                out.push_str(channel.as_deref().unwrap_or("-"));
                out.push('\n');
                out.push_str(version.as_deref().unwrap_or("-"));
                out.push('\n');
                out.push_str(commit.as_deref().unwrap_or("-"));
                out.push('\n');
            }
            ToolchainIdentity::Unknown { .. } => out.push_str("unknown\n"),
        }
        out.push_str("profile:\n");
        out.push_str(self.profile.as_deref().unwrap_or("-"));
        out.push('\n');
        let mut unknown: Vec<String> = self
            .unknown_inputs
            .iter()
            .map(|limitation| {
                format!(
                    "{}:{}",
                    limitation.kind.as_str(),
                    relativize(&limitation.detail)
                )
            })
            .collect();
        unknown.sort();
        out.push_str("unknown-inputs:\n");
        for entry in unknown {
            out.push_str(&entry);
            out.push('\n');
        }
        out
    }

    fn finish(mut self) -> Self {
        let encoding = self.canonical_encoding();
        self.digest = format!("environment-sha256:{}", sha256_hex_of(encoding.as_bytes()));
        self
    }
}

/// Options bounding environment discovery.
#[derive(Clone, Debug)]
pub struct EnvDiscoverOptions {
    /// Run the read-only `rustc -vV` probe for host triple and version.
    /// `false` yields `ToolchainUnknown` deterministically (used by tests and
    /// by hosts without a toolchain on PATH).
    pub probe_toolchain: bool,
    /// Expand workspace member patterns into member manifests.
    pub expand_members: bool,
    /// Explicit feature selection (CLI `--features` family; PR1 default
    /// `DefaultFeatures`).
    pub features: FeatureSelection,
}

impl Default for EnvDiscoverOptions {
    fn default() -> Self {
        Self {
            probe_toolchain: true,
            expand_members: true,
            features: FeatureSelection::DefaultFeatures,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Manifest {
    #[serde(default)]
    package: Option<PackageSection>,
    #[serde(default)]
    workspace: Option<WorkspaceSection>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    lib: Option<LibSection>,
    #[serde(default)]
    bin: Vec<BinSection>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageSection {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: VersionSpec,
}

/// A package version as written: literal, inherited from
/// `[workspace.package]`, or absent. Cargo permits
/// `version.workspace = true`; rejecting that table would abort discovery
/// (root) or drop the member, so both forms parse.
#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum VersionSpec {
    #[default]
    Missing,
    Literal(String),
    Inherited {
        #[serde(default)]
        workspace: bool,
    },
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceSection {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    package: Option<WorkspacePackageSection>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspacePackageSection {
    #[serde(default)]
    version: Option<String>,
}

/// Resolve a package version: literals as written, `workspace = true`
/// from the workspace root's `[workspace.package] version`, anything else
/// empty (identity keeps the name and manifest; the version is unknown).
fn package_version(package: &PackageSection, workspace_version: Option<&str>) -> String {
    match &package.version {
        VersionSpec::Literal(version) => version.clone(),
        VersionSpec::Inherited { workspace: true } => {
            workspace_version.unwrap_or_default().to_string()
        }
        VersionSpec::Inherited { workspace: false } | VersionSpec::Missing => String::new(),
    }
}

#[derive(Debug, Default, Deserialize)]
struct LibSection {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BinSection {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolchainFile {
    #[serde(default)]
    toolchain: Option<ToolchainSection>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolchainSection {
    #[serde(default)]
    channel: Option<String>,
}

/// Targets declared by one package manifest plus its `src/` layout:
/// the explicit `[lib]` name (or the package name with `-` as `_`) when
/// `src/lib.rs` exists, explicit `[[bin]]` names, the conventional
/// `src/main.rs` fallback, and every `src/bin/*.rs` file.
fn targets_in_manifest(manifest: &Manifest, manifest_dir: &Path) -> Vec<CargoTargetIdentity> {
    let mut targets = Vec::new();
    let lib_name = manifest
        .lib
        .as_ref()
        .and_then(|lib| lib.name.clone())
        .unwrap_or_else(|| {
            manifest
                .package
                .as_ref()
                .map(|package| package.name.replace('-', "_"))
                .unwrap_or_default()
        });
    if !lib_name.is_empty() && manifest_dir.join("src").join("lib.rs").exists() {
        targets.push(CargoTargetIdentity {
            name: lib_name,
            kind: CargoTargetKind::Lib,
        });
    }
    for bin in &manifest.bin {
        if let Some(name) = &bin.name {
            targets.push(CargoTargetIdentity {
                name: name.clone(),
                kind: CargoTargetKind::Bin,
            });
        }
    }
    // Cargo discovers `src/main.rs` as a same-named binary alongside any
    // explicit `[[bin]]` targets (unless `autobins = false`): skip it only
    // when that exact name is already present.
    if manifest_dir.join("src").join("main.rs").exists() {
        let fallback = manifest
            .package
            .as_ref()
            .map(|package| package.name.clone())
            .unwrap_or_default();
        if !fallback.is_empty()
            && !targets
                .iter()
                .any(|target| target.kind == CargoTargetKind::Bin && target.name == fallback)
        {
            targets.push(CargoTargetIdentity {
                name: fallback,
                kind: CargoTargetKind::Bin,
            });
        }
    }
    // Both binary layouts: `src/bin/<name>.rs` files and
    // `src/bin/<name>/main.rs` multi-file targets.
    if let Ok(entries) = std::fs::read_dir(manifest_dir.join("src").join("bin")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(ToString::to_string)
            } else if path.is_dir() && path.join("main.rs").is_file() {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(ToString::to_string)
            } else {
                None
            };
            if let Some(bin_name) = name
                && !targets.iter().any(|target| target.name == bin_name)
            {
                targets.push(CargoTargetIdentity {
                    name: bin_name,
                    kind: CargoTargetKind::Bin,
                });
            }
        }
    }
    targets
}

/// Discover the analysis environment for `start`, walking up to the
/// workspace root. Fails closed when no manifest is found.
pub fn discover_environment(
    start: &Path,
    source: EnvironmentSource,
    options: &EnvDiscoverOptions,
) -> Result<AnalysisEnvironment, String> {
    // Absolutize and lexically normalize once: a relative start
    // (notably the CLI default `.`) would otherwise collapse the
    // workspace-root walk (`Path::pop` on `.` yields an empty path), and
    // `..` segments would make the upward walk check the caller's
    // ancestors instead of the target's (lexical `pop` never resolves
    // `..`). Both break root detection and member matching.
    let joined = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| format!("resolve current dir for {} failed: {err}", start.display()))?
            .join(start)
    };
    let start_abs = normalize_lexical(&joined);
    let manifest_dir = find_manifest_dir(&start_abs).ok_or_else(|| {
        format!(
            "no Cargo.toml found above {}; environment discovery needs a manifest",
            start.display()
        )
    })?;
    let manifest_path = manifest_dir.join("Cargo.toml");
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("read {} failed: {err}", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&manifest_text)
        .map_err(|err| format!("parse {} failed: {err}", manifest_path.display()))?;

    // The workspace root is the nearest ancestor with a `[workspace]`
    // table (Cargo's own search rule); otherwise the manifest dir itself
    // is the root package. An outer workspace above never wins: discovery
    // would otherwise expand members from the wrong root.
    let workspace_root = workspace_root_above(&manifest_dir);
    let workspace_manifest =
        std::fs::read_to_string(workspace_root.join("Cargo.toml")).map_err(|err| {
            format!(
                "read {} failed: {err}",
                workspace_root.join("Cargo.toml").display()
            )
        })?;
    let workspace_parsed: Manifest = toml::from_str(&workspace_manifest).map_err(|err| {
        format!(
            "parse {} failed: {err}",
            workspace_root.join("Cargo.toml").display()
        )
    })?;

    let mut unknown_inputs = vec![
        EnvironmentLimitation {
            kind: EnvironmentLimitationKind::NoCargoMetadata,
            detail: "member and dependency facts come from manifests only; \
                     resolved dependency versions need Cargo metadata, which is never run"
                .to_string(),
        },
        EnvironmentLimitation {
            kind: EnvironmentLimitationKind::BuildScriptCfgsUnknown,
            detail: "custom cfgs produced only by build scripts remain unknown unless \
                     supplied explicitly or imported from a trusted receipt"
                .to_string(),
        },
        EnvironmentLimitation {
            kind: EnvironmentLimitationKind::CfgEvaluationDeferred,
            detail: "cfg applicability evaluation belongs to a later slice; no cfg \
                     atom is active or inactive yet"
                .to_string(),
        },
    ];

    let workspace_version: Option<String> = workspace_parsed
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.package.as_ref())
        .and_then(|package| package.version.clone());

    let mut packages = Vec::new();
    if let Some(package) = &manifest.package {
        let rel = manifest_dir
            .strip_prefix(&workspace_root)
            .unwrap_or(&manifest_dir)
            .join("Cargo.toml");
        packages.push(PackageIdentity {
            name: package.name.clone(),
            version: package_version(package, workspace_version.as_deref()),
            manifest: rel,
        });
    }
    if options.expand_members
        && let Some(workspace) = &workspace_parsed.workspace
    {
        expand_members(
            &workspace_root,
            &workspace.members,
            &workspace.exclude,
            workspace_version.as_deref(),
            &mut packages,
            &mut unknown_inputs,
        );
    }
    packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.manifest.cmp(&right.manifest))
    });

    // Targets come from every discovered package manifest plus `src/`
    // layout (explicit entries and conventional files only; no build, no
    // metadata). A virtual workspace root contributes no targets itself;
    // its members do. Unreadable member manifests were already recorded as
    // `MemberUnresolved` limitations during expansion, so a re-read failure
    // here (a file vanishing mid-discovery) only skips that package.
    let mut targets = Vec::new();
    for package in &packages {
        let member_dir = workspace_root.join(&package.manifest);
        let Some(member_dir) = member_dir.parent() else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(member_dir.join("Cargo.toml")) else {
            continue;
        };
        let Ok(parsed): Result<Manifest, _> = toml::from_str(&text) else {
            continue;
        };
        if parsed.package.is_none() {
            continue;
        }
        for target in targets_in_manifest(&parsed, member_dir) {
            if !targets.iter().any(|existing: &CargoTargetIdentity| {
                existing.kind == target.kind && existing.name == target.name
            }) {
                targets.push(target);
            }
        }
    }
    targets.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then(left.name.cmp(&right.name))
    });

    let known_features: Vec<String> = manifest.features.keys().cloned().collect();
    let default_features: Vec<String> = manifest
        .features
        .get("default")
        .cloned()
        .unwrap_or_default();

    let toolchain_channel = read_toolchain_channel(&workspace_root);
    let (host_triple, toolchain_version, toolchain_commit, toolchain_unknown) =
        if options.probe_toolchain {
            probe_rustc()
        } else {
            (
                None,
                None,
                None,
                Some("toolchain probe disabled by caller".to_string()),
            )
        };
    let toolchain = match (toolchain_channel, toolchain_version, toolchain_unknown) {
        (None, None, Some(reason)) => ToolchainIdentity::Unknown { reason },
        (channel, version, _) => ToolchainIdentity::Known {
            channel,
            version,
            commit: toolchain_commit,
        },
    };
    if matches!(toolchain, ToolchainIdentity::Unknown { .. }) {
        unknown_inputs.push(EnvironmentLimitation {
            kind: EnvironmentLimitationKind::ToolchainUnknown,
            detail: "no rust-toolchain.toml and no successful rustc probe; \
                     toolchain-sensitive facts (target features, editions) are unevaluated"
                .to_string(),
        });
    }
    unknown_inputs.push(EnvironmentLimitation {
        kind: EnvironmentLimitationKind::TripleUnselected,
        detail: "no explicit target triple selected; the host triple (when probed) \
                 is observed context, not a selection"
            .to_string(),
    });

    Ok(AnalysisEnvironment {
        source,
        workspace_root: workspace_root.clone(),
        packages,
        targets,
        features: options.features.clone(),
        known_features,
        default_features,
        host_triple,
        selected_triple: None,
        cfgs: BTreeSet::new(),
        toolchain,
        profile: None,
        unknown_inputs,
        digest: String::new(),
    }
    .finish())
}

/// Lexically normalize a path: drop `.`, resolve `..` against the
/// preceding normal component. No filesystem access, so symlinks are left
/// alone; callers pass the result to existence checks that resolve the
/// remainder through the OS.
fn normalize_lexical(path: &Path) -> PathBuf {
    use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Prefix(prefix) => out.push(prefix.as_os_str()),
            RootDir => out.push(std::path::MAIN_SEPARATOR_STR),
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            Normal(part) => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

/// Walk up from `start` to the nearest ancestor (or self) containing Cargo.toml.
fn find_manifest_dir(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if current.join("Cargo.toml").is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Nearest ancestor (or self) whose manifest carries `[workspace]`;
/// else `manifest_dir`. Cargo resolves the same way: an outer workspace
/// above never wins over the workspace that directly contains the start.
fn workspace_root_above(manifest_dir: &Path) -> PathBuf {
    let mut current = manifest_dir.to_path_buf();
    loop {
        let candidate = current.join("Cargo.toml");
        if let Ok(text) = std::fs::read_to_string(&candidate)
            && let Ok(parsed) = toml::from_str::<Manifest>(&text)
            && parsed.workspace.is_some()
        {
            return current;
        }
        if !current.pop() {
            return manifest_dir.to_path_buf();
        }
    }
}

/// A literal (non-glob) workspace member or exclude entry stays comparable
/// after normalization; glob entries compile separately.
fn is_literal_pattern(pattern: &str) -> bool {
    !pattern.contains(['*', '?', '['])
}

/// Expand workspace member patterns into member package identities.
///
/// Literal entries resolve directly at any depth (an explicitly listed
/// member wins over `exclude`, matching Cargo). Glob entries use
/// single-component `*` semantics (`literal_separator`): `crates/*` matches
/// `crates/alpha` but not `crates/group/alpha`; deeper layouts need deeper
/// patterns. `exclude` prunes glob matches, including whole subtrees.
/// Entries that resolve to no manifest become `MemberUnresolved`
/// limitations; unexpanded globs are never silently treated as empty
/// selections.
fn expand_members(
    workspace_root: &Path,
    patterns: &[String],
    exclude: &[String],
    workspace_version: Option<&str>,
    packages: &mut Vec<PackageIdentity>,
    unknown_inputs: &mut Vec<EnvironmentLimitation>,
) {
    use globset::{GlobBuilder, GlobSetBuilder};
    // Literal members first: direct manifest check, any depth, exclude
    // does not apply to explicitly listed members.
    let mut glob_patterns: Vec<&String> = Vec::new();
    for pattern in patterns {
        if !is_literal_pattern(pattern) {
            glob_patterns.push(pattern);
            continue;
        }
        let manifest_path = workspace_root.join(pattern).join("Cargo.toml");
        match read_package_manifest(&manifest_path) {
            Ok(Some(package)) => push_package(
                packages,
                &package,
                workspace_version,
                manifest_path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&manifest_path)
                    .to_path_buf(),
            ),
            Ok(None) => unknown_inputs.push(EnvironmentLimitation {
                kind: EnvironmentLimitationKind::MemberUnresolved,
                detail: format!(
                    "workspace member entry `{pattern}` has a manifest without a [package] table"
                ),
            }),
            Err(detail) => unknown_inputs.push(EnvironmentLimitation {
                kind: EnvironmentLimitationKind::MemberUnresolved,
                detail: format!("workspace member entry `{pattern}`: {detail}"),
            }),
        }
    }
    // Per-pattern glob matchers: a glob with zero hits becomes a
    // `MemberUnresolved` limitation rather than a silently narrowed
    // selection.
    let mut pattern_matchers: Vec<(&String, globset::GlobSet)> = Vec::new();
    for pattern in &glob_patterns {
        let mut builder = GlobSetBuilder::new();
        match GlobBuilder::new(pattern).literal_separator(true).build() {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(_) => {
                unknown_inputs.push(EnvironmentLimitation {
                    kind: EnvironmentLimitationKind::MemberUnresolved,
                    detail: format!(
                        "workspace member pattern `{pattern}` is not valid glob syntax"
                    ),
                });
                continue;
            }
        }
        match builder.build() {
            Ok(matcher) => pattern_matchers.push((pattern, matcher)),
            Err(err) => unknown_inputs.push(EnvironmentLimitation {
                kind: EnvironmentLimitationKind::MemberUnresolved,
                detail: format!("workspace member pattern `{pattern}` failed to compile: {err}"),
            }),
        }
    }
    // Exclude matchers: glob excludes match like member globs; literal
    // excludes prune the whole subtree beneath them.
    let mut exclude_builder = GlobSetBuilder::new();
    let mut exclude_literals: Vec<String> = Vec::new();
    for pattern in exclude {
        if is_literal_pattern(pattern) {
            exclude_literals.push(pattern.clone());
            continue;
        }
        if let Ok(glob) = GlobBuilder::new(pattern).literal_separator(true).build() {
            exclude_builder.add(glob);
        }
    }
    let exclude_globs = exclude_builder
        .build()
        .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap_or_default());
    let is_excluded = |rel: &Path| {
        if exclude_globs.is_match(rel) {
            return true;
        }
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        exclude_literals
            .iter()
            .any(|literal| rel_str == *literal || rel_str.starts_with(&format!("{literal}/")))
    };
    if pattern_matchers.is_empty() {
        return;
    }
    let matchers: Vec<&globset::GlobSet> = pattern_matchers
        .iter()
        .map(|(_, matcher)| matcher)
        .collect();
    let is_match = |rel: &Path| matchers.iter().any(|matcher| matcher.is_match(rel));
    // Candidate member dirs: hidden directories (`.git`, `.github`),
    // build outputs (`target`), and vendored trees (`node_modules`) never
    // contain members; anything deeper than three nesting levels needs an
    // explicit literal entry instead of the walk.
    let mut manifest_paths = BTreeSet::new();
    let mut stack = vec![workspace_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with('.') || name == "target" || name == "node_modules"
                })
            {
                continue;
            }
            let Ok(rel) = path.strip_prefix(workspace_root) else {
                continue;
            };
            if is_excluded(rel) {
                continue;
            }
            if path.join("Cargo.toml").is_file() {
                if is_match(rel) {
                    manifest_paths.insert(path.join("Cargo.toml"));
                }
                continue;
            }
            if rel.components().count() < 4 {
                stack.push(path);
            }
        }
    }
    // Per-pattern coverage: entries with zero manifest hits stay visible.
    for (pattern, matcher) in &pattern_matchers {
        let covered = manifest_paths.iter().any(|manifest_path| {
            manifest_path
                .parent()
                .and_then(|dir| dir.strip_prefix(workspace_root).ok())
                .is_some_and(|rel| matcher.is_match(rel))
        });
        if !covered {
            unknown_inputs.push(EnvironmentLimitation {
                kind: EnvironmentLimitationKind::MemberUnresolved,
                detail: format!("workspace member entry `{pattern}` matched no manifests"),
            });
        }
    }
    for manifest_path in manifest_paths {
        let rel = manifest_path
            .strip_prefix(workspace_root)
            .unwrap_or(&manifest_path)
            .to_path_buf();
        match read_package_manifest(&manifest_path) {
            Ok(Some(package)) => push_package(packages, &package, workspace_version, rel),
            Ok(None) => unknown_inputs.push(EnvironmentLimitation {
                kind: EnvironmentLimitationKind::MemberUnresolved,
                detail: format!(
                    "workspace member manifest {} has no [package] table",
                    manifest_path.display()
                ),
            }),
            Err(detail) => unknown_inputs.push(EnvironmentLimitation {
                kind: EnvironmentLimitationKind::MemberUnresolved,
                detail,
            }),
        }
    }
}

/// Read one member manifest: the parsed `[package]` table, or `None` when
/// the manifest parses but declares no package. I/O and parse failures
/// return a human detail for the `MemberUnresolved` limitation.
fn read_package_manifest(manifest_path: &Path) -> Result<Option<PackageSection>, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|err| format!("read {} failed: {err}", manifest_path.display()))?;
    let parsed: Manifest = toml::from_str(&text)
        .map_err(|err| format!("parse {} failed: {err}", manifest_path.display()))?;
    Ok(parsed.package)
}

/// Push one package identity unless the same manifest is already recorded.
fn push_package(
    packages: &mut Vec<PackageIdentity>,
    package: &PackageSection,
    workspace_version: Option<&str>,
    rel: PathBuf,
) {
    if packages.iter().any(|existing| existing.manifest == rel) {
        return;
    }
    packages.push(PackageIdentity {
        name: package.name.clone(),
        version: package_version(package, workspace_version),
        manifest: rel,
    });
}

/// Channel from the rustup toolchain file, when present. Rustup accepts
/// both `rust-toolchain.toml` and `rust-toolchain`, and the latter may be
/// TOML (`[toolchain] channel = ...`) or a plaintext channel name alone.
fn read_toolchain_channel(workspace_root: &Path) -> Option<String> {
    for file in ["rust-toolchain.toml", "rust-toolchain"] {
        let Ok(text) = std::fs::read_to_string(workspace_root.join(file)) else {
            continue;
        };
        if let Ok(parsed) = toml::from_str::<ToolchainFile>(&text)
            && let Some(section) = parsed.toolchain
            && let Some(channel) = section.channel
        {
            return Some(channel);
        }
        if file == "rust-toolchain" {
            let trimmed = text.trim();
            if !trimmed.is_empty() && !trimmed.contains('[') && !trimmed.contains('\n') {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Read-only `rustc -vV` probe: host triple, version, commit.
///
/// This executes the compiler driver with a version query only: no build,
/// no code execution, no network. Failure yields an unknown reason, never a
/// panic or an assumed toolchain.
fn probe_rustc() -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let output = Command::new("rustc").arg("-vV").output();
    let Ok(output) = output else {
        return (
            None,
            None,
            None,
            Some("rustc not found on PATH".to_string()),
        );
    };
    if !output.status.success() {
        return (
            None,
            None,
            None,
            Some(format!(
                "rustc -vV failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut host = None;
    let mut version = None;
    let mut commit = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("host: ") {
            host = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("release: ") {
            version = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("commit-hash: ") {
            commit = Some(value.trim().to_string());
        }
    }
    (host, version, commit, None)
}

/// One-screen human summary: envelope source, workspace, selection, unknown
/// inputs, and digest. Quiet results name the envelope that was quiet.
pub fn render_environment_human(env: &AnalysisEnvironment) -> String {
    let mut out = String::new();
    out.push_str(&format!("environment-source: {}\n", env.source.as_str()));
    out.push_str(&format!(
        "workspace-root: {}\n",
        env.workspace_root.display()
    ));
    out.push_str(&format!("packages: {}\n", env.packages.len()));
    for package in env.packages.iter().take(20) {
        out.push_str(&format!(
            "  {} {} ({})\n",
            package.name,
            package.version,
            package.manifest.display()
        ));
    }
    out.push_str(&format!("targets: {}\n", env.targets.len()));
    for target in &env.targets {
        out.push_str(&format!("  {} [{}]\n", target.name, target.kind.as_str()));
    }
    out.push_str(&format!("features: {}\n", env.features.as_str()));
    if let FeatureSelection::Explicit(selected) = &env.features {
        out.push_str(&format!("  selected: {}\n", selected.join(", ")));
    }
    if !env.known_features.is_empty() {
        out.push_str(&format!("  known: {}\n", env.known_features.join(", ")));
    }
    if !env.default_features.is_empty() {
        out.push_str(&format!("  default: {}\n", env.default_features.join(", ")));
    }
    if let Some(host) = &env.host_triple {
        out.push_str(&format!("host-triple: {host}\n"));
    }
    if let Some(selected) = &env.selected_triple {
        out.push_str(&format!("target-triple: {selected}\n"));
    }
    match &env.toolchain {
        ToolchainIdentity::Known {
            channel,
            version,
            commit,
        } => {
            out.push_str(&format!(
                "toolchain: {} {} {}\n",
                channel.as_deref().unwrap_or("?"),
                version.as_deref().unwrap_or("?"),
                commit.as_deref().unwrap_or("?")
            ));
        }
        ToolchainIdentity::Unknown { reason } => {
            out.push_str(&format!("toolchain: unknown ({reason})\n"));
        }
    }
    out.push_str(&format!("unknown-inputs: {}\n", env.unknown_inputs.len()));
    for limitation in &env.unknown_inputs {
        out.push_str(&format!(
            "  [{}] {}\n",
            limitation.kind.as_str(),
            limitation.detail
        ));
    }
    out.push_str(&format!("digest: {}\n", env.digest));
    out
}

/// Canonical JSON projection of the same [`AnalysisEnvironment`].
pub fn render_environment_json(env: &AnalysisEnvironment) -> Result<String, String> {
    serde_json::to_string_pretty(env).map_err(|err| format!("serialize environment failed: {err}"))
}
