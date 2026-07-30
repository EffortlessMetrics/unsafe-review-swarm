use std::fs;

const RECEIPT: &str = "docs/dogfood/density-receipts/memchr-target-feature.toml";

pub(crate) fn check(corpus: &toml::Value) -> Result<(), String> {
    let text = fs::read_to_string(crate::workspace_path(RECEIPT))
        .map_err(|error| format!("read {RECEIPT}: {error}"))?;
    let receipt = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|error| format!("parse {RECEIPT}: {error}"))?;
    check_value(&receipt, corpus)
}

fn check_value(receipt: &toml::Value, corpus: &toml::Value) -> Result<(), String> {
    require_string(receipt, "schema_version", "1.0")?;
    require_string(receipt, "family", "target_feature")?;
    crate::require_boundary_text(required_string(receipt, "trust_boundary")?, RECEIPT)?;

    let target_id = required_string(receipt, "target")?;
    let target = corpus
        .get("targets")
        .and_then(toml::Value::as_array)
        .and_then(|targets| {
            targets
                .iter()
                .find(|target| target.get("id").and_then(toml::Value::as_str) == Some(target_id))
        })
        .ok_or_else(|| format!("{RECEIPT} references unknown dogfood target `{target_id}`"))?;
    for field in ["repository", "commit"] {
        let expected = required_string(target, field)?;
        require_string(receipt, field, expected)?;
    }

    let before_inventory = table(receipt, "inventory_before")?;
    let after_inventory = table(receipt, "inventory_after")?;
    let before_cards = integer(before_inventory, "raw_card_count")?;
    let after_cards = integer(after_inventory, "raw_card_count")?;
    let before_family = integer(before_inventory, "family_card_count")?;
    let after_family = integer(after_inventory, "family_card_count")?;
    if before_cards != after_cards || before_family != after_family {
        return Err(format!(
            "{RECEIPT} must preserve raw and target_feature card counts"
        ));
    }
    let before_digest = table_string(before_inventory, "card_inventory_sha256")?;
    let after_digest = table_string(after_inventory, "card_inventory_sha256")?;
    if before_digest.len() != 64
        || !before_digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        || before_digest != after_digest
    {
        return Err(format!(
            "{RECEIPT} must preserve a 64-hex card inventory digest"
        ));
    }

    let before_summary = table(receipt, "summary_before")?;
    let after_summary = table(receipt, "summary_after")?;
    check_summary(before_summary, before_family, "summary_before")?;
    check_summary(after_summary, after_family, "summary_after")?;
    if integer(after_summary, "rows")? >= integer(before_summary, "rows")? {
        return Err(format!(
            "{RECEIPT} must record reduced target_feature summary rows"
        ));
    }

    let before_comments = table(receipt, "comment_plan_before")?;
    let after_comments = table(receipt, "comment_plan_after")?;
    check_comment_plan(before_comments, before_family, "comment_plan_before")?;
    check_comment_plan(after_comments, after_family, "comment_plan_after")?;
    if integer(after_comments, "selected")? > integer(before_comments, "selected")? {
        return Err(format!(
            "{RECEIPT} must not increase selected target_feature comments"
        ));
    }
    Ok(())
}

fn check_summary(table: &toml::value::Table, cards: i64, name: &str) -> Result<(), String> {
    let rows = integer(table, "rows")?;
    let sites = integer(table, "sites")?;
    let rendered = integer(table, "representatives_rendered")?;
    let omitted = integer(table, "representatives_omitted")?;
    if rows <= 0 || sites != cards || rendered + omitted != sites || rendered < rows {
        return Err(format!(
            "{RECEIPT} {name} has inconsistent row/site density arithmetic"
        ));
    }
    Ok(())
}

fn check_comment_plan(table: &toml::value::Table, cards: i64, name: &str) -> Result<(), String> {
    let selected = integer(table, "selected")?;
    let grouped = integer(table, "grouped_repetition_not_selected")?;
    let other = integer(table, "other_not_selected")?;
    if selected + grouped + other != cards {
        return Err(format!(
            "{RECEIPT} {name} does not account for every target_feature card"
        ));
    }
    Ok(())
}

fn table<'a>(value: &'a toml::Value, field: &str) -> Result<&'a toml::value::Table, String> {
    value
        .get(field)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{RECEIPT} is missing [{field}]"))
}

fn required_string<'a>(value: &'a toml::Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{RECEIPT} is missing non-empty `{field}`"))
}

fn require_string(value: &toml::Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = required_string(value, field)?;
    if actual != expected {
        return Err(format!(
            "{RECEIPT} `{field}` must be `{expected}`, got `{actual}`"
        ));
    }
    Ok(())
}

fn integer(table: &toml::value::Table, field: &str) -> Result<i64, String> {
    table
        .get(field)
        .and_then(toml::Value::as_integer)
        .filter(|value| *value >= 0)
        .ok_or_else(|| format!("{RECEIPT} is missing non-negative `{field}`"))
}

fn table_string<'a>(table: &'a toml::value::Table, field: &str) -> Result<&'a str, String> {
    table
        .get(field)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{RECEIPT} is missing non-empty `{field}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_density_receipt_is_self_consistent() -> Result<(), String> {
        let corpus = fs::read_to_string(crate::workspace_path("docs/dogfood/corpus.toml"))
            .map_err(|error| error.to_string())?
            .parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|error| error.to_string())?;
        check(&corpus)
    }

    #[test]
    fn density_receipt_rejects_pin_and_inventory_drift() -> Result<(), String> {
        let corpus = corpus()?;
        let mut receipt = receipt_value()?;
        receipt["commit"] = toml::Value::String("floating-main".to_string());
        assert!(check_value(&receipt, &corpus).is_err());

        let mut receipt = receipt_value()?;
        receipt["inventory_after"]["family_card_count"] = toml::Value::Integer(60);
        assert!(check_value(&receipt, &corpus).is_err());
        Ok(())
    }

    #[test]
    fn density_receipt_rejects_broken_projection_accounting() -> Result<(), String> {
        let corpus = corpus()?;
        let mut receipt = receipt_value()?;
        receipt["summary_after"]["representatives_omitted"] = toml::Value::Integer(22);
        assert!(check_value(&receipt, &corpus).is_err());

        let mut receipt = receipt_value()?;
        receipt["comment_plan_after"]["other_not_selected"] = toml::Value::Integer(60);
        assert!(check_value(&receipt, &corpus).is_err());
        Ok(())
    }

    fn corpus() -> Result<toml::Value, String> {
        fs::read_to_string(crate::workspace_path("docs/dogfood/corpus.toml"))
            .map_err(|error| error.to_string())?
            .parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|error| error.to_string())
    }

    fn receipt_value() -> Result<toml::Value, String> {
        fs::read_to_string(crate::workspace_path(RECEIPT))
            .map_err(|error| error.to_string())?
            .parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|error| error.to_string())
    }
}
