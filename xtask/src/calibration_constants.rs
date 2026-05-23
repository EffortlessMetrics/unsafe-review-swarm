pub const CALIBRATION_REQUIRED_KINDS: &[&str] = &["positive", "negative", "false_positive_control"];
pub const CALIBRATION_CASE_FIELDS: &[&str] = &[
    "fixture",
    "kind",
    "claim",
    "support_tier",
    "expected_cards",
    "expected_class",
    "expected_operation_family",
    "expected_hazard",
];
pub const OPERATION_FAMILY_REGISTRY: &str =
    "docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md";
pub const OPERATION_FAMILY_REGISTRY_COLUMNS: usize = 9;
pub const OPERATION_FAMILY_REGISTRY_HEADER: &[&str] = &[
    "operation_family",
    "detected syntax shapes",
    "hazards",
    "not hazards",
    "obligation / evidence keys",
    "witness route",
    "fixture proof",
    "known false-positive controls",
    "known limits",
];
pub const OPERATION_FAMILY_REGISTRY_REQUIRED_TEXT_COLUMNS: &[(usize, &str)] = &[
    (1, "detected syntax shapes"),
    (7, "known false-positive controls"),
    (8, "known limits"),
];
pub const OPERATION_FAMILY_REGISTRY_OBLIGATION_KEYS_COLUMN: (usize, &str) =
    (4, "obligation / evidence keys");
pub const OPERATION_FAMILY_SOURCE: &str = "crates/unsafe-review-core/src/domain/operation.rs";
pub const SAFETY_OBLIGATION_SOURCE: &str = "crates/unsafe-review-core/src/analysis/obligations.rs";
pub const HAZARD_KIND_SOURCE: &str = "crates/unsafe-review-core/src/domain/hazard.rs";
pub const WITNESS_KIND_SOURCE: &str = "crates/unsafe-review-core/src/domain/witness.rs";
pub const ZERO_CARD_EXPECTATION_FIELDS: &[&str] = &[
    "expected_class",
    "expected_operation_family",
    "expected_hazard",
];
