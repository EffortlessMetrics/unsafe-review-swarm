use serde::{Deserialize, Serialize};

mod summary;

pub const WITNESS_RECEIPT_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessReceipt {
    pub schema_version: String,
    pub card_id: String,
    pub tool: String,
    pub strength: String,
    pub author: Option<String>,
    pub recorded_at: Option<String>,
    pub expires_at: Option<String>,
    pub summary: Option<String>,
    pub command: Option<String>,
    pub limitations: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiriReceiptInput {
    pub card_id: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoCarefulReceiptInput {
    pub card_id: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizerReceiptInput {
    pub card_id: String,
    pub tool: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcurrencyReceiptInput {
    pub card_id: String,
    pub tool: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofReceiptInput {
    pub card_id: String,
    pub tool: String,
    pub output: String,
    pub author: String,
    pub recorded_at: String,
    pub expires_at: String,
    pub command: String,
    pub limitations: Vec<String>,
}

impl WitnessReceipt {
    pub fn validate(&self) -> Result<(), String> {
        validate_required(&self.schema_version, "schema_version")?;
        validate_required(&self.card_id, "card_id")?;
        validate_required(&self.tool, "tool")?;
        validate_tool(&self.tool)?;
        validate_strength(&self.strength)?;
        if !looks_like_counted_card_id(&self.card_id) {
            return Err("card_id must be an exact counted UR-* identity ending in -cN".to_string());
        }
        let author = validate_required_option(&self.author, "author")?;
        validate_required(author, "author")?;
        let recorded_at = validate_required_option(&self.recorded_at, "recorded_at")?;
        let expires_at = validate_required_option(&self.expires_at, "expires_at")?;
        validate_utc_timestamp(recorded_at, "recorded_at")?;
        validate_date(expires_at, "expires_at")?;
        if expires_at < &recorded_at[..10] {
            return Err("`expires_at` must be on or after the `recorded_at` date".to_string());
        }
        Ok(())
    }

    pub fn evidence_summary(&self) -> String {
        summary::evidence_summary(self)
    }

    pub fn to_pretty_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map(|mut text| {
                text.push('\n');
                text
            })
            .map_err(|err| format!("serialize witness receipt failed: {err}"))
    }

    pub fn from_miri_output(input: MiriReceiptInput) -> Result<Self, String> {
        validate_saved_success_output(&input.output, "Miri")?;
        validate_required(&input.command, "command")?;
        if !input.command.to_ascii_lowercase().contains("miri") {
            return Err("Miri receipt command must mention `miri`".to_string());
        }
        let mut limitations = vec![
            "saved-output adapter; unsafe-review did not run Miri".to_string(),
            "receipt strength is `ran`; site reach is not claimed".to_string(),
        ];
        limitations.extend(input.limitations);
        let receipt = Self {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: input.card_id,
            tool: "miri".to_string(),
            strength: "ran".to_string(),
            author: Some(input.author),
            recorded_at: Some(input.recorded_at),
            expires_at: Some(input.expires_at),
            summary: Some("saved Miri output reported `test result: ok`".to_string()),
            command: Some(input.command),
            limitations: Some(limitations),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn from_cargo_careful_output(input: CargoCarefulReceiptInput) -> Result<Self, String> {
        validate_saved_success_output(&input.output, "cargo-careful")?;
        validate_required(&input.command, "command")?;
        if !input.command.to_ascii_lowercase().contains("careful") {
            return Err("cargo-careful receipt command must mention `careful`".to_string());
        }
        let mut limitations = vec![
            "saved-output adapter; unsafe-review did not run cargo-careful".to_string(),
            "receipt strength is `ran`; site reach is not claimed".to_string(),
        ];
        limitations.extend(input.limitations);
        let receipt = Self {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: input.card_id,
            tool: "cargo-careful".to_string(),
            strength: "ran".to_string(),
            author: Some(input.author),
            recorded_at: Some(input.recorded_at),
            expires_at: Some(input.expires_at),
            summary: Some("saved cargo-careful output reported `test result: ok`".to_string()),
            command: Some(input.command),
            limitations: Some(limitations),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn from_sanitizer_output(input: SanitizerReceiptInput) -> Result<Self, String> {
        validate_sanitizer_tool(&input.tool)?;
        validate_saved_success_output(&input.output, &input.tool)?;
        validate_sanitizer_success_output(&input.output, &input.tool)?;
        validate_required(&input.command, "command")?;
        validate_sanitizer_command(&input.command)?;
        let mut limitations = vec![
            "saved-output adapter; unsafe-review did not run a sanitizer".to_string(),
            "receipt strength is `ran`; site reach is not claimed".to_string(),
        ];
        limitations.extend(input.limitations);
        let receipt = Self {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: input.card_id,
            tool: input.tool.clone(),
            strength: "ran".to_string(),
            author: Some(input.author),
            recorded_at: Some(input.recorded_at),
            expires_at: Some(input.expires_at),
            summary: Some(format!(
                "saved {} output reported `test result: ok`",
                input.tool
            )),
            command: Some(input.command),
            limitations: Some(limitations),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn from_concurrency_output(input: ConcurrencyReceiptInput) -> Result<Self, String> {
        validate_concurrency_tool(&input.tool)?;
        validate_saved_success_output(&input.output, &input.tool)?;
        validate_required(&input.command, "command")?;
        validate_concurrency_command(&input.command)?;
        let mut limitations = vec![
            "saved-output adapter; unsafe-review did not run a concurrency witness".to_string(),
            "receipt strength is `ran`; site reach is not claimed".to_string(),
        ];
        limitations.extend(input.limitations);
        let receipt = Self {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: input.card_id,
            tool: input.tool.clone(),
            strength: "ran".to_string(),
            author: Some(input.author),
            recorded_at: Some(input.recorded_at),
            expires_at: Some(input.expires_at),
            summary: Some(format!(
                "saved {} output reported `test result: ok`",
                input.tool
            )),
            command: Some(input.command),
            limitations: Some(limitations),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn from_proof_output(input: ProofReceiptInput) -> Result<Self, String> {
        validate_proof_tool(&input.tool)?;
        validate_saved_proof_success_output(&input.output, &input.tool)?;
        validate_required(&input.command, "command")?;
        validate_proof_command(&input.command)?;
        let mut limitations = vec![
            "saved-output adapter; unsafe-review did not run a proof tool".to_string(),
            "receipt strength is `ran`; site reach is not claimed".to_string(),
            "proof scope is limited to the recorded harness/output".to_string(),
        ];
        limitations.extend(input.limitations);
        let receipt = Self {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: input.card_id,
            tool: input.tool.clone(),
            strength: "ran".to_string(),
            author: Some(input.author),
            recorded_at: Some(input.recorded_at),
            expires_at: Some(input.expires_at),
            summary: Some(format!(
                "saved {} proof output reported verification success",
                input.tool
            )),
            command: Some(input.command),
            limitations: Some(limitations),
        };
        receipt.validate()?;
        Ok(receipt)
    }
}

fn is_supported_receipt_strength(value: &str) -> bool {
    matches!(
        value,
        "configured" | "ran" | "test_targeted" | "site_reached"
    )
}

fn is_supported_receipt_tool(value: &str) -> bool {
    matches!(
        value,
        "miri"
            | "cargo-careful"
            | "asan"
            | "msan"
            | "tsan"
            | "lsan"
            | "loom"
            | "shuttle"
            | "kani"
            | "crux"
            | "human-deep-review"
            | "unsupported"
    )
}

fn validate_required(value: &str, key: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("`{key}` is required"))
    } else {
        Ok(())
    }
}

fn validate_required_option<'a>(value: &'a Option<String>, key: &str) -> Result<&'a str, String> {
    let Some(value) = value.as_deref() else {
        return Err(format!("`{key}` is required"));
    };
    validate_required(value, key)?;
    Ok(value)
}

fn validate_strength(value: &str) -> Result<(), String> {
    if is_supported_receipt_strength(value) {
        Ok(())
    } else {
        Err(format!("uses unknown receipt strength `{value}`"))
    }
}

fn validate_tool(value: &str) -> Result<(), String> {
    if is_supported_receipt_tool(value) {
        Ok(())
    } else {
        Err(format!("uses unknown receipt tool `{value}`"))
    }
}

fn validate_sanitizer_tool(value: &str) -> Result<(), String> {
    if matches!(value, "asan" | "msan" | "tsan" | "lsan") {
        Ok(())
    } else {
        Err(format!(
            "sanitizer receipt tool must be one of `asan`, `msan`, `tsan`, or `lsan`, got `{value}`"
        ))
    }
}

fn validate_concurrency_tool(value: &str) -> Result<(), String> {
    if matches!(value, "loom" | "shuttle") {
        Ok(())
    } else {
        Err(format!(
            "concurrency receipt tool must be one of `loom` or `shuttle`, got `{value}`"
        ))
    }
}

fn validate_proof_tool(value: &str) -> Result<(), String> {
    if matches!(value, "kani" | "crux") {
        Ok(())
    } else {
        Err(format!(
            "proof receipt tool must be one of `kani` or `crux`, got `{value}`"
        ))
    }
}

fn validate_sanitizer_command(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    if ["sanitizer", "asan", "msan", "tsan", "lsan"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        Ok(())
    } else {
        Err(
            "sanitizer receipt command must mention `sanitizer`, `asan`, `msan`, `tsan`, or `lsan`"
                .to_string(),
        )
    }
}

fn validate_concurrency_command(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    if ["loom", "shuttle"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        Ok(())
    } else {
        Err("concurrency receipt command must mention `loom` or `shuttle`".to_string())
    }
}

fn validate_proof_command(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    if ["kani", "crux"].iter().any(|needle| lower.contains(needle)) {
        Ok(())
    } else {
        Err("proof receipt command must mention `kani` or `crux`".to_string())
    }
}

fn validate_saved_success_output(output: &str, tool: &str) -> Result<(), String> {
    if output.trim().is_empty() {
        return Err(format!("saved {tool} output is empty"));
    }
    let lower = output.to_ascii_lowercase();
    for needle in [
        "undefined behavior",
        "test result: failed",
        "failures:",
        "panicked at",
        "error:",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "saved {tool} output contains failure marker `{needle}`"
            ));
        }
    }
    if !lower.contains("test result: ok") {
        return Err(format!(
            "saved {tool} output must contain `test result: ok`"
        ));
    }
    Ok(())
}

fn validate_saved_proof_success_output(output: &str, tool: &str) -> Result<(), String> {
    if output.trim().is_empty() {
        return Err(format!("saved {tool} proof output is empty"));
    }
    let lower = output.to_ascii_lowercase();
    for needle in [
        "verification failed",
        "verification:- failed",
        "counterexample",
        "assertion failed",
        "panicked at",
        "panic",
        "error:",
        "failed",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "saved {tool} proof output contains failure marker `{needle}`"
            ));
        }
    }
    if [
        "verification:- successful",
        "verification successful",
        "verification result: verified",
        "proof successful",
        "status: verified",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Ok(())
    } else {
        Err(format!(
            "saved {tool} proof output must contain a verification success marker"
        ))
    }
}

fn validate_sanitizer_success_output(output: &str, tool: &str) -> Result<(), String> {
    let lower = output.to_ascii_lowercase();
    for needle in [
        "addresssanitizer:",
        "memorysanitizer:",
        "threadsanitizer:",
        "leaksanitizer:",
        "detected memory leaks",
        "data race",
        "deadlysignal",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "saved {tool} output contains sanitizer failure marker `{needle}`"
            ));
        }
    }
    Ok(())
}

