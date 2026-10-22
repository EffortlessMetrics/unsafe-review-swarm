//! Bounded `cfg` applicability evaluation (issue #2318 PR2).
//!
//! An [`Applicability`] states whether a `#[cfg]`-gated item is active under
//! a selected configuration envelope: which features are on, which target
//! triple was selected, and which toolchain facts are known. The same source
//! under different features or targets exposes different unsafe code, so a
//! card that cannot name its envelope is not a reproducible result.
//!
//! Scope notes, all deliberate:
//! - `#[cfg(...)]` gates an item directly. `#[cfg_attr(pred, cfg(...))]`
//!   gates conditionally: when `pred` holds the inner `cfg(...)` must also
//!   hold; when `pred` is false the item is ungated; when `pred` is unknown
//!   the gate is unknown. Other `cfg_attr` shapes are unsupported, never
//!   silently active or inactive.
//! - Same-file scope is structural: parsed syntax ranges with ancestor
//!   containment decide which attributes enclose a site. Fixed line windows
//!   survive only as a bounded fallback diagnostic, never as authority.
//! - Parent-file module gates (`mod child;` gated elsewhere) and macro
//!   expansion are not resolved here; they are reported as explicit
//!   unavailable states with reasons.
//! - The `test` atom always evaluates to unknown: the build-free analyzer
//!   does not know the compilation profile, and guessing `false` would mark
//!   every test-region card inactive. Unknown never becomes inactive.
//! - Target atoms (`target_arch`, `target_os`, `unix`, ...) evaluate only
//!   against an explicitly selected triple. The observed host triple is
//!   context, not a selection.
//! - Custom, build-script, and `target_feature` atoms stay unknown unless
//!   supplied explicitly: they are not derivable from static inputs.
//! - Unparseable expressions are `configuration_expression_unsupported` or
//!   `configuration_parse_failed`, never a silent active or inactive.

use std::collections::{BTreeMap, BTreeSet};

/// One `cfg` predicate: `name` or `name = "value"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CfgAtom {
    pub name: String,
    pub value: Option<String>,
}

/// A parsed `cfg` expression: atoms composed with `not`/`all`/`any`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CfgExpr {
    Atom(CfgAtom),
    Not(Box<CfgExpr>),
    All(Vec<CfgExpr>),
    Any(Vec<CfgExpr>),
}

/// Kleene verdict for one expression: unknown propagates through `all`
/// (`false` dominates) and `any` (`true` dominates).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CfgVerdict {
    True,
    False,
    Unknown,
}

/// Where a gated item stands relative to the selected envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    ActiveInSelectedEnvironment,
    InactiveInSelectedEnvironment,
    ActiveInOtherKnownEnvironment,
    ConfigurationUnknown,
    ConfigurationExpressionUnsupported,
    ConfigurationParseFailed,
    ParentModuleApplicabilityUnavailable,
    MacroExpansionUnavailable,
    AlternativeEnvironmentNotAnalyzed,
}

impl Applicability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ActiveInSelectedEnvironment => "active_in_selected_environment",
            Self::InactiveInSelectedEnvironment => "inactive_in_selected_environment",
            Self::ActiveInOtherKnownEnvironment => "active_in_other_known_environment",
            Self::ConfigurationUnknown => "configuration_unknown",
            Self::ConfigurationExpressionUnsupported => "configuration_expression_unsupported",
            Self::ConfigurationParseFailed => "configuration_parse_failed",
            Self::ParentModuleApplicabilityUnavailable => "parent_module_applicability_unavailable",
            Self::MacroExpansionUnavailable => "macro_expansion_unavailable",
            Self::AlternativeEnvironmentNotAnalyzed => "alternative_environment_not_analyzed",
        }
    }
}

/// The configuration inputs one evaluation runs against.
#[derive(Clone, Debug, Default)]
pub struct CfgInputs {
    /// Features the selection turns on.
    pub active_features: BTreeSet<String>,
    /// `--all-features`: every *known* feature is on; unknown names stay
    /// unknown rather than assumed.
    pub all_features: bool,
    /// Feature names seen in the root manifest `[features]` table.
    pub known_features: BTreeSet<String>,
    /// The `default = [...]` list from the root manifest.
    pub default_features: BTreeSet<String>,
    /// Target atoms from the explicitly selected triple. `None` means no
    /// triple was selected: every target atom is unknown.
    pub target_atoms: Option<BTreeMap<String, String>>,
    /// Explicitly supplied extra atoms (`key` or `key=value`), empty in PR2.
    pub extra_cfgs: BTreeSet<String>,
}

impl CfgInputs {
    /// Inputs from an explicit feature selection without manifest facts:
    /// used when environment discovery fails, so feature atoms stay unknown
    /// instead of failing the run.
    pub fn for_selection(selection: &crate::FeatureSelection) -> Self {
        let mut inputs = Self::default();
        match selection {
            crate::FeatureSelection::DefaultFeatures => {}
            crate::FeatureSelection::NoDefaultFeatures => {}
            crate::FeatureSelection::Explicit(selected) => {
                inputs.active_features = selected.iter().cloned().collect();
            }
            crate::FeatureSelection::AllFeatures => {
                inputs.all_features = true;
            }
        }
        inputs
    }

    /// Inputs from a discovered environment plus an optional explicitly
    /// selected target triple (`--target`).
    pub fn from_environment(
        env: &crate::AnalysisEnvironment,
        selected_triple: Option<&str>,
    ) -> Self {
        let mut inputs = Self::for_selection(&env.features);
        inputs.known_features = env.known_features.iter().cloned().collect();
        inputs.default_features = env.default_features.iter().cloned().collect();
        if inputs.all_features {
            inputs.active_features = inputs.known_features.clone();
        } else if matches!(env.features, crate::FeatureSelection::DefaultFeatures) {
            inputs.active_features = inputs.default_features.clone();
        }
        inputs.target_atoms = selected_triple.map(target_atoms_for_triple);
        inputs.extra_cfgs = env.cfgs.clone();
        inputs
    }

