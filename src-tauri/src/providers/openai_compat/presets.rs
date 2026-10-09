//! Summary LLM provider presets (single source of truth for the backend).
//!
//! Every preset speaks the OpenAI Chat Completions protocol. Presets only
//! differ in their default Base URL / recommended model and in which optional
//! request-body extensions they are known to accept.

use serde_json::Value;

/// Provider id used when nothing has been configured (pre-upgrade behavior).
pub const DEFAULT_PROVIDER_ID: &str = "dashscope";

/// Free-form OpenAI-compatible endpoint (no defaults, no extensions).
pub const CUSTOM_PROVIDER_ID: &str = "custom";

#[derive(Debug, Clone, Copy)]
pub struct ProviderPreset {
    /// Stable id persisted in SQLite and sent over IPC.
    pub id: &'static str,
    /// Human-readable name used in user-facing error messages.
    pub label: &'static str,
    /// Default OpenAI-compatible base URL (without `/chat/completions`); empty for `custom`.
    pub default_base_url: &'static str,
    /// Recommended model; `None` means the user must fill it in.
    pub default_model: Option<&'static str>,
    /// Whether `response_format: { type: "json_object" }` is sent for summaries.
    pub json_mode: bool,
    /// Name of the output-token cap field used by the connection test.
    pub max_tokens_field: &'static str,
    /// Vendor-specific body additions (e.g. DashScope `enable_thinking`).
    pub extra_body: fn(&mut Value),
}

fn no_extra_body(_body: &mut Value) {}

/// DashScope Qwen3 models default to thinking mode, which is not allowed for
/// non-streaming calls; disable it explicitly.
fn dashscope_extra_body(body: &mut Value) {
    if let Some(map) = body.as_object_mut() {
        map.insert("enable_thinking".into(), Value::Bool(false));
    }
}

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "dashscope",
        label: "阿里云百炼 DashScope",
        default_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        default_model: Some("qwen3.7-plus"),
        json_mode: true,
        max_tokens_field: "max_tokens",
        extra_body: dashscope_extra_body,
    },
    ProviderPreset {
        id: "deepseek",
        label: "DeepSeek",
        default_base_url: "https://api.deepseek.com/v1",
        default_model: Some("deepseek-chat"),
        json_mode: true,
        max_tokens_field: "max_tokens",
        extra_body: no_extra_body,
    },
    ProviderPreset {
        id: "openai",
        label: "OpenAI",
        default_base_url: "https://api.openai.com/v1",
        default_model: None,
        json_mode: true,
        // OpenAI reasoning models reject `max_tokens`; `max_completion_tokens`
        // is accepted by all current chat models.
        max_tokens_field: "max_completion_tokens",
        extra_body: no_extra_body,
    },
    ProviderPreset {
        id: "moonshot",
        label: "Moonshot（Kimi）",
        default_base_url: "https://api.moonshot.cn/v1",
        default_model: None,
        json_mode: true,
        max_tokens_field: "max_tokens",
        extra_body: no_extra_body,
    },
    ProviderPreset {
        id: "zhipu",
        label: "智谱 BigModel",
        default_base_url: "https://open.bigmodel.cn/api/paas/v4",
        default_model: None,
        json_mode: true,
        max_tokens_field: "max_tokens",
        extra_body: no_extra_body,
    },
    ProviderPreset {
        id: "ark",
        label: "火山方舟（豆包）",
        default_base_url: "https://ark.cn-beijing.volces.com/api/v3",
        default_model: None,
        json_mode: false,
        max_tokens_field: "max_tokens",
        extra_body: no_extra_body,
    },
    ProviderPreset {
        id: CUSTOM_PROVIDER_ID,
        label: "自定义（OpenAI 兼容）",
        default_base_url: "",
        default_model: None,
        json_mode: false,
        max_tokens_field: "max_tokens",
        extra_body: no_extra_body,
    },
];

/// Look up a preset by id (exact match after trim).
pub fn find_preset(id: &str) -> Option<&'static ProviderPreset> {
    let id = id.trim();
    PRESETS.iter().find(|p| p.id == id)
}

/// Resolve a persisted provider id; unknown ids fall back to `custom`
/// (no defaults, no vendor extensions), empty falls back to the default.
pub fn resolve_preset(id: &str) -> &'static ProviderPreset {
    let id = id.trim();
    if id.is_empty() {
        return find_preset(DEFAULT_PROVIDER_ID).expect("default preset");
    }
    find_preset(id).unwrap_or_else(|| find_preset(CUSTOM_PROVIDER_ID).expect("custom preset"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_ids_are_unique_and_include_required() {
        let ids: Vec<&str> = PRESETS.iter().map(|p| p.id).collect();
        for required in [
            "dashscope",
            "deepseek",
            "openai",
            "moonshot",
            "zhipu",
            "ark",
            "custom",
        ] {
            assert!(ids.contains(&required), "missing preset {required}");
        }
        let mut dedup = ids.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), ids.len());
    }

    #[test]
    fn non_custom_presets_have_https_base_url() {
        for preset in PRESETS.iter().filter(|p| p.id != CUSTOM_PROVIDER_ID) {
            assert!(
                preset.default_base_url.starts_with("https://"),
                "{}",
                preset.id
            );
            assert!(!preset.default_base_url.ends_with('/'), "{}", preset.id);
        }
    }

    #[test]
    fn default_preset_matches_pre_upgrade_behavior() {
        let preset = resolve_preset("");
        assert_eq!(preset.id, "dashscope");
        assert_eq!(
            preset.default_base_url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(preset.default_model, Some("qwen3.7-plus"));
        assert!(preset.json_mode);
    }

    #[test]
    fn unknown_provider_resolves_to_custom() {
        assert_eq!(resolve_preset("future-vendor").id, CUSTOM_PROVIDER_ID);
        assert!(find_preset("future-vendor").is_none());
    }

    #[test]
    fn only_dashscope_adds_enable_thinking() {
        for preset in PRESETS {
            let mut body = serde_json::json!({});
            (preset.extra_body)(&mut body);
            if preset.id == "dashscope" {
                assert_eq!(body["enable_thinking"], false);
            } else {
                assert!(body.get("enable_thinking").is_none(), "{}", preset.id);
            }
        }
    }
}
