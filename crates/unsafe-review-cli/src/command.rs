use std::path::PathBuf;
use unsafe_review_core::{DiscoveryOptions, PolicyMode};

/// Query surface for `context` — either a single card by id or a file:line range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ContextQuery {
    /// Existing card-id lookup (bounded_repair_packet mode).
    CardId(String),
    /// File:line range scan (file_range_scan mode, SPEC-0033).
    FileRange {
        file: PathBuf,
        line_start: u32,
        line_end: u32,
        changed_only: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffInput {
    File(PathBuf),
    Stdin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Human,
    Json,
    Markdown,
    PrSummary,
    GithubSummary,
    Sarif,
    CommentPlan,
    Lsp,
    WitnessPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckOptions {
    pub root: PathBuf,
    pub base: Option<String>,
    pub diff: Option<DiffInput>,
    pub format: Format,
    pub policy: PolicyMode,
    pub out: Option<PathBuf>,
    pub max_cards: Option<usize>,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            base: None,
            diff: None,
            format: Format::Human,
            policy: PolicyMode::Advisory,
            out: None,
            max_cards: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FirstPrOptions {
    pub check: CheckOptions,
    pub out_dir: PathBuf,
}

impl Default for FirstPrOptions {
    fn default() -> Self {
        Self {
            check: CheckOptions::default(),
            out_dir: PathBuf::from("target/unsafe-review"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoOptions {
    pub check: CheckOptions,
    pub discovery: DiscoveryOptions,
    pub list_files: bool,
    pub progress: bool,
    pub timeout_seconds: Option<u64>,
}

impl Default for RepoOptions {
    fn default() -> Self {
        Self {
            check: CheckOptions::default(),
            discovery: DiscoveryOptions::repo_defaults(),
            list_files: false,
            progress: false,
            timeout_seconds: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReceiptTemplateOptions {
    pub card_id: String,
    pub tool: String,
    pub strength: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub summary: Option<String>,
    pub command: Option<String>,
    pub limitations: Vec<String>,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SavedOutputReceiptOptions {
    pub card_id: String,
    pub tool: Option<String>,
    pub log: PathBuf,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
    pub out: Option<PathBuf>,
    /// When true, accept a runtime/program-level sanitizer log (no `test result: ok` required).
    pub allow_runtime: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConfirmOptions {
    pub card_id: String,
    pub root: PathBuf,
    pub base: Option<String>,
    pub diff: Option<DiffInput>,
    pub dry_run: bool,
    pub author: String,
    pub expires_at: Option<String>,
    pub timeout_seconds: u64,
    pub command: Option<String>,
    pub out: Option<PathBuf>,
}

impl Default for ConfirmOptions {
    fn default() -> Self {
        Self {
            card_id: String::new(),
            root: PathBuf::from("."),
            base: None,
            diff: None,
            dry_run: false,
            author: String::new(),
            expires_at: None,
            timeout_seconds: 600,
            command: None,
            out: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutcomeOptions {
    pub before: PathBuf,
    pub after: PathBuf,
    pub format: Format,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CandidateCommand {
    New(CandidateNewOptions),
    Import(CandidateImportOptions),
    Lint(CandidateLintOptions),
    List(CandidateListOptions),
    WitnessPlan(CandidateWitnessPlanOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateNewOptions {
    pub class: String,
    pub id: String,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateImportOptions {
    pub input: PathBuf,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateLintOptions {
    pub input: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateListOptions {
    pub root: PathBuf,
    pub format: Format,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateWitnessPlanOptions {
    pub root: PathBuf,
    pub id: String,
    pub out: Option<PathBuf>,
}

/// Options for `baseline init` (SPEC-0030).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaselineInitOptions {
    pub root: PathBuf,
    /// Override the default ledger output path (`policy/unsafe-review-baseline.toml`).
    pub out: Option<PathBuf>,
    /// Override the default `review_after` date (ISO 8601 YYYY-MM-DD).
    pub review_after: Option<String>,
}

impl Default for BaselineInitOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            out: None,
            review_after: None,
        }
    }
}

/// Options for `baseline add` (SPEC-0030).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaselineAddOptions {
    pub root: PathBuf,
    pub card_id: String,
    pub owner: String,
    pub reason: String,
    pub evidence: String,
    pub review_after: Option<String>,
    /// Override the default ledger output path.
    pub out: Option<PathBuf>,
}

/// Subcommand variants for `baseline`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BaselineCommand {
    Init(BaselineInitOptions),
    Add(BaselineAddOptions),
    Help,
}

/// Target subcommand for per-subcommand help pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SubcommandHelpTarget {
    Check,
    FirstPr,
    Pilot,
    Explain,
    Context,
    Confirm,
    Receipt,
    Outcome,
    Policy,
    Doctor,
    Badges,
    Lsp,
    Support,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    RepoHelp,
    CandidateHelp,
    BaselineHelp,
    SubcommandHelp(SubcommandHelpTarget),
    Version,
    Support,
    Doctor {
        root: PathBuf,
    },
    Check(CheckOptions),
    Repo(RepoOptions),
    Pilot(CheckOptions),
    FirstPr(FirstPrOptions),
    Badges {
        root: PathBuf,
        out: PathBuf,
    },
    Explain {
        root: PathBuf,
        id: String,
        format: Format,
    },
    Context {
        root: PathBuf,
        query: ContextQuery,
    },
    Candidate(CandidateCommand),
    Baseline(BaselineCommand),
    Confirm(ConfirmOptions),
    ReceiptTemplate(ReceiptTemplateOptions),
    ReceiptValidate {
        root: PathBuf,
    },
    ReceiptAudit(CheckOptions),
    ReceiptImportMiri(SavedOutputReceiptOptions),
    ReceiptImportCareful(SavedOutputReceiptOptions),
    ReceiptImportSanitizer(SavedOutputReceiptOptions),
    ReceiptImportConcurrency(SavedOutputReceiptOptions),
    ReceiptImportProof(SavedOutputReceiptOptions),
    Outcome(OutcomeOptions),
    PolicyReport(CheckOptions),
    Lsp,
}
