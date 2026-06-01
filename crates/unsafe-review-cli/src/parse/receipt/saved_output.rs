use super::*;
use crate::command::SavedOutputReceiptOptions;
use arg_value::{inline_path, inline_string, next_path, next_string};

pub(super) fn parse_saved_output_receipt(
    args: Vec<String>,
    command_name: &str,
    allow_tool: bool,
) -> Result<SavedOutputReceiptOptions, String> {
    let mut parser = SavedOutputParser::new(command_name, allow_tool);
    parser.parse(args)?;
    parser.finish()
}

struct SavedOutputParser<'a> {
    command_name: &'a str,
    allow_tool: bool,
    options: SavedOutputReceiptOptions,
    card_id: Option<String>,
}

impl<'a> SavedOutputParser<'a> {
    fn new(command_name: &'a str, allow_tool: bool) -> Self {
        Self {
            command_name,
            allow_tool,
            options: SavedOutputReceiptOptions::default(),
            card_id: None,
        }
    }

    fn parse(&mut self, args: Vec<String>) -> Result<(), String> {
        let mut idx = 0usize;
        while idx < args.len() {
            self.parse_arg(&args, &mut idx)?;
            idx += 1;
        }
        Ok(())
    }

    fn parse_arg(&mut self, args: &[String], idx: &mut usize) -> Result<(), String> {
        let arg = args[*idx].as_str();
        if self.parse_tool(args, idx, arg)? || self.parse_common(args, idx, arg)? {
            return Ok(());
        }
        match arg {
            value if value.starts_with('-') => Err(format!(
                "unknown receipt {} argument `{value}`",
                self.command_name
            )),
            value => set_card_id(&mut self.card_id, value),
        }
    }

    fn parse_tool(&mut self, args: &[String], idx: &mut usize, arg: &str) -> Result<bool, String> {
        match arg {
            "--tool" if self.allow_tool => {
                self.options.tool = Some(next_string(args, idx, "--tool")?);
                Ok(true)
            }
            _ if self.allow_tool && arg.starts_with("--tool=") => {
                self.options.tool = Some(inline_string(arg, "--tool")?);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn parse_common(
        &mut self,
        args: &[String],
        idx: &mut usize,
        arg: &str,
    ) -> Result<bool, String> {
        match arg {
            "--log" => {
                self.options.log = next_path(args, idx, "--log")?;
                Ok(true)
            }
            _ if arg.starts_with("--log=") => {
                self.options.log = inline_path(arg, "--log")?;
                Ok(true)
            }
            "--author" => {
                self.options.author = next_string(args, idx, "--author")?;
                Ok(true)
            }
            _ if arg.starts_with("--author=") => {
                self.options.author = inline_string(arg, "--author")?;
                Ok(true)
            }
            "--recorded-at" => {
                self.options.recorded_at = next_string(args, idx, "--recorded-at")?;
                Ok(true)
            }
            _ if arg.starts_with("--recorded-at=") => {
                self.options.recorded_at = inline_string(arg, "--recorded-at")?;
                Ok(true)
            }
            "--expires-at" => {
                self.options.expires_at = next_string(args, idx, "--expires-at")?;
                Ok(true)
            }
            _ if arg.starts_with("--expires-at=") => {
                self.options.expires_at = inline_string(arg, "--expires-at")?;
                Ok(true)
            }
            "--command" => {
                self.options.command = next_string(args, idx, "--command")?;
                Ok(true)
            }
            _ if arg.starts_with("--command=") => {
                self.options.command = inline_string(arg, "--command")?;
                Ok(true)
            }
            "--limitation" => {
                self.options
                    .limitations
                    .push(next_string(args, idx, "--limitation")?);
                Ok(true)
            }
            _ if arg.starts_with("--limitation=") => {
                self.options
                    .limitations
                    .push(inline_string(arg, "--limitation")?);
                Ok(true)
            }
            "--out" => {
                self.options.out = Some(next_path(args, idx, "--out")?);
                Ok(true)
            }
            _ if arg.starts_with("--out=") => {
                self.options.out = Some(inline_path(arg, "--out")?);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn finish(mut self) -> Result<SavedOutputReceiptOptions, String> {
        self.options.card_id = self.card_id.ok_or_else(|| "missing card id".to_string())?;
        if self.options.log.as_os_str().is_empty() {
            return Err("missing value for --log".to_string());
        }
        validate_required_cli_value(&self.options.author, "--author")?;
        validate_required_cli_value(&self.options.recorded_at, "--recorded-at")?;
        validate_required_cli_value(&self.options.expires_at, "--expires-at")?;
        validate_required_cli_value(&self.options.command, "--command")?;
        if self.allow_tool && self.options.tool.as_deref().unwrap_or("").trim().is_empty() {
            return Err("missing value for --tool".to_string());
        }
        Ok(self.options)
    }
}
