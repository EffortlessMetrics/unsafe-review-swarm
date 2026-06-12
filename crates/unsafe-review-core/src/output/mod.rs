pub(crate) mod agent;
pub(crate) mod badges;
pub(crate) mod comment_plan;
pub(crate) mod confirmation;
pub(crate) mod gate_manifest;
pub(crate) mod human;
pub(crate) mod json;
pub(crate) mod lsp;
pub(crate) mod markdown;
pub(crate) mod outcome;
pub(crate) mod policy_report;
pub(crate) mod receipt_audit;
pub(crate) mod repair_queue;
pub(crate) mod sarif;
pub(crate) mod usefulness_telemetry;
pub(crate) mod witness_plan;

pub(crate) const NO_CHANGED_GAPS_MESSAGE: &str = "No changed unsafe-review gaps were found.";
pub(crate) const NO_CHANGED_GAPS_LIMITATION: &str =
    "This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.";
pub(crate) const REVIEWCARD_TRUST_BOUNDARY: &str = "static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so.";
pub(crate) const REPAIR_QUEUE_TRUST_BOUNDARY: &str = "static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, and not an automatic repair queue. It does not run agents, does not run witnesses, does not edit source, does not post comments, does not suppress cards, and does not resolve cards.";
