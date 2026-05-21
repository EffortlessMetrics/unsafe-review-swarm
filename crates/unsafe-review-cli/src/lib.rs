#![forbid(unsafe_code)]
//! CLI implementation crate for `unsafe-review`.
//!
//! Most users should install the `unsafe-review` product façade binary instead
//! of depending on this crate directly.
//!
//! This crate owns command parsing, terminal output, and artifact rendering. It
//! depends on `unsafe-review-core` for ReviewCard analysis.

mod command;
mod execute;
mod lsp;
mod parse;

pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let command = parse::parse(args.into_iter().collect())?;
    execute::execute(command)
}
