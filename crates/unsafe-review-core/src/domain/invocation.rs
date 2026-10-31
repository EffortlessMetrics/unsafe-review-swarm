//! Witness invocation parsing shared by capture and consumer.
//!
//! Moved verbatim from the `confirm` capture path so the receipt-import
//! consumer re-derives the expected invocation digest with exactly the same
//! grammar the capture side recorded. Any divergence here would silently
//! break subject-applicability comparison, so this module must stay the
//! single definition of witness command parsing.
//!
//! Grammar (no shell involved): whitespace separates words; single quotes
//! preserve bytes literally; double quotes preserve literally except that
//! backslash escapes only `"` and `\` (so Windows paths survive); a
//! backslash outside quotes escapes the next character; shell
//! metacharacters (`| & ; < > ( ) $` and backtick) are rejected explicitly
//! instead of being reinterpreted.

use super::receipt::SubjectBinding;

/// One parsed witness invocation: program, arguments, environment overrides,
/// and provenance. This single value feeds the dry-run preview, the process
/// spawner, and the receipt identity, so the previewed command is the
/// executed command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Invocation {
    pub fn exec_argv(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(self.args.len() + 1);
        argv.push(self.program.clone());
        argv.extend(self.args.iter().cloned());
        argv
    }

    /// Canonical digest of this invocation for subject-applicability
    /// comparison: program, arguments in order, and environment assignments
    /// sorted by key. Values are digested, never stored.
    pub fn digest(&self) -> String {
        SubjectBinding::digest_invocation(&self.program, &self.args, &self.env)
    }
}

pub fn parse_command_line(command: &str) -> Result<Invocation, String> {
    let words = split_command_words(command)?;
    let mut env = Vec::new();
    let mut words = words.into_iter();
    let mut program = None;
    for word in words.by_ref() {
        if program.is_none()
            && let Some(assignment) = env_assignment(&word)
        {
            env.push(assignment);
            continue;
        }
        program = Some(word);
        break;
    }
    let Some(program) = program else {
        return Err(
            "witness command has no executable to run after environment assignments".to_string(),
        );
    };
    Ok(Invocation {
        program,
        args: words.collect(),
        env,
    })
}

fn split_command_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ch if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        None => {
                            return Err(
                                "witness command has an unterminated single quote".to_string()
                            );
                        }
                        Some('\'') => break,
                        Some(next) => current.push(next),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        None => {
                            return Err(
                                "witness command has an unterminated double quote".to_string()
                            );
                        }
                        Some('"') => break,
                        // Inside double quotes only `"` and `\` are escapable,
                        // so Windows paths like `"C:\tools\runner"` survive
                        // unchanged. Every other backslash stays literal.
                        Some('\\') => match chars.peek() {
                            Some('"') | Some('\\') => {
                                current.push(chars.next().unwrap_or('\\'));
                            }
                            _ => current.push('\\'),
                        },
                        Some(next) => current.push(next),
                    }
                }
            }
            '\\' => {
                let Some(escaped) = chars.next() else {
                    return Err("witness command has a trailing backslash".to_string());
                };
                started = true;
                current.push(escaped);
            }
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' => {
                return Err(format!(
                    "witness command contains unsupported shell syntax `{ch}`; quote it to pass it literally or run the pipeline manually and import a receipt"
                ));
            }
            _ => {
                started = true;
                current.push(ch);
            }
        }
    }
    if started {
        words.push(current);
    }
    Ok(words)
}

fn env_assignment(token: &str) -> Option<(String, String)> {
    let (name, value) = token.split_once('=')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invocation_digest_matches_subject_binding_invocation_digest() -> Result<(), String> {
        let invocation = parse_command_line(
            "MIRIFLAGS=-Zmiri-strict-provenance cargo +nightly test read_header",
        )?;
        assert_eq!(
            invocation.digest(),
            SubjectBinding::digest_invocation(
                "cargo",
                &[
                    "+nightly".to_string(),
                    "test".to_string(),
                    "read_header".to_string()
                ],
                &[(
                    "MIRIFLAGS".to_string(),
                    "-Zmiri-strict-provenance".to_string()
                )],
            )
        );
        Ok(())
    }

    #[test]
    fn unparseable_command_digests_to_nothing() {
        let err = parse_command_line("RUST_BACKTRACE=1")
            .err()
            .unwrap_or_default();
        assert!(
            err.contains("no executable to run"),
            "empty command must fail explicitly, got `{err}`"
        );
    }
}
