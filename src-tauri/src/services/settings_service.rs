use rusqlite::Connection;

use crate::error::{AppErrorDto, CmdResult};
use crate::models::{
    Settings, SettingsUpdate, THEME_PREFERENCE_DARK, THEME_PREFERENCE_LIGHT,
    THEME_PREFERENCE_SYSTEM,
};
use crate::providers::openai_compat::presets::{self, DEFAULT_PROVIDER_ID};
use crate::services::credentials;

/// Max length of a single hotword (characters).
pub const MAX_HOTWORD_LEN: usize = 100;

/// Validate hotwords without writing. Empty / whitespace-only / too-long → SETTINGS_INVALID.
pub fn validate_hotwords(hotwords: &[String]) -> CmdResult<()> {
    for (index, word) in hotwords.iter().enumerate() {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            return Err(AppErrorDto::with_details(
                "SETTINGS_INVALID",
                "Hotwords cannot be empty",
                serde_json::json!({ "field": "hotwords", "index": index }),
            ));
        }
        if trimmed.chars().count() > MAX_HOTWORD_LEN {
            return Err(AppErrorDto::with_details(
                "SETTINGS_INVALID",
                format!("Hotword exceeds {MAX_HOTWORD_LEN} characters"),
                serde_json::json!({ "field": "hotwords", "index": index }),
            ));
        }
    }
    Ok(())
}

fn compute_tos_configured(region: &str, bucket: &str) -> bool {
    credentials::is_tos_secrets_configured()
        && !region.trim().is_empty()
        && !bucket.trim().is_empty()
}

/// Summary LLM columns exactly as stored in SQLite (empty = preset default).
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredSummaryLlm {
    provider: String,
    base_url: String,
    model: String,
}

impl Default for StoredSummaryLlm {
    fn default() -> Self {
        Self {
            provider: DEFAULT_PROVIDER_ID.to_string(),
            base_url: String::new(),
            model: String::new(),
        }
    }
}

/// Effective (provider id, base URL, model) after applying preset defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSummaryLlm {
    pub provider: String,
    pub base_url: String,
    pub model: String,
}

impl EffectiveSummaryLlm {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            provider: settings.summary_llm_provider.clone(),
            base_url: settings.summary_llm_base_url.clone(),
            model: settings.summary_llm_model.clone(),
        }
    }
}

fn resolve_summary_llm(stored: &StoredSummaryLlm) -> EffectiveSummaryLlm {
    let preset = presets::resolve_preset(&stored.provider);
    let base_url = match stored.base_url.trim() {
        "" => preset.default_base_url.to_string(),
        url => url.to_string(),
    };
    let model = match stored.model.trim() {
        "" => preset.default_model.unwrap_or_default().to_string(),
        model => model.to_string(),
    };
    EffectiveSummaryLlm {
        provider: preset.id.to_string(),
        base_url,
        model,
    }
}

fn read_stored_summary_llm(conn: &Connection) -> CmdResult<StoredSummaryLlm> {
    let row = conn.query_row(
        "SELECT summary_llm_provider, summary_llm_base_url, summary_llm_model
         FROM settings WHERE id = 1",
        [],
        |row| {
            Ok(StoredSummaryLlm {
                provider: row.get(0)?,
                base_url: row.get(1)?,
                model: row.get(2)?,
            })
        },
    );
    match row {
        Ok(stored) => Ok(stored),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(StoredSummaryLlm::default()),
        Err(err) => Err(AppErrorDto::from(err)),
    }
}

/// Validate a summary base URL: absolute `http(s)://` URL with a host.
pub fn validate_summary_llm_base_url(raw: &str) -> CmdResult<()> {
    let invalid = || {
        AppErrorDto::with_details(
            "SETTINGS_INVALID",
            "Base URL 必须以 http:// 或 https:// 开头",
            serde_json::json!({ "field": "summary_llm_base_url" }),
        )
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(trimmed).map_err(|_| invalid())?;
    let scheme_ok = matches!(url.scheme(), "http" | "https");
    let host_ok = url.host_str().map(|h| !h.is_empty()).unwrap_or(false);
    if !scheme_ok || !host_ok {
        return Err(invalid());
    }
    Ok(())
}

/// Validate a summary model name (non-empty after trim).
pub fn validate_summary_llm_model(raw: &str) -> CmdResult<()> {
    if raw.trim().is_empty() {
        return Err(AppErrorDto::with_details(
            "SETTINGS_INVALID",
            "模型名不能为空",
            serde_json::json!({ "field": "summary_llm_model" }),
        ));
    }
    Ok(())
}

/// Validate a provider id; only known presets are accepted.
pub fn validate_summary_llm_provider(raw: &str) -> CmdResult<&'static str> {
    presets::find_preset(raw).map(|p| p.id).ok_or_else(|| {
        AppErrorDto::with_details(
            "SETTINGS_INVALID",
            "未知的摘要模型服务商",
            serde_json::json!({ "field": "summary_llm_provider" }),
        )
    })
}