    fn feature_verdict(&self, name: &str) -> CfgVerdict {
        // `from_environment` folds the selection into `active_features`
        // (`--all-features` turns on every known feature, a default run
        // exactly the default set), so membership decides. A name outside
        // both sets is unevaluable: it may be a typo, an optional
        // dependency, or a feature from another member.
        if self.active_features.contains(name) {
            return CfgVerdict::True;
        }
        if self.known_features.contains(name) {
            return CfgVerdict::False;
        }
        CfgVerdict::Unknown
    }

    fn target_verdict(&self, name: &str, value: Option<&str>) -> CfgVerdict {
        let Some(atoms) = &self.target_atoms else {
            return CfgVerdict::Unknown;
        };
        match value {
            Some(want) => match atoms.get(name) {
                Some(have) => {
                    if have == want {
                        CfgVerdict::True
                    } else {
                        CfgVerdict::False
                    }
                }
                None => CfgVerdict::Unknown,
            },
            None => {
                if name == "unix" {
                    return match atoms.get("target_family").map(String::as_str) {
                        Some("unix") => CfgVerdict::True,
                        Some(_) => CfgVerdict::False,
                        None => CfgVerdict::Unknown,
                    };
                }
                if name == "windows" {
                    return match atoms.get("target_family").map(String::as_str) {
                        Some("windows") => CfgVerdict::True,
                        Some(_) => CfgVerdict::False,
                        None => CfgVerdict::Unknown,
                    };
                }
                CfgVerdict::Unknown
            }
        }
    }

    /// Evaluate one atom. `test` is always unknown (no compilation profile
    /// is known); `feature` consults the selection; `target_*` consult the
    /// selected triple; everything else consults explicit extras, else
    /// unknown.
    pub fn eval_atom(&self, atom: &CfgAtom) -> CfgVerdict {
        if atom.name == "test" {
            return CfgVerdict::Unknown;
        }
        if atom.name == "feature" {
            let Some(name) = atom.value.as_deref() else {
                return CfgVerdict::Unknown;
            };
            return self.feature_verdict(name);
        }
        if atom.name == "target_feature" {
            return CfgVerdict::Unknown;
        }
        if atom.name.starts_with("target_") || atom.name == "unix" || atom.name == "windows" {
            return self.target_verdict(&atom.name, atom.value.as_deref());
        }
        let key = match &atom.value {
            Some(value) => format!("{}={value}", atom.name),
            None => atom.name.clone(),
        };
        if self.extra_cfgs.contains(&key) {
            CfgVerdict::True
        } else {
            CfgVerdict::Unknown
        }
    }

    /// Evaluate one expression with Kleene logic.
    pub fn eval(&self, expr: &CfgExpr) -> CfgVerdict {
        match expr {
            CfgExpr::Atom(atom) => self.eval_atom(atom),
            CfgExpr::Not(inner) => match self.eval(inner) {
                CfgVerdict::True => CfgVerdict::False,
                CfgVerdict::False => CfgVerdict::True,
                CfgVerdict::Unknown => CfgVerdict::Unknown,
            },
            CfgExpr::All(parts) => {
                let mut unknown = false;
                for part in parts {
                    match self.eval(part) {
                        CfgVerdict::False => return CfgVerdict::False,
                        CfgVerdict::Unknown => unknown = true,
                        CfgVerdict::True => {}
                    }
                }
                if unknown {
                    CfgVerdict::Unknown
                } else {
                    CfgVerdict::True
                }
            }
            CfgExpr::Any(parts) => {
                let mut unknown = false;
                for part in parts {
                    match self.eval(part) {
                        CfgVerdict::True => return CfgVerdict::True,
                        CfgVerdict::Unknown => unknown = true,
                        CfgVerdict::False => {}
                    }
                }
                if unknown {
                    CfgVerdict::Unknown
                } else {
                    CfgVerdict::False
                }
            }
        }
    }
}

/// Target atoms for an explicitly selected triple. Only mapped components
/// appear: unmapped arches, OSes, or shapes leave their atoms absent, which
/// evaluates to unknown rather than a guessed false.
fn target_atoms_for_triple(triple: &str) -> BTreeMap<String, String> {
    let mut atoms = BTreeMap::new();
    let parts: Vec<&str> = triple.split('-').collect();
    let Some(arch) = parts.first() else {
        return atoms;
    };
    if let Some((name, width, endian)) = arch_atoms(arch) {
        atoms.insert("target_arch".to_string(), name.to_string());
        atoms.insert("target_pointer_width".to_string(), width.to_string());
        atoms.insert("target_endian".to_string(), endian.to_string());
    }
    if parts.len() >= 3
        && let Some((os, family)) = os_atoms(parts[2])
    {
        atoms.insert("target_os".to_string(), os.to_string());
        if let Some(family) = family {
            atoms.insert("target_family".to_string(), family.to_string());
        }
    }
    if parts.len() >= 4 {
        atoms.insert("target_env".to_string(), parts[3].to_string());
    }
    if let Some(vendor) = parts.get(1) {
        atoms.insert("target_vendor".to_string(), (*vendor).to_string());
    }
    atoms
}

/// (arch atom, pointer width, endianness) for known architectures.
fn arch_atoms(arch: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match arch {
        "x86_64" => Some(("x86_64", "64", "little")),
        "aarch64" => Some(("aarch64", "64", "little")),
        "x86" | "i586" | "i686" => Some(("x86", "32", "little")),
        "arm" | "armv7" => Some(("arm", "32", "little")),
        "riscv64" => Some(("riscv64", "64", "little")),
        "wasm32" => Some(("wasm32", "32", "little")),
        "s390x" => Some(("s390x", "64", "big")),
        _ => None,
    }
}

/// (os atom, family) for known OS tokens. `darwin` is the legacy spelling
/// of `macos`. Unmapped tokens (and their family) stay unknown.
fn os_atoms(os: &str) -> Option<(&'static str, Option<&'static str>)> {
    match os {
        "linux" => Some(("linux", Some("unix"))),
        "windows" => Some(("windows", Some("windows"))),
        "macos" | "darwin" => Some(("macos", Some("unix"))),
        "ios" => Some(("ios", Some("unix"))),
        "tvos" => Some(("tvos", Some("unix"))),
        "watchos" => Some(("watchos", Some("unix"))),
        "visionos" => Some(("visionos", Some("unix"))),
        "android" => Some(("android", Some("unix"))),
        "freebsd" => Some(("freebsd", Some("unix"))),
        "netbsd" => Some(("netbsd", Some("unix"))),
        "openbsd" => Some(("openbsd", Some("unix"))),
        "dragonfly" => Some(("dragonfly", Some("unix"))),
        _ => None,
    }
}