fn validate_utc_timestamp(value: &str, key: &str) -> Result<(), String> {
    if !timestamp_validation::has_utc_shape(value) {
        return Err(format!(
            "`{key}` must use UTC timestamp format YYYY-MM-DDTHH:MM:SSZ"
        ));
    }
    validate_date(&value[..10], key)?;
    timestamp_validation::validate_time_components(value, key)
}

fn validate_date(value: &str, key: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|index| bytes[*index].is_ascii_digit());
    if !valid_shape {
        return Err(format!("`{key}` must use date format YYYY-MM-DD"));
    }
    validate_range(decimal_at(value, 0, 4), 1, 9999, key)?;
    validate_range(decimal_at(value, 5, 2), 1, 12, key)?;
    validate_range(decimal_at(value, 8, 2), 1, 31, key)
}

fn decimal_at(value: &str, start: usize, len: usize) -> Option<u32> {
    value.get(start..start + len)?.parse().ok()
}

fn validate_range(value: Option<u32>, min: u32, max: u32, key: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Err(format!("`{key}` contains an invalid number"));
    };
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("`{key}` is out of range"))
    }
}

fn looks_like_counted_card_id(value: &str) -> bool {
    let Some((prefix, count)) = value.rsplit_once("-c") else {
        return false;
    };
    value.starts_with("UR-")
        && !prefix.is_empty()
        && !count.is_empty()
        && count.bytes().all(|byte| byte.is_ascii_digit())
}

