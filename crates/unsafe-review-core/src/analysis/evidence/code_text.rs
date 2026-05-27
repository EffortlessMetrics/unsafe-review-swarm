use super::is_receiver_path_char;

pub(super) fn strip_block_comments_and_literals(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = '\0';
            for comment_ch in chars.by_ref() {
                if prev == '*' && comment_ch == '/' {
                    break;
                }
                prev = comment_ch;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }
        if ch == '"' {
            output.push('"');
            let mut escaped = false;
            for literal_ch in chars.by_ref() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if literal_ch == '\\' {
                    escaped = true;
                    continue;
                }
                if literal_ch == '"' {
                    output.push('"');
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

pub(super) fn contains_executable_return(text: &str) -> bool {
    let code = compact_code(&strip_block_comments_and_literals(text));
    code.starts_with("return")
        || code.contains(";return")
        || code.contains("{return")
        || code.contains("}return")
        || code.contains("=>return")
}

pub(super) fn compact_contains_identifier(text: &str, ident: &str) -> bool {
    let mut cursor = text;
    while let Some(pos) = cursor.find(ident) {
        let before = cursor[..pos].chars().next_back();
        let after = cursor[pos + ident.len()..].chars().next();
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && after.is_none_or(|ch| !is_receiver_path_char(ch))
        {
            return true;
        }
        let next = pos + ident.len();
        cursor = &cursor[next..];
    }
    false
}

pub(super) fn compact_code(lower: &str) -> String {
    lower
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{contains_executable_return, strip_block_comments_and_literals};

    #[test]
    fn strips_line_comment_text_without_removing_later_code() {
        let stripped = strip_block_comments_and_literals(
            "if ptr.is_null() { // return None\n    return Some(ptr);\n}",
        );

        assert!(!stripped.contains("return None"));
        assert!(stripped.contains("return Some(ptr);"));
    }

    #[test]
    fn strips_block_comments_and_string_literals() {
        let stripped = strip_block_comments_and_literals(
            "if guard { /* return None */ let note = \"return None\"; return Some(()); }",
        );

        assert!(!stripped.contains("/* return None */"));
        assert!(!stripped.contains("\"return None\""));
        assert!(stripped.contains("\"\""));
        assert!(stripped.contains("return Some(());"));
    }

    #[test]
    fn executable_return_detector_ignores_comments_and_literals() {
        assert!(contains_executable_return("log(); return None;"));
        assert!(contains_executable_return("Err(err) => return Err(err),"));
        assert!(!contains_executable_return("/* return None */"));
        assert!(!contains_executable_return("// return None\nlog();"));
        assert!(!contains_executable_return("let note = \"return None\";"));
    }
}
