use serde::{Deserialize, Serialize};

use crate::util::stable_hash_hex;

mod summary;

pub const WITNESS_RECEIPT_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessReceipt {
    pub schema_version: String,
    pub card_id: String,
    pub tool: String,
    pub strength: String,
    pub author: Option<String>,
    pub recorded_at: Option<String>,
    pub expires_at: Option<String>,
    pub summary: Option<String>,
    pub command: Option<String>,
    pub command_hash: Option<String>,
    pub limitations: Option<Vec<String>>,
    /// Optional structured verdict for the saved run. "confirmed" means the
    /// UB-risk hypothesis reproduced; "not_reproduced" means this single run
    /// did not reproduce it — it is NOT a safety claim. "inconclusive" marks
    /// an ambiguous or partial run. Absent on older receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// Exit code of the executed child process. Present only on receipts
    /// built from an executed command; absent means the terminal status is
    /// unknown (all saved-output imports and older receipts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Whether the executed child process was terminated by a signal.
    /// Absent means unknown, never assumed clean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminated_by_signal: Option<bool>,
    /// Structured identity of the reviewed subject and the observation
    /// context. Absent on unbound legacy receipts, which remain readable
    /// but explicitly limited: they never gain invented source identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectBinding>,
}

/// Minimum structured identity binding a witness observation to the exact
/// reviewed subject, source revision, analysis configuration, and capture
/// facts. Only non-secret configuration is recorded: digests and revision
/// strings, never file contents, ambient environment, or absolute paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectBinding {
    /// Digest of the reviewed subject: card identity, operation family,
    /// owner, source file, and operation snippet. A changed guard or
    /// ownership context changes the subject even when the operation text
    /// does not, so callers must include that context in the digest input.
    pub subject_digest: String,
    /// Digest of the executed invocation (program, arguments, environment).
    /// Two confirmations of the same card with different invocations are
    /// different executions and never compare applicable. `None` means the
    /// invocation was not recorded and therefore cannot match anything,
    /// not even another `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_digest: Option<String>,
    /// Analysis scope that produced the subject (`diff`, `repo`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Source revision the subject was reviewed at, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_commit: Option<String>,
    /// Whether the working tree was dirty when the observation was recorded.
    /// Observations from dirty trees never count as clean-tree evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_dirty: Option<bool>,
    /// Working directory of the invocation, relative to the reviewed root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    /// Digest of the captured output bytes the verdict was classified from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_digest: Option<String>,
    /// Whether output capture completed. Partial capture cannot qualify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_complete: Option<bool>,
    /// Analyzer/tool version that produced the subject, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
}

/// Applicability of a saved observation to a current subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubjectApplicability {
    Applicable,
    Stale { reason: String },
    Unknown { reason: String },
}

impl SubjectBinding {
    pub fn digest_subject(parts: &[&str]) -> String {
        // Length-prefixed framing: a bare newline join lets ["a\nb"] and
        // ["a", "b"] hash identically. Lengths make the split unambiguous.
        let mut framed = String::new();
        for part in parts {
            framed.push_str(&format!("{}:\n", part.len()));
            framed.push_str(part);
            framed.push('\n');
        }
        stable_hash_hex(&framed)
    }

    pub fn digest_output(output: &str) -> String {
        stable_hash_hex(output)
    }

    /// Canonical digest of an executed invocation: program, arguments in
    /// order, and environment assignments sorted by key. Environment values
    /// are digested, never stored: the digest proves which invocation ran
    /// without recording possibly-secret configuration.
    pub fn digest_invocation(program: &str, args: &[String], env: &[(String, String)]) -> String {
        let mut parts = vec![program.to_string()];
        parts.extend(args.iter().cloned());
        let mut env: Vec<String> = env
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        env.sort();
        parts.extend(env);
        Self::digest_subject(&parts.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// Decide whether an observation recorded under `recorded` still applies
    /// to the current subject `current`. Unknown never implies applicable;
    /// only full equivalence of subject, scope, revision, and clean capture
    /// retains applicability. Unrelated changes keep applicability only
    /// through this digest equivalence, never through unchanged text alone.
    pub fn applicability(
        recorded: Option<&SubjectBinding>,
        current: &SubjectBinding,
    ) -> SubjectApplicability {
        let Some(recorded) = recorded else {
            return SubjectApplicability::Unknown {
                reason: "unbound legacy receipt carries no subject identity".to_string(),
            };
        };
        if recorded.subject_digest != current.subject_digest {
            return SubjectApplicability::Stale {
                reason: "reviewed subject changed".to_string(),
            };
        }
        if recorded.scope != current.scope {
            return SubjectApplicability::Stale {
                reason: "analysis scope changed".to_string(),
            };
        }
        if recorded.invocation_digest != current.invocation_digest
            || recorded.invocation_digest.is_none()
        {
            // Different invocations are different executions, and a missing
            // digest matches nothing, not even another missing digest.
            return SubjectApplicability::Unknown {
                reason: "executed invocation identity is not established on both sides".to_string(),
            };
        }
        if recorded.head_commit != current.head_commit {
            return SubjectApplicability::Stale {
                reason: "source revision changed".to_string(),
            };
        }
        if recorded.head_commit.is_none() {
            // Equal `None` revisions carry no identity: two revision-less
            // observations must never compare applicable.
            return SubjectApplicability::Unknown {
                reason: "source revision unavailable on both sides of the comparison".to_string(),
            };
        }
        if recorded.repo_dirty != Some(false) || current.repo_dirty != Some(false) {
            // Clean-tree provenance must be established on both sides: a
            // `None` dirtiness probe (git unavailable, not a repository, or
            // timed out) carries no clean evidence, exactly like the
            // `captured_complete` rule below.
            return SubjectApplicability::Unknown {
                reason: "clean-tree provenance is not established on both sides".to_string(),
            };
        }
        if recorded.captured_complete != Some(true) || current.captured_complete != Some(true) {
            return SubjectApplicability::Unknown {
                reason: "output capture completeness is not established".to_string(),
            };
        }
        SubjectApplicability::Applicable
    }
}

/// Terminal facts for a witness child process, retained by the opt-in
/// executor and consumed by executed-output classification so that
/// success-looking text can never hide a failed process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalStatus {
    pub exit_code: Option<i32>,
    pub signaled: bool,
    pub captured_complete: bool,
}

impl TerminalStatus {
    pub fn exited(exit_code: i32) -> Self {
        Self {
            exit_code: Some(exit_code),
            signaled: false,
            captured_complete: true,
        }
    }

    pub fn signaled() -> Self {
        Self {
            exit_code: None,
            signaled: true,
            captured_complete: true,
        }
    }

    pub fn unknown() -> Self {
        Self {
            exit_code: None,
            signaled: false,
            captured_complete: false,
        }
    }

