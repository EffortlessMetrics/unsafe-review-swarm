pub(crate) mod agent;
pub(crate) mod badges;
pub(crate) mod comment_plan;
pub(crate) mod human;
pub(crate) mod json;
pub(crate) mod lsp;
pub(crate) mod markdown;
pub(crate) mod outcome;
pub(crate) mod policy_report;
pub(crate) mod receipt_audit;
pub(crate) mod sarif;
pub(crate) mod witness_plan;

pub(crate) const NO_CHANGED_GAPS_MESSAGE: &str = "No changed unsafe-review gaps were found.";
pub(crate) const NO_CHANGED_GAPS_LIMITATION: &str =
    "This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.";