/// Parse the inside of one `#[cfg(...)]` attribute into an expression.
/// Accepts atoms (`unix`, `test`), key-values (`feature = "x"`), and
/// `not`/`all`/`any` compositions. Anything else is an error, which the
/// caller reports as `configuration_expression_unsupported`.
pub fn parse_cfg_expr(text: &str) -> Result<CfgExpr, String> {
    let mut parser = CfgParser {
        chars: text.chars().peekable(),
    };
    parser.skip_ws();
    let expr = parser.parse_expr()?;
    parser.skip_ws();
    if parser.chars.peek().is_some() {
        return Err(format!("trailing characters in cfg expression: `{text}`"));
    }
    Ok(expr)
}

struct CfgParser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl CfgParser<'_> {
    fn skip_ws(&mut self) {
        while self.chars.peek().is_some_and(|ch| ch.is_whitespace()) {
            self.chars.next();
        }
    }

    fn expect(&mut self, want: char) -> Result<(), String> {
        match self.chars.next() {
            Some(got) if got == want => Ok(()),
            other => Err(format!(
                "expected `{want}` in cfg expression, found `{other:?}`"
            )),
        }
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        let mut ident = String::new();
        while self
            .chars
            .peek()
            .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
        {
            ident.push(self.chars.next().unwrap_or_default());
        }
        if ident.is_empty() {
            return Err("expected an identifier in cfg expression".to_string());
        }
        Ok(ident)
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut value = String::new();
        loop {
            match self.chars.next() {
                Some('"') => return Ok(value),
                Some('\\') => match self.chars.next() {
                    Some(escaped) => value.push(escaped),
                    None => return Err("unterminated string in cfg expression".to_string()),
                },
                Some(ch) => value.push(ch),
                None => return Err("unterminated string in cfg expression".to_string()),
            }
        }
    }

    fn parse_expr(&mut self) -> Result<CfgExpr, String> {
        self.skip_ws();
        let name = self.parse_ident()?;
        self.skip_ws();
        match self.chars.peek() {
            Some(&'(') => {
                self.chars.next();
                let expr = match name.as_str() {
                    "not" => {
                        let inner = self.parse_expr()?;
                        self.skip_ws();
                        self.expect(')')?;
                        CfgExpr::Not(Box::new(inner))
                    }
                    "all" => CfgExpr::All(self.parse_list()?),
                    "any" => CfgExpr::Any(self.parse_list()?),
                    _ => {
                        // `key(...)` is not a cfg combinator; only
                        // `key = "value"` atoms take arguments.
                        return Err(format!("unknown cfg combinator `{name}`"));
                    }
                };
                Ok(expr)
            }
            Some(&'=') => {
                self.chars.next();
                self.skip_ws();
                let value = self.parse_string()?;
                Ok(CfgExpr::Atom(CfgAtom {
                    name,
                    value: Some(value),
                }))
            }
            _ => Ok(CfgExpr::Atom(CfgAtom { name, value: None })),
        }
    }

    fn parse_list(&mut self) -> Result<Vec<CfgExpr>, String> {
        let mut parts = Vec::new();
        loop {
            self.skip_ws();
            if self.chars.peek() == Some(&')') {
                self.chars.next();
                return Ok(parts);
            }
            parts.push(self.parse_expr()?);
            self.skip_ws();
            match self.chars.peek() {
                Some(&',') => {
                    self.chars.next();
                }
                Some(&')') => continue,
                other => {
                    return Err(format!(
                        "expected `,` or `)` in cfg list, found `{other:?}`"
                    ));
                }
            }
        }
    }
}

/// One bounded `#[cfg_attr(pred, cfg(...), ...)]` gate.
///
/// `condition` is the raw predicate text; `inner` holds each inner
/// `cfg(...)` expression text. Non-`cfg` inner attributes do not gate and
/// are dropped. Anything else is reported unsupported by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CfgAttrGate {
    pub condition: String,
    pub inner: Vec<String>,
    pub raw: String,
}

/// Split the inside of a `#[cfg_attr(...)]` into predicate and inner
/// attribute texts at top-level commas. Returns `None` when parentheses
/// never balance.
fn split_top_level_commas(text: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in text.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if in_string || depth != 0 {
        return None;
    }
    if !current.trim().is_empty() || !parts.is_empty() {
        parts.push(current.trim().to_string());
    }
    Some(parts)
}

/// Parse one `#[cfg_attr ...]` attribute text (including the leading
/// `#[cfg_attr(` or the bare inside) into a bounded gate. Only the
/// `pred, cfg(...), ...` shape is supported; everything else yields
/// `Err` so the caller reports unsupported rather than guessing.
pub fn parse_cfg_attr_gate(attr_text: &str) -> Result<CfgAttrGate, String> {
    let trimmed = attr_text.trim();
    let inside = if let Some(stripped) = trimmed.strip_prefix("#[cfg_attr(") {
        stripped
            .strip_suffix(']')
            .ok_or_else(|| format!("unbalanced cfg_attr attribute: `{attr_text}`"))?
    } else if let Some(stripped) = trimmed.strip_prefix("cfg_attr(") {
        stripped.strip_suffix(')').unwrap_or(stripped)
    } else {
        trimmed
    };
    // Strip the final `)` closing `cfg_attr(` plus the trailing `]`.
    let inside = inside.trim_end();
    let inside = inside
        .strip_suffix(']')
        .map(str::trim_end)
        .unwrap_or(inside);
    let inside = inside.strip_suffix(')').unwrap_or(inside);
    let parts = split_top_level_commas(inside)
        .ok_or_else(|| format!("unbalanced cfg_attr attribute: `{attr_text}`"))?;
    if parts.len() < 2 {
        return Err(format!(
            "cfg_attr needs a predicate and an attribute: `{attr_text}`"
        ));
    }
    let condition = parts[0].clone();
    // Predicate must parse as a cfg expression; anything else is unsupported.
    parse_cfg_expr(&condition)
        .map_err(|err| format!("unsupported cfg_attr predicate `{condition}`: {err}"))?;
    let mut inner = Vec::new();
    for part in parts.iter().skip(1) {
        let piece = part.trim();
        if let Some(cfg_inside) = piece
            .strip_prefix("cfg(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            parse_cfg_expr(cfg_inside)
                .map_err(|err| format!("unsupported cfg_attr inner `{piece}`: {err}"))?;
            inner.push(cfg_inside.trim().to_string());
        } else if piece.starts_with("cfg_attr(") || piece.starts_with("#[cfg_attr(") {
            return Err(format!(
                "nested cfg_attr beyond one level is unsupported: `{piece}`"
            ));
        } else {
            // Non-cfg attributes (derive helpers, lints, docs) never gate.
            continue;
        }
    }
    if inner.is_empty() {
        return Err(format!("cfg_attr without an inner cfg gate: `{attr_text}`"));
    }
    Ok(CfgAttrGate {
        condition,
        inner,
        raw: trimmed.to_string(),
    })
}

