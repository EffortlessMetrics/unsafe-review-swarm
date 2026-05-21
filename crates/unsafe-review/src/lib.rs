#![forbid(unsafe_code)]
//! Advisory unsafe-contract review for Rust PRs.
//!
//! `unsafe-review` points reviewers and coding agents at changed Rust `unsafe`
//! seams that are missing review evidence: a safety contract, local guard, test
//! reach, or witness receipt.
//!
//! It does **not** prove unsafe Rust sound. It makes unsafe Rust reviewable.
//!
//! # Install
//!
//! ```text
//! cargo install unsafe-review --locked
//! unsafe-review doctor
//! unsafe-review first-pr --base origin/main
//! ```
//!
//! # Trust boundary
//!
//! `unsafe-review` reports static review evidence. It is not a proof of memory
//! safety, not a UB-free claim, and not a Miri result unless a matching witness
//! receipt is attached.
//!
//! It is advisory by default: no witness execution, no automatic comments, no
//! source edits, and no default blocking policy.
//!
//! # Programmatic use
//!
//! Most users should install this façade crate. Programmatic integrations should
//! depend on `unsafe-review-core` directly.

pub use unsafe_review_core::*;