mod timestamp_validation {
    use super::{decimal_at, validate_range};

    pub(super) fn has_utc_shape(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 20
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'T'
            && bytes[13] == b':'
            && bytes[16] == b':'
            && bytes[19] == b'Z'
            && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
                .iter()
                .all(|index| bytes[*index].is_ascii_digit())
    }

    pub(super) fn validate_time_components(value: &str, key: &str) -> Result<(), String> {
        validate_range(decimal_at(value, 11, 2), 0, 23, key)?;
        validate_range(decimal_at(value, 14, 2), 0, 59, key)?;
        validate_range(decimal_at(value, 17, 2), 0, 59, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_receipt_json_round_trips_and_validates() -> Result<(), String> {
        let receipt = fixture_receipt();
        receipt.validate()?;

        let json = receipt.to_pretty_json()?;
        let decoded: WitnessReceipt = serde_json::from_str(&json)
            .map_err(|err| format!("deserialize receipt failed: {err}"))?;

        assert_eq!(decoded, receipt);
        assert!(decoded.evidence_summary().contains("Imported miri receipt"));
        assert!(decoded.evidence_summary().contains("fixture only"));
        Ok(())
    }

    #[test]
    fn evidence_summary_omits_whitespace_only_optional_fields() {
        let mut receipt = fixture_receipt();
        receipt.summary = Some(" ".to_string());
        receipt.author = Some("\t".to_string());
        receipt.recorded_at = Some("\n".to_string());
        receipt.expires_at = Some("   ".to_string());
        receipt.command = Some("\r\n".to_string());
        receipt.limitations = Some(vec!["fixture only".to_string()]);

        assert_eq!(
            receipt.evidence_summary(),
            "Imported miri receipt with `ran` strength; limitations: fixture only"
        );
    }

    #[test]
    fn witness_receipt_validation_rejects_unknown_tool() {
        let mut receipt = fixture_receipt();
        receipt.tool = "proof-bot".to_string();

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("unknown receipt tool")
        );
    }

    #[test]
    fn witness_receipt_validation_rejects_missing_author() {
        let mut receipt = fixture_receipt();
        receipt.author = None;

        assert!(
            receipt
                .validate()
                .err()
                .unwrap_or_default()
                .contains("`author` is required")
        );
    }

    #[test]
    fn miri_receipt_from_saved_output_uses_ran_strength_without_site_reach() -> Result<(), String> {
        let receipt = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: vec!["fixture only".to_string()],
        })?;