/// Maximum lines above the site to search for `#[cfg(...)]` attributes.
const CFG_SCAN_BACK_LINES: usize = 40;

/// Maximum lines a single attribute may span (multi-line `all(...)`).
const CFG_ATTR_SPAN_LINES: usize = 20;

/// Maximum lines to walk forward when balancing a gated region.
const CFG_REGION_LIMIT_LINES: usize = 2000;

/// Collect the `#[cfg(...)]` expression texts enclosing the 1-based
/// `site_line`. Every enclosing gate must hold for the item to compile, so
/// callers AND the verdicts. `#[cfg_attr(...)]` never gates and is skipped.
/// Comment lines (`//`, `///`, `//!`) never contribute attributes: only a
/// line whose first non-whitespace text is `#[cfg(` opens a gate. Unbalanced
/// attributes are ignored rather than assumed.
pub fn enclosing_cfg_exprs(lines: &[&str], site_line: usize) -> Vec<String> {
    if site_line == 0 || site_line > lines.len() {
        return Vec::new();
    }
    if !lines
        .iter()
        .any(|line| line.contains("#[cfg(") || line.contains("#[cfg_attr("))
    {
        return Vec::new();
    }
    let site_idx = site_line - 1;
    let scan_from = site_idx.saturating_sub(CFG_SCAN_BACK_LINES);
    let mut exprs = Vec::new();
    for attr_idx in (scan_from..site_idx).rev() {
        let Some(inner) = cfg_attr_inner(lines, attr_idx) else {
            continue;
        };
        if let Some((open_idx, close_idx)) = balanced_region(lines, attr_idx)
            && attr_idx <= site_idx
            && site_idx <= close_idx
            && open_idx <= site_idx
        {
            exprs.push(inner);
        }
    }
    exprs.sort();
    exprs.dedup();
    exprs
}

/// The inside of a `#[cfg(...)]` attribute starting on `lines[idx]`,
/// joining continuation lines until the parentheses balance (capped at
/// [`CFG_ATTR_SPAN_LINES`]). Returns `None` for `cfg_attr`, comments, and
/// unbalanced text.
fn cfg_attr_inner(lines: &[&str], idx: usize) -> Option<String> {
    let first = lines[idx].trim_start();
    if !first.starts_with("#[cfg(") {
        return None;
    }
    let mut text = first["#[cfg(".len()..].to_string();
    let mut depth = 1i32;
    for ch in text.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
        }
    }
    let mut span = 1usize;
    while depth > 0 && span < CFG_ATTR_SPAN_LINES {
        let next = lines.get(idx + span)?;
        text.push('\n');
        text.push_str(next);
        for ch in next.chars() {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
            }
        }
        span += 1;
    }
    if depth != 0 {
        return None;
    }
    // A well-formed attribute ends in `)]`; anything else is not a gate.
    let inner = text.trim_end().strip_suffix(']')?;
    inner
        .trim_end()
        .strip_suffix(')')
        .map(|expr| expr.trim_end().to_string())
}

/// Balance the first `{...}` region starting at or after `from`, capped at
/// [`CFG_REGION_LIMIT_LINES`] lines. Same shape as the source-role
/// classifier's region walk: string literals are not tracked, which fits
/// the common module/function gate shapes.
///
/// Bounded fallback diagnostic only: authoritative scope uses
/// [`collect_structural_gates`].
fn balanced_region(lines: &[&str], from: usize) -> Option<(usize, usize)> {
    let limit = (from + CFG_REGION_LIMIT_LINES).min(lines.len());
    let mut depth = 0i32;
    let mut open_idx = None;
    for (idx, line) in lines.iter().enumerate().take(limit).skip(from) {
        let code = line.split("//").next().unwrap_or(line);
        for ch in code.chars() {
            if ch == '{' {
                if open_idx.is_none() {
                    open_idx = Some(idx);
                }
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return open_idx.map(|open| (open, idx));
                }
                if depth < 0 {
                    return None;
                }
            }
        }
    }
    None
}

/// Convert a 1-based line/column (columns count chars) to a byte offset.
fn file_offset(text: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if current_line == line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    if current_line != line {
        return None;
    }
    let mut current_col = 1usize;
    for (idx, ch) in text[line_start..].char_indices() {
        if current_col == column {
            let offset = line_start + idx;
            return Some(offset.min(text.len()));
        }
        if ch == '\n' {
            break;
        }
        current_col += 1;
    }
    if current_col == column {
        return Some(text.len().min(line_start + text[line_start..].len()));
    }
    // Column past end of line: clamp to line end so the site still maps.
    let line_end = text[line_start..]
        .find('\n')
        .map(|rel| line_start + rel)
        .unwrap_or(text.len());
    Some(line_end)
}

fn text_size_to_offset(value: ra_ap_syntax::TextSize) -> usize {
    u32::from(value) as usize
}

/// Extract the inside of `#[cfg(...)]` from raw attribute text.
fn cfg_direct_inner(attr_text: &str) -> Option<String> {
    let trimmed = attr_text.trim();
    let stripped = trimmed.strip_prefix("#[cfg(")?;
    // Balanced paren scan so multiline `all(...)` survives.
    let mut depth = 0i32;
    let mut end = None;
    for (idx, ch) in stripped.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    end = Some(idx);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let end = end?;
    let inner = stripped[..end].trim().to_string();
    let rest = stripped[end + 1..].trim_start();
    if !rest.starts_with(']') {
        return None;
    }
    Some(inner)
}

