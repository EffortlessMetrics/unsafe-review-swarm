use std::path::PathBuf;
use unsafe_review_core::{DiscoveryOptions, PolicyMode};

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
    Import(CandidateImportOptions),
    WitnessPlan(CandidateWitnessPlanOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateImportOptions {
    pub input: PathBuf,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateWitnessPlanOptions {
    pub root: PathBuf,
    pub id: String,
    pub out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    RepoHelp,
    CandidateHelp,
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
        id: String,
    },
    Candidate(CandidateCommand),
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
