#![forbid(unsafe_code)]
//! Product façade crate for `unsafe-review`.
//!
//! `unsafe-review` finds unsafe Rust changes missing a safety contract, guard,
//! test, or witness. It is advisory static review evidence: it does not prove
//! memory safety, UB-free status, or Miri-clean status, and it does not run
//! witnesses, post comments, edit source, or block by default.
//!
//! Most users want the command-line interface:
//!
//! ```text
//! cargo install unsafe-review --locked
//! unsafe-review pr --base origin/main
//! ```
//!
//! Programmatic integrations should depend on `unsafe-review-core` directly:
//! build an [`AnalyzeInput`], call [`analyze`], and render or consume the
//! returned [`AnalyzeOutput`]. Every output surface (CLI, JSON, Markdown PR
//! summary, SARIF, LSP diagnostics, agent packets) projects from the same
//! [`ReviewCard`] truth object.
//!
//! Start with `unsafe-review doctor` to check a repository is set up for
//! review, then `unsafe-review pr` or `unsafe-review check --base origin/main`
//! for the first advisory review.

pub use unsafe_review_core::*;