/// Collect same-file gates authoritatively: parsed syntax ranges with
/// ancestor containment. Raw strings, comments, and macro token trees
/// cannot misattribute scope because ranges come from the parser.
pub fn collect_structural_gates(text: &str, line: usize, column: usize) -> StructuralGates {
    use ra_ap_syntax::{AstNode, Edition, SourceFile};
    let mut gates = StructuralGates::default();
    let Some(site_offset) = file_offset(text, line, column.max(1)) else {
        return gates;
    };
    let parse = SourceFile::parse(text, Edition::CURRENT);
    gates.file_parse_errors = parse.errors().iter().map(ToString::to_string).collect();
    let tree = parse.tree();
    let root = tree.syntax();
    // Deepest node containing the site decides macro context and mapping.
    let mut site_node: Option<ra_ap_syntax::SyntaxNode> = None;
    for node in root.descendants() {
        let range = node.text_range();
        let start = text_size_to_offset(range.start());
        let end = text_size_to_offset(range.end());
        if start <= site_offset && site_offset < end.max(start + 1) {
            // Keep the smallest containing node.
            let replace = match &site_node {
                None => true,
                Some(current) => node.text_range().len() < current.text_range().len(),
            };
            if replace {
                site_node = Some(node.clone());
            }
        }
    }
    let Some(site) = site_node else {
        return gates;
    };
    gates.site_mapped = true;
    let mut ancestor = Some(site.clone());
    while let Some(node) = ancestor {
        let kind = format!("{:?}", node.kind());
        if kind == "MACRO_CALL"
            || kind == "MACRO_EXPR"
            || kind == "MACRO_DEF"
            || kind == "MACRO_RULES"
            || kind == "TOKEN_TREE"
            || kind == "MACRO_STMTS"
        {
            gates.in_macro = true;
        }
        ancestor = node.parent();
    }
    // Every attribute whose owner contains the site and starts before it.
    for node in root.descendants() {
        if format!("{:?}", node.kind()) != "ATTR" {
            continue;
        }
        let attr_text = node.text().to_string();
        let attr_start = text_size_to_offset(node.text_range().start());
        let Some(owner) = node.parent() else {
            continue;
        };
        let owner_start = text_size_to_offset(owner.text_range().start());
        let owner_end = text_size_to_offset(owner.text_range().end());
        if !(owner_start <= site_offset && site_offset <= owner_end) {
            continue;
        }
        if !(attr_start < site_offset.max(1) || attr_start == site_offset) {
            continue;
        }
        // Owner must strictly contain the attribute (skip stray attrs).
        if !(owner_start <= attr_start && attr_start < owner_end) {
            continue;
        }
        let trimmed = attr_text.trim_start();
        if trimmed.starts_with("#[cfg(") || trimmed.starts_with("#[cfg (") {
            if let Some(inner) = cfg_direct_inner(&attr_text) {
                gates.direct.push(inner);
            } else {
                gates
                    .cfg_attr_unsupported
                    .push(attr_text.trim().to_string());
            }
        } else if trimmed.starts_with("#[cfg_attr") {
            match parse_cfg_attr_gate(&attr_text) {
                Ok(gate) => gates.cfg_attr.push(gate),
                Err(_) => gates
                    .cfg_attr_unsupported
                    .push(attr_text.trim().to_string()),
            }
        }
    }
    gates.direct.sort();
    gates.direct.dedup();
    gates
        .cfg_attr
        .sort_by(|a, b| a.condition.cmp(&b.condition).then(a.inner.cmp(&b.inner)));
    gates.cfg_attr_unsupported.sort();
    gates.cfg_attr_unsupported.dedup();
    gates
}

/// Structural gate collection for one site: direct `#[cfg]` expressions,
/// bounded `#[cfg_attr]` gates, and explicit limitations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralGates {
    pub direct: Vec<String>,
    pub cfg_attr: Vec<CfgAttrGate>,
    pub cfg_attr_unsupported: Vec<String>,
    pub in_macro: bool,
    pub file_parse_errors: Vec<String>,
    pub site_mapped: bool,
}

/// One gated card's configuration state under the selected envelope.
///
/// `column` keeps two sites on one line distinguishable; `byte_start` and
/// `byte_end` name the exact site offset the ancestry was computed from.
/// `reason` states what the evidence establishes and which limitation
/// applies (parent-file gates and macro expansion are never resolved here).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CardConfiguration {
    pub card_id: String,
    pub file: std::path::PathBuf,
    pub line: usize,
    #[serde(default)]
    pub column: usize,
    #[serde(default)]
    pub end_line: Option<usize>,
    #[serde(default)]
    pub end_column: Option<usize>,
    #[serde(default)]
    pub byte_start: Option<usize>,
    #[serde(default)]
    pub byte_end: Option<usize>,
    pub expressions: Vec<String>,
    pub applicability: Applicability,
    pub environment_digest: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Evaluate the gating expressions of every card with an enclosing
