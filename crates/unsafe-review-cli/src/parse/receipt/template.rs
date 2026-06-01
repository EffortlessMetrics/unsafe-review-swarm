use super::*;
use crate::command::ReceiptTemplateOptions;
use arg_value::{inline_path, inline_string, next_path, next_string};

pub(in crate::parse) fn parse_receipt_template(
    args: Vec<String>,
) -> Result<ReceiptTemplateOptions, String> {
    let mut parser = TemplateParser::default();
    parser.parse(args)?;
    parser.finish()
}

#[derive(Default)]
struct TemplateParser {
    options: ReceiptTemplateOptions,
    card_id: Option<String>,
}

impl TemplateParser {
    fn parse(&mut self, args: Vec<String>) -> Result<(), String> {
        let mut idx = 0usize;
        while idx < args.len() {
            self.parse_arg(&args, &mut idx)?;
            idx += 1;
        }
        Ok(())
    }

    fn parse_arg(&mut self, args: &[String], idx: &mut usize) -> Result<(), String> {
        match args[*idx].as_str() {
            "--tool" => self.options.tool = next_string(args, idx, "--tool")?,
            arg if arg.starts_with("--tool=") => {
                self.options.tool = inline_string(arg, "--tool")?;
            }
            "--strength" => self.options.strength = next_string(args, idx, "--strength")?,
            arg if arg.starts_with("--strength=") => {
                self.options.strength = inline_string(arg, "--strength")?;
            }
            "--author" => self.options.author = next_string(args, idx, "--author")?,
            arg if arg.starts_with("--author=") => {
                self.options.author = inline_string(arg, "--author")?;
            }
            "--recorded-at" => {
                self.options.recorded_at = next_string(args, idx, "--recorded-at")?;
            }
            arg if arg.starts_with("--recorded-at=") => {
                self.options.recorded_at = inline_string(arg, "--recorded-at")?;
            }
            "--expires-at" => {
                self.options.expires_at = next_string(args, idx, "--expires-at")?;
            }
            arg if arg.starts_with("--expires-at=") => {
                self.options.expires_at = inline_string(arg, "--expires-at")?;
            }
            "--summary" => self.options.summary = Some(next_string(args, idx, "--summary")?),
            arg if arg.starts_with("--summary=") => {
                self.options.summary = Some(inline_string(arg, "--summary")?);
            }
            "--command" => self.options.command = Some(next_string(args, idx, "--command")?),
            arg if arg.starts_with("--command=") => {
                self.options.command = Some(inline_string(arg, "--command")?);
            }
            "--limitation" => {
                self.options
                    .limitations
                    .push(next_string(args, idx, "--limitation")?)
            }
            arg if arg.starts_with("--limitation=") => self
                .options
                .limitations
                .push(inline_string(arg, "--limitation")?),
            "--out" => self.options.out = Some(next_path(args, idx, "--out")?),
            arg if arg.starts_with("--out=") => {
                self.options.out = Some(inline_path(arg, "--out")?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown receipt template argument `{value}`"));
            }
            value => set_card_id(&mut self.card_id, value)?,
        }
        Ok(())
    }

    fn finish(mut self) -> Result<ReceiptTemplateOptions, String> {
        self.options.card_id = self.card_id.ok_or_else(|| "missing card id".to_string())?;
        validate_required_cli_value(&self.options.tool, "--tool")?;
        validate_required_cli_value(&self.options.strength, "--strength")?;
        validate_required_cli_value(&self.options.author, "--author")?;
        validate_required_cli_value(&self.options.recorded_at, "--recorded-at")?;
        validate_required_cli_value(&self.options.expires_at, "--expires-at")?;
        Ok(self.options)
    }
}