    fn describe(self) -> &'static str {
        if self.signaled {
            "process was terminated by a signal"
        } else {
            match self.exit_code {
                Some(0) => "process exited 0",
                Some(_) => "process exited nonzero",
                None => "process exit code unavailable",
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptCardIdKind {
    AnalyzerReviewCard,
    ManualCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiriReceiptInput {
    pub card_id: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
    /// Terminal facts for the executed child. `None` means unknown: all
    /// saved-output imports and older receipts predate status retention and
    /// must never be read as exit 0.
    pub terminal_status: Option<TerminalStatus>,
    /// Structured subject/observation identity. `None` on saved imports
    /// and legacy observations, which stay explicitly unbound.
    pub subject: Option<SubjectBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoCarefulReceiptInput {
    pub card_id: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
    /// Terminal facts for the executed child. `None` means unknown: all
    /// saved-output imports and older receipts predate status retention and
    /// must never be read as exit 0.
    pub terminal_status: Option<TerminalStatus>,
    /// Structured subject/observation identity. `None` on saved imports
    /// and legacy observations, which stay explicitly unbound.
    pub subject: Option<SubjectBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizerReceiptInput {
    pub card_id: String,
    pub tool: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
    /// Terminal facts for the executed child. `None` means unknown: all
    /// saved-output imports and older receipts predate status retention and
    /// must never be read as exit 0.
    pub terminal_status: Option<TerminalStatus>,
    /// Structured subject/observation identity. `None` on saved imports
    /// and legacy observations, which stay explicitly unbound.
    pub subject: Option<SubjectBinding>,
    /// When `true`, accept output from a runtime/program-level sanitizer run
    /// that is not a `cargo test` harness. A clean run (no sanitizer markers)
    /// records `not_reproduced`; a run with sanitizer markers records
    /// `confirmed` (failure observed — not a safety claim).
    pub allow_runtime: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcurrencyReceiptInput {
    pub card_id: String,
    pub tool: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
    /// Terminal facts for the executed child. `None` means unknown: all
    /// saved-output imports and older receipts predate status retention and
    /// must never be read as exit 0.
    pub terminal_status: Option<TerminalStatus>,
    /// Structured subject/observation identity. `None` on saved imports
    /// and legacy observations, which stay explicitly unbound.
    pub subject: Option<SubjectBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofReceiptInput {
    pub card_id: String,
    pub tool: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
    /// Terminal facts for the executed child. `None` means unknown: all
    /// saved-output imports and older receipts predate status retention and
    /// must never be read as exit 0.
    pub terminal_status: Option<TerminalStatus>,
    /// Structured subject/observation identity. `None` on saved imports
    /// and legacy observations, which stay explicitly unbound.
    pub subject: Option<SubjectBinding>,
}

/// Typed output captured by an explicit unsafe-review witness execution.
///
/// The wrapped inputs intentionally match the saved-output constructors so
/// callers can preserve the same tool-specific validation and receipt shape
/// while recording truthful execution provenance.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutedReceiptInput {
    Miri(MiriReceiptInput),
    CargoCareful(CargoCarefulReceiptInput),
    Sanitizer(SanitizerReceiptInput),
    Concurrency(ConcurrencyReceiptInput),
    Proof(ProofReceiptInput),
}

impl ExecutedReceiptInput {
    fn terminal_status(&self) -> Option<TerminalStatus> {
        match self {
            Self::Miri(input) => input.terminal_status,
            Self::CargoCareful(input) => input.terminal_status,
            Self::Sanitizer(input) => input.terminal_status,
            Self::Concurrency(input) => input.terminal_status,
            Self::Proof(input) => input.terminal_status,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputProvenance {
    SavedOutput,
    ExecutedCommand,
}

impl OutputProvenance {
    fn summary_prefix(self) -> &'static str {
        match self {
            Self::SavedOutput => "saved",
            Self::ExecutedCommand => "executed",
        }
    }

    fn limitation(self, executor: &str) -> String {
        match self {
            Self::SavedOutput => {
                format!("saved-output adapter; unsafe-review did not run {executor}")
            }
            Self::ExecutedCommand => {
                format!("executed-output adapter; unsafe-review ran {executor}")
            }
        }
    }

    fn captured_output(self, subject: &str) -> String {
        match self {
            Self::SavedOutput => format!("saved {subject} output"),
            Self::ExecutedCommand => format!("executed {subject} output"),
        }
    }
}

struct ClassifiedOutput {
    card_id: String,
    tool: String,
    summary: String,
    verdict: String,
    author: String,
    recorded_at: String,
    expires_at: String,
    command: String,
    executor: &'static str,
    extra_limitations: Vec<String>,
    limitations: Vec<String>,
    subject: Option<SubjectBinding>,
}

impl WitnessReceipt {
    pub fn validate(&self) -> Result<(), String> {
        validate_required(&self.schema_version, "schema_version")?;
        validate_required(&self.card_id, "card_id")?;
        validate_required(&self.tool, "tool")?;
        validate_tool(&self.tool)?;
        validate_strength(&self.strength)?;
        validate_strength_for_tool(&self.tool, &self.strength)?;
        validate_verdict(self.verdict.as_deref())?;
        if receipt_card_id_kind(&self.card_id).is_none() {
            return Err(
                "card_id must be an exact counted UR-* identity ending in -cN or a path-safe manual candidate id"
                    .to_string(),
            );
        }
        let author = validate_required_option(&self.author, "author")?;
        validate_required(author, "author")?;
        let recorded_at = validate_required_option(&self.recorded_at, "recorded_at")?;
        let expires_at = validate_required_option(&self.expires_at, "expires_at")?;
        validate_utc_timestamp(recorded_at, "recorded_at")?;
        validate_date(expires_at, "expires_at")?;
        if expires_at < &recorded_at[..10] {
            return Err("`expires_at` must be on or after the `recorded_at` date".to_string());
        }
        if self.tool == "external-integration-test" {
            validate_required_option(&self.command, "command")?;
        }
        self.validate_command_hash()?;
        Ok(())
    }

    pub fn command_hash(command: &str) -> String {
        stable_hash_hex(command)
    }

    pub fn card_id_kind(card_id: &str) -> Option<ReceiptCardIdKind> {
        receipt_card_id_kind(card_id)
    }

    pub fn evidence_summary(&self) -> String {
        summary::evidence_summary(self)
    }

    pub fn to_pretty_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map(|mut text| {
                text.push('\n');
                text
            })
            .map_err(|err| format!("serialize witness receipt failed: {err}"))
    }

    pub fn from_miri_output(input: MiriReceiptInput) -> Result<Self, String> {
        Self::from_output(
            OutputProvenance::SavedOutput,
            ExecutedReceiptInput::Miri(input),
        )
    }

    pub fn from_cargo_careful_output(input: CargoCarefulReceiptInput) -> Result<Self, String> {
        Self::from_output(
            OutputProvenance::SavedOutput,
            ExecutedReceiptInput::CargoCareful(input),
        )
    }

    pub fn from_sanitizer_output(input: SanitizerReceiptInput) -> Result<Self, String> {
        Self::from_output(
            OutputProvenance::SavedOutput,
            ExecutedReceiptInput::Sanitizer(input),
        )
    }

    pub fn from_concurrency_output(input: ConcurrencyReceiptInput) -> Result<Self, String> {
        Self::from_output(
            OutputProvenance::SavedOutput,
            ExecutedReceiptInput::Concurrency(input),
        )
    }

    pub fn from_proof_output(input: ProofReceiptInput) -> Result<Self, String> {
        Self::from_output(
            OutputProvenance::SavedOutput,
            ExecutedReceiptInput::Proof(input),
        )
    }

    /// Classifies output from an already-executed witness command.
    ///
    /// This constructor does not execute the command. The caller attests that
    /// `output` was captured from the exact `command` recorded in the wrapped
    /// input. The CLI's `confirm --allow-heavy` path provides the stronger
    /// authorization and execution boundary and appends its single-local-run
    /// limitation before calling this classifier.
    pub fn from_executed_output(input: ExecutedReceiptInput) -> Result<Self, String> {
        Self::from_output(OutputProvenance::ExecutedCommand, input)
    }

    fn from_output(
        provenance: OutputProvenance,
        input: ExecutedReceiptInput,
    ) -> Result<Self, String> {
        let terminal = match provenance {
            OutputProvenance::ExecutedCommand => {
                let status = input.terminal_status().ok_or_else(|| {
                    "executed output requires terminal process status; unknown status is never assumed exit 0"
                        .to_string()
                })?;
                if !status.captured_complete {
                    return Err(
                        "executed output capture is incomplete; partial output cannot qualify as evidence"
                            .to_string(),
                    );
                }
                Some(status)
            }
            OutputProvenance::SavedOutput => None,
        };
        let classified = classify_output(provenance, input, terminal)?;
        finalize_output_receipt(provenance, classified, terminal)
    }

    fn validate_command_hash(&self) -> Result<(), String> {
        let Some(command_hash) = self.command_hash.as_deref() else {
            return Ok(());
        };
        validate_required(command_hash, "command_hash")?;
        let command = validate_required_option(&self.command, "command")?;
        let expected = Self::command_hash(command);
        if command_hash == expected {
            Ok(())
        } else {
            Err("`command_hash` does not match `command`".to_string())
        }
    }
}

fn classify_output(
    provenance: OutputProvenance,
    input: ExecutedReceiptInput,
    terminal: Option<TerminalStatus>,
) -> Result<ClassifiedOutput, String> {
    let classified = classify_output_text(provenance, input)?;
    Ok(apply_terminal_gate(classified, terminal))
}

/// A clean exit is required for an unqualified successful run. A failed or
/// signaled process behind success-looking output downgrades the verdict to
/// `inconclusive` with the contradiction surfaced in the summary; observed
/// failure markers (`confirmed`) are preserved as negative evidence.
fn apply_terminal_gate(
    mut classified: ClassifiedOutput,
    terminal: Option<TerminalStatus>,
) -> ClassifiedOutput {
    let Some(status) = terminal else {
        return classified;
    };
    if classified.verdict != "not_reproduced" {
        return classified;
    }
    if !status.signaled && status.exit_code == Some(0) {
        return classified;
    }
    classified.verdict = "inconclusive".to_string();
    classified.summary = format!("{}; {}", classified.summary, status.describe());
    classified.extra_limitations.push(
        "executed process did not report a clean exit; success-looking output is not an unqualified pass"
            .to_string(),
    );
    classified
}

fn classify_output_text(
    provenance: OutputProvenance,
    input: ExecutedReceiptInput,
) -> Result<ClassifiedOutput, String> {
    match input {
        ExecutedReceiptInput::Miri(input) => {
            validate_success_output(provenance, &input.output, "Miri")?;
            validate_required(&input.command, "command")?;
            if !input.command.to_ascii_lowercase().contains("miri") {
                return Err("Miri receipt command must mention `miri`".to_string());
            }
            Ok(ClassifiedOutput {
                card_id: input.card_id,
                tool: "miri".to_string(),
                summary: format!(
                    "{} Miri output reported `test result: ok`",
                    provenance.summary_prefix()
                ),
                verdict: "not_reproduced".to_string(),
                author: input.author,
                recorded_at: input.recorded_at,
                expires_at: input.expires_at,
                command: input.command,
                executor: "Miri",
                extra_limitations: Vec::new(),
                limitations: input.limitations,
                subject: input.subject,
            })
        }
        ExecutedReceiptInput::CargoCareful(input) => {
            validate_success_output(provenance, &input.output, "cargo-careful")?;
            validate_required(&input.command, "command")?;
            if !input.command.to_ascii_lowercase().contains("careful") {
                return Err("cargo-careful receipt command must mention `careful`".to_string());
            }
            Ok(ClassifiedOutput {
                card_id: input.card_id,
                tool: "cargo-careful".to_string(),
                summary: format!(
                    "{} cargo-careful output reported `test result: ok`",
                    provenance.summary_prefix()
                ),
                verdict: "not_reproduced".to_string(),
                author: input.author,
                recorded_at: input.recorded_at,
                expires_at: input.expires_at,
                command: input.command,
                executor: "cargo-careful",
                extra_limitations: Vec::new(),
                limitations: input.limitations,
                subject: input.subject,
            })
        }
        ExecutedReceiptInput::Sanitizer(input) => {
            validate_sanitizer_tool(&input.tool)?;
            validate_required(&input.command, "command")?;
            validate_sanitizer_command(&input.command)?;
            let (summary, verdict, extra_limitations) = if input.allow_runtime {
                sanitizer_runtime_classify(provenance, &input.output, &input.tool)?
            } else {
                // Tool-specific failure markers report before generic
                // validation: a run that failed is failing regardless of how
                // many tests it ran.
                validate_sanitizer_success_output(provenance, &input.output, &input.tool)?;
                validate_success_output(provenance, &input.output, &input.tool)?;
                (
                    format!(
                        "{} {} output reported `test result: ok`",
                        provenance.summary_prefix(),
                        input.tool
                    ),
                    "not_reproduced".to_string(),
                    Vec::new(),
                )
            };
            Ok(ClassifiedOutput {
                card_id: input.card_id,
                tool: input.tool,
                summary,
                verdict,
                author: input.author,
                recorded_at: input.recorded_at,
                expires_at: input.expires_at,
                command: input.command,
                executor: "a sanitizer",
                extra_limitations,
                limitations: input.limitations,
                subject: input.subject,
            })
        }
        ExecutedReceiptInput::Concurrency(input) => {
            validate_concurrency_tool(&input.tool)?;
            validate_success_output(provenance, &input.output, &input.tool)?;
            validate_required(&input.command, "command")?;
            validate_concurrency_command(&input.command)?;
            Ok(ClassifiedOutput {
                card_id: input.card_id,
                summary: format!(
                    "{} {} output reported `test result: ok`",
                    provenance.summary_prefix(),
                    input.tool
                ),
                tool: input.tool,
                verdict: "not_reproduced".to_string(),
                author: input.author,
                recorded_at: input.recorded_at,
                expires_at: input.expires_at,
                command: input.command,
                executor: "a concurrency witness",
                extra_limitations: Vec::new(),
                limitations: input.limitations,
                subject: input.subject,
            })
        }
        ExecutedReceiptInput::Proof(input) => {
            validate_proof_tool(&input.tool)?;
            validate_proof_success_output(provenance, &input.output, &input.tool)?;
            validate_required(&input.command, "command")?;
            validate_proof_command(&input.command)?;
            Ok(ClassifiedOutput {
                card_id: input.card_id,
                summary: format!(
                    "{} {} proof output reported verification success",
                    provenance.summary_prefix(),
                    input.tool
                ),
                tool: input.tool,
                verdict: "not_reproduced".to_string(),
                author: input.author,
                recorded_at: input.recorded_at,
                expires_at: input.expires_at,
                command: input.command,
                executor: "a proof tool",
                extra_limitations: vec![
                    "proof scope is limited to the recorded harness/output".to_string(),
                ],
                limitations: input.limitations,
                subject: input.subject,
            })
        }
    }
}

fn finalize_output_receipt(
    provenance: OutputProvenance,
    classified: ClassifiedOutput,
    terminal: Option<TerminalStatus>,
) -> Result<WitnessReceipt, String> {
    let command_hash = WitnessReceipt::command_hash(&classified.command);
    let mut limitations = vec![
        provenance.limitation(classified.executor),
        "receipt strength is `ran`; site reach is not claimed".to_string(),
    ];
    limitations.extend(classified.extra_limitations);
    limitations.extend(classified.limitations);
    let receipt = WitnessReceipt {
        schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
        card_id: classified.card_id,
        tool: classified.tool,
        strength: "ran".to_string(),
        author: Some(classified.author),
        recorded_at: Some(classified.recorded_at),
        expires_at: Some(classified.expires_at),
        summary: Some(classified.summary),
        command: Some(classified.command),
        command_hash: Some(command_hash),
        limitations: Some(limitations),
        verdict: Some(classified.verdict),
        subject: classified.subject,
        exit_code: terminal.and_then(|status| status.exit_code),
        terminated_by_signal: terminal.map(|status| status.signaled),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn is_supported_receipt_strength(value: &str) -> bool {
    matches!(
        value,
        "configured" | "ran" | "test_targeted" | "site_reached" | "reviewed"
    )
}

fn is_supported_receipt_tool(value: &str) -> bool {
    matches!(
        value,
        "miri"
            | "cargo-careful"
            | "asan"
            | "msan"
            | "tsan"
            | "lsan"
            | "loom"
            | "shuttle"
            | "kani"
            | "crux"
            | "human-deep-review"
            | "external-integration-test"
            | "unsupported"
    )
}

fn validate_required(value: &str, key: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("`{key}` is required"))
    } else {
        Ok(())
    }
}

fn validate_required_option<'a>(value: &'a Option<String>, key: &str) -> Result<&'a str, String> {
    let Some(value) = value.as_deref() else {
        return Err(format!("`{key}` is required"));
    };
    validate_required(value, key)?;
    Ok(value)
}

fn validate_verdict(value: Option<&str>) -> Result<(), String> {
    match value {
        // Absent keeps back-compat with every receipt written before the
        // verdict field existed.
        None => Ok(()),
        Some("confirmed" | "not_reproduced" | "inconclusive") => Ok(()),
        Some(other) => Err(format!(
            "uses unknown receipt verdict `{other}`; expected `confirmed`, `not_reproduced`, or `inconclusive` (or omit the field)"
        )),
    }
}

fn validate_strength(value: &str) -> Result<(), String> {
    if is_supported_receipt_strength(value) {
        Ok(())
    } else {
        Err(format!(
            "uses unknown receipt strength `{value}`; valid values: `configured`, `ran`, `test_targeted`, `site_reached`, `reviewed`"
        ))
    }
}

fn validate_strength_for_tool(tool: &str, strength: &str) -> Result<(), String> {
    if strength == "reviewed" && tool != "human-deep-review" {
        return Err(
            "receipt strength `reviewed` is only supported for `human-deep-review` receipts"
                .to_string(),
        );
    }
    if tool == "external-integration-test" && strength != "site_reached" {
        return Err(
            "external-integration-test receipt strength must be `site_reached`".to_string(),
        );
    }
    // loom/shuttle/kani/crux receipts cannot claim `site_reached` via a
    // hand-authored template: the tool-output import adapters
    // (`from_concurrency_output`, `from_proof_output`) only produce `ran`
    // strength; a hand-authored `site_reached` claim for these tools cannot
    // carry verifiable tool provenance, so it is rejected here. Use
    // `external-integration-test` with `site_reached` for integration-level
    // site-reach evidence, or import a tool-output receipt (`ran`) via the
    // appropriate adapter.
    if matches!(tool, "loom" | "shuttle" | "kani" | "crux") && strength == "site_reached" {
        return Err(format!(
            "receipt tool `{tool}` does not support `site_reached` strength in a hand-authored receipt; \
             the tool-output import adapters only produce `ran` strength, which carries verifiable tool provenance; \
             use `external-integration-test` with `site_reached` for integration-level reach evidence"
        ));
    }
    Ok(())
}

fn validate_tool(value: &str) -> Result<(), String> {
    if is_supported_receipt_tool(value) {
        Ok(())
    } else {
        Err(format!("uses unknown receipt tool `{value}`"))
    }
}

fn validate_sanitizer_tool(value: &str) -> Result<(), String> {
    if matches!(value, "asan" | "msan" | "tsan" | "lsan") {
        Ok(())
    } else {
        Err(format!(
            "sanitizer receipt tool must be one of `asan`, `msan`, `tsan`, or `lsan`, got `{value}`"
        ))
    }
}

fn validate_concurrency_tool(value: &str) -> Result<(), String> {
    if matches!(value, "loom" | "shuttle") {
        Ok(())
    } else {
        Err(format!(
            "concurrency receipt tool must be one of `loom` or `shuttle`, got `{value}`"
        ))
    }
}

fn validate_proof_tool(value: &str) -> Result<(), String> {
    if matches!(value, "kani" | "crux") {
        Ok(())
    } else {
        Err(format!(
            "proof receipt tool must be one of `kani` or `crux`, got `{value}`"
        ))
    }
}

fn validate_sanitizer_command(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    if ["sanitizer", "asan", "msan", "tsan", "lsan"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        Ok(())
    } else {
        Err(
            "sanitizer receipt command must mention `sanitizer`, `asan`, `msan`, `tsan`, or `lsan`"
                .to_string(),
        )
    }
}

fn validate_concurrency_command(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    if ["loom", "shuttle"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        Ok(())
    } else {
        Err("concurrency receipt command must mention `loom` or `shuttle`".to_string())
    }
}

fn validate_proof_command(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    if ["kani", "crux"].iter().any(|needle| lower.contains(needle)) {
        Ok(())
    } else {
        Err("proof receipt command must mention `kani` or `crux`".to_string())
    }
}

fn validate_success_output(
    provenance: OutputProvenance,
    output: &str,
    tool: &str,
) -> Result<(), String> {
    let captured_output = provenance.captured_output(tool);
    if output.trim().is_empty() {
        return Err(format!("{captured_output} is empty"));
    }
    let lower = output.to_ascii_lowercase();
    for needle in [
        "undefined behavior",
        "test result: failed",
        "failures:",
        "panicked at",
        "error:",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "{captured_output} contains failure marker `{needle}`"
            ));
        }
    }
    if !lower.contains("test result: ok") {
        return Err(format!("{captured_output} must contain `test result: ok`"));
    }
    if executed_pass_count(&lower) == 0 {
        return Err(format!(
            "{captured_output} reports `test result: ok` with 0 passed: no test executed, so the run cannot qualify as witness evidence"
        ));
    }
    Ok(())
}

/// Number of executed passes across libtest result summaries in `output`.
///
/// `cargo test` prints one `test result: ok. N passed; ...` summary per
/// target, and an owner filter matching nothing still exits 0 with every
/// summary at zero. Evidence requires at least one executed pass somewhere;
/// per-target zeros (e.g. an empty bin target next to a tested lib) stay
/// acceptable.
fn executed_pass_count(lower: &str) -> u64 {
    let mut total = 0u64;
    let mut search_from = 0usize;
    while let Some(pos) = lower[search_from..].find(" passed") {
        let abs_pos = search_from + pos;
        let digits: String = lower[..abs_pos]
            .chars()
            .rev()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if let Ok(count) = digits.parse::<u64>() {
            total = total.saturating_add(count);
        }
        search_from = abs_pos + " passed".len();
    }
    total
}

fn validate_proof_success_output(
    provenance: OutputProvenance,
    output: &str,
    tool: &str,
) -> Result<(), String> {
    let captured_output = provenance.captured_output(&format!("{tool} proof"));
    if output.trim().is_empty() {
        return Err(format!("{captured_output} is empty"));
    }
    let lower = output.to_ascii_lowercase();
    for needle in [
        "verification failed",
        "verification:- failed",
        "counterexample",
        "assertion failed",
        "panicked at",
        "panic",
        "error:",
        "failed",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "{captured_output} contains failure marker `{needle}`"
            ));
        }
    }
    if [
        "verification:- successful",
        "verification successful",
        "verification result: verified",
        "proof successful",
        "status: verified",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Ok(())
    } else {
        Err(format!(
            "{captured_output} must contain a verification success marker"
        ))
    }
}

fn validate_sanitizer_success_output(
    provenance: OutputProvenance,
    output: &str,
    tool: &str,
) -> Result<(), String> {
    let captured_output = provenance.captured_output(tool);
    let lower = output.to_ascii_lowercase();
    for needle in [
        "addresssanitizer:",
        "memorysanitizer:",
        "threadsanitizer:",
        "leaksanitizer:",
        "detected memory leaks",
        "data race",
        "deadlysignal",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "{captured_output} contains sanitizer failure marker `{needle}`"
            ));
        }
    }
    Ok(())
}