/// same-file `#[cfg]` or bounded `#[cfg_attr(pred, cfg(...))]` gate.
/// Ungated cards are trivially active and omitted. Unreadable files yield
/// `parent_module_applicability_unavailable`, never silence; unmappable
/// sites yield `configuration_parse_failed`; sites inside macro context
/// yield `macro_expansion_unavailable`. `inactive` is emitted only when a
/// known gate evaluates false with no unsupported or parse-failed gate.
pub fn evaluate_configurations(
    root: &std::path::Path,
    cards: &[crate::domain::ReviewCard],
    inputs: &CfgInputs,
    environment_digest: &str,
) -> Vec<CardConfiguration> {
    const PARENT_LIMITATION: &str =
        "same-file scope only; parent-file module gates and macro expansion are not resolved";
    let mut cached: std::collections::BTreeMap<std::path::PathBuf, Option<String>> =
        std::collections::BTreeMap::new();
    let mut out = Vec::new();
    for card in cards {
        let rel = card.site.location.file.clone();
        let line = card.site.location.line;
        let column = card.site.location.column.max(1);
        let text = match cached.get(&rel) {
            Some(cached) => cached.clone(),
            None => {
                let loaded = std::fs::read_to_string(root.join(&rel)).ok();
                cached.insert(rel.clone(), loaded.clone());
                loaded
            }
        };
        let Some(text) = text else {
            out.push(CardConfiguration {
                card_id: card.id.to_string(),
                file: rel,
                line,
                column,
                end_line: None,
                end_column: None,
                byte_start: None,
                byte_end: None,
                expressions: Vec::new(),
                applicability: Applicability::ParentModuleApplicabilityUnavailable,
                environment_digest: environment_digest.to_string(),
                reason: Some(
                    "file unreadable from analysis root; same-file gates unevaluated".to_string(),
                ),
            });
            continue;
        };
        let gates = collect_structural_gates(&text, line, column);
        let offset = file_offset(&text, line, column);
        if !gates.site_mapped || offset.is_none() {
            out.push(CardConfiguration {
                card_id: card.id.to_string(),
                file: rel,
                line,
                column,
                end_line: Some(line),
                end_column: Some(column),
                byte_start: offset,
                byte_end: offset,
                expressions: Vec::new(),
                applicability: Applicability::ConfigurationParseFailed,
                environment_digest: environment_digest.to_string(),
                reason: Some(format!(
                    "site does not map into parsed syntax; {}",
                    PARENT_LIMITATION
                )),
            });
            continue;
        }
        if gates.direct.is_empty()
            && gates.cfg_attr.is_empty()
            && gates.cfg_attr_unsupported.is_empty()
        {
            continue;
        }
        if gates.in_macro {
            let mut expressions: Vec<String> = gates.direct.clone();
            for gate in &gates.cfg_attr {
                for inner in &gate.inner {
                    expressions.push(format!("cfg_attr({} => {inner})", gate.condition));
                }
            }
            expressions.extend(gates.cfg_attr_unsupported.clone());
            expressions.sort();
            expressions.dedup();
            out.push(CardConfiguration {
                card_id: card.id.to_string(),
                file: rel,
                line,
                column,
                end_line: Some(line),
                end_column: Some(column),
                byte_start: offset,
                byte_end: offset,
                expressions,
                applicability: Applicability::MacroExpansionUnavailable,
                environment_digest: environment_digest.to_string(),
                reason: Some(format!(
                    "site inside macro context; cfg scope behind macro expansion; {}",
                    PARENT_LIMITATION
                )),
            });
            continue;
        }
        let mut expressions: Vec<String> = Vec::new();
        let mut applicability = Applicability::ActiveInSelectedEnvironment;
        let mut unsupported = false;
        let mut parse_failed = false;
        let mut unknown = false;
        let mut inactive = false;
        for direct in &gates.direct {
            expressions.push(direct.clone());
            match parse_cfg_expr(direct) {
                Err(err) => {
                    if err.contains("unknown cfg combinator")
                        || err.contains("unknown cfg")
                        || err.contains("unsupported")
                    {
                        unsupported = true;
                    } else {
                        parse_failed = true;
                    }
                }
                Ok(expr) => match inputs.eval(&expr) {
                    CfgVerdict::False => inactive = true,
                    CfgVerdict::Unknown => unknown = true,
                    CfgVerdict::True => {}
                },
            }
        }
        for gate in &gates.cfg_attr {
            for inner in &gate.inner {
                expressions.push(format!("cfg_attr({} => {inner})", gate.condition));
            }
            let condition = match parse_cfg_expr(&gate.condition) {
                Err(_) => {
                    unsupported = true;
                    continue;
                }
                Ok(expr) => expr,
            };
            match inputs.eval(&condition) {
                CfgVerdict::False => {}
                CfgVerdict::True => {
                    for inner in &gate.inner {
                        match parse_cfg_expr(inner) {
                            Err(err) => {
                                if err.contains("unknown cfg combinator") {
                                    unsupported = true;
                                } else {
                                    parse_failed = true;
                                }
                            }
                            Ok(expr) => match inputs.eval(&expr) {
                                CfgVerdict::False => inactive = true,
                                CfgVerdict::Unknown => unknown = true,
                                CfgVerdict::True => {}
                            },
                        }
                    }
                }
                CfgVerdict::Unknown => unknown = true,
            }
        }
        if !gates.cfg_attr_unsupported.is_empty() {
            unsupported = true;
            expressions.extend(gates.cfg_attr_unsupported.clone());
        }
        if unsupported {
            applicability = Applicability::ConfigurationExpressionUnsupported;
        } else if parse_failed {
            applicability = Applicability::ConfigurationParseFailed;
        } else if inactive {
            applicability = Applicability::InactiveInSelectedEnvironment;
        } else if unknown {
            applicability = Applicability::ConfigurationUnknown;
        }
        if !gates.file_parse_errors.is_empty()
            && applicability == Applicability::ActiveInSelectedEnvironment
        {
            applicability = Applicability::ConfigurationParseFailed;
        }
        expressions.sort();
        expressions.dedup();
        let reason = match applicability {
            Applicability::ActiveInSelectedEnvironment => Some(format!(
                "all same-file gates hold under the selected envelope; {}",
                PARENT_LIMITATION
            )),
            Applicability::InactiveInSelectedEnvironment => Some(format!(
                "a same-file gate is false under the selected envelope; {}",
                PARENT_LIMITATION
            )),
            Applicability::ConfigurationUnknown => Some(format!(
                "a same-file gate depends on unknown input (feature, target, test, custom cfg); {}",
                PARENT_LIMITATION
            )),
            Applicability::ConfigurationExpressionUnsupported => Some(format!(
                "a same-file gate uses an unsupported expression; {}",
                PARENT_LIMITATION
            )),
            Applicability::ConfigurationParseFailed => Some(format!(
                "same-file scope or gate text failed to parse; {}",
                PARENT_LIMITATION
            )),
            _ => Some(PARENT_LIMITATION.to_string()),
        };
        out.push(CardConfiguration {
            card_id: card.id.to_string(),
            file: rel,
            line,
            column,
            end_line: Some(line),
            end_column: Some(column),
            byte_start: offset,
            byte_end: offset,
            expressions,
            applicability,
            environment_digest: environment_digest.to_string(),
            reason,
        });
    }
    out.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.card_id.cmp(&right.card_id))
    });
    out
}