        assert_eq!(receipt.tool, "miri");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved Miri output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run Miri"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn miri_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "error: Undefined Behavior: pointer must be aligned\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly miri test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn miri_receipt_from_saved_output_requires_miri_command() {
        let result = WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("must mention `miri`")
        );
    }

    #[test]
    fn cargo_careful_receipt_from_saved_output_uses_ran_strength_without_site_reach()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly careful test read_header".to_string(),
            limitations: vec!["fixture only".to_string()],
        })?;

        assert_eq!(receipt.tool, "cargo-careful");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved cargo-careful output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run cargo-careful"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn cargo_careful_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: FAILED. 0 passed; 1 failed\nfailures:\nread_header\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo +nightly careful test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn cargo_careful_receipt_from_saved_output_requires_careful_command() {
        let result = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("must mention `careful`")
        );
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_uses_ran_strength_without_site_reach()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: vec!["fixture only".to_string()],
        })?;

        assert_eq!(receipt.tool, "asan");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved asan output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run a sanitizer"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_rejects_unsupported_sanitizer_tool() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "ubsan".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("sanitizer receipt tool")
        );
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "==123==ERROR: AddressSanitizer: heap-use-after-free\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn sanitizer_receipt_from_saved_output_requires_sanitizer_command() {
        let result = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "asan".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test read_header".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("sanitizer receipt command")
        );
    }

    #[test]
    fn concurrency_receipt_from_saved_output_uses_ran_strength_without_site_reach()
    -> Result<(), String> {
        let receipt = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "loom".to_string(),
            output: "running 1 test\ntest shared_cell_loom ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell_loom -- --nocapture".to_string(),
            limitations: vec!["fixture only".to_string()],
        })?;

        assert_eq!(receipt.tool, "loom");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved loom output reported `test result: ok`")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run a concurrency witness"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn concurrency_receipt_from_saved_output_rejects_unsupported_tool() {
        let result = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "kani".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell_loom -- --nocapture".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("concurrency receipt tool")
        );
    }

    #[test]
    fn concurrency_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "loom".to_string(),
            output: "test result: FAILED. 0 passed; 1 failed\nfailures:\nshared_cell_loom\n"
                .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell_loom -- --nocapture".to_string(),
            limitations: Vec::new(),
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn concurrency_receipt_from_saved_output_requires_concurrency_command() {
        let result = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
            card_id: "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
                .to_string(),
            tool: "loom".to_string(),
            output: "test result: ok. 1 passed; 0 failed; finished in 0.01s\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test shared_cell -- --nocapture".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("concurrency receipt command")
        );
    }

    #[test]
    fn proof_receipt_from_saved_output_uses_ran_strength_without_site_reach() -> Result<(), String>
    {
        let receipt = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "kani".to_string(),
            output:
                "Kani Rust Verifier\nVERIFICATION:- SUCCESSFUL\nVerification result: verified\n"
                    .to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: vec!["fixture only".to_string()],
        })?;

        assert_eq!(receipt.tool, "kani");
        assert_eq!(receipt.strength, "ran");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved kani proof output reported verification success")
        );
        let limitations = receipt.limitations.as_ref().ok_or("missing limitations")?;
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("unsafe-review did not run a proof tool"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("site reach is not claimed"))
        );
        assert!(
            limitations
                .iter()
                .any(|item| item.contains("recorded harness/output"))
        );
        assert!(limitations.iter().any(|item| item == "fixture only"));
        Ok(())
    }

    #[test]
    fn proof_receipt_from_saved_output_accepts_crux_success_marker() -> Result<(), String> {
        let receipt = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "crux".to_string(),
            output: "Crux verification\nStatus: Verified\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "crux prove byte_to_bool".to_string(),
            limitations: Vec::new(),
        })?;

        assert_eq!(receipt.tool, "crux");
        assert_eq!(
            receipt.summary.as_deref(),
            Some("saved crux proof output reported verification success")
        );
        Ok(())
    }

    #[test]
    fn proof_receipt_from_saved_output_rejects_unsupported_tool() {
        let result = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "prusti".to_string(),
            output: "VERIFICATION:- SUCCESSFUL\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("proof receipt tool")
        );
    }

    #[test]
    fn proof_receipt_from_saved_output_rejects_failure_markers() {
        let result = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "kani".to_string(),
            output: "VERIFICATION:- FAILED\nCounterexample generated\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo kani --harness byte_to_bool_harness".to_string(),
            limitations: Vec::new(),
        });

        assert!(result.err().unwrap_or_default().contains("failure marker"));
    }

    #[test]
    fn proof_receipt_from_saved_output_requires_proof_command() {
        let result = WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id:
                "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
                    .to_string(),
            tool: "kani".to_string(),
            output: "VERIFICATION:- SUCCESSFUL\n".to_string(),
            author: "core/fixtures".to_string(),
            recorded_at: "2026-05-18T00:00:00Z".to_string(),
            expires_at: "2026-08-18".to_string(),
            command: "cargo test byte_to_bool".to_string(),
            limitations: Vec::new(),
        });

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("proof receipt command")
        );
    }

    fn fixture_receipt() -> WitnessReceipt {
        WitnessReceipt {
            schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
            card_id: "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                .to_string(),
            tool: "miri".to_string(),
            strength: "ran".to_string(),
            author: Some("core/fixtures".to_string()),
            recorded_at: Some("2026-05-18T00:00:00Z".to_string()),
            expires_at: Some("2026-08-18".to_string()),
            summary: Some("focused witness passed".to_string()),
            command: Some("cargo +nightly miri test read_header".to_string()),
            limitations: Some(vec!["fixture only".to_string()]),
        }
    }
}