/// Classify a runtime (non-cargo-test) sanitizer run.
///
/// Returns `(summary, verdict, extra_limitations)`.
/// A run with sanitizer markers → `confirmed` (signal observed, not a safety
/// claim). A clean run → `not_reproduced` (no signal this run, not a safety
/// claim).
fn sanitizer_runtime_classify(
    provenance: OutputProvenance,
    output: &str,
    tool: &str,
) -> Result<(String, String, Vec<String>), String> {
    if output.trim().is_empty() {
        return Err(format!("{} is empty", provenance.captured_output(tool)));
    }
    let lower = output.to_ascii_lowercase();
    let sanitizer_fired = [
        "addresssanitizer:",
        "memorysanitizer:",
        "threadsanitizer:",
        "leaksanitizer:",
        "detected memory leaks",
        "data race",
        "deadlysignal",
        "undefined behavior",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    let extra =
        vec!["runtime witness mode: this was a program run, not a cargo test harness".to_string()];
    if sanitizer_fired {
        Ok((
            format!("{tool} runtime run: sanitizer signal observed"),
            "confirmed".to_string(),
            extra,
        ))
    } else {
        Ok((
            format!("{tool} runtime run: clean runtime run, no sanitizer signal observed"),
            "not_reproduced".to_string(),
            extra,
        ))
    }
}

fn validate_utc_timestamp(value: &str, key: &str) -> Result<(), String> {
    if !timestamp_validation::has_utc_shape(value) {
        return Err(format!(
            "`{key}` must use UTC timestamp format YYYY-MM-DDTHH:MM:SSZ"
        ));
    }
    validate_date(&value[..10], key)?;
    timestamp_validation::validate_time_components(value, key)
}

fn validate_date(value: &str, key: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|index| bytes[*index].is_ascii_digit());
    if !valid_shape {
        return Err(format!("`{key}` must use date format YYYY-MM-DD"));
    }
    let year = decimal_at(value, 0, 4);
    let month = decimal_at(value, 5, 2);
    let day = decimal_at(value, 8, 2);
    validate_range(year, 1, 9999, key)?;
    validate_range(month, 1, 12, key)?;
    validate_range(day, 1, 31, key)?;
    let Some(year) = year else {
        return Err(format!("`{key}` contains an invalid number"));
    };
    let Some(month) = month else {
        return Err(format!("`{key}` contains an invalid number"));
    };
    let Some(day) = day else {
        return Err(format!("`{key}` contains an invalid number"));
    };
    let Some(max_day) = days_in_month(year, month) else {
        return Err(format!("`{key}` is out of range"));
    };
    if day > max_day {
        return Err(format!("`{key}` is not a valid calendar date"));
    }
    Ok(())
}

