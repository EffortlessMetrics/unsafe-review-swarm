pub(super) fn parse_impl_owner(line: &str) -> Option<String> {
    let rest = strip_impl_declaration_prefixes(line);
    if !starts_with_impl_keyword(rest) {
        return None;
    }
    let owner_start = rest
        .find(" for ")
        .map(|pos| pos + " for ".len())
        .or_else(|| impl_self_type_start(rest))?;
    parse_ident(&rest[owner_start..])
}

pub(super) fn parse_impl_declaration_owner(line: &str) -> Option<String> {
    is_impl_declaration_line(line).then(|| parse_impl_owner(line))?
}

pub(super) fn parse_impl_trait_name(line: &str) -> Option<String> {
    let rest = impl_after_keyword(line)?;
    let (trait_path, _self_type) = rest.split_once(" for ")?;
    let trait_name = trait_path.trim().rsplit("::").next()?.trim();
    parse_ident(trait_name)
}

pub(super) fn is_impl_declaration_line(line: &str) -> bool {
    starts_with_impl_keyword(strip_impl_declaration_prefixes(line))
}

fn strip_impl_declaration_prefixes(line: &str) -> &str {
    let mut rest = line.trim_start();
    if let Some(after_pub) = rest.strip_prefix("pub ") {
        rest = after_pub.trim_start();
    } else if let Some(after_pub) = rest.strip_prefix("pub(") {
        let after_pub = after_pub.trim_start();
        if let Some((_visibility, after_visibility)) = after_pub.split_once(')') {
            rest = after_visibility.trim_start();
        }
    }
    if let Some(after_unsafe) = rest.strip_prefix("unsafe ") {
        rest = after_unsafe.trim_start();
    }
    rest
}

fn starts_with_impl_keyword(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("impl") else {
        return false;
    };
    rest.chars()
        .next()
        .is_some_and(|ch| ch == '<' || ch.is_whitespace())
}

fn impl_self_type_start(line: &str) -> Option<usize> {
    let rest = impl_after_keyword(line)?;
    let offset = line.len().saturating_sub(rest.len());
    Some(offset)
}

fn impl_after_keyword(line: &str) -> Option<&str> {
    let mut rest = strip_impl_declaration_prefixes(line)
        .strip_prefix("impl")?
        .trim_start();
    if rest.starts_with('<') {
        rest = rest.get(generic_param_list_len(rest)?..)?.trim_start();
    }
    Some(rest)
}

fn generic_param_list_len(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_ident(rest: &str) -> Option<String> {
    let mut name = String::new();
    for ch in rest.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}
