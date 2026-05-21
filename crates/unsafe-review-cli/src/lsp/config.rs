use serde_json::Value;

#[derive(Clone, Debug)]
pub(super) struct LspConfig {
    pub(super) mode: String,
    pub(super) base: Option<String>,
    pub(super) max_cards: Option<usize>,
    pub(super) refresh_on_open: bool,
    pub(super) refresh_on_save: bool,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            mode: "repo".to_string(),
            base: None,
            max_cards: None,
            refresh_on_open: false,
            refresh_on_save: true,
        }
    }
}

pub(super) fn parse_config(v: Value) -> LspConfig {
    let mut cfg = LspConfig::default();
    if let Some(u) = v.get("unsafeReview") {
        if let Some(mode) = u.get("mode").and_then(Value::as_str)
            && matches!(mode, "repo" | "diff")
        {
            cfg.mode = mode.to_string();
        }
        if let Some(base) = u.get("base").and_then(Value::as_str) {
            cfg.base = Some(base.to_string());
        }
        if let Some(m) = u.get("maxCards").and_then(Value::as_u64) {
            cfg.max_cards = Some(m as usize);
        }
        if let Some(b) = u.get("refreshOnOpen").and_then(Value::as_bool) {
            cfg.refresh_on_open = b;
        }
        if let Some(b) = u.get("refreshOnSave").and_then(Value::as_bool) {
            cfg.refresh_on_save = b;
        }
    }
    cfg
}

pub(super) fn should_refresh_on_change(_cfg: &LspConfig) -> bool {
    false
}