fn decimal_at(value: &str, start: usize, len: usize) -> Option<u32> {
    value.get(start..start + len)?.parse().ok()
}

fn days_in_month(year: u32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn validate_range(value: Option<u32>, min: u32, max: u32, key: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Err(format!("`{key}` contains an invalid number"));
    };
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("`{key}` is out of range"))
    }
}

fn looks_like_counted_card_id(value: &str) -> bool {
    let Some((prefix, count)) = value.rsplit_once("-c") else {
        return false;
    };
    value.starts_with("UR-")
        && !prefix.is_empty()
        && !count.is_empty()
        && count.bytes().all(|byte| byte.is_ascii_digit())
}

fn receipt_card_id_kind(value: &str) -> Option<ReceiptCardIdKind> {
    if looks_like_counted_card_id(value) {
        return Some(ReceiptCardIdKind::AnalyzerReviewCard);
    }
    if looks_like_manual_candidate_id(value) {
        return Some(ReceiptCardIdKind::ManualCandidate);
    }
    None
}

fn looks_like_manual_candidate_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && !value.starts_with("UR-")
        && !value.contains('/')
        && !value.contains('\\')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

mod timestamp_validation {
    use super::{decimal_at, validate_range};

    pub(super) fn has_utc_shape(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 20
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'T'
            && bytes[13] == b':'
            && bytes[16] == b':'
            && bytes[19] == b'Z'
            && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
                .iter()
                .all(|index| bytes[*index].is_ascii_digit())
    }

