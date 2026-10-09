use rusqlite::Connection;

use crate::error::{AppErrorDto, CmdResult};
use crate::models::{
    Settings, SettingsUpdate, THEME_PREFERENCE_DARK, THEME_PREFERENCE_LIGHT,
    THEME_PREFERENCE_SYSTEM,
};
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

fn with_configured(mut settings: Settings) -> Settings {
    settings.doubao_configured = credentials::is_configured();
    settings.dashscope_configured = credentials::is_dashscope_configured();
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
            Ok(with_configured(Settings {
                hotwords,
                context_text,
                doubao_configured: false,
                dashscope_configured: false,
                tos_configured: false,
                tos_region,
                tos_bucket,
                tos_endpoint,
                recording_dir,
                recording_dir_resolved: String::new(),
                theme_preference,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(with_configured(Settings::default())),
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

    if let Some(ref key) = update.dashscope_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            credentials::set_dashscope_credentials(trimmed)?;
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
    get_settings(conn)
}

pub fn clear_doubao_credentials(conn: &Connection) -> CmdResult<Settings> {
    credentials::clear_credentials()?;
    get_settings(conn)
}

pub fn clear_dashscope_credentials(conn: &Connection) -> CmdResult<Settings> {
    credentials::clear_dashscope_credentials()?;
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
        assert!(!settings.dashscope_configured);
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
    fn dashscope_configured_flag_without_leaking_key() {
        reset_for_test();
        let conn = crate::db::pool::open_memory().expect("memory db");
        let updated = update_settings(
            &conn,
            SettingsUpdate {
                dashscope_api_key: Some("sk-dash-secret".into()),
                ..Default::default()
            },
        )
        .expect("creds");

        assert!(updated.dashscope_configured);
        let json = serde_json::to_string(&updated).expect("ser");
        assert!(!json.contains("sk-dash-secret"));
        assert!(json.contains("dashscope_configured"));

        let cleared = clear_dashscope_credentials(&conn).expect("clear");
        assert!(!cleared.dashscope_configured);
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