/// Counts of gated cards by applicability state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConfigurationCounts {
    pub gated: usize,
    pub active: usize,
    pub inactive: usize,
    pub unknown: usize,
    pub unsupported: usize,
    pub parse_failed: usize,
    pub parent_unavailable: usize,
    pub macro_unavailable: usize,
    pub alternative_not_analyzed: usize,
}

/// Count evaluated configurations by state.
pub fn summarize_configurations(configs: &[CardConfiguration]) -> ConfigurationCounts {
    let mut counts = ConfigurationCounts {
        gated: configs.len(),
        ..ConfigurationCounts::default()
    };
    for config in configs {
        match config.applicability {
            Applicability::ActiveInSelectedEnvironment
            | Applicability::ActiveInOtherKnownEnvironment => counts.active += 1,
            Applicability::InactiveInSelectedEnvironment => counts.inactive += 1,
            Applicability::ConfigurationUnknown => counts.unknown += 1,
            Applicability::ConfigurationExpressionUnsupported => counts.unsupported += 1,
            Applicability::ConfigurationParseFailed => counts.parse_failed += 1,
            Applicability::ParentModuleApplicabilityUnavailable => {
                counts.parent_unavailable += 1;
            }
            Applicability::MacroExpansionUnavailable => counts.macro_unavailable += 1,
            Applicability::AlternativeEnvironmentNotAnalyzed => {
                counts.alternative_not_analyzed += 1;
            }
        }
    }
    counts
}

