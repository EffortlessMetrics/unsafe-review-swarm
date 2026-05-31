#[derive(Clone, Debug)]
pub(super) struct StringDetectionState {
    raw_hashes: Option<usize>,
    escaped: bool,
}

#[derive(Default)]
pub(super) struct LineCommentState {
    pub(super) block_depth: usize,
    string: Option<StringDetectionState>,
}

pub(super) fn line_for_text_detection(line: &str, state: &mut LineCommentState) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if state.block_depth > 0 {
            if ch == '/' && chars.peek() == Some(&'*') {
                state.block_depth += 1;
                let _ = chars.next();
            } else if ch == '*' && chars.peek() == Some(&'/') {
                state.block_depth = state.block_depth.saturating_sub(1);
                let _ = chars.next();
            }
            continue;
        }

        if let Some(string) = &mut state.string {
            let mut closed = false;
            if let Some(raw_hashes) = string.raw_hashes {
                if ch == '"' && raw_string_hashes_at_end(&mut chars, raw_hashes) {
                    closed = true;
                }
            } else if string.escaped {
                string.escaped = false;
            } else if ch == '\\' {
                string.escaped = true;
            } else if ch == '"' {
                closed = true;
            }

            if closed {
                state.string = None;
                out.push('"');
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            break;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            state.block_depth += 1;
            let _ = chars.next();
            continue;
        }
        if ch == '\'' && consume_char_literal(&mut chars) {
            out.push('\'');
            continue;
        }
        if ch == 'r'
            && let Some(hashes) = raw_string_hashes_at_start(&mut chars)
        {
            for _ in 0..hashes {
                let _ = chars.next();
            }
            let _ = chars.next();
            state.string = Some(StringDetectionState {
                raw_hashes: Some(hashes),
                escaped: false,
            });
            out.push('"');
            continue;
        }
        if ch == '"' {
            state.string = Some(StringDetectionState {
                raw_hashes: None,
                escaped: false,
            });
            out.push('"');
            continue;
        }
        out.push(ch);
    }

    out
}

fn consume_char_literal<I>(chars: &mut std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = char> + Clone,
{
    let mut clone = chars.clone();
    let Some(first) = clone.next() else {
        return false;
    };
    if first == '\n' || first == '\r' {
        return false;
    }
    if first == '\\' {
        let Some(escaped) = clone.next() else {
            return false;
        };
        if escaped == '\n' || escaped == '\r' {
            return false;
        }
    }
    if clone.next() != Some('\'') {
        return false;
    }

    while chars.next() != Some('\'') {}
    true
}

fn raw_string_hashes_at_start<I>(chars: &mut std::iter::Peekable<I>) -> Option<usize>
where
    I: Iterator<Item = char> + Clone,
{
    let mut clone = chars.clone();
    let mut hashes = 0usize;
    while clone.peek() == Some(&'#') {
        hashes += 1;
        let _ = clone.next();
    }
    (clone.peek() == Some(&'"')).then_some(hashes)
}

fn raw_string_hashes_at_end<I>(chars: &mut std::iter::Peekable<I>, hashes: usize) -> bool
where
    I: Iterator<Item = char> + Clone,
{
    let mut clone = chars.clone();
    for _ in 0..hashes {
        if clone.next() != Some('#') {
            return false;
        }
    }
    for _ in 0..hashes {
        let _ = chars.next();
    }
    true
}