/// Comparable form of a base URL (trim + drop trailing `/`).
pub fn normalize_base_url(raw: &str) -> &str {
    raw.trim().trim_end_matches('/')
}

/// True when moving from `current` to `next` would send a key to a different endpoint.
pub fn summary_llm_endpoint_changed(
    current: &EffectiveSummaryLlm,
    next: &EffectiveSummaryLlm,
) -> bool {
    current.provider != next.provider
        || normalize_base_url(&current.base_url) != normalize_base_url(&next.base_url)
}

fn non_empty(value: &Option<String>) -> bool {
    value
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Validate summary LLM fields of an update without writing.
/// Returns the new stored row when any summary LLM non-secret field is present.
fn plan_summary_llm_update(
    conn: &Connection,
    update: &SettingsUpdate,
) -> CmdResult<Option<StoredSummaryLlm>> {
    let touched = update.summary_llm_provider.is_some()
        || update.summary_llm_base_url.is_some()
        || update.summary_llm_model.is_some();
    if !touched {
        return Ok(None);
    }

    let stored = read_stored_summary_llm(conn)?;
    let current = resolve_summary_llm(&stored);

    let provider = match update.summary_llm_provider.as_deref() {
        Some(raw) => validate_summary_llm_provider(raw)?.to_string(),
        None => current.provider.clone(),
    };
    let provider_changed = provider != current.provider;

    // Explicit empty values are rejected (R5); omitted fields keep the stored
    // value, or reset to the new preset's default when the provider changes (D2).
    let base_url = match update.summary_llm_base_url.as_deref() {
        Some(raw) => {
            validate_summary_llm_base_url(raw)?;
            raw.trim().to_string()
        }
        None if provider_changed => String::new(),
        None => stored.base_url.trim().to_string(),
    };
    let model = match update.summary_llm_model.as_deref() {
        Some(raw) => {
            validate_summary_llm_model(raw)?;
            raw.trim().to_string()
        }
        None if provider_changed => String::new(),
        None => stored.model.trim().to_string(),
    };

    let next_stored = StoredSummaryLlm {
        provider,
        base_url,
        model,
    };
    let next = resolve_summary_llm(&next_stored);
    validate_summary_llm_base_url(&next.base_url)?;
    validate_summary_llm_model(&next.model)?;

    // D5: never let a saved key follow the user to a different endpoint.
    if summary_llm_endpoint_changed(&current, &next)
        && credentials::is_summary_llm_configured()
        && !non_empty(&update.summary_llm_api_key)
    {
        return Err(AppErrorDto::with_details(
            "SETTINGS_INVALID",
            "切换服务商或修改 Base URL 后需重新填写 API Key",
            serde_json::json!({ "field": "summary_llm_api_key" }),
        ));
    }

    Ok(Some(next_stored))
}

fn persist_summary_llm_row(conn: &Connection, stored: &StoredSummaryLlm) -> CmdResult<()> {
    conn.execute(
        "INSERT INTO settings (id, summary_llm_provider, summary_llm_base_url, summary_llm_model)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET
           summary_llm_provider = excluded.summary_llm_provider,
           summary_llm_base_url = excluded.summary_llm_base_url,
           summary_llm_model = excluded.summary_llm_model",
        rusqlite::params![stored.provider, stored.base_url, stored.model],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

fn with_configured(mut settings: Settings, stored_llm: &StoredSummaryLlm) -> Settings {
    settings.doubao_configured = credentials::is_configured();
    settings.summary_llm_configured = credentials::is_summary_llm_configured();
    let llm = resolve_summary_llm(stored_llm);
    settings.summary_llm_provider = llm.provider;
    settings.summary_llm_base_url = llm.base_url;
    settings.summary_llm_model = llm.model;
    settings.tos_configured = compute_tos_configured(&settings.tos_region, &settings.tos_bucket);
    settings.recording_dir_resolved =
        crate::services::recording_service::resolve_recording_dir(&settings.recording_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
    settings
}

/// True when TOS AK/SK and region/bucket are all present (endpoint optional).
pub fn is_tos_configured(conn: &Connection) -> bool {
    match get_settings(conn) {
        Ok(s) => s.tos_configured,
        Err(_) => false,
    }
}

/// Validate theme_preference without writing. Unknown values → SETTINGS_INVALID.
pub fn validate_theme_preference(raw: &str) -> CmdResult<String> {
    let trimmed = raw.trim();
    match trimmed {
        THEME_PREFERENCE_SYSTEM | THEME_PREFERENCE_LIGHT | THEME_PREFERENCE_DARK => {
            Ok(trimmed.to_string())
        }
        _ => Err(AppErrorDto::with_details(
            "SETTINGS_INVALID",
            "theme_preference must be system, light, or dark",
            serde_json::json!({ "field": "theme_preference" }),
        )),
    }
}

pub fn get_settings(conn: &Connection) -> CmdResult<Settings> {
    let mut stmt = conn
        .prepare(
            "SELECT hotwords, context_text, tos_region, tos_bucket, tos_endpoint, recording_dir, theme_preference
             FROM settings WHERE id = 1",
        )
        .map_err(AppErrorDto::from)?;

    let row = stmt.query_row([], |row| {
        let hotwords_json: String = row.get(0)?;
        let context_text: String = row.get(1)?;
        let tos_region: String = row.get(2)?;
        let tos_bucket: String = row.get(3)?;
        let tos_endpoint: String = row.get(4)?;
        let recording_dir: String = row.get(5)?;
        let theme_preference: String = row.get(6)?;
        Ok((
            hotwords_json,
            context_text,
            tos_region,
            tos_bucket,
            tos_endpoint,
            recording_dir,
            theme_preference,
        ))
    });

    match row {
        Ok((
            hotwords_json,
            context_text,
            tos_region,
            tos_bucket,
            tos_endpoint,
            recording_dir,
            theme_preference,
        )) => {
            let hotwords: Vec<String> =
                serde_json::from_str(&hotwords_json).map_err(AppErrorDto::from)?;
            let theme_preference = validate_theme_preference(&theme_preference)
                .unwrap_or_else(|_| THEME_PREFERENCE_SYSTEM.to_string());
            let stored_llm = read_stored_summary_llm(conn)?;
            Ok(with_configured(
                Settings {
                    hotwords,
                    context_text,
                    tos_region,
                    tos_bucket,
                    tos_endpoint,
                    recording_dir,
                    theme_preference,
                    ..Settings::default()
                },
                &stored_llm,
            ))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(with_configured(
            Settings::default(),
            &StoredSummaryLlm::default(),
        )),
        Err(err) => Err(AppErrorDto::from(err)),
    }
}

fn apply_credential_update(update: &SettingsUpdate) -> CmdResult<()> {
    if let Some(ref key) = update.doubao_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            credentials::set_credentials(trimmed)?;
        }
    }

    if let Some(ref key) = update.summary_llm_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            credentials::set_summary_llm_credentials(trimmed)?;
        }
    }

    let has_tos_ak = update
        .tos_access_key_id
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_tos_sk = update
        .tos_secret_access_key
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if has_tos_ak || has_tos_sk {
        if !(has_tos_ak && has_tos_sk) {
            return Err(AppErrorDto::settings_invalid(
                "TOS access key id and secret access key must both be provided together",
            ));
        }
        credentials::set_tos_credentials(
            update.tos_access_key_id.as_ref().unwrap(),
            update.tos_secret_access_key.as_ref().unwrap(),
        )?;
    }

    Ok(())
}

fn persist_settings_row(conn: &Connection, settings: &Settings) -> CmdResult<()> {
    let hotwords_json = serde_json::to_string(&settings.hotwords).map_err(AppErrorDto::from)?;
    conn.execute(
        "INSERT INTO settings (id, hotwords, context_text, tos_region, tos_bucket, tos_endpoint, recording_dir, theme_preference)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
           hotwords = excluded.hotwords,
           context_text = excluded.context_text,
           tos_region = excluded.tos_region,
           tos_bucket = excluded.tos_bucket,
           tos_endpoint = excluded.tos_endpoint,
           recording_dir = excluded.recording_dir,
           theme_preference = excluded.theme_preference",
        rusqlite::params![
            hotwords_json,
            settings.context_text,
            settings.tos_region,
            settings.tos_bucket,
            settings.tos_endpoint,
            settings.recording_dir,
            settings.theme_preference
        ],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

pub fn update_settings(conn: &Connection, update: SettingsUpdate) -> CmdResult<Settings> {
    // Validate SQLite fields before touching the keyring so a bad payload
    // cannot partially apply credentials.
    if let Some(ref hotwords) = update.hotwords {
        validate_hotwords(hotwords)?;
    }
    let recording_dir_validated = if let Some(ref raw) = update.recording_dir {
        Some(crate::services::recording_service::validate_recording_dir_override(raw)?)
    } else {
        None
    };
    let theme_preference_validated = if let Some(ref raw) = update.theme_preference {
        Some(validate_theme_preference(raw)?)
    } else {
        None
    };
    let summary_llm_planned = plan_summary_llm_update(conn, &update)?;
    apply_credential_update(&update)?;

    let mut current = get_settings(conn)?;

    if let Some(hotwords) = update.hotwords {
        // Persist trimmed forms so empty-looking values never sneak in.
        current.hotwords = hotwords.into_iter().map(|w| w.trim().to_string()).collect();
    }

    if let Some(context_text) = update.context_text {
        current.context_text = context_text;
    }

    if let Some(tos_region) = update.tos_region {
        current.tos_region = tos_region.trim().to_string();
    }
    if let Some(tos_bucket) = update.tos_bucket {
        current.tos_bucket = tos_bucket.trim().to_string();
    }
    if let Some(tos_endpoint) = update.tos_endpoint {
        current.tos_endpoint = tos_endpoint.trim().to_string();
    }
    if let Some(recording_dir) = recording_dir_validated {
        current.recording_dir = recording_dir;
    }
    if let Some(theme_preference) = theme_preference_validated {
        current.theme_preference = theme_preference;
    }

    persist_settings_row(conn, &current)?;
    if let Some(ref stored) = summary_llm_planned {
        persist_summary_llm_row(conn, stored)?;
    }
    get_settings(conn)
}

pub fn clear_doubao_credentials(conn: &Connection) -> CmdResult<Settings> {
    credentials::clear_credentials()?;
    get_settings(conn)
}

/// Clear only the summary LLM API key; provider / base URL / model are kept.
pub fn clear_summary_llm_credentials(conn: &Connection) -> CmdResult<Settings> {
    credentials::clear_summary_llm_credentials()?;
    get_settings(conn)
}

/// Clear TOS secrets from keyring and wipe region/bucket/endpoint in SQLite.
pub fn clear_tos_credentials(conn: &Connection) -> CmdResult<Settings> {
    credentials::clear_tos_credentials()?;
    let mut current = get_settings(conn)?;
    current.tos_region.clear();
    current.tos_bucket.clear();
    current.tos_endpoint.clear();
    persist_settings_row(conn, &current)?;
    get_settings(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::credentials::reset_for_test;

    #[test]
    fn empty_db_returns_defaults() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let settings = get_settings(&conn).expect("get");
        assert_eq!(settings.hotwords, Vec::<String>::new());
        assert_eq!(settings.context_text, "");
        assert!(!settings.doubao_configured);
        assert!(!settings.summary_llm_configured);
        assert_eq!(settings.summary_llm_provider, "dashscope");
        assert_eq!(
            settings.summary_llm_base_url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(settings.summary_llm_model, "qwen3.7-plus");
        assert!(!settings.tos_configured);
        assert_eq!(settings.tos_region, "");
        assert_eq!(settings.tos_bucket, "");
        assert_eq!(settings.recording_dir, "");
        assert_eq!(settings.theme_preference, THEME_PREFERENCE_SYSTEM);
        assert!(
            settings
                .recording_dir_resolved
                .replace('\\', "/")
                .ends_with("Meetphant/Recordings"),
            "resolved={}",
            settings.recording_dir_resolved
        );
    }

    #[test]
    fn update_persists_hotwords_and_context() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let updated = update_settings(
            &conn,
            SettingsUpdate {
                hotwords: Some(vec!["Meetphant".into(), "豆包".into()]),
                context_text: Some("周会摘要上下文".into()),
                ..Default::default()
            },
        )
        .expect("update");

        assert_eq!(updated.hotwords, vec!["Meetphant", "豆包"]);
        assert_eq!(updated.context_text, "周会摘要上下文");
        assert!(!updated.doubao_configured);

        let loaded = get_settings(&conn).expect("reload");
        assert_eq!(loaded.hotwords, updated.hotwords);
        assert_eq!(loaded.context_text, updated.context_text);
    }

    #[test]
    fn empty_hotword_rejects_without_write() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        update_settings(
            &conn,
            SettingsUpdate {
                hotwords: Some(vec!["ok".into()]),
                context_text: Some("keep".into()),
                ..Default::default()
            },
        )
        .expect("seed");

        let err = update_settings(
            &conn,
            SettingsUpdate {
                hotwords: Some(vec!["ok".into(), "".into()]),
                context_text: Some("should-not-write".into()),
                ..Default::default()
            },
        )
        .expect_err("empty hotword");

        assert_eq!(err.code, "SETTINGS_INVALID");

        let loaded = get_settings(&conn).expect("reload");
        assert_eq!(loaded.hotwords, vec!["ok"]);
        assert_eq!(loaded.context_text, "keep");
    }

    #[test]
    fn whitespace_hotword_rejects_without_write() {
        let err = validate_hotwords(&[String::from("   ")]).expect_err("whitespace");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn too_long_hotword_rejects() {
        let long = "a".repeat(MAX_HOTWORD_LEN + 1);
        let err = validate_hotwords(&[long]).expect_err("too long");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn partial_update_context_only() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        update_settings(
            &conn,
            SettingsUpdate {
                hotwords: Some(vec!["Meetphant".into()]),
                context_text: None,
                ..Default::default()
            },
        )
        .expect("hotwords");

        let updated = update_settings(
            &conn,
            SettingsUpdate {
                hotwords: None,
                context_text: Some("only context".into()),
                ..Default::default()
            },
        )
        .expect("context");

        assert_eq!(updated.hotwords, vec!["Meetphant"]);
        assert_eq!(updated.context_text, "only context");
    }

    #[test]
    fn credentials_set_flip_configured_without_leaking_secrets() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let updated = update_settings(
            &conn,
            SettingsUpdate {
                doubao_api_key: Some("secret-doubao-key".into()),
                ..Default::default()
            },
        )
        .expect("creds");

        assert!(updated.doubao_configured);
        let json = serde_json::to_string(&updated).expect("ser");
        assert!(!json.contains("secret-doubao-key"));
        assert!(json.contains("doubao_configured"));

        let cleared = clear_doubao_credentials(&conn).expect("clear");
        assert!(!cleared.doubao_configured);
    }

    #[test]
    fn summary_llm_configured_flag_without_leaking_key() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let updated = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_api_key: Some("sk-dash-secret".into()),
                ..Default::default()
            },
        )
        .expect("creds");

        assert!(updated.summary_llm_configured);
        let json = serde_json::to_string(&updated).expect("ser");
        assert!(!json.contains("sk-dash-secret"));
        assert!(json.contains("summary_llm_configured"));
        assert!(!json.contains("api_key"));

        let cleared = clear_summary_llm_credentials(&conn).expect("clear");
        assert!(!cleared.summary_llm_configured);
        // Clearing the key keeps provider / base URL / model.
        assert_eq!(cleared.summary_llm_provider, "dashscope");
    }

    #[test]
    fn upgraded_install_with_legacy_key_keeps_dashscope_defaults() {
        reset_for_test();
        credentials::seed_legacy_dashscope_key_for_test("sk-legacy");
        credentials::migrate_legacy_credentials();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let settings = get_settings(&conn).expect("get");
        assert!(settings.summary_llm_configured);
        assert_eq!(settings.summary_llm_provider, "dashscope");
        assert_eq!(
            settings.summary_llm_base_url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(settings.summary_llm_model, "qwen3.7-plus");
        assert!(!serde_json::to_string(&settings)
            .unwrap()
            .contains("sk-legacy"));
    }

    fn summary_llm_row(conn: &Connection) -> (String, String, String) {
        conn.query_row(
            "SELECT summary_llm_provider, summary_llm_base_url, summary_llm_model
             FROM settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap_or_default()
    }

    #[test]
    fn summary_llm_switch_to_deepseek_persists_config() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let updated = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_provider: Some("deepseek".into()),
                summary_llm_base_url: Some(" https://api.deepseek.com/v1/ ".into()),
                summary_llm_model: Some(" deepseek-chat ".into()),
                summary_llm_api_key: Some("sk-deepseek".into()),
                ..Default::default()
            },
        )
        .expect("update");
        assert_eq!(updated.summary_llm_provider, "deepseek");
        assert_eq!(updated.summary_llm_base_url, "https://api.deepseek.com/v1/");
        assert_eq!(updated.summary_llm_model, "deepseek-chat");
        assert!(updated.summary_llm_configured);
        assert!(!serde_json::to_string(&updated)
            .unwrap()
            .contains("sk-deepseek"));

        // Unrelated updates do not rewrite summary LLM columns.
        update_settings(
            &conn,
            SettingsUpdate {
                context_text: Some("ctx".into()),
                ..Default::default()
            },
        )
        .expect("ctx");
        assert_eq!(
            summary_llm_row(&conn),
            (
                "deepseek".into(),
                "https://api.deepseek.com/v1/".into(),
                "deepseek-chat".into()
            )
        );
    }

    #[test]
    fn summary_llm_provider_switch_resets_to_preset_defaults() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let updated = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_provider: Some("deepseek".into()),
                ..Default::default()
            },
        )
        .expect("provider only, no saved key");
        assert_eq!(updated.summary_llm_base_url, "https://api.deepseek.com/v1");
        assert_eq!(updated.summary_llm_model, "deepseek-chat");
        assert_eq!(summary_llm_row(&conn).1, "");

        // A preset without a recommended model requires one.
        let err = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_provider: Some("openai".into()),
                ..Default::default()
            },
        )
        .expect_err("model required");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn summary_llm_invalid_values_reject_without_write() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        update_settings(
            &conn,
            SettingsUpdate {
                context_text: Some("keep".into()),
                ..Default::default()
            },
        )
        .expect("seed");
        let before = summary_llm_row(&conn);

        let cases: Vec<SettingsUpdate> = vec![
            SettingsUpdate {
                summary_llm_base_url: Some("ftp://x".into()),
                ..Default::default()
            },
            SettingsUpdate {
                summary_llm_base_url: Some("".into()),
                ..Default::default()
            },
            SettingsUpdate {
                summary_llm_base_url: Some("https://".into()),
                ..Default::default()
            },
            SettingsUpdate {
                summary_llm_model: Some("   ".into()),
                ..Default::default()
            },
            SettingsUpdate {
                summary_llm_provider: Some("nope".into()),
                ..Default::default()
            },
            // `custom` has no default base URL, so one must be given.
            SettingsUpdate {
                summary_llm_provider: Some("custom".into()),
                summary_llm_model: Some("m".into()),
                ..Default::default()
            },
        ];
        for case in cases {
            let err = update_settings(
                &conn,
                SettingsUpdate {
                    context_text: Some("should-not-write".into()),
                    summary_llm_api_key: Some("sk-should-not-write".into()),
                    ..case
                },
            )
            .expect_err("invalid");
            assert_eq!(err.code, "SETTINGS_INVALID");
        }

        assert_eq!(summary_llm_row(&conn), before);
        let loaded = get_settings(&conn).expect("reload");
        assert_eq!(loaded.context_text, "keep");
        assert!(!loaded.summary_llm_configured);
        assert_eq!(loaded.summary_llm_provider, "dashscope");
    }

    #[test]
    fn summary_llm_endpoint_change_requires_new_key() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        credentials::set_summary_llm_credentials("sk-old").unwrap();

        let err = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_provider: Some("deepseek".into()),
                summary_llm_model: Some("deepseek-chat".into()),
                ..Default::default()
            },
        )
        .expect_err("provider switch without key");
        assert_eq!(err.code, "SETTINGS_INVALID");

        let err = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_base_url: Some("https://proxy.example.com/v1".into()),
                ..Default::default()
            },
        )
        .expect_err("base url change without key");
        assert_eq!(err.code, "SETTINGS_INVALID");
        assert_eq!(
            get_settings(&conn).unwrap().summary_llm_provider,
            "dashscope"
        );

        // Same endpoint (trailing slash only) + model change keeps the key.
        let ok = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_base_url: Some(
                    "https://dashscope.aliyuncs.com/compatible-mode/v1/".into(),
                ),
                summary_llm_model: Some("qwen-plus".into()),
                ..Default::default()
            },
        )
        .expect("model change");
        assert_eq!(ok.summary_llm_model, "qwen-plus");

        let switched = update_settings(
            &conn,
            SettingsUpdate {
                summary_llm_provider: Some("deepseek".into()),
                summary_llm_api_key: Some("sk-new".into()),
                ..Default::default()
            },
        )
        .expect("switch with key");
        assert_eq!(switched.summary_llm_provider, "deepseek");
        let saved = credentials::get_summary_llm_credentials().unwrap().unwrap();
        assert_eq!(saved.api_key, "sk-new");
    }

    #[test]
    fn tos_configured_requires_secrets_and_bucket_region() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");

        let only_non_secret = update_settings(
            &conn,
            SettingsUpdate {
                tos_region: Some("cn-beijing".into()),
                tos_bucket: Some("meetphant-audio".into()),
                ..Default::default()
            },
        )
        .expect("region/bucket");
        assert!(!only_non_secret.tos_configured);

        let with_secrets = update_settings(
            &conn,
            SettingsUpdate {
                tos_access_key_id: Some("AKTEST".into()),
                tos_secret_access_key: Some("SKTEST".into()),
                ..Default::default()
            },
        )
        .expect("secrets");
        assert!(with_secrets.tos_configured);
        assert_eq!(with_secrets.tos_region, "cn-beijing");
        assert_eq!(with_secrets.tos_bucket, "meetphant-audio");

        let json = serde_json::to_string(&with_secrets).expect("ser");
        assert!(!json.contains("AKTEST"));
        assert!(!json.contains("SKTEST"));
        assert!(json.contains("tos_configured"));

        let cleared = clear_tos_credentials(&conn).expect("clear");
        assert!(!cleared.tos_configured);
        assert_eq!(cleared.tos_region, "");
        assert_eq!(cleared.tos_bucket, "");
    }

    #[test]
    fn recording_dir_persists_and_resolves() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let base =
            std::env::temp_dir().join(format!("meetphant-settings-rec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);

        let updated = update_settings(
            &conn,
            SettingsUpdate {
                recording_dir: Some(base.to_string_lossy().to_string()),
                ..Default::default()
            },
        )
        .expect("update");

        assert_eq!(updated.recording_dir, base.to_string_lossy());
        assert_eq!(updated.recording_dir_resolved, base.to_string_lossy());

        let reset = update_settings(
            &conn,
            SettingsUpdate {
                recording_dir: Some(String::new()),
                ..Default::default()
            },
        )
        .expect("reset");
        assert_eq!(reset.recording_dir, "");
        assert!(reset
            .recording_dir_resolved
            .replace('\\', "/")
            .ends_with("Meetphant/Recordings"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn recording_dir_relative_rejects() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let err = update_settings(
            &conn,
            SettingsUpdate {
                recording_dir: Some("relative/recs".into()),
                ..Default::default()
            },
        )
        .expect_err("relative");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn theme_preference_persists_and_rejects_invalid() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");

        let updated = update_settings(
            &conn,
            SettingsUpdate {
                theme_preference: Some(THEME_PREFERENCE_DARK.into()),
                ..Default::default()
            },
        )
        .expect("update dark");
        assert_eq!(updated.theme_preference, THEME_PREFERENCE_DARK);

        let loaded = get_settings(&conn).expect("reload");
        assert_eq!(loaded.theme_preference, THEME_PREFERENCE_DARK);

        let light = update_settings(
            &conn,
            SettingsUpdate {
                theme_preference: Some(THEME_PREFERENCE_LIGHT.into()),
                ..Default::default()
            },
        )
        .expect("update light");
        assert_eq!(light.theme_preference, THEME_PREFERENCE_LIGHT);

        let err = update_settings(
            &conn,
            SettingsUpdate {
                theme_preference: Some("sepia".into()),
                ..Default::default()
            },
        )
        .expect_err("invalid");
        assert_eq!(err.code, "SETTINGS_INVALID");

        let still = get_settings(&conn).expect("unchanged");
        assert_eq!(still.theme_preference, THEME_PREFERENCE_LIGHT);
    }
}
