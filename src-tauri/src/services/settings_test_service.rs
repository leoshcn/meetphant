//! Credential connectivity probes — merge form overrides with keyring/SQLite, never persist.

use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{AppErrorDto, CmdResult};
use crate::providers::doubao::HttpAsyncClient;
use crate::providers::openai_compat::presets;
use crate::providers::openai_compat::{HttpChatClient, LlmConfig};
use crate::providers::tos::{HttpTosClient, TosConfig};
use crate::services::credentials::{self, DoubaoCredentials, TosCredentials};
use crate::services::settings_service::{self, EffectiveSummaryLlm};

/// IPC success payload for `settings_test_*`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SettingsTestResult {
    pub ok: bool,
}

impl SettingsTestResult {
    pub fn ok() -> Self {
        Self { ok: true }
    }
}

/// Pick override when non-empty after trim; otherwise use saved.
pub fn merge_secret_field(override_val: Option<&str>, saved: Option<&str>) -> Option<String> {
    if let Some(raw) = override_val {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    saved
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Merge Doubao API key override with keyring. Does not write.
pub fn merge_doubao_credentials(doubao_api_key: Option<&str>) -> CmdResult<DoubaoCredentials> {
    let saved = credentials::get_credentials()?;
    let api_key = merge_secret_field(doubao_api_key, saved.as_ref().map(|c| c.api_key.as_str()));
    api_key
        .map(|api_key| DoubaoCredentials { api_key })
        .ok_or_else(AppErrorDto::asr_not_configured)
}

/// Form overrides for the summary LLM connection test; empty/omitted → saved value.
#[derive(Debug, Clone, Copy, Default)]
pub struct SummaryLlmTestOverrides<'a> {
    pub api_key: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub model: Option<&'a str>,
}

/// Merge summary LLM overrides with SQLite + keyring into a request config. Does not write.
///
/// When the merged provider or base URL differs from the saved one, the saved
/// key is never reused (D5): an API key override is required.
pub fn merge_summary_llm_config(
    conn: &Connection,
    overrides: SummaryLlmTestOverrides<'_>,
) -> CmdResult<LlmConfig> {
    let settings = settings_service::get_settings(conn)?;
    let saved = EffectiveSummaryLlm::from_settings(&settings);

    let provider = match merge_secret_field(overrides.provider, None) {
        Some(raw) => settings_service::validate_summary_llm_provider(&raw)?.to_string(),
        None => saved.provider.clone(),
    };
    let provider_changed = provider != saved.provider;
    let preset = presets::resolve_preset(&provider);

    let base_url = match merge_secret_field(overrides.base_url, None) {
        Some(url) => url,
        None if provider_changed => preset.default_base_url.to_string(),
        None => saved.base_url.clone(),
    };
    let model = match merge_secret_field(overrides.model, None) {
        Some(model) => model,
        None if provider_changed => preset.default_model.unwrap_or_default().to_string(),
        None => saved.model.clone(),
    };
    settings_service::validate_summary_llm_base_url(&base_url)?;
    settings_service::validate_summary_llm_model(&model)?;

    let next = EffectiveSummaryLlm {
        provider,
        base_url,
        model,
    };
    let api_key = match merge_secret_field(overrides.api_key, None) {
        Some(key) => key,
        None if settings_service::summary_llm_endpoint_changed(&saved, &next) => {
            return Err(AppErrorDto::with_details(
                "SETTINGS_INVALID",
                "切换服务商或修改 Base URL 后需重新填写 API Key",
                serde_json::json!({ "field": "summary_llm_api_key" }),
            ));
        }
        None => credentials::get_summary_llm_credentials()?
            .map(|c| c.api_key)
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(AppErrorDto::summary_not_configured)?,
    };

    Ok(LlmConfig {
        provider: next.provider,
        base_url: next.base_url,
        model: next.model,
        api_key,
    })
}

/// Merged TOS config for probes (secrets + non-secrets). Does not write.
pub fn merge_tos_config(
    conn: &Connection,
    tos_access_key_id: Option<&str>,
    tos_secret_access_key: Option<&str>,
    tos_region: Option<&str>,
    tos_bucket: Option<&str>,
    tos_endpoint: Option<&str>,
) -> CmdResult<TosConfig> {
    let saved_secrets = credentials::get_tos_credentials()?;
    let settings = settings_service::get_settings(conn)?;

    let access_key_id = merge_secret_field(
        tos_access_key_id,
        saved_secrets.as_ref().map(|c| c.access_key_id.as_str()),
    );
    let secret_access_key = merge_secret_field(
        tos_secret_access_key,
        saved_secrets.as_ref().map(|c| c.secret_access_key.as_str()),
    );
    let region = merge_secret_field(tos_region, Some(settings.tos_region.as_str()));
    let bucket = merge_secret_field(tos_bucket, Some(settings.tos_bucket.as_str()));
    // Endpoint is optional; empty override keeps saved (may also be empty → default).
    let endpoint =
        merge_secret_field(tos_endpoint, Some(settings.tos_endpoint.as_str())).unwrap_or_default();

    let (Some(access_key_id), Some(secret_access_key), Some(region), Some(bucket)) =
        (access_key_id, secret_access_key, region, bucket)
    else {
        let secrets_missing = saved_secrets.is_none()
            && merge_secret_field(tos_access_key_id, None).is_none()
            && merge_secret_field(tos_secret_access_key, None).is_none();
        let region_bucket_missing = settings.tos_region.trim().is_empty()
            && settings.tos_bucket.trim().is_empty()
            && merge_secret_field(tos_region, None).is_none()
            && merge_secret_field(tos_bucket, None).is_none();

        if secrets_missing && region_bucket_missing {
            return Err(AppErrorDto::tos_not_configured());
        }
        return Err(AppErrorDto::settings_invalid(
            "测试 TOS 连接需要 Access Key、Secret Key、Region 与 Bucket（可与已保存配置合并）",
        ));
    };

    Ok(TosConfig::from_parts(
        TosCredentials {
            access_key_id,
            secret_access_key,
        },
        region,
        bucket,
        endpoint,
    ))
}

pub fn test_doubao(doubao_api_key: Option<&str>) -> CmdResult<SettingsTestResult> {
    let credentials = merge_doubao_credentials(doubao_api_key)?;
    // Never log credentials. Auth-only probe: needs no TOS and no recognition quota.
    let client = HttpAsyncClient::new()?;
    client.probe_auth(&credentials)?;
    Ok(SettingsTestResult::ok())
}

/// Holds the DB lock only while merging saved config, not during the network probe.
pub fn test_tos(
    db: &Mutex<Connection>,
    tos_access_key_id: Option<&str>,
    tos_secret_access_key: Option<&str>,
    tos_region: Option<&str>,
    tos_bucket: Option<&str>,
    tos_endpoint: Option<&str>,
) -> CmdResult<SettingsTestResult> {
    let config = merge_tos_config(
        &*db::lock(db)?,
        tos_access_key_id,
        tos_secret_access_key,
        tos_region,
        tos_bucket,
        tos_endpoint,
    )?;
    let client = HttpTosClient::new();
    client.head_bucket(&config)?;
    Ok(SettingsTestResult::ok())
}

/// Minimal chat completion against the merged config. Holds the DB lock only
/// while merging, not during the network call. Never persists overrides.
pub fn test_summary_llm(
    db: &Mutex<Connection>,
    overrides: SummaryLlmTestOverrides<'_>,
) -> CmdResult<SettingsTestResult> {
    let config = merge_summary_llm_config(&*db::lock(db)?, overrides)?;
    let client = HttpChatClient::for_probe()?;
    client.test_connection(&config)?;
    Ok(SettingsTestResult::ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SettingsUpdate;
    use crate::services::credentials::reset_for_test;
    use crate::services::settings_service::update_settings;

    #[test]
    fn merge_secret_prefers_non_empty_override() {
        assert_eq!(
            merge_secret_field(Some("  new  "), Some("saved")).as_deref(),
            Some("new")
        );
        assert_eq!(
            merge_secret_field(Some("   "), Some("saved")).as_deref(),
            Some("saved")
        );
        assert_eq!(
            merge_secret_field(None, Some("saved")).as_deref(),
            Some("saved")
        );
        assert_eq!(merge_secret_field(Some(""), None), None);
        assert_eq!(merge_secret_field(None, None), None);
    }

    #[test]
    fn doubao_not_configured_without_saved_or_override() {
        reset_for_test();
        let err = merge_doubao_credentials(None).expect_err("missing");
        assert_eq!(err.code, "ASR_NOT_CONFIGURED");
        let err = merge_doubao_credentials(Some("   ")).expect_err("blank");
        assert_eq!(err.code, "ASR_NOT_CONFIGURED");
    }

    #[test]
    fn doubao_override_wins_over_saved_key() {
        reset_for_test();
        credentials::set_credentials("saved-key").unwrap();
        let merged = merge_doubao_credentials(Some(" form-key ")).expect("merge");
        assert_eq!(merged.api_key, "form-key");
        let merged = merge_doubao_credentials(None).expect("saved");
        assert_eq!(merged.api_key, "saved-key");
    }

    #[test]
    fn summary_llm_not_configured() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let err = merge_summary_llm_config(&conn, SummaryLlmTestOverrides::default())
            .expect_err("missing");
        assert_eq!(err.code, "SUMMARY_NOT_CONFIGURED");
    }

    #[test]
    fn summary_llm_override_without_saved_uses_defaults() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let merged = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                api_key: Some(" sk-form "),
                ..Default::default()
            },
        )
        .expect("merge");
        assert_eq!(merged.api_key, "sk-form");
        assert_eq!(merged.provider, "dashscope");
        assert_eq!(
            merged.base_url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(merged.model, "qwen3.7-plus");
    }

    #[test]
    fn summary_llm_saved_key_reused_for_same_endpoint() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        credentials::set_summary_llm_credentials("sk-saved").unwrap();
        let merged = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                model: Some("qwen-max"),
                ..Default::default()
            },
        )
        .expect("merge");
        assert_eq!(merged.api_key, "sk-saved");
        assert_eq!(merged.model, "qwen-max");
    }

    #[test]
    fn summary_llm_saved_key_not_sent_to_new_endpoint() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        credentials::set_summary_llm_credentials("sk-saved").unwrap();

        let err = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                provider: Some("deepseek"),
                ..Default::default()
            },
        )
        .expect_err("provider change");
        assert_eq!(err.code, "SETTINGS_INVALID");

        let err = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                base_url: Some("https://evil.example.com/v1"),
                ..Default::default()
            },
        )
        .expect_err("base url change");
        assert_eq!(err.code, "SETTINGS_INVALID");

        let merged = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                provider: Some("deepseek"),
                api_key: Some("sk-deepseek"),
                ..Default::default()
            },
        )
        .expect("with key");
        assert_eq!(merged.base_url, "https://api.deepseek.com/v1");
        assert_eq!(merged.model, "deepseek-chat");
        assert_eq!(merged.api_key, "sk-deepseek");
    }

    #[test]
    fn summary_llm_test_rejects_invalid_values_and_does_not_persist() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let err = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                api_key: Some("k"),
                base_url: Some("ftp://x"),
                ..Default::default()
            },
        )
        .expect_err("ftp");
        assert_eq!(err.code, "SETTINGS_INVALID");

        let err = merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                api_key: Some("k"),
                provider: Some("openai"),
                ..Default::default()
            },
        )
        .expect_err("openai needs a model");
        assert_eq!(err.code, "SETTINGS_INVALID");

        merge_summary_llm_config(
            &conn,
            SummaryLlmTestOverrides {
                api_key: Some("sk-form"),
                provider: Some("custom"),
                base_url: Some("http://localhost:11434/v1"),
                model: Some("llama3"),
            },
        )
        .expect("custom");

        let settings = settings_service::get_settings(&conn).expect("get");
        assert_eq!(settings.summary_llm_provider, "dashscope");
        assert!(!settings.summary_llm_configured);
        assert!(credentials::get_summary_llm_credentials()
            .unwrap()
            .is_none());
    }

    #[test]
    fn tos_not_configured_when_empty() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let err = merge_tos_config(&conn, None, None, None, None, None).expect_err("missing");
        assert_eq!(err.code, "TOS_NOT_CONFIGURED");
    }

    #[test]
    fn tos_partial_merge_is_invalid() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        update_settings(
            &conn,
            SettingsUpdate {
                tos_region: Some("cn-beijing".into()),
                ..Default::default()
            },
        )
        .expect("region");
        let err = merge_tos_config(&conn, None, None, None, None, None).expect_err("partial");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn tos_merges_form_secrets_with_saved_region_bucket() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        update_settings(
            &conn,
            SettingsUpdate {
                tos_region: Some("cn-beijing".into()),
                tos_bucket: Some("meetphant-audio".into()),
                tos_endpoint: Some("".into()),
                ..Default::default()
            },
        )
        .expect("non-secrets");

        let config = merge_tos_config(&conn, Some("AKFORM"), Some("SKFORM"), None, None, None)
            .expect("merge");
        assert_eq!(config.credentials.access_key_id, "AKFORM");
        assert_eq!(config.credentials.secret_access_key, "SKFORM");
        assert_eq!(config.region, "cn-beijing");
        assert_eq!(config.bucket, "meetphant-audio");
        assert!(config.endpoint.contains("cn-beijing"));
    }

    #[test]
    fn tos_merge_does_not_persist_overrides() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        update_settings(
            &conn,
            SettingsUpdate {
                tos_access_key_id: Some("AKSAVED".into()),
                tos_secret_access_key: Some("SKSAVED".into()),
                tos_region: Some("cn-beijing".into()),
                tos_bucket: Some("meetphant-audio".into()),
                ..Default::default()
            },
        )
        .expect("seed");

        let _ = merge_tos_config(
            &conn,
            Some("AKNEW"),
            Some("SKNEW"),
            Some("cn-shanghai"),
            Some("other-bucket"),
            None,
        )
        .expect("merge");

        let settings = settings_service::get_settings(&conn).expect("get");
        assert_eq!(settings.tos_region, "cn-beijing");
        assert_eq!(settings.tos_bucket, "meetphant-audio");
        let saved = credentials::get_tos_credentials()
            .expect("get")
            .expect("some");
        assert_eq!(saved.access_key_id, "AKSAVED");
        assert_eq!(saved.secret_access_key, "SKSAVED");
    }
}
