#![forbid(unsafe_code)]
mod command;
mod execute;
mod lsp;
mod parse;

pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let command = parse::parse(args)?;
    execute::execute(command)
}