/// Human configuration section: one summary line plus one line per gated
/// card. Rendered only for explicit envelope selections; default runs print
/// no section so their output stays byte-stable.
pub fn render_configuration_human(
    configs: &[CardConfiguration],
    environment_digest: &str,
    note: Option<&str>,
) -> String {
    let counts = summarize_configurations(configs);
    let mut out = String::new();
    out.push_str(&format!("Configuration (envelope {environment_digest}):\n"));
    if let Some(note) = note {
        out.push_str(&format!("- note: {note}\n"));
    }
    out.push_str(&format!(
        "- {} gated cards: {} active, {} inactive, {} unknown, {} unsupported",
        counts.gated, counts.active, counts.inactive, counts.unknown, counts.unsupported
    ));
    if counts.parse_failed > 0 {
        out.push_str(&format!(", {} parse failed", counts.parse_failed));
    }
    if counts.parent_unavailable > 0 {
        out.push_str(&format!(
            ", {} parent unavailable",
            counts.parent_unavailable
        ));
    }
    if counts.macro_unavailable > 0 {
        out.push_str(&format!(", {} macro unavailable", counts.macro_unavailable));
    }
    if counts.alternative_not_analyzed > 0 {
        out.push_str(&format!(
            ", {} alternative not analyzed",
            counts.alternative_not_analyzed
        ));
    }
    out.push('\n');
    for config in configs {
        out.push_str(&format!(
            "- {} {}:{}:{} {} [{}]",
            config.card_id,
            config.file.display(),
            config.line,
            config.column,
            config.applicability.as_str(),
            config.expressions.join(", ")
        ));
        if let Some(reason) = &config.reason {
            out.push_str(&format!(" ({reason})"));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs_for(features: &[&str], known: &[&str], default: &[&str]) -> CfgInputs {
        CfgInputs {
            active_features: features.iter().map(ToString::to_string).collect(),
            known_features: known.iter().map(ToString::to_string).collect(),
            default_features: default.iter().map(ToString::to_string).collect(),
            ..CfgInputs::default()
        }
    }

    fn eval_ok(text: &str, inputs: &CfgInputs) -> Result<CfgVerdict, String> {
        let expr = parse_cfg_expr(text)?;
        Ok(inputs.eval(&expr))
    }

    #[test]
    fn parses_atoms_combinators_and_key_values() -> Result<(), String> {
        assert!(matches!(parse_cfg_expr("unix"), Ok(CfgExpr::Atom(_))));
        assert!(matches!(
            parse_cfg_expr("all(unix, not(target_os = \"windows\"))"),
            Ok(CfgExpr::All(_))
        ));
        let Err(_) = parse_cfg_expr("feature(") else {
            return Err("unbalanced cfg must not parse".to_string());
        };
        let Err(_) = parse_cfg_expr("all(unix))") else {
            return Err("trailing paren must not parse".to_string());
        };
        let Err(_) = parse_cfg_expr("bogus_combinator(x)") else {
            return Err("unknown combinator must not parse".to_string());
        };
        Ok(())
    }

    #[test]
    fn kleene_logic_holds_for_partial_knowledge() -> Result<(), String> {
        let inputs = inputs_for(&["a"], &["a", "b"], &["a"]);
        let on = "feature = \"a\"";
        let off = "feature = \"b\"";
        let missing = "feature = \"missing\"";
        assert_eq!(
            eval_ok(&format!("all({on}, {off})"), &inputs)?,
            CfgVerdict::False
        );
        assert_eq!(
            eval_ok(&format!("any({on}, {off})"), &inputs)?,
            CfgVerdict::True
        );
        assert_eq!(
            eval_ok(&format!("all({on}, {missing})"), &inputs)?,
            CfgVerdict::Unknown
        );
        assert_eq!(
            eval_ok(&format!("not({missing})"), &inputs)?,
            CfgVerdict::Unknown
        );
        Ok(())
    }

    #[test]
    fn known_but_off_features_are_false_unknown_names_unknown() -> Result<(), String> {
        let inputs = inputs_for(&["checked"], &["checked", "fast"], &["checked"]);
        assert_eq!(eval_ok("feature = \"checked\"", &inputs)?, CfgVerdict::True);
        assert_eq!(eval_ok("feature = \"fast\"", &inputs)?, CfgVerdict::False);
        assert_eq!(eval_ok("feature = \"nope\"", &inputs)?, CfgVerdict::Unknown);
        Ok(())
    }

    #[test]
    fn test_atom_is_always_unknown() -> Result<(), String> {
        let inputs = inputs_for(&[], &[], &[]);
        assert_eq!(eval_ok("test", &inputs)?, CfgVerdict::Unknown);
        assert_eq!(eval_ok("not(test)", &inputs)?, CfgVerdict::Unknown);
        Ok(())
    }

    #[test]
    fn target_atoms_need_an_explicit_triple() -> Result<(), String> {
        let unselected = CfgInputs::default();
        assert_eq!(
            eval_ok("target_arch = \"x86_64\"", &unselected)?,
            CfgVerdict::Unknown
        );
        let selected = CfgInputs {
            target_atoms: Some(target_atoms_for_triple("x86_64-unknown-linux-gnu")),
            ..CfgInputs::default()
        };
        assert_eq!(
            eval_ok("target_arch = \"x86_64\"", &selected)?,
            CfgVerdict::True
        );
        assert_eq!(
            eval_ok("target_arch = \"aarch64\"", &selected)?,
            CfgVerdict::False
        );
        assert_eq!(eval_ok("unix", &selected)?, CfgVerdict::True);
        assert_eq!(eval_ok("windows", &selected)?, CfgVerdict::False);
        assert_eq!(
            eval_ok("target_feature = \"neon\"", &selected)?,
            CfgVerdict::Unknown
        );
        Ok(())
    }

    #[test]
    fn enclosing_cfgs_collect_nested_gates_not_comments() {
        let body = "// #[cfg(unix)]\n#[cfg(all(unix, target_arch = \"x86_64\"))]\npub mod gated {\n    pub fn run() {}\n}\n";
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(
            enclosing_cfg_exprs(&lines, 4),
            vec!["all(unix, target_arch = \"x86_64\")".to_string()]
        );
    }

    #[test]
    fn production_site_beside_gated_module_is_ungated() {
        let body = "pub fn prod() {}\n\n#[cfg(windows)]\npub mod gated {\n    pub fn run() {}\n}\n";
        let lines: Vec<&str> = body.lines().collect();
        assert!(enclosing_cfg_exprs(&lines, 1).is_empty());
        assert_eq!(enclosing_cfg_exprs(&lines, 5), vec!["windows".to_string()]);
    }

    #[test]
    fn structural_scope_finds_distant_outer_module_gate() {
        let mut body = String::from("#[cfg(feature = \"fast\")]\npub mod outer {\n");
        for _ in 0..60 {
            body.push_str("    // padding line\n");
        }
        body.push_str("    pub unsafe fn read_fast(ptr: *const u8) -> u8 {\n        unsafe { *ptr }\n    }\n}\n");
        let site_line = body.lines().count() - 2;
        let gates = collect_structural_gates(&body, site_line, 9);
        assert_eq!(gates.direct, vec!["feature = \"fast\"".to_string()]);
    }

    #[test]
    fn structural_scope_ignores_brace_like_raw_strings_and_comments() {
        let body = "#[cfg(unix)]\npub mod gated {\n    pub fn run() {\n        let s = r#\"{\"#;\n        // }\n        unsafe { core::ptr::read(0 as *const u8); }\n    }\n}\n";
        let gates = collect_structural_gates(body, 6, 9);
        assert_eq!(gates.direct, vec!["unix".to_string()]);
    }

    #[test]
    fn same_line_sites_share_gates_but_keep_distinct_columns() {
        let body = "#[cfg(unix)]\npub fn both() { unsafe { f(); } unsafe { g(); } }\n";
        let first = collect_structural_gates(body, 2, 24);
        let second = collect_structural_gates(body, 2, 40);
        assert_eq!(first.direct, vec!["unix".to_string()]);
        assert_eq!(second.direct, vec!["unix".to_string()]);
    }

    #[test]
    fn cfg_attr_conditional_gate_holds_only_when_predicate_holds() -> Result<(), String> {
        let gate = parse_cfg_attr_gate(
            "#[cfg_attr(feature = \"portable\", cfg(target_pointer_width = \"64\"))]",
        )?;
        assert_eq!(gate.condition, "feature = \"portable\"");
        assert_eq!(
            gate.inner,
            vec!["target_pointer_width = \"64\"".to_string()]
        );
        let on_both = CfgInputs {
            active_features: ["portable".to_string()].into_iter().collect(),
            known_features: ["portable".to_string()].into_iter().collect(),
            target_atoms: Some(target_atoms_for_triple("x86_64-unknown-linux-gnu")),
            ..CfgInputs::default()
        };
        assert_eq!(
            on_both.eval(&parse_cfg_expr(&gate.condition)?),
            CfgVerdict::True
        );
        assert_eq!(
            on_both.eval(&parse_cfg_expr(&gate.inner[0])?),
            CfgVerdict::True
        );
        let off = CfgInputs {
            known_features: ["portable".to_string()].into_iter().collect(),
            target_atoms: Some(target_atoms_for_triple("x86_64-unknown-linux-gnu")),
            ..CfgInputs::default()
        };
        assert_eq!(
            off.eval(&parse_cfg_expr(&gate.condition)?),
            CfgVerdict::False
        );
        Ok(())
    }

    #[test]
    fn cfg_attr_unknown_predicate_stays_unknown() -> Result<(), String> {
        let gate = parse_cfg_attr_gate("#[cfg_attr(feature = \"maybe\", cfg(unix))]")?;
        let inputs = CfgInputs::default();
        assert_eq!(
            inputs.eval(&parse_cfg_expr(&gate.condition)?),
            CfgVerdict::Unknown
        );
        Ok(())
    }

    #[test]
    fn cfg_attr_without_inner_cfg_is_unsupported() -> Result<(), String> {
        let Err(_) = parse_cfg_attr_gate("#[cfg_attr(feature = \"a\", allow(dead_code))]") else {
            return Err("cfg_attr without an inner cfg must not parse".to_string());
        };
        Ok(())
    }

    #[test]
    fn macro_sites_report_macro_expansion_unavailable() {
        let body = "macro_rules! m { () => { unsafe { f(); } }; }\nm!();\n";
        let gates = collect_structural_gates(body, 1, 32);
        assert!(gates.in_macro);
    }

    #[test]
    fn cfg_test_stays_unknown_never_inactive() -> Result<(), String> {
        let inputs = CfgInputs::default();
        assert_eq!(eval_ok("test", &inputs)?, CfgVerdict::Unknown);
        assert_eq!(
            inputs.eval(&parse_cfg_expr("all(test, unix)")?),
            CfgVerdict::Unknown
        );
        Ok(())
    }
}
