use super::SnapshotCard;

pub(super) struct WitnessState {
    pub(super) label: String,
    pub(super) rank: u8,
}

pub(super) fn witness_state(card: &SnapshotCard) -> WitnessState {
    if let Some(strength) = imported_receipt_strength(&card.witness) {
        return WitnessState {
            rank: witness_rank(&strength),
            label: strength,
        };
    }
    if card
        .missing
        .iter()
        .any(|item| item.to_ascii_lowercase().contains("witness"))
        || card.witness.contains("No imported witness receipt")
        || card.witness.trim().is_empty()
    {
        return WitnessState {
            label: "missing".to_string(),
            rank: 0,
        };
    }
    WitnessState {
        label: "present".to_string(),
        rank: witness_rank("ran"),
    }
}

fn imported_receipt_strength(summary: &str) -> Option<String> {
    let marker = " receipt with `";
    let start = summary.find(marker)? + marker.len();
    let end = summary[start..].find("` strength")? + start;
    Some(summary[start..end].to_string())
}

pub(super) fn witness_rank(value: &str) -> u8 {
    match value {
        "missing" => 0,
        "configured" => 1,
        "ran" => 2,
        "test_targeted" => 3,
        "site_reached" => 4,
        _ => 2,
    }
}