    pub(super) fn validate_time_components(value: &str, key: &str) -> Result<(), String> {
        validate_range(decimal_at(value, 11, 2), 0, 23, key)?;
        validate_range(decimal_at(value, 14, 2), 0, 59, key)?;
        validate_range(decimal_at(value, 17, 2), 0, 59, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_receipt_json_round_trips_and_validates() -> Result<(), String> {
        let receipt = fixture_receipt();
        receipt.validate()?;

        let json = receipt.to_pretty_json()?;
        let decoded: WitnessReceipt = serde_json::from_str(&json)
            .map_err(|err| format!("deserialize receipt failed: {err}"))?;

        assert_eq!(decoded, receipt);
        assert!(decoded.evidence_summary().contains("Imported miri receipt"));
        assert!(decoded.evidence_summary().contains("fixture only"));
        Ok(())
    }

    #[test]
    fn witness_receipt_validation_accepts_missing_command_hash_for_compatibility()
    -> Result<(), String> {
        let mut receipt = fixture_receipt();
        receipt.command_hash = None;

        receipt.validate()
    }

    #[test]
    fn witness_receipt_validation_rejects_command_hash_mismatch() {
        let mut receipt = fixture_receipt();
        receipt.command_hash = Some("0000000000000000".to_string());

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("command_hash")
        );
    }

    #[test]
    fn witness_receipt_validation_accepts_manual_candidate_ids() -> Result<(), String> {
        let mut receipt = fixture_receipt();
        receipt.card_id = "R4R2-S001".to_string();

        receipt.validate()?;
        assert_eq!(
            WitnessReceipt::card_id_kind(&receipt.card_id),
            Some(ReceiptCardIdKind::ManualCandidate)
        );
        assert_eq!(
            WitnessReceipt::card_id_kind(
                "UR-crate-src-lib-rs-owner-operation-read-read-deadbeef1234-alignment-c1"
            ),
            Some(ReceiptCardIdKind::AnalyzerReviewCard)
        );
        assert!(WitnessReceipt::card_id_kind("UR-not-counted").is_none());
        assert!(WitnessReceipt::card_id_kind("../R4R2-S001").is_none());
        Ok(())
    }

    #[test]
    fn evidence_summary_omits_whitespace_only_optional_fields() {
        let mut receipt = fixture_receipt();
        receipt.summary = Some(" ".to_string());
        receipt.author = Some("\t".to_string());
        receipt.recorded_at = Some("\n".to_string());
        receipt.expires_at = Some("   ".to_string());
        receipt.command = Some("\r\n".to_string());
        receipt.limitations = Some(vec!["fixture only".to_string()]);

        assert_eq!(
            receipt.evidence_summary(),
            "Imported miri receipt with `ran` strength; command_hash: 3e163b0bce29ff2e; limitations: fixture only"
        );
    }

    #[test]
    fn witness_receipt_validation_accepts_absent_and_known_verdicts() -> Result<(), String> {
        let mut receipt = fixture_receipt();
        receipt.verdict = None;
        receipt.validate()?;

        for verdict in ["confirmed", "not_reproduced", "inconclusive"] {
            receipt.verdict = Some(verdict.to_string());
            receipt.validate()?;
        }
        Ok(())
    }

    #[test]
    fn witness_receipt_validation_rejects_unknown_verdict() {
        let mut receipt = fixture_receipt();
        receipt.verdict = Some("proved-safe".to_string());

        let err = receipt.validate().err().unwrap_or_default();
        assert!(err.contains("unknown receipt verdict `proved-safe`"));
        assert!(err.contains("`confirmed`, `not_reproduced`, or `inconclusive`"));
    }

    #[test]
    fn witness_receipt_json_without_verdict_field_still_parses() -> Result<(), String> {
        let json = r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-08-18"
}"#;
        let receipt: WitnessReceipt = serde_json::from_str(json)
            .map_err(|err| format!("deserialize receipt without verdict failed: {err}"))?;
        assert_eq!(receipt.verdict, None);
        receipt.validate()?;
        let rendered = receipt.to_pretty_json()?;
        assert!(
            !rendered.contains("verdict"),
            "absent verdict must stay absent on re-serialization: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn saved_output_constructors_record_not_reproduced_verdict() -> Result<(), String> {
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let miri = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: card_id.to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;
        assert_eq!(miri.verdict.as_deref(), Some("not_reproduced"));

        let proof = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id: card_id.to_string(),
            tool: "kani".to_string(),
            output: "VERIFICATION:- SUCCESSFUL\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;
        assert_eq!(proof.verdict.as_deref(), Some("not_reproduced"));
        Ok(())
    }

    #[test]
    fn witness_receipt_validation_rejects_unknown_tool() {
        let mut receipt = fixture_receipt();
        receipt.tool = "proof-bot".to_string();

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("unknown receipt tool")
        );
    }

    #[test]
    fn witness_receipt_validation_accepts_reviewed_human_deep_review() -> Result<(), String> {
        let mut receipt = fixture_receipt();
        receipt.tool = "human-deep-review".to_string();
        receipt.strength = "reviewed".to_string();

        receipt.validate()?;
        assert!(
            receipt
                .evidence_summary()
                .contains("human-deep-review receipt with `reviewed` strength")
        );
        Ok(())
    }

