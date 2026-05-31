pub(super) fn parse_fn_name(line: &str) -> Option<String> {
    let marker = "fn ";
    let pos = line.find(marker)?;
    let rest = &line[pos + marker.len()..];
    parse_ident(rest)
}

pub(super) fn parse_mod_name(line: &str) -> Option<String> {
    let mut rest = line.trim_start();
    if let Some(after_pub) = rest.strip_prefix("pub ") {
        rest = after_pub.trim_start();
    } else if let Some(after_pub) = rest.strip_prefix("pub(") {
        let after_pub = after_pub.trim_start();
        if let Some((_visibility, after_visibility)) = after_pub.split_once(')') {
            rest = after_visibility.trim_start();
        }
    }
    let rest = rest.strip_prefix("mod ")?.trim_start();
    parse_ident(rest)
}

pub(super) fn parse_trait_name(line: &str) -> Option<String> {
    let marker = "trait ";
    let pos = line.find(marker)?;
    let rest = &line[pos + marker.len()..];
    parse_ident(rest)
}

pub(super) fn parse_macro_rules_name(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("macro_rules!")?.trim_start();
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
