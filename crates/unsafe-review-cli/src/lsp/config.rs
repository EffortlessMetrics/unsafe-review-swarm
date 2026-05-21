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
    v.get("unsafeReview")
        .map_or_else(LspConfig::default, parse_unsafe_review_config)
}

fn parse_unsafe_review_config(config: &Value) -> LspConfig {
    let mut cfg = LspConfig::default();
    if let Some(mode) = parse_mode(config) {
        cfg.mode = mode.to_string();
    }
    if let Some(base) = config.get("base").and_then(Value::as_str) {
        cfg.base = Some(base.to_string());
    }
    cfg.max_cards = parse_max_cards(config);
    if let Some(refresh_on_open) = config.get("refreshOnOpen").and_then(Value::as_bool) {
        cfg.refresh_on_open = refresh_on_open;
    }
    if let Some(refresh_on_save) = config.get("refreshOnSave").and_then(Value::as_bool) {
        cfg.refresh_on_save = refresh_on_save;
    }
    cfg
}

fn parse_mode(config: &Value) -> Option<&str> {
    config
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| matches!(*mode, "repo" | "diff"))
}

fn parse_max_cards(config: &Value) -> Option<usize> {
    config
        .get("maxCards")
        .and_then(Value::as_u64)
        .and_then(|raw| usize::try_from(raw).ok())
}

pub(super) fn should_refresh_on_change(_cfg: &LspConfig) -> bool {
    false
}
