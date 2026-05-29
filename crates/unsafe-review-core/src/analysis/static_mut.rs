pub(super) fn is_static_mut_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("static mut ") {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix("pub ") {
        return rest.trim_start().starts_with("static mut ");
    }
    if trimmed.starts_with("pub(") {
        return trimmed
            .split_once(')')
            .is_some_and(|(_visibility, rest)| rest.trim_start().starts_with("static mut "));
    }
    false
}

pub(super) fn parse_static_mut_name(line: &str) -> Option<String> {
    let mut rest = line.trim_start();
    if let Some(after_pub) = rest.strip_prefix("pub ") {
        rest = after_pub.trim_start();
    } else if rest.starts_with("pub(") {
        rest = rest.split_once(')')?.1.trim_start();
    }
    let rest = rest.strip_prefix("static mut ")?.trim_start();
    parse_ident(rest)
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