    #[test]
    fn witness_receipt_validation_rejects_reviewed_executable_tool() {
        let mut receipt = fixture_receipt();
        receipt.strength = "reviewed".to_string();

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("only supported for `human-deep-review`")
        );
    }

    #[test]
    fn witness_receipt_validation_accepts_external_integration_site_reached() -> Result<(), String>
    {
        let mut receipt = fixture_receipt();
        receipt.tool = "external-integration-test".to_string();
        receipt.strength = "site_reached".to_string();
        receipt.command = Some("bun test test/js/sab-copy-to-unshared.test.ts".to_string());
        receipt.command_hash = receipt.command.as_deref().map(WitnessReceipt::command_hash);

        receipt.validate()?;
        assert!(
            receipt
                .evidence_summary()
                .contains("external-integration-test receipt with `site_reached` strength")
        );
        Ok(())
    }

    #[test]
    fn witness_receipt_validation_rejects_external_integration_without_site_reached() {
        let mut receipt = fixture_receipt();
        receipt.tool = "external-integration-test".to_string();
        receipt.strength = "ran".to_string();

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("external-integration-test receipt strength must be `site_reached`")
        );
    }

    #[test]
    fn witness_receipt_validation_rejects_external_integration_without_command() {
        let mut receipt = fixture_receipt();
        receipt.tool = "external-integration-test".to_string();
        receipt.strength = "site_reached".to_string();
        receipt.command = None;
        receipt.command_hash = None;

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("`command` is required")
        );
    }

    #[test]
    fn witness_receipt_validation_rejects_missing_author() {
        let mut receipt = fixture_receipt();
        receipt.author = None;

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("`author` is required")
        );
    }

    #[test]
    fn witness_receipt_validation_rejects_invalid_calendar_dates() {
        let mut bad_expiry = fixture_receipt();
        bad_expiry.expires_at = Some("2026-02-29".to_string());
        assert!(
            bad_expiry
                .validate()
                .err()
                .unwrap_or_default()
                .contains("valid calendar date")
        );

        let mut bad_recorded_at = fixture_receipt();
        bad_recorded_at.recorded_at = Some("2026-04-31T00:00:00Z".to_string());
        assert!(
            bad_recorded_at
                .validate()
                .err()
                .unwrap_or_default()
                .contains("valid calendar date")
        );
    }

    #[test]
    fn witness_receipt_validation_accepts_leap_day_dates() -> Result<(), String> {
        let mut receipt = fixture_receipt();
        receipt.recorded_at = Some("2028-02-29T00:00:00Z".to_string());
        receipt.expires_at = Some("2028-02-29".to_string());

        receipt.validate()
    }

    #[test]
    fn miri_receipt_from_saved_output_uses_ran_strength_without_site_reach() -> Result<(), String> {
        let receipt = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;

        assert_eq!(receipt.tool, "miri");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved Miri output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run Miri"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn executed_output_records_truthful_provenance_without_changing_receipt_shape()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_executed_output(ExecutedReceiptInput::Miri(
            MiriReceiptInput {
                card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                    .to_string(),
                output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-05-18T00:00:00Z".to_string(),
                expires_at: "2026-08-18".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                limitations: vec!["single explicit run".to_string()],
                terminal_status: Some(TerminalStatus::exited(0)),
                subject: None,
            },
        ))?;

        assert_eq!(
            receipt.summary.as_deref(),
            Some("executed Miri output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item == "executed-output adapter; unsafe-review ran Miri")
        );
        assert!(limitations.iter().all(|item| !item.contains("did not run")));
        assert_eq!(receipt.verdict.as_deref(), Some("not_reproduced"));

        let value = serde_json::to_value(&receipt)
            .map_err(|err| format!("serialize executed receipt failed: {err}"))?;
        let object = value
            .as_object()
            .ok_or("executed receipt did not serialize as an object")?;
        for key in [
            "schema_version",
            "card_id",
            "tool",
            "strength",
            "author",
            "recorded_at",
            "expires_at",
            "summary",
            "command",
            "command_hash",
            "limitations",
            "verdict",
            "exit_code",
            "terminated_by_signal",
        ] {
            assert!(object.contains_key(key), "missing receipt field `{key}`");
        }
        assert_eq!(object.len(), 14);
        assert_eq!(receipt.exit_code, Some(0));
        assert_eq!(receipt.terminated_by_signal, Some(false));
        Ok(())
    }

    fn executed_miri_input(output: &str, terminal: Option<TerminalStatus>) -> ExecutedReceiptInput {
        ExecutedReceiptInput::Miri(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: output.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: terminal,
            subject: None,
        })
    }

    const OK_OUTPUT: &str = "test result: ok. 1 passed; 0 failed; finished in 0.01s\n";

    #[test]
    fn executed_nonzero_exit_behind_success_output_is_inconclusive() -> Result<(), String> {
        let receipt = WitnessReceipt::from_executed_output(executed_miri_input(
            OK_OUTPUT,
            Some(TerminalStatus::exited(7)),
        ))?;

        assert_eq!(receipt.verdict.as_deref(), Some("inconclusive"));
        assert_eq!(receipt.exit_code, Some(7));
        assert_eq!(receipt.terminated_by_signal, Some(false));
        let summary = receipt.summary.as_deref().ok_or("missing summary")?;
        assert!(summary.contains("exited nonzero"), "summary: {summary}");
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("not an unqualified pass")),
            "limitations: {limitations:?}"
        );
        Ok(())
    }

    #[test]
    fn executed_signal_termination_behind_success_output_is_inconclusive() -> Result<(), String> {
        let receipt = WitnessReceipt::from_executed_output(executed_miri_input(
            OK_OUTPUT,
            Some(TerminalStatus::signaled()),
        ))?;

        assert_eq!(receipt.verdict.as_deref(), Some("inconclusive"));
        assert_eq!(receipt.exit_code, None);
        assert_eq!(receipt.terminated_by_signal, Some(true));
        let summary = receipt.summary.as_deref().ok_or("missing summary")?;
        assert!(
            summary.contains("terminated by a signal"),
            "summary: {summary}"
        );
        Ok(())
    }

    #[test]
    fn executed_unknown_terminal_status_is_rejected_never_assumed_clean() {
        let result = WitnessReceipt::from_executed_output(executed_miri_input(OK_OUTPUT, None));

        assert_eq!(
            result,
            Err("executed output requires terminal process status; unknown status is never assumed exit 0"
                .to_string())
        );
    }

    #[test]
    fn executed_zero_passed_success_output_is_rejected_as_vacuous() {
        // A filter matching zero tests still prints `test result: ok` with
        // exit 0. Recording that as a receipt would let a run that executed
        // nothing upgrade witness evidence.
        let vacuous = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s\n";
        let result = WitnessReceipt::from_executed_output(executed_miri_input(
            vacuous,
            Some(TerminalStatus::exited(0)),
        ));

        assert_eq!(
            result,
            Err("executed Miri output reports `test result: ok` with 0 passed: no test executed, so the run cannot qualify as witness evidence"
                .to_string())
        );
    }

    #[test]
    fn executed_multi_target_run_with_one_passing_target_is_accepted() -> Result<(), String> {
        // `cargo test` prints one summary per target; an empty bin target
        // reports 0 passed next to a lib target that ran. Evidence requires
        // at least one executed pass, not every summary nonzero.
        let mixed = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; finished in 0.01s\n\nrunning 2 tests\ntest ok_case ... ok\ntest second ... ok\n\ntest result: ok. 2 passed; 0 failed; finished in 0.02s\n";
        let receipt = WitnessReceipt::from_executed_output(executed_miri_input(
            mixed,
            Some(TerminalStatus::exited(0)),
        ))?;

        assert_eq!(receipt.verdict.as_deref(), Some("not_reproduced"));
        Ok(())
    }

    #[test]
    fn executed_incomplete_capture_is_rejected() {
        let result = WitnessReceipt::from_executed_output(executed_miri_input(
            OK_OUTPUT,
            Some(TerminalStatus::unknown()),
        ));

        assert_eq!(
            result,
            Err(
                "executed output capture is incomplete; partial output cannot qualify as evidence"
                    .to_string()
            )
        );
    }

    #[test]
    fn saved_output_ignores_terminal_status_for_legacy_compatibility() -> Result<(), String> {
        let receipt = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: OK_OUTPUT.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(7)),
            subject: None,
        })?;

        assert_eq!(receipt.verdict.as_deref(), Some("not_reproduced"));
        assert_eq!(receipt.exit_code, None);
        assert_eq!(receipt.terminated_by_signal, None);
        Ok(())
    }

    #[test]
    fn executed_sanitizer_markers_stay_confirmed_on_nonzero_exit() -> Result<(), String> {
        let receipt = WitnessReceipt::from_executed_output(ExecutedReceiptInput::Sanitizer(
            SanitizerReceiptInput {
                card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                    .to_string(),
                tool: "asan".to_string(),
                output: "AddressSanitizer: use-after-free\n".to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-05-18T00:00:00Z".to_string(),
                expires_at: "2026-08-18".to_string(),
                command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header"
                    .to_string(),
                limitations: Vec::new(),
                terminal_status: Some(TerminalStatus::exited(1)),
                subject: None,
                allow_runtime: true,
            },
        ))?;

        assert_eq!(receipt.verdict.as_deref(), Some("confirmed"));
        assert_eq!(receipt.exit_code, Some(1));
        Ok(())
    }

    fn bound_subject() -> SubjectBinding {
        SubjectBinding {
            subject_digest: SubjectBinding::digest_subject(&[
                "UR-test-c1",
                "nonnull_unchecked",
                "owner",
                "src/lib.rs",
                "NonNull::new_unchecked(p)",
            ]),
            scope: Some("repo".to_string()),
            invocation_digest: Some(SubjectBinding::digest_invocation(
                "cargo",
                &["test".to_string()],
                &[],
            )),
            head_commit: Some("abc123".to_string()),
            repo_dirty: Some(false),
            workdir: Some(".".to_string()),
            output_digest: Some(SubjectBinding::digest_output("test result: ok\n")),
            captured_complete: Some(true),
            tool_version: Some("0.5.0".to_string()),
        }
    }

    #[test]
    fn bound_observation_applies_to_identical_subject() {
        let binding = bound_subject();
        assert_eq!(
            SubjectBinding::applicability(Some(&binding), &binding),
            SubjectApplicability::Applicable
        );
    }

    #[test]
    fn digest_subject_framing_separates_embedded_newlines() {
        assert_ne!(
            SubjectBinding::digest_subject(&["a\nb"]),
            SubjectBinding::digest_subject(&["a", "b"])
        );
    }

    #[test]
    fn revisionless_bindings_on_both_sides_are_unknown() {
        let current = bound_subject();
        let mut recorded = current.clone();
        recorded.head_commit = None;
        let mut now = current.clone();
        now.head_commit = None;
        assert!(matches!(
            SubjectBinding::applicability(Some(&recorded), &now),
            SubjectApplicability::Unknown { .. }
        ));
    }

    #[test]
    fn different_invocations_are_not_applicable() {
        let current = bound_subject();
        let mut recorded = current.clone();
        recorded.invocation_digest = Some(SubjectBinding::digest_invocation(
            "cargo",
            &["test".to_string(), "--release".to_string()],
            &[],
        ));
        assert!(matches!(
            SubjectBinding::applicability(Some(&recorded), &current),
            SubjectApplicability::Unknown { .. }
        ));
    }

    #[test]
    fn missing_invocation_digest_matches_nothing() {
        let current = bound_subject();
        let mut recorded = current.clone();
        recorded.invocation_digest = None;
        assert!(matches!(
            SubjectBinding::applicability(Some(&recorded), &current),
            SubjectApplicability::Unknown { .. }
        ));
    }

    #[test]
    fn unestablished_clean_tree_on_both_sides_is_unknown() {
        let current = bound_subject();
        let mut recorded = current.clone();
        recorded.repo_dirty = None;
        let mut now = current.clone();
        now.repo_dirty = None;
        assert!(matches!(
            SubjectBinding::applicability(Some(&recorded), &now),
            SubjectApplicability::Unknown { .. }
        ));
    }

    #[test]
    fn unbound_legacy_observation_is_unknown_never_applicable() {
        assert_eq!(
            SubjectBinding::applicability(None, &bound_subject()),
            SubjectApplicability::Unknown {
                reason: "unbound legacy receipt carries no subject identity".to_string(),
            }
        );
    }

    #[test]
    fn changed_subject_scope_or_revision_is_stale() {
        let current = bound_subject();
        let mut changed = current.clone();
        changed.subject_digest = "different".to_string();
        assert!(matches!(
            SubjectBinding::applicability(Some(&changed), &current),
            SubjectApplicability::Stale { .. }
        ));

        let mut rescoped = current.clone();
        rescoped.scope = Some("diff".to_string());
        assert!(matches!(
            SubjectBinding::applicability(Some(&rescoped), &current),
            SubjectApplicability::Stale { .. }
        ));

        let mut revised = current.clone();
        revised.head_commit = Some("def456".to_string());
        assert!(matches!(
            SubjectBinding::applicability(Some(&revised), &current),
            SubjectApplicability::Stale { .. }
        ));
    }

    #[test]
    fn dirty_or_incomplete_capture_is_unknown() {
        let current = bound_subject();
        let mut dirty_recorded = current.clone();
        dirty_recorded.repo_dirty = Some(true);
        assert!(matches!(
            SubjectBinding::applicability(Some(&dirty_recorded), &current),
            SubjectApplicability::Unknown { .. }
        ));

        let mut dirty_current = current.clone();
        dirty_current.repo_dirty = Some(true);
        assert!(matches!(
            SubjectBinding::applicability(Some(&current), &dirty_current),
            SubjectApplicability::Unknown { .. }
        ));

        let mut partial = current.clone();
        partial.captured_complete = Some(false);
        assert!(matches!(
            SubjectBinding::applicability(Some(&partial), &current),
            SubjectApplicability::Unknown { .. }
        ));
    }

    #[test]
    fn executed_receipt_carries_subject_binding() -> Result<(), String> {
        let receipt = WitnessReceipt::from_executed_output(ExecutedReceiptInput::Miri(
            MiriReceiptInput {
                card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                    .to_string(),
                output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-05-18T00:00:00Z".to_string(),
                expires_at: "2026-08-18".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                limitations: Vec::new(),
                terminal_status: Some(TerminalStatus::exited(0)),
                subject: Some(bound_subject()),
            },
        ))?;

        let subject = receipt.subject.as_ref().ok_or("missing subject")?;
        assert_eq!(
            SubjectBinding::applicability(Some(subject), &bound_subject()),
            SubjectApplicability::Applicable
        );
        Ok(())
    }

    #[test]
    fn saved_receipt_stays_unbound_legacy() -> Result<(), String> {
        let receipt = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: None,
            subject: None,
        })?;

        assert_eq!(receipt.subject, None);
        assert!(matches!(
            SubjectBinding::applicability(receipt.subject.as_ref(), &bound_subject()),
            SubjectApplicability::Unknown { .. }
        ));
        Ok(())
    }

    #[test]
    fn executed_output_supports_all_five_typed_input_variants() -> Result<(), String> {
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let output = "test result: ok. 1 passed; 0 failed; finished in 0.01s\n";
        let receipts = vec![
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Miri(MiriReceiptInput {
                card_id: card_id.to_string(),
                output: output.to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-05-18T00:00:00Z".to_string(),
                expires_at: "2026-08-18".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                limitations: Vec::new(),
                terminal_status: Some(TerminalStatus::exited(0)),
                subject: None,
            }))?,
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::CargoCareful(
                CargoCarefulReceiptInput {
                    card_id: card_id.to_string(),
                    output: output.to_string(),
                    author: "core/fixtures".to_string(),
                    recorded_at: "2026-05-18T00:00:00Z".to_string(),
                    expires_at: "2026-08-18".to_string(),
                    command: "cargo +nightly careful test read_header".to_string(),
                    limitations: Vec::new(),
                    terminal_status: Some(TerminalStatus::exited(0)),
                    subject: None,
                },
            ))?,
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Sanitizer(
                SanitizerReceiptInput {
                    card_id: card_id.to_string(),
                    tool: "asan".to_string(),
                    output: output.to_string(),
                    author: "core/fixtures".to_string(),
                    recorded_at: "2026-05-18T00:00:00Z".to_string(),
                    expires_at: "2026-08-18".to_string(),
                    command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header"
                        .to_string(),
                    limitations: Vec::new(),
                    terminal_status: Some(TerminalStatus::exited(0)),
                    subject: None,
                    allow_runtime: false,
                },
            ))?,
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Concurrency(
                ConcurrencyReceiptInput {
                    card_id: card_id.to_string(),
                    tool: "loom".to_string(),
                    output: output.to_string(),
                    author: "core/fixtures".to_string(),
                    recorded_at: "2026-05-18T00:00:00Z".to_string(),
                    expires_at: "2026-08-18".to_string(),
                    command: "cargo test --features loom read_header".to_string(),
                    limitations: Vec::new(),
                    terminal_status: Some(TerminalStatus::exited(0)),
                    subject: None,
                },
            ))?,
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Proof(ProofReceiptInput {
                card_id: card_id.to_string(),
                tool: "kani".to_string(),
                output: "verification result: verified\n".to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-05-18T00:00:00Z".to_string(),
                expires_at: "2026-08-18".to_string(),
                command: "cargo kani --harness read_header".to_string(),
                limitations: Vec::new(),
                terminal_status: Some(TerminalStatus::exited(0)),
                subject: None,
            }))?,
        ];

        assert_eq!(
            receipts
                .iter()
                .map(|receipt| receipt.tool.as_str())
                .collect::<Vec<_>>(),
            vec!["miri", "cargo-careful", "asan", "loom", "kani"]
        );
        for receipt in receipts {
            let limitations = receipt.limitations.ok_or("missing limitations")?;
            assert!(
                limitations
                    .iter()
                    .any(|item| item.starts_with("executed-output adapter; unsafe-review ran"))
            );
            assert!(limitations.iter().all(|item| !item.contains("did not run")));
        }
        Ok(())
    }

    #[test]
    fn output_validation_errors_preserve_saved_and_executed_provenance() {
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let miri = |output: &str| MiriReceiptInput {
            card_id: card_id.to_string(),
            output: output.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        };
        assert_eq!(
            WitnessReceipt::from_miri_output(miri("warning: nothing ran\n")),
            Err("saved Miri output must contain `test result: ok`".to_string())
        );
        assert_eq!(
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Miri(miri(
                "warning: nothing ran\n",
            ))),
            Err("executed Miri output must contain `test result: ok`".to_string())
        );

        let careful = |output: &str| CargoCarefulReceiptInput {
            card_id: card_id.to_string(),
            output: output.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly careful test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        };
        assert_eq!(
            WitnessReceipt::from_cargo_careful_output(careful("")),
            Err("saved cargo-careful output is empty".to_string())
        );
        assert_eq!(
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::CargoCareful(careful(""))),
            Err("executed cargo-careful output is empty".to_string())
        );

        let concurrency = |output: &str| ConcurrencyReceiptInput {
            card_id: card_id.to_string(),
            tool: "loom".to_string(),
            output: output.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test --features loom read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        };
        assert_eq!(
            WitnessReceipt::from_concurrency_output(concurrency("error: scheduler failed\n")),
            Err("saved loom output contains failure marker `error:`".to_string())
        );
        assert_eq!(
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Concurrency(concurrency(
                "error: scheduler failed\n",
            ))),
            Err("executed loom output contains failure marker `error:`".to_string())
        );

        let sanitizer = |output: &str, allow_runtime: bool| SanitizerReceiptInput {
            card_id: card_id.to_string(),
            tool: "asan".to_string(),
            output: output.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime,
        };
        let sanitizer_output = "test result: ok\nAddressSanitizer: use-after-free\n";
        assert_eq!(
            WitnessReceipt::from_sanitizer_output(sanitizer(sanitizer_output, false)),
            Err(
                "saved asan output contains sanitizer failure marker `addresssanitizer:`"
                    .to_string()
            )
        );
        assert_eq!(
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Sanitizer(sanitizer(
                sanitizer_output,
                false,
            ))),
            Err(
                "executed asan output contains sanitizer failure marker `addresssanitizer:`"
                    .to_string()
            )
        );
        assert_eq!(
            WitnessReceipt::from_sanitizer_output(sanitizer("", true)),
            Err("saved asan output is empty".to_string())
        );
        assert_eq!(
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Sanitizer(sanitizer(
                "", true,
            ))),
            Err("executed asan output is empty".to_string())
        );

        let proof = |output: &str| ProofReceiptInput {
            card_id: card_id.to_string(),
            tool: "kani".to_string(),
            output: output.to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        };
        assert_eq!(
            WitnessReceipt::from_proof_output(proof("verification failed\n")),
            Err(
                "saved kani proof output contains failure marker `verification failed`".to_string()
            )
        );
        assert_eq!(
            WitnessReceipt::from_executed_output(ExecutedReceiptInput::Proof(proof(
                "verification failed\n",
            ))),
            Err(
                "executed kani proof output contains failure marker `verification failed`"
                    .to_string()
            )
        );
    }

    #[test]
    fn miri_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "error: Undefined Behavior: pointer must be aligned\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn miri_receipt_from_saved_output_requires_miri_command() {
        let result = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("must mention `miri`")
        );
    }

    #[test]
    fn cargo_careful_receipt_from_saved_output_uses_ran_strength_without_site_reach()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly careful test read_header".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;

        assert_eq!(receipt.tool, "cargo-careful");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved cargo-careful output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run cargo-careful"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn cargo_careful_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: FAILED. 0 passed; 1 failed\nfailures:\nread_header\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly careful test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn cargo_careful_receipt_from_saved_output_requires_careful_command() {
        let result = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("must mention `careful`")
        );
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_uses_ran_strength_without_site_reach()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: false,
        })?;

        assert_eq!(receipt.tool, "asan");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved asan output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run a sanitizer"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_rejects_unsupported_sanitizer_tool() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "ubsan".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: false,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("sanitizer receipt tool")
        );
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "==123==ERROR: AddressSanitizer: heap-use-after-free\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: false,
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_requires_sanitizer_command() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test read_header".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: false,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("sanitizer receipt command")
        );
    }

    #[test]
    fn concurrency_receipt_from_saved_output_uses_ran_strength_without_site_reach()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "loom".to_string(),
            output: "running 1 test\ntest shared_cell_loom ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell_loom -- --nocapture".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;

        assert_eq!(receipt.tool, "loom");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved loom output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run a concurrency witness"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn concurrency_receipt_from_saved_output_rejects_unsupported_tool() {
        let result = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "kani".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell_loom -- --nocapture".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("concurrency receipt tool")
        );
    }

    #[test]
    fn concurrency_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "loom".to_string(),
            output: "test result: FAILED. 0 passed; 1 failed\nfailures:\nshared_cell_loom\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell_loom -- --nocapture".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn concurrency_receipt_from_saved_output_requires_concurrency_command() {
        let result = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "loom".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell -- --nocapture".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("concurrency receipt command")
        );
    }

    #[test]
    fn proof_receipt_from_saved_output_uses_ran_strength_without_site_reach() -> Result<(), String>
    {
        let receipt = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "kani".to_string(),
            output:
                "Kani Rust Verifier\nVERIFICATION:- SUCCESSFUL\nVerification result: verified\n"
                    .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;

        assert_eq!(receipt.tool, "kani");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved kani proof output reported verification success")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run a proof tool"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("recorded harness/output"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn proof_receipt_from_saved_output_accepts_crux_success_marker() -> Result<(), String> {
        let receipt = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "crux".to_string(),
            output: "Crux verification\nStatus: Verified\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "crux prove byte_to_bool".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        })?;

        assert_eq!(receipt.tool, "crux");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved crux proof output reported verification success")
        );
        Ok(())
    }

    #[test]
    fn proof_receipt_from_saved_output_rejects_unsupported_tool() {
        let result = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "prusti".to_string(),
            output: "VERIFICATION:- SUCCESSFUL\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("proof receipt tool")
        );
    }

    #[test]
    fn proof_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "kani".to_string(),
            output: "VERIFICATION:- FAILED\nCounterexample generated\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn proof_receipt_from_saved_output_requires_proof_command() {
        let result = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "kani".to_string(),
            output: "VERIFICATION:- SUCCESSFUL\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test byte_to_bool".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("proof receipt command")
        );
    }

    // ── Part 2: formal-tool site_reached provenance gate ─────────────────────

    #[test]
    fn validate_strength_for_tool_rejects_loom_site_reached() {
        // A hand-authored loom receipt with site_reached must be rejected:
        // the import adapter only produces `ran`; a hand-authored `site_reached`
        // claim cannot carry verifiable tool provenance.
        let mut receipt = fixture_receipt();
        receipt.tool = "loom".to_string();
        receipt.strength = "site_reached".to_string();

        let err = receipt.validate().err().unwrap_or_default();
        assert!(
            err.contains("loom") && err.contains("site_reached"),
            "expected rejection of loom+site_reached, got: {err}"
        );
        assert!(
            err.contains("verifiable tool provenance"),
            "error should mention verifiable tool provenance: {err}"
        );
    }

    #[test]
    fn validate_strength_for_tool_rejects_shuttle_site_reached() {
        let mut receipt = fixture_receipt();
        receipt.tool = "shuttle".to_string();
        receipt.strength = "site_reached".to_string();

        let err = receipt.validate().err().unwrap_or_default();
        assert!(
            err.contains("shuttle") && err.contains("site_reached"),
            "expected rejection of shuttle+site_reached, got: {err}"
        );
    }

    #[test]
    fn validate_strength_for_tool_rejects_kani_site_reached() {
        let mut receipt = fixture_receipt();
        receipt.tool = "kani".to_string();
        receipt.strength = "site_reached".to_string();

        let err = receipt.validate().err().unwrap_or_default();
        assert!(
            err.contains("kani") && err.contains("site_reached"),
            "expected rejection of kani+site_reached, got: {err}"
        );
    }

    #[test]
    fn validate_strength_for_tool_rejects_crux_site_reached() {
        let mut receipt = fixture_receipt();
        receipt.tool = "crux".to_string();
        receipt.strength = "site_reached".to_string();

        let err = receipt.validate().err().unwrap_or_default();
        assert!(
            err.contains("crux") && err.contains("site_reached"),
            "expected rejection of crux+site_reached, got: {err}"
        );
    }

    #[test]
    fn validate_strength_for_tool_accepts_formal_tools_with_ran_strength() -> Result<(), String> {
        // Tool-provenance receipts from import adapters produce `ran`;
        // these must remain valid.
        for tool in ["loom", "shuttle", "kani", "crux"] {
            let mut receipt = fixture_receipt();
            receipt.tool = tool.to_string();
            receipt.strength = "ran".to_string();
            receipt
                .validate()
                .map_err(|err| format!("unexpected rejection of {tool}+ran: {err}"))?;
        }
        Ok(())
    }

    #[test]
    fn validate_strength_for_tool_accepts_external_integration_site_reached_unchanged()
    -> Result<(), String> {
        // external-integration-test+site_reached is the existing allowed path
        // and must not be broken by the formal-tool gate.
        let mut receipt = fixture_receipt();
        receipt.tool = "external-integration-test".to_string();
        receipt.strength = "site_reached".to_string();
        receipt.command = Some("bun test test/js/sab-copy-to-unshared.test.ts".to_string());
        receipt.command_hash = receipt.command.as_deref().map(WitnessReceipt::command_hash);
        receipt.validate()
    }

    // ── Part 1: sanitizer runtime result semantics ────────────────────────────

    #[test]
    fn sanitizer_receipt_observed_failure_imports_as_confirmed_not_clearing_obligation()
    -> Result<(), String> {
        // A sanitizer runtime run that fires: verdict must be `confirmed`.
        // A `confirmed` verdict means the site was witnessed AND the hazard
        // reproduced. It is NOT a safety claim and does not clear the safety
        // obligation.
        let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "==42==ERROR: AddressSanitizer: heap-buffer-overflow on address\n==42==    #0 0x... in fn\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "ASAN_OPTIONS=abort_on_error=0 ./target/debug/my-program".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: true,
        })?;

        assert_eq!(
            receipt.verdict.as_deref(),
            Some("confirmed"),
            "observed sanitizer failure must import as `confirmed`"
        );
        // The summary must NOT claim safety or say the obligation is cleared.
        let summary = receipt.summary.as_deref().unwrap_or("");
        assert!(
            summary.contains("signal observed"),
            "summary should note the observed signal, got: {summary}"
        );
        assert!(
            !summary.to_ascii_lowercase().contains("safe"),
            "summary must not claim safety, got: {summary}"
        );
        // The strength is `ran` (not `site_reached`) because the import
        // adapter does not claim precise site reach.
        assert_eq!(receipt.strength, "ran");
        // Limitations must note this is not a safety claim.
        let limitations = receipt.limitations.as_deref().unwrap_or(&[]);
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("runtime witness mode")),
            "limitations should mention runtime witness mode: {limitations:?}"
        );
        Ok(())
    }

    #[test]
    fn sanitizer_pass_satisfies_witness_coverage_with_not_reproduced_verdict() -> Result<(), String>
    {
        // A clean sanitizer run must import as `not_reproduced`.
        // `not_reproduced` means no signal was observed in this run —
        // it is NOT a safety claim.
        let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "tsan".to_string(),
            output: "running 1 test\ntest my_test ... ok\n\ntest result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=thread' cargo +nightly test my_test".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: false,
        })?;

        // A `not_reproduced` verdict satisfies witness coverage (present=true
        // when imported) while preserving result semantics: no signal this run.
        assert_eq!(
            receipt.verdict.as_deref(),
            Some("not_reproduced"),
            "clean sanitizer run must import as `not_reproduced`"
        );
        assert_eq!(receipt.strength, "ran");
        // The strength satisfies `imports_witness_evidence` (tool != eit,
        // strength == "ran") so a ReceiptIndex will set present=true.
        Ok(())
    }

    #[test]
    fn sanitizer_receipt_from_runtime_output_records_confirmed_when_sanitizer_fires()
    -> Result<(), String> {
        // A runtime ASAN run that fires: verdict should be "confirmed"
        let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "==123==ERROR: AddressSanitizer: attempting free on address which was not malloc()-ed\n==123==    #0 0x... in free\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "ASAN_OPTIONS=abort_on_error=0 ./target/release/my-program".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: true,
        })?;

        assert_eq!(receipt.tool, "asan");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(receipt.verdict.as_deref(), Some("confirmed"));
        let summary = receipt.summary.as_deref().unwrap_or("");
        assert!(
            summary.contains("sanitizer signal observed"),
            "summary was: {summary}"
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("runtime witness mode"))
        );
        Ok(())
    }

    #[test]
    fn sanitizer_receipt_from_runtime_output_records_not_reproduced_on_clean_run()
    -> Result<(), String> {
        // A runtime ASAN run with no sanitizer signal: verdict should be "not_reproduced"
        let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "Program output: processed 100 items successfully\nProcess exited with code 0\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "ASAN_OPTIONS=abort_on_error=0 ./target/release/my-program".to_string(),
            limitations: vec!["fixture only".to_string()],
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: true,
        })?;

        assert_eq!(receipt.verdict.as_deref(), Some("not_reproduced"));
        let summary = receipt.summary.as_deref().unwrap_or("");
        assert!(
            summary.contains("no sanitizer signal"),
            "summary was: {summary}"
        );
        Ok(())
    }

    #[test]
    fn sanitizer_receipt_runtime_mode_still_rejects_empty_output() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "ASAN_OPTIONS=abort_on_error=0 ./target/release/my-program".to_string(),
            limitations: Vec::new(),
            terminal_status: Some(TerminalStatus::exited(0)),
            subject: None,
            allow_runtime: true,
        });

        assert!(result.err().unwrap_or_default().contains("empty"));
    }

    fn fixture_receipt() -> WitnessReceipt {
        WitnessReceipt {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "miri".to_string(),
            strength: "ran".to_string(),
            author: Some("core/fixtures".to_string()),
            recorded_at: Some("2026-05-18T00:00:00Z".to_string()),
            expires_at: Some("2026-08-18".to_string()),
            summary: Some("focused witness passed".to_string()),
            command: Some("cargo +nightly miri test read_header".to_string()),
            command_hash: Some(WitnessReceipt::command_hash(
                "cargo +nightly miri test read_header",
            )),
            limitations: Some(vec!["fixture only".to_string()]),
            verdict: None,
            exit_code: None,
            terminated_by_signal: None,
            subject: None,
        }
    }
}
